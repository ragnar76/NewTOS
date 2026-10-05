//! Minimal ELF32 / ELF64 AArch64 parser for the process loader
//! (Phase 06 D3; ELF32 path added for the ILP32 pivot, ADR-020).
//!
//! Reads the bare minimum to feed [`super::loader`]: the ELF header,
//! the program-header table, and a pre-validated array of `PT_LOAD`
//! segment descriptors. Section headers are intentionally ignored —
//! they're only needed for dynamic linking, which we won't support
//! for source-port programs (each program is statically linked
//! against the NewTOS / `<gemdos.h>` shim).
//!
//! ## Scope
//!
//! Accepts:
//!
//! - `ELFCLASS64` (8-byte words) **and** `ELFCLASS32` (4-byte words).
//!   The class byte (`EI_CLASS`) is dispatched at parse time: LP64-native
//!   NewTOS programs and the embedded selftests are ELF64, while ILP32
//!   apps built with the GNU `-mabi=ilp32` toolchain are ELF32.
//!
//!   **Parsing ELF32 is not the same as running it.** Since 2026-09-04
//!   (ADR-020 addendum) NewTOS is 64-bit-only and no ILP32 program can be
//!   built; [`super::loader::load_to_slot`] refuses an `ELFCLASS32` image
//!   with `LoadError::Ilp32Unsupported` → `EPLFMT`. The class handling
//!   here is kept deliberately — it is the substrate the future m68k
//!   thunk needs — but nothing reaches EL0 through it today.
//!
//!   The two
//!   classes differ in header size, in the width of the address/offset/
//!   size fields, and — critically — in the *order* of the program-header
//!   fields (`p_flags` sits right after `p_type` in ELF64 but at the end
//!   of the ELF32 phdr). All of that is captured in [`ClassLayout`] so a
//!   single validated parse loop serves both. Every extracted `PT_LOAD`
//!   descriptor is class-agnostic (widened to `u64`), so the whole
//!   downstream loader — streaming, basepage, cache-sync — is unchanged.
//! - `ELFDATA2LSB` (little-endian — AArch64 default)
//! - `ET_DYN` only — a position-independent executable (ARCH03 stage 0d,
//!   `docs/analysis/arch03-implementation-plan.md` Frage 3 / E7). SYSVABI64
//!   2025Q4 §9 (p. 23): "Position-independent executables are mapped in the
//!   same way as shared objects. They have an ELF e_type of ET_DYN." A
//!   fixed-address `ET_EXEC` image is refused with
//!   [`ParseError::NotPositionIndependent`] (→ `EPLFMT`): every NewTOS
//!   program is rebuilt as PIE, and a second, non-relocating load path
//!   would be a second loader (E7).
//! - `EM_AARCH64` (183) and `e_flags == 0` — AAELF64 2025Q4 §5.2 (p. 13):
//!   "There are no processor-specific flags so this field shall contain
//!   zero."
//! - Program headers: `PT_LOAD` entries are extracted; the (at most one)
//!   `PT_DYNAMIC` entry is recorded for the relocation step
//!   ([`super::reloc`]); `PT_NOTE` entries (at most [`MAX_NOTE_SEGMENTS`])
//!   are recorded so the loader can read the NewTOS `PRGFLAGS` note
//!   ([`find_prgflags_note`], ARCH03 E8b); a `PT_INTERP` entry (a program
//!   asking for a dynamic linker, gABI "Program Interpreter") is refused.
//!   Other types (`PT_GNU_*`) are ignored without erroring.
//! - At most [`MAX_LOAD_SEGMENTS`] (currently 8) `PT_LOAD` segments
//!   per image — enough for text + data + bss splits, which is all
//!   a typical TOS source-port produces.
//!
//! The dynamic section itself is **not** read here: its bytes live inside
//! a `PT_LOAD` segment and are interpreted after the segments have been
//! streamed into the TPA (see [`super::reloc`]). That also keeps the
//! `Elf64_Dyn`/`Elf64_Rela` walk away from ELF32 images, which the loader
//! refuses before relocation.
//!
//! Rejects with a precise [`ParseError`] anything that doesn't
//! match. No allocation, no unsafe — pure byte parsing.

