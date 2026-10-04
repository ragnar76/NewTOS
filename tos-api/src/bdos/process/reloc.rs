//! Dynamic relocation of a position-independent executable (ARCH03 stage 0d).
//!
//! NewTOS programs are `ET_DYN` images (see [`super::elf`]). Their absolute
//! 64-bit data words — pointer tables, GOT entries, `static` pointer
//! initialisers — are left for the loader in a `DT_RELA` table, one
//! `R_AARCH64_RELATIVE` entry per word. This module walks the image's
//! dynamic section, finds that table, validates every entry and writes the
//! relocated values into the already-streamed segments. It is the
//! `Pexec`-time relocation EmuTOS performs for a PRG fixup table
//! (`bdos/kpgmld.c`), done for the AArch64 ELF format; doing it in the kernel
//! rather than in `crt0` is plan decision E8, so that a `Pexec(3)`-loaded
//! image is complete before anyone calls into it.
//!
//! ## Sources
//!
//! * **AAELF64** 2025Q4 (`references/arm-arm/aaelf64.pdf`) §5.7.15 "Dynamic
//!   relocations" (p. 40/41): `R_AARCH64_RELATIVE` = 1027, operation
//!   `Delta(S) + A`; "If S is the null symbol (ELF symbol index 0) then the
//!   adjustment is based on the segment defining the place"; `Delta(S)` for
//!   the null symbol is "the difference between the static link address of
//!   P and the execution address of P" (§5.7.3, p. 20); "all dynamic
//!   relocations require that the place being relocated is an 8-byte aligned
//!   64-bit data location in ELF64" (p. 40).
//! * **gABI** (System V ABI draft 2013-06-10, `references/gabi/`):
//!   `Elf64_Rela` {`r_offset`, `r_info`, `r_addend`} = 24 bytes and
//!   `ELF64_R_SYM(i) = i >> 32`, `ELF64_R_TYPE(i) = i & 0xffffffff`
//!   (ch4.reloc, Figure 4-23); `Elf64_Dyn` {`d_tag`, `d_un`} = 16 bytes,
//!   `DT_NULL` ends the array, `DT_RELA`/`DT_RELASZ`/`DT_RELAENT` = 7/8/9 and
//!   "If this element is present, the dynamic structure must also have
//!   DT_RELASZ and DT_RELAENT elements" (ch5.dynamic, Figures 5-9/5-10);
//!   `DT_TEXTREL` (22) and `DF_TEXTREL` (0x4 in `DT_FLAGS` = 30) announce
//!   relocations of non-writable segments.
//! * **SYSVABI64** 2025Q4 (`references/gabi/sysvabi64.pdf`) §9 (p. 23):
//!   PIEs are `ET_DYN` and mapped like shared objects.
//!
//! `DT_RELACOUNT` is **not** used: it is in none of the pinned sources (0a,
//! V0a H6). The loader walks the full `DT_RELASZ` range and checks the type
//! of every entry instead.
//!
//! ## What is refused (→ `EPLFMT` via `LoadError::Reloc`)
//!
//! Anything that would need a dynamic linker or a relocation this loader
//! does not perform: `DT_NEEDED` (a shared-library dependency), a PLT table
//! (`DT_JMPREL`, non-zero `DT_PLTRELSZ`), implicit-addend tables (`DT_REL`,
//! non-zero `DT_RELSZ`), text relocations (`DT_TEXTREL`, `DF_TEXTREL`), every
//! relocation type other than `R_AARCH64_RELATIVE`, a `RELATIVE` entry
//! against a symbol, and every entry whose place is unaligned, outside a
//! writable `PT_LOAD` segment or inside the relocation/dynamic tables
//! themselves. Silently skipping such an entry would leave an unrelocated
//! pointer in the program — a crash far away from its cause.
//!
//! ## The image view
//!
//! Every function takes `image: &[u8]` indexed by **link-time VA**:
//! `image[v]` is the byte the program was linked to find at VA `v` (the
//! loader streams `PT_LOAD` segment `s` to `image[s.vaddr..]`). `load_base`
//! is the execution address of link VA 0, so a byte linked at `v` executes
//! at `load_base + v` and `Delta = load_base` for every place. Stage 0 maps
//! the TPA at VA 0, i.e. `load_base = 0`; stage 1 will pass the TPA block
//! address. Pure byte manipulation, no `unsafe`, host-testable.

