//! ARCH03 stage 0d: NewTOS loads position-independent executables (`ET_DYN`)
//! and applies their `R_AARCH64_RELATIVE` relocations in the kernel.
//!
//! Sources the expectations come from:
//! * AAELF64 2025Q4 §5.2 (p. 13): `e_flags` "shall contain zero";
//!   §5.7.15 (p. 40/41): `R_AARCH64_RELATIVE` = 1027, `Delta(S) + A`, the
//!   place is an 8-byte aligned 64-bit data location.
//! * gABI 2013-06-10 ch4.reloc (`Elf64_Rela`, `ELF64_R_SYM/TYPE`) and
//!   ch5.dynamic (`Elf64_Dyn`, `DT_RELA/RELASZ/RELAENT` = 7/8/9, "must also
//!   have", `DT_TEXTREL` = 22, `DT_FLAGS` = 30 / `DF_TEXTREL` = 0x4).
//! * SYSVABI64 2025Q4 §9: PIEs have `e_type == ET_DYN`.
//! * Plan E7: `ET_EXEC` is refused once every in-tree program is PIE;
//!   EmuTOS answers an unloadable program with `EPLFMT` (`bdos/kpgmld.c`).
//!
//! The synthetic image mirrors what `rust-lld -pie` produces for an SDK
//! program (measured in stage 0d): text `PT_LOAD` (R-X) holding the
//! `.rela.dyn` table, data `PT_LOAD` (RW-) holding `.dynamic` and the
//! pointer words, a `PT_DYNAMIC` header over `.dynamic`, link base 0x1000.

use tos_api::bdos::process::elf::{self, ParseError};
use tos_api::bdos::process::loader::LoadError;
use tos_api::bdos::process::pexec::PexecError;
use tos_api::bdos::process::reloc::{self, RelocError, R_AARCH64_RELATIVE};
use tos_types::error::EPLFMT;

const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EM_AARCH64: u16 = 183;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;
const DT_PLTRELSZ: u64 = 2;
const DT_RELA: u64 = 7;
const DT_RELASZ: u64 = 8;
const DT_RELAENT: u64 = 9;
const DT_REL: u64 = 17;
const DT_TEXTREL: u64 = 22;
const DT_JMPREL: u64 = 23;
const DT_FLAGS: u64 = 30;
const DT_RELACOUNT: u64 = 0x6fff_fff9; // lld emits it; the loader ignores it

const R_AARCH64_ABS64: u32 = 257;
const R_AARCH64_GLOB_DAT: u32 = 1025;
const R_AARCH64_IRELATIVE: u32 = 1032;

// Layout (file offset == link VA, as lld lays out an SDK program).
const TEXT_VA: u64 = 0x1000;
const TEXT_SZ: u64 = 0x100;
const RELA_VA: u64 = 0x1040; // inside text
const DATA_VA: u64 = 0x1100;
const DYN_VA: u64 = 0x1100; // first thing in data, like .dynamic in newtos.ld
const DYN_SLOTS: u64 = 8; // 8 × 16 bytes reserved for the dynamic array
const WORDS_VA: u64 = DYN_VA + DYN_SLOTS * 16; // 0x1180: the pointer words
const DATA_FILESZ: u64 = 0xC0;
const DATA_MEMSZ: u64 = 0x140; // 0x80 BSS
const FILE_LEN: u64 = DATA_VA + DATA_FILESZ;
/// The TPA view the loader relocates in: index = link VA.
const VIEW_LEN: usize = 0x2000;