use core::convert::TryInto;

// ─── Header / phdr field offsets and constants ──────────────────────

const EI_NIDENT: usize = 16;
const EI_MAG: &[u8; 4] = b"\x7fELF";
const EI_CLASS_OFF: usize = 4;
const EI_DATA_OFF: usize = 5;
const EI_VERSION_OFF: usize = 6;
const ELFCLASS32: u8 = 1;
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u8 = 1;

// gABI `e_type` values (gABI 2013-06-10, ch4.eheader "ELF Header").
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EM_AARCH64: u16 = 0xb7;

// gABI `p_type` values (ch5.pheader "Program Header").
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_NOTE: u32 = 4;

/// `p_flags` bit indicating the segment is executable.
pub const PF_X: u32 = 0x1;
/// `p_flags` bit indicating the segment is writable.
pub const PF_W: u32 = 0x2;
/// `p_flags` bit indicating the segment is readable.
pub const PF_R: u32 = 0x4;

// The `e_ident` block (magic, class, data, version) is byte-identical
// across ELF32/ELF64, and so are `e_type` / `e_machine` / `e_version`,
// which sit at fixed offsets 16 / 18 / 20 regardless of class.
// Reference: ELF gABI § "ELF Header".
const E_TYPE_OFF: usize = 16;
const E_MACHINE_OFF: usize = 18;
const E_VERSION_OFF: usize = 20;

// ── ELF64 Ehdr / Phdr layout ────────────────────────────────────────
// e_entry / e_phoff are 8-byte words; the header is 64 bytes.
const E64_ENTRY_OFF: usize = 24;
const E64_PHOFF_OFF: usize = 32;
const E64_FLAGS_OFF: usize = 48;
const E64_PHENTSIZE_OFF: usize = 54;
const E64_PHNUM_OFF: usize = 56;
const ELF64_EHDR_SIZE: usize = 64;
const ELF64_PHDR_SIZE: u16 = 56;
// Phdr64 field offsets (ELF gABI § "Program Header"): p_flags sits
// immediately after p_type, and the address/size fields are 8-byte.
const P64_TYPE_OFF: usize = 0;
const P64_FLAGS_OFF: usize = 4;
const P64_OFFSET_OFF: usize = 8;
const P64_VADDR_OFF: usize = 16;
const P64_FILESZ_OFF: usize = 32;
const P64_MEMSZ_OFF: usize = 40;
const P64_ALIGN_OFF: usize = 48;

// ── ELF32 Ehdr / Phdr layout ────────────────────────────────────────
// e_entry / e_phoff are 4-byte words and therefore sit 4 bytes earlier
// than in ELF64; the header is 52 bytes.
const E32_ENTRY_OFF: usize = 24;
const E32_PHOFF_OFF: usize = 28;
const E32_FLAGS_OFF: usize = 36;
const E32_PHENTSIZE_OFF: usize = 42;
const E32_PHNUM_OFF: usize = 44;
const ELF32_EHDR_SIZE: usize = 52;
const ELF32_PHDR_SIZE: u16 = 32;
// Phdr32 field offsets — the ORDER DIFFERS from ELF64: p_flags is the
// second-to-last field (offset 24), not immediately after p_type. All
// fields are 4-byte. Copying the ELF64 offsets here would mis-parse.
const P32_TYPE_OFF: usize = 0;
const P32_OFFSET_OFF: usize = 4;
const P32_VADDR_OFF: usize = 8;
const P32_FILESZ_OFF: usize = 16;
const P32_MEMSZ_OFF: usize = 20;
const P32_FLAGS_OFF: usize = 24;
const P32_ALIGN_OFF: usize = 28;