use super::elf::{ElfImage, LoadSegment, PF_W};

/// `R_AARCH64_RELATIVE` (AAELF64 §5.7.15, p. 40).
pub const R_AARCH64_RELATIVE: u32 = 1027;

/// `sizeof(Elf64_Dyn)` — `Elf64_Sxword d_tag` + `Elf64_Xword/Addr d_un`.
const DYN_ENT_SIZE: u64 = 16;
/// `sizeof(Elf64_Rela)` — `r_offset`, `r_info`, `r_addend`, 8 bytes each.
pub const RELA_ENT_SIZE: u64 = 24;

// `d_tag` values, gABI ch5.dynamic Figure 5-10.
const DT_NULL: i64 = 0;
const DT_NEEDED: i64 = 1;
const DT_PLTRELSZ: i64 = 2;
const DT_RELA: i64 = 7;
const DT_RELASZ: i64 = 8;
const DT_RELAENT: i64 = 9;
const DT_REL: i64 = 17;
const DT_RELSZ: i64 = 18;
const DT_TEXTREL: i64 = 22;
const DT_JMPREL: i64 = 23;
const DT_FLAGS: i64 = 30;
/// `DF_TEXTREL` in `DT_FLAGS` (gABI ch5.dynamic, "DT_FLAGS values").
const DF_TEXTREL: u64 = 0x4;

/// Why an image's dynamic relocations were refused.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RelocError {
    /// The `PT_DYNAMIC` range is not 8-byte aligned, not a whole number of
    /// `Elf64_Dyn` entries, or not backed by file bytes of a `PT_LOAD`
    /// segment.
    DynamicOutOfImage,
    /// No `DT_NULL` entry inside the `PT_DYNAMIC` range.
    DynamicUnterminated,
    /// `DT_NEEDED`: the program depends on a shared object.
    NeedsSharedObject,
    /// `DT_JMPREL` or a non-zero `DT_PLTRELSZ`: a PLT relocation table.
    PltRelocations,
    /// `DT_REL` or a non-zero `DT_RELSZ`: an implicit-addend table.
    RelTable,
    /// `DT_TEXTREL` or `DF_TEXTREL`: relocations of a non-writable segment.
    TextRelocations,
    /// `DT_RELA` without `DT_RELASZ`/`DT_RELAENT`, or a size/entsize without
    /// the table address (gABI: "must also have").
    IncompleteRela,
    /// `DT_RELAENT` is not `sizeof(Elf64_Rela)` (24).
    BadRelaEntSize,
    /// `DT_RELASZ` is not a multiple of the entry size.
    BadRelaSize,
    /// The `DT_RELA` table is unaligned or not backed by file bytes of a
    /// `PT_LOAD` segment.
    RelaOutOfImage,
    /// A relocation type other than `R_AARCH64_RELATIVE`.
    UnsupportedType(u32),
    /// An `R_AARCH64_RELATIVE` entry naming a symbol (index ≠ 0). There is
    /// no symbol table processing; the static linker emits index 0.
    SymbolNotNull,
    /// The place (`r_offset`) is not 8-byte aligned (AAELF64 p. 40).
    PlaceMisaligned,
    /// The 8-byte place does not lie inside a writable `PT_LOAD` segment.
    PlaceNotWritable,
    /// The place overlaps the relocation table or the dynamic array.
    PlaceInTables,
    /// `load_base + r_addend` overflows 64 bits.
    ValueOverflow,
}

