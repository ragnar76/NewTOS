//! Minimal ELF64 parser for the AArch64 process loader (Phase 06 D3).
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
//! - `ELFCLASS64` (8-byte words)
//! - `ELFDATA2LSB` (little-endian — AArch64 default)
//! - `ET_EXEC` (statically-linked executable; `ET_DYN` / PIE will
//!   come when relocation support is needed)
//! - `EM_AARCH64`
//! - Program headers: only `PT_LOAD` entries are extracted. Other
//!   types (`PT_NOTE`, `PT_GNU_*`) are ignored without erroring.
//! - At most [`MAX_LOAD_SEGMENTS`] (currently 8) `PT_LOAD` segments
//!   per image — enough for text + data + bss splits, which is all
//!   a typical TOS source-port produces.
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
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u8 = 1;

const ET_EXEC: u16 = 2;
const EM_AARCH64: u16 = 0xb7;

const PT_LOAD: u32 = 1;

/// `p_flags` bit indicating the segment is executable.
pub const PF_X: u32 = 0x1;
/// `p_flags` bit indicating the segment is writable.
pub const PF_W: u32 = 0x2;
/// `p_flags` bit indicating the segment is readable.
pub const PF_R: u32 = 0x4;

// Offsets within the ELF64 header (Ehdr) — we only need a handful.
// Reference: ELF gABI § "ELF Header".
const E_TYPE_OFF: usize = 16;
const E_MACHINE_OFF: usize = 18;
const E_VERSION_OFF: usize = 20;
const E_ENTRY_OFF: usize = 24;
const E_PHOFF_OFF: usize = 32;
const E_PHENTSIZE_OFF: usize = 54;
const E_PHNUM_OFF: usize = 56;

const ELF64_EHDR_SIZE: usize = 64;
const ELF64_PHDR_SIZE: u16 = 56;

// Offsets within Phdr64. Reference: ELF gABI § "Program Header".
const P_TYPE_OFF: usize = 0;
const P_FLAGS_OFF: usize = 4;
const P_OFFSET_OFF: usize = 8;
const P_VADDR_OFF: usize = 16;
const P_FILESZ_OFF: usize = 32;
const P_MEMSZ_OFF: usize = 40;

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

/// A parsed ELF image — header summary plus the `PT_LOAD` table.
#[derive(Copy, Clone, Debug)]
pub struct ElfImage {
    /// User entry point (`e_entry`, AArch64 VA).
    pub entry_va: u64,
    /// `PT_LOAD` segments, in program-header order.
    segments: [LoadSegment; MAX_LOAD_SEGMENTS],
    /// Number of valid entries in `segments`.
    num_segments: usize,
}