/// Per-class byte layout of the header/phdr fields the loader needs.
///
/// The `e_ident` block and `e_type`/`e_machine`/`e_version` are checked
/// with fixed offsets before a layout is selected; everything captured
/// here differs between ELF32 and ELF64. `wide` selects 8-byte (ELF64)
/// vs 4-byte (ELF32) reads for the address / offset / size fields so a
/// single validated parse loop serves both classes.
#[derive(Copy, Clone)]
struct ClassLayout {
    /// Total ELF header size (`e_ehsize`): 52 (ELF32) or 64 (ELF64).
    ehdr_size: usize,
    /// Expected `e_phentsize`: 32 (ELF32) or 56 (ELF64).
    phdr_size: u16,
    e_entry: usize,
    e_phoff: usize,
    e_flags: usize,
    e_phentsize: usize,
    e_phnum: usize,
    p_type: usize,
    p_flags: usize,
    p_offset: usize,
    p_vaddr: usize,
    p_filesz: usize,
    p_memsz: usize,
    p_align: usize,
    /// `true` for ELF64 (8-byte words), `false` for ELF32 (4-byte).
    wide: bool,
}

const LAYOUT64: ClassLayout = ClassLayout {
    ehdr_size: ELF64_EHDR_SIZE,
    phdr_size: ELF64_PHDR_SIZE,
    e_entry: E64_ENTRY_OFF,
    e_phoff: E64_PHOFF_OFF,
    e_flags: E64_FLAGS_OFF,
    e_phentsize: E64_PHENTSIZE_OFF,
    e_phnum: E64_PHNUM_OFF,
    p_type: P64_TYPE_OFF,
    p_flags: P64_FLAGS_OFF,
    p_offset: P64_OFFSET_OFF,
    p_vaddr: P64_VADDR_OFF,
    p_filesz: P64_FILESZ_OFF,
    p_memsz: P64_MEMSZ_OFF,
    p_align: P64_ALIGN_OFF,
    wide: true,
};

const LAYOUT32: ClassLayout = ClassLayout {
    ehdr_size: ELF32_EHDR_SIZE,
    phdr_size: ELF32_PHDR_SIZE,
    e_entry: E32_ENTRY_OFF,
    e_phoff: E32_PHOFF_OFF,
    e_flags: E32_FLAGS_OFF,
    e_phentsize: E32_PHENTSIZE_OFF,
    e_phnum: E32_PHNUM_OFF,
    p_type: P32_TYPE_OFF,
    p_flags: P32_FLAGS_OFF,
    p_offset: P32_OFFSET_OFF,
    p_vaddr: P32_VADDR_OFF,
    p_filesz: P32_FILESZ_OFF,
    p_memsz: P32_MEMSZ_OFF,
    p_align: P32_ALIGN_OFF,
    wide: false,
};

// ─── Output types ──────────────────────────────────────────────────

/// Maximum number of `PT_LOAD` segments the parser will record per
/// image. Static cap so the parser is `no_std` / `no_alloc`.
pub const MAX_LOAD_SEGMENTS: usize = 8;

/// One `PT_LOAD` segment descriptor — by-value, byte-offsets into
/// the original ELF blob. The loader uses `file_offset` /
/// `file_size` to find the bytes to copy and `mem_size` minus
/// `file_size` to know how much BSS to zero past the file end.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct LoadSegment {
    /// User-space VA where this segment must be mapped.
    pub vaddr: u64,
    /// `p_flags` — combination of [`PF_R`], [`PF_W`], [`PF_X`].
    pub flags: u32,
    /// Byte offset into the ELF blob where this segment's file data
    /// lives.
    pub file_offset: u64,
    /// Number of bytes in the file image.
    pub file_size: u64,
    /// Total size in memory (≥ `file_size`); the trailing
    /// `mem_size - file_size` bytes are zero-initialised BSS.
    pub mem_size: u64,
}

/// The `PT_DYNAMIC` program header of an image — where its dynamic
/// section (`Elf64_Dyn` array, gABI ch5.dynamic) lies. Link-time VA and
/// sizes, exactly as in the program header; [`super::reloc`] validates
/// that the range is backed by file bytes of a `PT_LOAD` segment before
/// reading it.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct DynamicSegment {
    /// Link-time VA of the dynamic section (`p_vaddr`).
    pub vaddr: u64,
    /// `p_filesz` — the bytes of the array present in the file.
    pub file_size: u64,
    /// `p_memsz`.
    pub mem_size: u64,
}