/// The located `DT_RELA` table: link-time VA and size in bytes.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct RelaTable {
    /// `DT_RELA` — link-time VA of the first `Elf64_Rela`.
    pub vaddr: u64,
    /// `DT_RELASZ` — total size in bytes (a multiple of 24).
    pub size: u64,
}

#[inline]
fn rd_u64(image: &[u8], off: u64) -> Option<u64> {
    let off = usize::try_from(off).ok()?;
    let end = off.checked_add(8)?;
    image
        .get(off..end)
        .map(|b| u64::from_le_bytes(b.try_into().unwrap_or([0; 8])))
}

/// `true` when `[va, va + len)` lies inside the **file-backed** part of a
/// `PT_LOAD` segment (`[vaddr, vaddr + file_size)`) and inside `image`.
fn in_file_bytes(segs: &[LoadSegment], image_len: usize, va: u64, len: u64) -> bool {
    let Some(end) = va.checked_add(len) else {
        return false;
    };
    if end > image_len as u64 {
        return false;
    }
    segs.iter().any(|s| {
        s.vaddr
            .checked_add(s.file_size)
            .is_some_and(|seg_end| va >= s.vaddr && end <= seg_end)
    })
}

/// `true` when `[va, va + 8)` lies inside the memory range of a writable
/// `PT_LOAD` segment (`[vaddr, vaddr + mem_size)`, BSS included).
fn in_writable_segment(segs: &[LoadSegment], va: u64) -> bool {
    let Some(end) = va.checked_add(8) else {
        return false;
    };
    segs.iter().any(|s| {
        (s.flags & PF_W) != 0
            && s.vaddr
                .checked_add(s.mem_size)
                .is_some_and(|seg_end| va >= s.vaddr && end <= seg_end)
    })
}

#[inline]
fn overlaps(a: u64, a_len: u64, b: u64, b_len: u64) -> bool {
    // Half-open intervals; callers pass ranges already known not to wrap.
    a < b.saturating_add(b_len) && b < a.saturating_add(a_len)
}

/// Walk the `PT_DYNAMIC` array of `img` (already streamed into `image`) and
/// return the `DT_RELA` table, or `None` when the image has no dynamic
/// section or a dynamic section without relocations.
pub fn find_rela(image: &[u8], img: &ElfImage) -> Result<Option<RelaTable>, RelocError> {
    let Some(dynamic) = img.dynamic else {
        return Ok(None);
    };
    let segs = img.segments();
    if dynamic.vaddr % 8 != 0
        || dynamic.file_size % DYN_ENT_SIZE != 0
        || !in_file_bytes(segs, image.len(), dynamic.vaddr, dynamic.file_size)
    {
        return Err(RelocError::DynamicOutOfImage);
    }

    let mut rela: Option<u64> = None;
    let mut relasz: Option<u64> = None;
    let mut relaent: Option<u64> = None;
    let mut terminated = false;

    let count = dynamic.file_size / DYN_ENT_SIZE;
    for i in 0..count {
        let at = dynamic.vaddr + i * DYN_ENT_SIZE;
        // In range: the whole array was checked against `image` above.
        let tag = rd_u64(image, at).ok_or(RelocError::DynamicOutOfImage)? as i64;
        let val = rd_u64(image, at + 8).ok_or(RelocError::DynamicOutOfImage)?;
        match tag {
            DT_NULL => {
                terminated = true;
                break;
            }
            DT_NEEDED => return Err(RelocError::NeedsSharedObject),
            DT_JMPREL => return Err(RelocError::PltRelocations),
            DT_PLTRELSZ if val != 0 => return Err(RelocError::PltRelocations),
            DT_REL => return Err(RelocError::RelTable),
            DT_RELSZ if val != 0 => return Err(RelocError::RelTable),
            DT_TEXTREL => return Err(RelocError::TextRelocations),
            DT_FLAGS if val & DF_TEXTREL != 0 => return Err(RelocError::TextRelocations),
            DT_RELA => rela = Some(val),
            DT_RELASZ => relasz = Some(val),
            DT_RELAENT => relaent = Some(val),
            // Symbol/string/hash tables, init/fini arrays, DT_DEBUG and the
            // OS-specific tags lld adds (DT_FLAGS_1, DT_RELACOUNT,
            // DT_GNU_HASH) carry nothing the loader has to act on.
            _ => {}
        }
    }
    if !terminated {
        return Err(RelocError::DynamicUnterminated);
    }

    let (vaddr, size) = match (rela, relasz, relaent) {
        (None, None, None) => return Ok(None),
        (Some(v), Some(sz), Some(ent)) => {
            if ent != RELA_ENT_SIZE {
                return Err(RelocError::BadRelaEntSize);
            }
            (v, sz)
        }
        _ => return Err(RelocError::IncompleteRela),
    };
    if size % RELA_ENT_SIZE != 0 {
        return Err(RelocError::BadRelaSize);
    }
    if size == 0 {
        return Ok(None);
    }
    if vaddr % 8 != 0 || !in_file_bytes(segs, image.len(), vaddr, size) {
        return Err(RelocError::RelaOutOfImage);
    }
    Ok(Some(RelaTable { vaddr, size }))
}