impl ElfImage {
    /// Slice of the populated load segments.
    pub fn segments(&self) -> &[LoadSegment] {
        &self.segments[..self.num_segments]
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
    /// `EI_CLASS != ELFCLASS64`.
    NotElf64,
    /// `EI_DATA != ELFDATA2LSB`.
    NotLittleEndian,
    /// `EI_VERSION` or `e_version` not `EV_CURRENT`.
    BadVersion,
    /// `e_type != ET_EXEC`.
    NotExecutable,
    /// `e_machine != EM_AARCH64`.
    NotAArch64,
    /// `e_phentsize` is not 56 (ELF64 phdr size).
    BadPhEntSize,
    /// More than [`MAX_LOAD_SEGMENTS`] `PT_LOAD` entries.
    TooManySegments,
    /// A program header would extend past the blob.
    PhdrOutOfRange,
    /// A `PT_LOAD` segment's file range extends past the blob.
    SegmentOutOfRange,
    /// `p_filesz > p_memsz` for a `PT_LOAD` (BSS would be negative).
    BadSegmentSizes,
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

// ─── Parser ────────────────────────────────────────────────────────

/// Parse an ELF64 AArch64 executable from a byte blob.
///
/// On success, returns an [`ElfImage`] whose `segments()` slice is
/// ready for [`super::loader`] to consume. On failure, returns the
/// most specific [`ParseError`] that explains why the blob was
/// rejected.
pub fn parse(blob: &[u8]) -> Result<ElfImage, ParseError> {
    if blob.len() < ELF64_EHDR_SIZE {
        return Err(ParseError::TooShort);
    }

    // ─── e_ident ────────────────────────────────────────────────
    if &blob[0..4] != EI_MAG {
        return Err(ParseError::BadMagic);
    }
    if blob[EI_CLASS_OFF] != ELFCLASS64 {
        return Err(ParseError::NotElf64);
    }
    if blob[EI_DATA_OFF] != ELFDATA2LSB {
        return Err(ParseError::NotLittleEndian);
    }
    if blob[EI_VERSION_OFF] != EV_CURRENT {
        return Err(ParseError::BadVersion);
    }
    let _ = EI_NIDENT; // remaining e_ident bytes are OS/ABI — accept any.

    // ─── e_type / e_machine / e_version ─────────────────────────
    let e_type = read_u16_le(blob, E_TYPE_OFF).ok_or(ParseError::TooShort)?;
    if e_type != ET_EXEC {
        return Err(ParseError::NotExecutable);
    }
    let e_machine = read_u16_le(blob, E_MACHINE_OFF).ok_or(ParseError::TooShort)?;
    if e_machine != EM_AARCH64 {
        return Err(ParseError::NotAArch64);
    }
    let e_version = read_u32_le(blob, E_VERSION_OFF).ok_or(ParseError::TooShort)?;
    if e_version != EV_CURRENT as u32 {
        return Err(ParseError::BadVersion);
    }

    // ─── Header pointers ────────────────────────────────────────
    let entry_va = read_u64_le(blob, E_ENTRY_OFF).ok_or(ParseError::TooShort)?;
    let phoff = read_u64_le(blob, E_PHOFF_OFF).ok_or(ParseError::TooShort)?;
    let phentsize = read_u16_le(blob, E_PHENTSIZE_OFF).ok_or(ParseError::TooShort)?;
    if phentsize != ELF64_PHDR_SIZE {
        return Err(ParseError::BadPhEntSize);
    }
    let phnum = read_u16_le(blob, E_PHNUM_OFF).ok_or(ParseError::TooShort)?;

    // ─── Walk program headers ───────────────────────────────────
    let mut segments = [LoadSegment {
        vaddr: 0,
        flags: 0,
        file_offset: 0,
        file_size: 0,
        mem_size: 0,
    }; MAX_LOAD_SEGMENTS];
    let mut num_segments = 0usize;

    for i in 0..(phnum as usize) {
        let ph_off = (phoff as usize)
            .checked_add(i * (phentsize as usize))
            .ok_or(ParseError::PhdrOutOfRange)?;
        let end = ph_off
            .checked_add(phentsize as usize)
            .ok_or(ParseError::PhdrOutOfRange)?;
        if end > blob.len() {
            return Err(ParseError::PhdrOutOfRange);
        }

        let p_type = read_u32_le(blob, ph_off + P_TYPE_OFF)
            .ok_or(ParseError::PhdrOutOfRange)?;
        if p_type != PT_LOAD {
            // PT_NOTE, PT_GNU_STACK etc. — ignore.
            continue;
        }

        if num_segments >= MAX_LOAD_SEGMENTS {
            return Err(ParseError::TooManySegments);
        }

        let flags = read_u32_le(blob, ph_off + P_FLAGS_OFF)
            .ok_or(ParseError::PhdrOutOfRange)?;
        let file_offset = read_u64_le(blob, ph_off + P_OFFSET_OFF)
            .ok_or(ParseError::PhdrOutOfRange)?;
        let vaddr = read_u64_le(blob, ph_off + P_VADDR_OFF)
            .ok_or(ParseError::PhdrOutOfRange)?;
        let file_size = read_u64_le(blob, ph_off + P_FILESZ_OFF)
            .ok_or(ParseError::PhdrOutOfRange)?;
        let mem_size = read_u64_le(blob, ph_off + P_MEMSZ_OFF)
            .ok_or(ParseError::PhdrOutOfRange)?;

        if file_size > mem_size {
            return Err(ParseError::BadSegmentSizes);
        }
        // Validate the file range fits in the blob.
        let file_end = file_offset
            .checked_add(file_size)
            .ok_or(ParseError::SegmentOutOfRange)?;
        if file_end > blob.len() as u64 {
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

    Ok(ElfImage { entry_va, segments, num_segments })
}