/// Maximum number of `PT_NOTE` program headers the parser records. Our
/// linker script emits one; the cap keeps the parser `no_alloc` and bounds
/// the work the loader does before it allocates anything.
pub const MAX_NOTE_SEGMENTS: usize = 4;

/// Largest `PT_NOTE` segment the loader reads (bytes). The NewTOS note is
/// 24 bytes; anything larger than this is refused rather than skipped, so
/// a `PRGFLAGS` entry can never hide past the part that was looked at.
pub const MAX_NOTE_BYTES: u64 = 4096;

/// One `PT_NOTE` program header — where a note segment's bytes are in the
/// file (gABI ch5.pheader "Note Section"). The loader reads them before it
/// allocates the TPA, because the `PRGFLAGS` note picks the pool.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct NoteSegment {
    /// `p_offset` — byte offset of the notes in the file.
    pub file_offset: u64,
    /// `p_filesz` — byte length of the notes.
    pub file_size: u64,
    /// `p_align` — the padding unit of name and descriptor (0/1 mean 4).
    pub align: u64,
}

/// `namesz`/`name` of the NewTOS `PRGFLAGS` note (ARCH03 E8b): `"NewTOS"`.
pub const NOTE_NAME_NEWTOS: &[u8; 7] = b"NewTOS\0";
/// `type` of the NewTOS `PRGFLAGS` note.
pub const NT_NEWTOS_PRGFLAGS: u32 = 1;

/// A parsed ELF image — header summary plus the `PT_LOAD` table.
#[derive(Copy, Clone, Debug)]
pub struct ElfImage {
    /// User entry point (`e_entry`, AArch64 VA).
    pub entry_va: u64,
    /// The image's `PT_DYNAMIC` header, if it has one. A PIE without
    /// dynamic relocations (the hand-built selftest blobs) has none.
    pub dynamic: Option<DynamicSegment>,
    /// `true` for `ELFCLASS32` (ILP32, 4-byte pointers/`long`), `false`
    /// for `ELFCLASS64` (LP64). The loader records this in the process
    /// slot so the GEM trap readers use the program's pointer width; all
    /// other parsed fields are class-agnostic (widened to `u64`).
    is_ilp32: bool,
    /// `PT_LOAD` segments, in program-header order.
    segments: [LoadSegment; MAX_LOAD_SEGMENTS],
    /// Number of valid entries in `segments`.
    num_segments: usize,
    /// `PT_NOTE` headers, in program-header order.
    notes: [NoteSegment; MAX_NOTE_SEGMENTS],
    /// Number of valid entries in `notes`.
    num_notes: usize,
}

impl ElfImage {
    /// Slice of the populated load segments.
    pub fn segments(&self) -> &[LoadSegment] {
        &self.segments[..self.num_segments]
    }

    /// Slice of the image's `PT_NOTE` headers.
    pub fn notes(&self) -> &[NoteSegment] {
        &self.notes[..self.num_notes]
    }

    /// `true` when the image is `ELFCLASS32` (ILP32 / GNU `-mabi=ilp32`).
    #[inline]
    pub fn is_ilp32(&self) -> bool {
        self.is_ilp32
    }
}

// ─── Errors ────────────────────────────────────────────────────────