/// Apply every entry of `table` to `image`: write `load_base + r_addend` to
/// the 8-byte place at link VA `r_offset`. Returns the number of entries
/// applied. Stops at the first invalid entry; the caller discards the image
/// on error (the loader frees the TPA), so a half-relocated image never runs.
pub fn apply_rela(
    image: &mut [u8],
    img: &ElfImage,
    table: RelaTable,
    load_base: u64,
) -> Result<usize, RelocError> {
    let segs = img.segments();
    let dyn_range = img
        .dynamic
        .map(|d| (d.vaddr, d.file_size))
        .unwrap_or((0, 0));
    let count = table.size / RELA_ENT_SIZE;
    for i in 0..count {
        let at = table.vaddr + i * RELA_ENT_SIZE;
        // In range: `find_rela` checked the whole table against `image`.
        let r_offset = rd_u64(image, at).ok_or(RelocError::RelaOutOfImage)?;
        let r_info = rd_u64(image, at + 8).ok_or(RelocError::RelaOutOfImage)?;
        let r_addend = rd_u64(image, at + 16).ok_or(RelocError::RelaOutOfImage)? as i64;

        let r_type = (r_info & 0xffff_ffff) as u32;
        let r_sym = r_info >> 32;
        if r_type != R_AARCH64_RELATIVE {
            return Err(RelocError::UnsupportedType(r_type));
        }
        if r_sym != 0 {
            return Err(RelocError::SymbolNotNull);
        }
        if r_offset % 8 != 0 {
            return Err(RelocError::PlaceMisaligned);
        }
        if !in_writable_segment(segs, r_offset)
            || r_offset.checked_add(8).is_none_or(|e| e > image.len() as u64)
        {
            return Err(RelocError::PlaceNotWritable);
        }
        if overlaps(r_offset, 8, table.vaddr, table.size)
            || overlaps(r_offset, 8, dyn_range.0, dyn_range.1)
        {
            return Err(RelocError::PlaceInTables);
        }
        // Delta(S) + A with S the null symbol: the execution address of
        // link VA 0 is `load_base`, so every place moves by `load_base`.
        let value = load_base
            .checked_add_signed(r_addend)
            .ok_or(RelocError::ValueOverflow)?;
        let p = r_offset as usize;
        image[p..p + 8].copy_from_slice(&value.to_le_bytes());
    }
    Ok(count as usize)
}

/// [`find_rela`] + [`apply_rela`]: relocate `image` for execution at
/// `load_base`. Returns the number of relocations applied (0 for an image
/// without a dynamic section or without relocations).
pub fn relocate(image: &mut [u8], img: &ElfImage, load_base: u64) -> Result<usize, RelocError> {
    match find_rela(image, img)? {
        Some(table) => apply_rela(image, img, table, load_base),
        None => Ok(0),
    }
}