fn put16(b: &mut [u8], o: u64, v: u16) {
    b[o as usize..o as usize + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(b: &mut [u8], o: u64, v: u32) {
    b[o as usize..o as usize + 4].copy_from_slice(&v.to_le_bytes());
}
fn put64(b: &mut [u8], o: u64, v: u64) {
    b[o as usize..o as usize + 8].copy_from_slice(&v.to_le_bytes());
}
fn get64(b: &[u8], o: u64) -> u64 {
    u64::from_le_bytes(b[o as usize..o as usize + 8].try_into().unwrap())
}

/// One `Elf64_Rela`.
#[derive(Clone, Copy)]
struct Rela {
    offset: u64,
    sym: u32,
    rtype: u32,
    addend: i64,
}
fn relative(offset: u64, addend: i64) -> Rela {
    Rela { offset, sym: 0, rtype: R_AARCH64_RELATIVE, addend }
}

/// Knobs for a synthetic PIE.
#[derive(Clone)]
struct Spec {
    e_type: u16,
    e_flags: u32,
    interp: bool,
    with_dynamic: bool,
    /// `(tag, value)` pairs written before the terminating `DT_NULL`.
    dyn_tags: Vec<(u64, u64)>,
    /// Omit the terminating `DT_NULL` (all slots filled with this tag).
    unterminated: bool,
    relas: Vec<Rela>,
    /// Override for the dynamic header's VA/size.
    dyn_va: u64,
    dyn_sz: u64,
}

impl Spec {
    /// A well-formed PIE: two RELATIVE words in data.
    fn good() -> Self {
        let relas = vec![relative(WORDS_VA, 0x1010), relative(WORDS_VA + 8, 0x1188)];
        let n = relas.len() as u64;
        Spec {
            e_type: ET_DYN,
            e_flags: 0,
            interp: false,
            with_dynamic: true,
            dyn_tags: vec![
                (DT_RELA, RELA_VA),
                (DT_RELASZ, n * 24),
                (DT_RELAENT, 24),
                (DT_RELACOUNT, n),
            ],
            unterminated: false,
            relas,
            dyn_va: DYN_VA,
            dyn_sz: DYN_SLOTS * 16,
        }
    }
    fn tag(mut self, tag: u64, val: u64) -> Self {
        if let Some(e) = self.dyn_tags.iter_mut().find(|(t, _)| *t == tag) {
            e.1 = val;
        } else {
            self.dyn_tags.push((tag, val));
        }
        self
    }
    fn without_tag(mut self, tag: u64) -> Self {
        self.dyn_tags.retain(|(t, _)| *t != tag);
        self
    }
    fn relas(mut self, relas: Vec<Rela>) -> Self {
        let n = relas.len() as u64;
        self.relas = relas;
        self.tag(DT_RELASZ, n * 24)
    }

    /// The file bytes.
    fn file(&self) -> Vec<u8> {
        let mut b = vec![0u8; FILE_LEN as usize];
        b[0..4].copy_from_slice(b"\x7fELF");
        b[4] = 2; // ELFCLASS64
        b[5] = 1; // ELFDATA2LSB
        b[6] = 1; // EV_CURRENT
        put16(&mut b, 16, self.e_type);
        put16(&mut b, 18, EM_AARCH64);
        put32(&mut b, 20, 1);
        put64(&mut b, 24, TEXT_VA); // e_entry
        put64(&mut b, 32, 64); // e_phoff
        put32(&mut b, 48, self.e_flags);
        put16(&mut b, 52, 64); // e_ehsize
        put16(&mut b, 54, 56); // e_phentsize

        let mut phdrs: Vec<(u32, u32, u64, u64, u64)> = vec![]; // type, flags, off/va, filesz, memsz
        if self.interp {
            phdrs.push((PT_INTERP, PF_R, 0x200, 0x10, 0x10));
        }
        phdrs.push((PT_LOAD, PF_R | PF_X, TEXT_VA, TEXT_SZ, TEXT_SZ));
        phdrs.push((PT_LOAD, PF_R | PF_W, DATA_VA, DATA_FILESZ, DATA_MEMSZ));
        if self.with_dynamic {
            phdrs.push((PT_DYNAMIC, PF_R | PF_W, self.dyn_va, self.dyn_sz, self.dyn_sz));
        }
        put16(&mut b, 56, phdrs.len() as u16);
        for (i, (t, f, va, fsz, msz)) in phdrs.iter().enumerate() {
            let p = 64 + i as u64 * 56;
            put32(&mut b, p, *t);
            put32(&mut b, p + 4, *f);
            put64(&mut b, p + 8, *va); // p_offset == p_vaddr
            put64(&mut b, p + 16, *va);
            put64(&mut b, p + 24, *va);
            put64(&mut b, p + 32, *fsz);
            put64(&mut b, p + 40, *msz);
            put64(&mut b, p + 48, 0x1000);
        }

        // text: a recognisable byte pattern, then the rela table.
        for i in 0..0x40u64 {
            b[(TEXT_VA + i) as usize] = 0xC0 | (i as u8 & 0x0F);
        }
        for (i, r) in self.relas.iter().enumerate() {
            let p = RELA_VA + i as u64 * 24;
            if p + 24 > TEXT_VA + TEXT_SZ {
                break; // tests that need many entries only claim them
            }
            put64(&mut b, p, r.offset);
            put64(&mut b, p + 8, ((r.sym as u64) << 32) | r.rtype as u64);
            put64(&mut b, p + 16, r.addend as u64);
        }

        // dynamic array at DYN_VA (only if it lies in the file).
        if self.dyn_va == DYN_VA {
            let mut slot = 0u64;
            for (t, v) in &self.dyn_tags {
                put64(&mut b, DYN_VA + slot * 16, *t);
                put64(&mut b, DYN_VA + slot * 16 + 8, *v);
                slot += 1;
            }
            if self.unterminated {
                while slot < DYN_SLOTS {
                    put64(&mut b, DYN_VA + slot * 16, DT_RELAENT);
                    put64(&mut b, DYN_VA + slot * 16 + 8, 24);
                    slot += 1;
                }
            } else {
                put64(&mut b, DYN_VA + slot * 16, DT_NULL);
            }
        }
        // The pointer words start out as 0, as lld leaves RELA places.
        b
    }
}

/// Parse + stream the segments into a TPA view the way the loader does.
fn load(spec: &Spec) -> Result<(Vec<u8>, elf::ElfImage), ParseError> {
    let file = spec.file();
    let img = elf::parse(&file, file.len() as u64)?;
    let mut view = vec![0u8; VIEW_LEN];
    for s in img.segments() {
        let (o, n) = (s.file_offset as usize, s.file_size as usize);
        view[s.vaddr as usize..s.vaddr as usize + n].copy_from_slice(&file[o..o + n]);
    }
    Ok((view, img))
}

fn relocate(spec: &Spec, base: u64) -> Result<(Vec<u8>, usize), RelocError> {
    let (mut view, img) = load(spec).expect("spec must parse");
    let n = reloc::relocate(&mut view, &img, base)?;
    Ok((view, n))
}

// ─── header checks ──────────────────────────────────────────────────────

#[test]
fn pie_parses_and_records_its_dynamic_header() {
    let (_, img) = load(&Spec::good()).unwrap();
    assert_eq!(img.entry_va, TEXT_VA);
    assert_eq!(img.segments().len(), 2);
    let d = img.dynamic.expect("PT_DYNAMIC must be recorded");
    assert_eq!((d.vaddr, d.file_size), (DYN_VA, DYN_SLOTS * 16));
}

#[test]
fn et_exec_is_refused_with_eplfmt() {
    let mut s = Spec::good();
    s.e_type = ET_EXEC;
    let file = s.file();
    let e = elf::parse(&file, file.len() as u64).unwrap_err();
    assert_eq!(e, ParseError::NotPositionIndependent);
    assert_eq!(PexecError::LoadFailed(LoadError::Parse(e)).errno(), EPLFMT);
}

#[test]
fn other_e_type_is_not_an_executable() {
    let mut s = Spec::good();
    s.e_type = 1; // ET_REL
    let file = s.file();
    assert_eq!(elf::parse(&file, file.len() as u64).unwrap_err(), ParseError::NotExecutable);
}

#[test]
fn nonzero_e_flags_are_refused() {
    let mut s = Spec::good();
    s.e_flags = 1;
    let file = s.file();
    assert_eq!(elf::parse(&file, file.len() as u64).unwrap_err(), ParseError::BadFlags);
}

#[test]
fn interpreter_request_is_refused() {
    let mut s = Spec::good();
    s.interp = true;
    let file = s.file();
    assert_eq!(elf::parse(&file, file.len() as u64).unwrap_err(), ParseError::HasInterpreter);
}

// ─── valid relocation ───────────────────────────────────────────────────

/// Bias 0 is stage 0 (TPA at VA 0): the place receives the addend itself —
/// which lld does NOT store in the place, so a skipped relocation shows.
#[test]
fn relative_at_base_zero_writes_the_addend() {
    let (view, n) = relocate(&Spec::good(), 0).unwrap();
    assert_eq!(n, 2);
    assert_eq!(get64(&view, WORDS_VA), 0x1010);
    assert_eq!(get64(&view, WORDS_VA + 8), 0x1188);
}

/// Stage 1 will pass the TPA block address: value = base + addend.
#[test]
fn relative_adds_the_load_base() {
    let base = 0x0040_0000;
    let (view, _) = relocate(&Spec::good(), base).unwrap();
    assert_eq!(get64(&view, WORDS_VA), base + 0x1010);
    assert_eq!(get64(&view, WORDS_VA + 8), base + 0x1188);
}

/// Relocation touches the places and nothing else: text bytes and the
/// rela table itself are unchanged, the rest of the data is still zero.
#[test]
fn relocation_only_writes_the_places() {
    let (before, img) = load(&Spec::good()).unwrap();
    let mut after = before.clone();
    reloc::relocate(&mut after, &img, 0x10_0000).unwrap();
    for (i, (a, b)) in before.iter().zip(after.iter()).enumerate() {
        let i = i as u64;
        let is_place = (WORDS_VA..WORDS_VA + 16).contains(&i);
        if !is_place {
            assert_eq!(a, b, "byte at VA 0x{i:x} changed");
        }
    }
}

#[test]
fn image_without_dynamic_header_needs_no_relocation() {
    let mut s = Spec::good();
    s.with_dynamic = false;
    let (view, n) = relocate(&s, 0x1234_0000).unwrap();
    assert_eq!(n, 0);
    assert_eq!(get64(&view, WORDS_VA), 0, "nothing may be written");
}

#[test]
fn dynamic_without_rela_needs_no_relocation() {
    let s = Spec::good()
        .without_tag(DT_RELA)
        .without_tag(DT_RELASZ)
        .without_tag(DT_RELAENT);
    assert_eq!(relocate(&s, 0).unwrap().1, 0);
}

/// `DT_RELACOUNT` is not in the pinned sources (0a H6); the loader walks
/// `DT_RELASZ`. A lying count must not change the result.
#[test]
fn relacount_is_ignored() {
    let s = Spec::good().tag(DT_RELACOUNT, 0);
    let (view, n) = relocate(&s, 0).unwrap();
    assert_eq!(n, 2);
    assert_eq!(get64(&view, WORDS_VA + 8), 0x1188);
}

/// A place in the BSS tail of the writable segment is a legal place.
#[test]
fn place_in_bss_is_writable() {
    let bss = DATA_VA + DATA_FILESZ; // 0x11C0
    let s = Spec::good().relas(vec![relative(bss, 0x2000)]);
    let (view, _) = relocate(&s, 0).unwrap();
    assert_eq!(get64(&view, bss), 0x2000);
}

// ─── unsupported types / symbols ────────────────────────────────────────

#[test]
fn every_other_relocation_type_is_refused() {
    for t in [R_AARCH64_ABS64, R_AARCH64_GLOB_DAT, R_AARCH64_IRELATIVE, 0, 1026] {
        let mut r = relative(WORDS_VA, 0x1000);
        r.rtype = t;
        let s = Spec::good().relas(vec![relative(WORDS_VA + 8, 0x1000), r]);
        assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::UnsupportedType(t));
    }
    assert_eq!(
        PexecError::LoadFailed(LoadError::Reloc(RelocError::UnsupportedType(257))).errno(),
        EPLFMT
    );
}

#[test]
fn relative_against_a_symbol_is_refused() {
    let mut r = relative(WORDS_VA, 0x1000);
    r.sym = 1;
    let s = Spec::good().relas(vec![r]);
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::SymbolNotNull);
}