/// Why an ELF blob was rejected. Precise enough that the caller
/// can blame the producer (e.g. wrong `e_machine`) without needing
/// to re-parse.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ParseError {
    /// The blob is shorter than the ELF header itself.
    TooShort,
    /// First four bytes are not `\x7fELF`.
    BadMagic,
    /// `EI_CLASS` is neither `ELFCLASS32` (1) nor `ELFCLASS64` (2).
    NotElf64,
    /// `EI_DATA != ELFDATA2LSB`.
    NotLittleEndian,
    /// `EI_VERSION` or `e_version` not `EV_CURRENT`.
    BadVersion,
    /// `e_type` is neither `ET_DYN` nor `ET_EXEC` (a relocatable object,
    /// a core file, …).
    NotExecutable,
    /// `e_type == ET_EXEC`: a fixed-address executable. NewTOS loads
    /// position-independent executables (`ET_DYN`) only — ARCH03 E7, the
    /// hard cut after every in-tree program was rebuilt as PIE (stage 0d).
    NotPositionIndependent,
    /// `e_machine != EM_AARCH64`.
    NotAArch64,
    /// `e_flags != 0`. AAELF64 §5.2 (p. 13): "There are no
    /// processor-specific flags so this field shall contain zero."
    BadFlags,
    /// The image has a `PT_INTERP` header — it asks for a dynamic linker
    /// (gABI ch5.dynamic "Program Interpreter"). NewTOS has none.
    HasInterpreter,
    /// More than one `PT_DYNAMIC` header. gABI ch5.dynamic "Dynamic
    /// Section": the `PT_DYNAMIC` element is the segment that "contains the
    /// .dynamic section" — one array; two would leave it ambiguous which
    /// relocation table is meant.
    MultipleDynamic,
    /// `e_phentsize` is not the class's phdr size (32 for ELF32, 56 for
    /// ELF64).
    BadPhEntSize,
    /// More than [`MAX_LOAD_SEGMENTS`] `PT_LOAD` entries.
    TooManySegments,
    /// A program header would extend past the blob.
    PhdrOutOfRange,
    /// A `PT_LOAD` segment's file range extends past the blob.
    SegmentOutOfRange,
    /// `p_filesz > p_memsz` for a `PT_LOAD` (BSS would be negative).
    BadSegmentSizes,
    /// More than [`MAX_NOTE_SEGMENTS`] `PT_NOTE` headers, or one larger than
    /// [`MAX_NOTE_BYTES`], or one whose bytes are not inside the file.
    BadNoteSegment,
    /// A note entry that does not fit its segment, a `p_align` other than
    /// 0/1/4/8, or a NewTOS `PRGFLAGS` note that is not exactly one `u32`
    /// or occurs twice (ARCH03 E8b, `docs/api/gemdos/process/4b-pexec.md`).
    BadNote,
}

// ─── Helpers ────────────────────────────────────────────────────────

#[inline]
fn read_u16_le(b: &[u8], off: usize) -> Option<u16> {
    b.get(off..off + 2).map(|s| u16::from_le_bytes(s.try_into().unwrap()))
}
#[inline]
fn read_u32_le(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()))
}
#[inline]
fn read_u64_le(b: &[u8], off: usize) -> Option<u64> {
    b.get(off..off + 8).map(|s| u64::from_le_bytes(s.try_into().unwrap()))
}
/// Read an address / offset / size field, widening to `u64`: an 8-byte
/// word in ELF64, a 4-byte (zero-extended) word in ELF32.
#[inline]
fn read_word(b: &[u8], off: usize, wide: bool) -> Option<u64> {
    if wide {
        read_u64_le(b, off)
    } else {
        read_u32_le(b, off).map(u64::from)
    }
}

// ─── Parser ────────────────────────────────────────────────────────

/// Parse an ELF32 or ELF64 AArch64 position-independent executable header.
///
/// The `EI_CLASS` byte selects a [`ClassLayout`]; from there a single
/// validated loop parses either class. `header` must hold at least the
/// ELF header **and** the entire program-header table — the loader reads
/// them from a small fixed buffer for a streamed file, or hands the whole
/// in-memory blob for an embedded image. Segment *payloads* are never read
/// here, only their program-header descriptors. `total_len` is the full
/// image length in bytes (the on-disk file size, which may be far larger
/// than `header`) and is what the `PT_LOAD` file-range check validates
/// against — so a segment whose bytes stream straight into the TPA is
/// accepted even though they are absent from `header`.
///
/// On success, returns an [`ElfImage`] whose `segments()` slice is
/// ready for [`super::loader`] to consume (addresses/sizes widened to
/// `u64`, so an ILP32 image is indistinguishable downstream). On failure,
/// returns the most specific [`ParseError`] that explains why the image
/// was rejected.
pub fn parse(header: &[u8], total_len: u64) -> Result<ElfImage, ParseError> {
    // ─── e_ident (class-independent) ────────────────────────────
    // Magic, class, data and version all live in the 16-byte e_ident
    // block, so validate them and pick the class layout before assuming
    // any class-specific header size.
    if header.len() < EI_NIDENT {
        return Err(ParseError::TooShort);
    }
    if &header[0..4] != EI_MAG {
        return Err(ParseError::BadMagic);
    }
    let layout = match header[EI_CLASS_OFF] {
        ELFCLASS64 => LAYOUT64,
        ELFCLASS32 => LAYOUT32,
        _ => return Err(ParseError::NotElf64),
    };
    if header[EI_DATA_OFF] != ELFDATA2LSB {
        return Err(ParseError::NotLittleEndian);
    }
    if header[EI_VERSION_OFF] != EV_CURRENT {
        return Err(ParseError::BadVersion);
    }

    // With the class fixed, the whole ELF header must be present.
    if header.len() < layout.ehdr_size {
        return Err(ParseError::TooShort);
    }

    // ─── e_type / e_machine / e_version (fixed offsets) ─────────
    let e_type = read_u16_le(header, E_TYPE_OFF).ok_or(ParseError::TooShort)?;
    match e_type {
        ET_DYN => {}
        ET_EXEC => return Err(ParseError::NotPositionIndependent),
        _ => return Err(ParseError::NotExecutable),
    }
    let e_machine = read_u16_le(header, E_MACHINE_OFF).ok_or(ParseError::TooShort)?;
    if e_machine != EM_AARCH64 {
        return Err(ParseError::NotAArch64);
    }
    let e_version = read_u32_le(header, E_VERSION_OFF).ok_or(ParseError::TooShort)?;
    if e_version != EV_CURRENT as u32 {
        return Err(ParseError::BadVersion);
    }
    let e_flags = read_u32_le(header, layout.e_flags).ok_or(ParseError::TooShort)?;
    if e_flags != 0 {
        return Err(ParseError::BadFlags);
    }

    // ─── Header pointers (class-specific offset + width) ────────
    let entry_va =
        read_word(header, layout.e_entry, layout.wide).ok_or(ParseError::TooShort)?;
    let phoff =
        read_word(header, layout.e_phoff, layout.wide).ok_or(ParseError::TooShort)?;
    let phentsize =
        read_u16_le(header, layout.e_phentsize).ok_or(ParseError::TooShort)?;
    if phentsize != layout.phdr_size {
        return Err(ParseError::BadPhEntSize);
    }
    let phnum = read_u16_le(header, layout.e_phnum).ok_or(ParseError::TooShort)?;

    // ─── Walk program headers ───────────────────────────────────
    let mut segments = [LoadSegment {
        vaddr: 0,
        flags: 0,
        file_offset: 0,
        file_size: 0,
        mem_size: 0,
    }; MAX_LOAD_SEGMENTS];
    let mut num_segments = 0usize;
    let mut dynamic: Option<DynamicSegment> = None;
    let mut notes = [NoteSegment {
        file_offset: 0,
        file_size: 0,
        align: 0,
    }; MAX_NOTE_SEGMENTS];
    let mut num_notes = 0usize;

    for i in 0..(phnum as usize) {
        let ph_off = (phoff as usize)
            .checked_add(i * (phentsize as usize))
            .ok_or(ParseError::PhdrOutOfRange)?;
        let end = ph_off
            .checked_add(phentsize as usize)
            .ok_or(ParseError::PhdrOutOfRange)?;
        // The phdr table must lie inside the header window; if a producer
        // placed it beyond the loader's header buffer this fails cleanly
        // rather than mis-parsing (normal toolchains emit phoff == ehdr
        // size: 52 for ELF32, 64 for ELF64).
        if end > header.len() {
            return Err(ParseError::PhdrOutOfRange);
        }

        let p_type = read_u32_le(header, ph_off + layout.p_type)
            .ok_or(ParseError::PhdrOutOfRange)?;
        match p_type {
            PT_LOAD => {}
            PT_INTERP => return Err(ParseError::HasInterpreter),
            PT_DYNAMIC => {
                if dynamic.is_some() {
                    return Err(ParseError::MultipleDynamic);
                }
                let vaddr = read_word(header, ph_off + layout.p_vaddr, layout.wide)
                    .ok_or(ParseError::PhdrOutOfRange)?;
                let file_size = read_word(header, ph_off + layout.p_filesz, layout.wide)
                    .ok_or(ParseError::PhdrOutOfRange)?;
                let mem_size = read_word(header, ph_off + layout.p_memsz, layout.wide)
                    .ok_or(ParseError::PhdrOutOfRange)?;
                if file_size > mem_size {
                    return Err(ParseError::BadSegmentSizes);
                }
                dynamic = Some(DynamicSegment {
                    vaddr,
                    file_size,
                    mem_size,
                });
                continue;
            }
            PT_NOTE => {
                if num_notes >= MAX_NOTE_SEGMENTS {
                    return Err(ParseError::BadNoteSegment);
                }
                let file_offset = read_word(header, ph_off + layout.p_offset, layout.wide)
                    .ok_or(ParseError::PhdrOutOfRange)?;
                let file_size = read_word(header, ph_off + layout.p_filesz, layout.wide)
                    .ok_or(ParseError::PhdrOutOfRange)?;
                let align = read_word(header, ph_off + layout.p_align, layout.wide)
                    .ok_or(ParseError::PhdrOutOfRange)?;
                let in_file = file_offset
                    .checked_add(file_size)
                    .is_some_and(|end| end <= total_len);
                if file_size > MAX_NOTE_BYTES || !in_file {
                    return Err(ParseError::BadNoteSegment);
                }
                notes[num_notes] = NoteSegment {
                    file_offset,
                    file_size,
                    align,
                };
                num_notes += 1;
                continue;
            }
            // PT_GNU_STACK etc. — ignore.
            _ => continue,
        }

        if num_segments >= MAX_LOAD_SEGMENTS {
            return Err(ParseError::TooManySegments);
        }

        let flags = read_u32_le(header, ph_off + layout.p_flags)
            .ok_or(ParseError::PhdrOutOfRange)?;
        let file_offset = read_word(header, ph_off + layout.p_offset, layout.wide)
            .ok_or(ParseError::PhdrOutOfRange)?;
        let vaddr = read_word(header, ph_off + layout.p_vaddr, layout.wide)
            .ok_or(ParseError::PhdrOutOfRange)?;
        let file_size = read_word(header, ph_off + layout.p_filesz, layout.wide)
            .ok_or(ParseError::PhdrOutOfRange)?;
        let mem_size = read_word(header, ph_off + layout.p_memsz, layout.wide)
            .ok_or(ParseError::PhdrOutOfRange)?;

        if file_size > mem_size {
            return Err(ParseError::BadSegmentSizes);
        }
        // Validate the file range fits in the full image. `total_len` is
        // the on-disk file size, not the header window, so a large segment
        // that streams straight into the TPA passes here (this is exactly
        // the check that used to cap the program at the fixed load buffer).
        let file_end = file_offset
            .checked_add(file_size)
            .ok_or(ParseError::SegmentOutOfRange)?;
        if file_end > total_len {
            return Err(ParseError::SegmentOutOfRange);
        }

        segments[num_segments] = LoadSegment {
            vaddr,
            flags,
            file_offset,
            file_size,
            mem_size,
        };
        num_segments += 1;
    }

    // `layout.wide` is true for ELF64 (8-byte words); its negation is the
    // ILP32 (ELFCLASS32) flag the loader records for the GEM trap readers.
    Ok(ElfImage {
        entry_va,
        dynamic,
        is_ilp32: !layout.wide,
        segments,
        num_segments,
        notes,
        num_notes,
    })
}