// ─── bad offsets ────────────────────────────────────────────────────────

#[test]
fn misaligned_place_is_refused() {
    let s = Spec::good().relas(vec![relative(WORDS_VA + 4, 0)]);
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::PlaceMisaligned);
}

#[test]
fn place_in_text_is_refused() {
    let s = Spec::good().relas(vec![relative(TEXT_VA + 8, 0)]);
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::PlaceNotWritable);
}

#[test]
fn place_outside_every_segment_is_refused() {
    // Page 0 (basepage/sysvars), the gap below the image, the first word
    // past the writable segment's memory, far beyond the view, and the top
    // of the address space (where `r_offset + 8` would wrap).
    for off in [0, 0x800, DATA_VA + DATA_MEMSZ, 0x1_0000_0000, u64::MAX & !7] {
        let s = Spec::good().relas(vec![relative(off, 0)]);
        assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::PlaceNotWritable, "offset 0x{off:x}");
    }
    // The last aligned word of the segment (in its BSS) is still inside.
    let s = Spec::good().relas(vec![relative(DATA_VA + DATA_MEMSZ - 8, 7)]);
    assert_eq!(relocate(&s, 0).unwrap().1, 1);
}

#[test]
fn place_inside_the_dynamic_array_is_refused() {
    let s = Spec::good().relas(vec![relative(DYN_VA + 8, 0)]);
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::PlaceInTables);
}

// ─── overflow ───────────────────────────────────────────────────────────

#[test]
fn value_overflow_is_refused() {
    let s = Spec::good().relas(vec![relative(WORDS_VA, 0x100)]);
    assert_eq!(relocate(&s, u64::MAX - 0x10).unwrap_err(), RelocError::ValueOverflow);
    let s = Spec::good().relas(vec![relative(WORDS_VA, -8)]);
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::ValueOverflow);
}

/// A negative addend that stays non-negative after the bias is fine
/// (pointer arithmetic below a symbol).
#[test]
fn negative_addend_within_range_is_applied() {
    let s = Spec::good().relas(vec![relative(WORDS_VA, -0x10)]);
    let (view, _) = relocate(&s, 0x1000).unwrap();
    assert_eq!(get64(&view, WORDS_VA), 0xFF0);
}

// ─── table / dynamic-array validation ───────────────────────────────────

#[test]
fn rela_table_outside_the_file_is_refused() {
    for va in [0x800, DATA_VA + DATA_FILESZ, 0xFFFF_FFFF_FFFF_FFF0] {
        let s = Spec::good().tag(DT_RELA, va);
        assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::RelaOutOfImage, "va 0x{va:x}");
    }
    // A size running past the end of the file (and a size that would wrap).
    for sz in [24 * 1000, u64::MAX / 24 * 24] {
        let s = Spec::good().tag(DT_RELASZ, sz);
        assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::RelaOutOfImage, "size {sz}");
    }
    let s = Spec::good().tag(DT_RELA, RELA_VA + 4);
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::RelaOutOfImage);
}