/// Round `v` up to a multiple of `a` (a power of two).
#[inline]
fn pad_to(v: usize, a: usize) -> Option<usize> {
    v.checked_add(a - 1).map(|x| x & !(a - 1))
}

/// Where the NewTOS `PRGFLAGS` note of one `PT_NOTE` segment is: its value
/// and the byte offset of its descriptor within `notes` (what the host tool
/// rewrites). `None` when the segment carries no such note.
///
/// `notes` is the segment's bytes (`p_filesz` of them), `align` its
/// `p_align`. Entries are 4-byte `namesz`/`descsz`/`type` words followed by
/// the name and the descriptor, each padded to the alignment (4 for
/// `p_align` 0/1/4, 8 for 8) — the layout `rust-lld`/`llvm-readelf` use for
/// ELF64 notes (the format is a NewTOS definition, ARCH03 E8b; spec
/// `docs/api/gemdos/process/4b-pexec.md` "`PRGFLAGS` of a NewTOS program").
/// Entries of other owners or types are skipped.
///
/// # Errors
///
/// [`ParseError::BadNote`] for an entry that does not fit, an unsupported
/// alignment, or a `"NewTOS"` type-1 entry whose descriptor is not exactly
/// 4 bytes or that occurs twice.
pub fn find_prgflags_note(notes: &[u8], align: u64) -> Result<Option<(u32, usize)>, ParseError> {
    let a: usize = match align {
        0 | 1 | 4 => 4,
        8 => 8,
        _ => return Err(ParseError::BadNote),
    };
    let mut found: Option<(u32, usize)> = None;
    let mut off = 0usize;
    while off < notes.len() {
        let namesz = read_u32_le(notes, off).ok_or(ParseError::BadNote)? as usize;
        let descsz = read_u32_le(notes, off + 4).ok_or(ParseError::BadNote)? as usize;
        let ntype = read_u32_le(notes, off + 8).ok_or(ParseError::BadNote)?;
        let name_off = off + 12;
        let desc_off = pad_to(
            name_off.checked_add(namesz).ok_or(ParseError::BadNote)?,
            a,
        )
        .ok_or(ParseError::BadNote)?;
        let desc_end = desc_off.checked_add(descsz).ok_or(ParseError::BadNote)?;
        if desc_end > notes.len() {
            return Err(ParseError::BadNote);
        }
        let name = &notes[name_off..name_off + namesz];
        if name == NOTE_NAME_NEWTOS && ntype == NT_NEWTOS_PRGFLAGS {
            if descsz != 4 || found.is_some() {
                return Err(ParseError::BadNote);
            }
            let v = read_u32_le(notes, desc_off).ok_or(ParseError::BadNote)?;
            found = Some((v, desc_off));
        }
        off = pad_to(desc_end, a).ok_or(ParseError::BadNote)?;
    }
    Ok(found)
}

/// The `PRGFLAGS` value an image's note segments carry — [`find_prgflags_note`]
/// over every `PT_NOTE`, `bytes_of(seg)` supplying each segment's bytes (or
/// `None` when they cannot be read, which is [`ParseError::BadNoteSegment`]).
/// A second `PRGFLAGS` note in another segment is [`ParseError::BadNote`].
///
/// # Errors
///
/// As [`find_prgflags_note`], plus [`ParseError::BadNoteSegment`].
pub fn prgflags_of<'a>(
    img: &ElfImage,
    mut bytes_of: impl FnMut(&NoteSegment) -> Option<&'a [u8]>,
) -> Result<Option<u32>, ParseError> {
    let mut found = None;
    for seg in img.notes() {
        let bytes = bytes_of(seg).ok_or(ParseError::BadNoteSegment)?;
        if let Some((v, _)) = find_prgflags_note(bytes, seg.align)? {
            if found.is_some() {
                return Err(ParseError::BadNote);
            }
            found = Some(v);
        }
    }
    Ok(found)
}