#[test]
fn rela_size_and_entsize_are_checked() {
    let s = Spec::good().tag(DT_RELASZ, 40);
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::BadRelaSize);
    let s = Spec::good().tag(DT_RELAENT, 16);
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::BadRelaEntSize);
    let s = Spec::good().without_tag(DT_RELASZ);
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::IncompleteRela);
    let s = Spec::good().without_tag(DT_RELAENT);
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::IncompleteRela);
    let s = Spec::good().without_tag(DT_RELA);
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::IncompleteRela);
}

#[test]
fn tags_needing_a_dynamic_linker_are_refused() {
    let cases = [
        (DT_NEEDED, 1, RelocError::NeedsSharedObject),
        (DT_JMPREL, RELA_VA, RelocError::PltRelocations),
        (DT_PLTRELSZ, 24, RelocError::PltRelocations),
        (DT_REL, RELA_VA, RelocError::RelTable),
        (DT_TEXTREL, 0, RelocError::TextRelocations),
        (DT_FLAGS, 0x4, RelocError::TextRelocations),
    ];
    for (tag, val, want) in cases {
        let s = Spec::good().tag(tag, val);
        assert_eq!(relocate(&s, 0).unwrap_err(), want, "tag {tag}");
    }
    // Harmless values of the same tags pass.
    let s = Spec::good().tag(DT_PLTRELSZ, 0).tag(DT_FLAGS, 0x8 /* DF_BIND_NOW */);
    assert_eq!(relocate(&s, 0).unwrap().1, 2);
}

#[test]
fn dynamic_array_without_dt_null_is_refused() {
    let mut s = Spec::good();
    s.unterminated = true;
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::DynamicUnterminated);
}

#[test]
fn dynamic_array_outside_file_bytes_is_refused() {
    // In the BSS tail: inside the segment's memory, but not file-backed.
    let mut s = Spec::good();
    s.dyn_va = DATA_VA + DATA_FILESZ;
    s.dyn_sz = 0x40;
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::DynamicOutOfImage);
    // Misaligned, and a size that is not a whole number of entries.
    let mut s = Spec::good();
    s.dyn_sz = 0x48;
    assert_eq!(relocate(&s, 0).unwrap_err(), RelocError::DynamicOutOfImage);
}
