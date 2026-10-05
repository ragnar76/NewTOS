//! ARCH03 stage 2a (plan E8b, user decision 2026-10-05): a NewTOS program
//! carries its `PRGFLAGS` in an ELF note — owner `"NewTOS"`, type 1, one
//! `u32` — instead of the `h01_flags` word of a `0x601A` PRG header.
//!
//! Sources the expectations come from:
//! * the format definition, `docs/api/gemdos/process/4b-pexec.md` (NewTOS
//!   notes, "`PRGFLAGS` of a NewTOS program"): 4-byte `namesz`/`descsz`/
//!   `type` words, name and desc padded to the `PT_NOTE` alignment (4, or 8
//!   for `p_align` 8) — the layout `rust-lld`/`llvm-readelf` use;
//! * gABI ch5.pheader "Note Section" (`PT_NOTE` = 4; entries of other owners
//!   are someone else's business);
//! * EmuTOS `bdos/proc.c:283,292,313,425`: the flags choose the pool of the
//!   environment and the TPA (`PF_TTRAMLOAD` → `MX_PREFTTRAM`, else
//!   `MX_STRAM`) and become `p_flags`; an unloadable program is `EPLFMT`.

use tos_api::bdos::process::elf::{self, ParseError, MAX_NOTE_BYTES, MAX_NOTE_SEGMENTS};
use tos_api::bdos::process::loader::{self, LoadError};
use tos_api::bdos::process::pexec::PexecError;
use tos_types::error::EPLFMT;
use tos_types::{MX_PREFTTRAM, MX_STRAM, PF_STANDARD, PF_TTRAMLOAD, PF_TTRAMMEM};

const PT_LOAD: u32 = 1;
const PT_NOTE: u32 = 4;

/// One note entry: owner name (without the NUL), type, descriptor, padded
/// to `align`.
fn entry(name: &[u8], ntype: u32, desc: &[u8], align: usize) -> Vec<u8> {
    let mut v = Vec::new();
    let namesz = if name.is_empty() { 0 } else { name.len() + 1 };
    v.extend_from_slice(&(namesz as u32).to_le_bytes());
    v.extend_from_slice(&(desc.len() as u32).to_le_bytes());
    v.extend_from_slice(&ntype.to_le_bytes());
    v.extend_from_slice(name);
    if namesz != 0 {
        v.push(0);
    }
    while v.len() % align != 0 {
        v.push(0);
    }
    v.extend_from_slice(desc);
    while v.len() % align != 0 {
        v.push(0);
    }
    v
}

fn prgflags_entry(v: u32, align: usize) -> Vec<u8> {
    entry(b"NewTOS", 1, &v.to_le_bytes(), align)
}

#[test]
fn the_sdk_note_reads_back_its_value_and_place() {
    let n = prgflags_entry(0x1234_0007, 4);
    assert_eq!(n.len(), 24, "12-byte header + \"NewTOS\\0\" padded to 8 + u32");
    assert_eq!(elf::find_prgflags_note(&n, 4), Ok(Some((0x1234_0007, 20))));
    // p_align 0 and 1 mean "no constraint": the same 4-byte layout.
    assert_eq!(elf::find_prgflags_note(&n, 0), Ok(Some((0x1234_0007, 20))));
    assert_eq!(elf::find_prgflags_note(&n, 1), Ok(Some((0x1234_0007, 20))));
}

#[test]
fn an_eight_aligned_note_segment_pads_to_eight() {
    let n = prgflags_entry(PF_TTRAMLOAD, 8);
    assert_eq!(n.len(), 32, "header 12 + name padded to 24 + desc padded to 32");
    assert_eq!(elf::find_prgflags_note(&n, 8), Ok(Some((PF_TTRAMLOAD, 24))));
}

#[test]
fn notes_of_other_owners_and_types_are_skipped() {
    let mut n = entry(b"GNU", 3, &[0xAB; 20], 4); // a build-id
    n.extend(entry(b"NewTOS", 2, &[1, 2, 3, 4, 5, 6], 4)); // a future NewTOS type
    n.extend(entry(b"", 7, &[], 4)); // nameless, empty
    let at = n.len() + 20;
    n.extend(prgflags_entry(5, 4));
    assert_eq!(elf::find_prgflags_note(&n, 4), Ok(Some((5, at))));
    // Without our entry: nothing found, nothing wrong.
    let only_gnu = entry(b"GNU", 3, &[0; 20], 4);
    assert_eq!(elf::find_prgflags_note(&only_gnu, 4), Ok(None));
    assert_eq!(elf::find_prgflags_note(&[], 4), Ok(None));
    // "NewTOSX" is somebody else.
    assert_eq!(elf::find_prgflags_note(&entry(b"NewTOSX", 1, &[0; 4], 4), 4), Ok(None));
}

#[test]
fn a_malformed_prgflags_note_is_refused() {
    // Descriptor not exactly one u32.
    assert_eq!(elf::find_prgflags_note(&entry(b"NewTOS", 1, &[0; 8], 4), 4), Err(ParseError::BadNote));
    assert_eq!(elf::find_prgflags_note(&entry(b"NewTOS", 1, &[], 4), 4), Err(ParseError::BadNote));
    // Twice.
    let mut two = prgflags_entry(1, 4);
    two.extend(prgflags_entry(2, 4));
    assert_eq!(elf::find_prgflags_note(&two, 4), Err(ParseError::BadNote));
    // Running past the segment: truncated desc, truncated header.
    let n = prgflags_entry(7, 4);
    assert_eq!(elf::find_prgflags_note(&n[..22], 4), Err(ParseError::BadNote));
    assert_eq!(elf::find_prgflags_note(&n[..8], 4), Err(ParseError::BadNote));
    // A huge namesz must not wrap.
    let mut wild = n.clone();
    wild[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(elf::find_prgflags_note(&wild, 4), Err(ParseError::BadNote));
    // Alignments other than 0/1/4/8.
    assert_eq!(elf::find_prgflags_note(&n, 2), Err(ParseError::BadNote));
    assert_eq!(elf::find_prgflags_note(&n, 16), Err(ParseError::BadNote));
}

// ── whole images: parse records PT_NOTE, prgflags_of reads it ──────────

const NOTE_OFF: u64 = 0x1040;

/// A minimal ET_DYN image: one PT_LOAD (text, file offset == VA 0x1000,
/// 0x100 bytes) with `notes` placed at 0x1040, plus `extra_phdrs`
/// `(type, offset, filesz, align)`.
fn image(notes: &[u8], note_phdrs: &[(u32, u64, u64, u64)]) -> Vec<u8> {
    let phnum = 1 + note_phdrs.len();
    let mut b = vec![0u8; 0x1100];
    b[0..4].copy_from_slice(b"\x7fELF");
    b[4] = 2; // ELFCLASS64
    b[5] = 1; // little endian
    b[6] = 1; // EV_CURRENT
    b[16..18].copy_from_slice(&3u16.to_le_bytes()); // ET_DYN
    b[18..20].copy_from_slice(&183u16.to_le_bytes()); // EM_AARCH64
    b[20..24].copy_from_slice(&1u32.to_le_bytes());
    b[24..32].copy_from_slice(&0x1000u64.to_le_bytes()); // e_entry
    b[32..40].copy_from_slice(&64u64.to_le_bytes()); // e_phoff
    b[52..54].copy_from_slice(&64u16.to_le_bytes()); // e_ehsize
    b[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
    b[56..58].copy_from_slice(&(phnum as u16).to_le_bytes());
    let ph = |b: &mut Vec<u8>, i: usize, t: u32, flags: u32, off: u64, va: u64, fsz: u64, msz: u64, al: u64| {
        let o = 64 + i * 56;
        b[o..o + 4].copy_from_slice(&t.to_le_bytes());
        b[o + 4..o + 8].copy_from_slice(&flags.to_le_bytes());
        b[o + 8..o + 16].copy_from_slice(&off.to_le_bytes());
        b[o + 16..o + 24].copy_from_slice(&va.to_le_bytes());
        b[o + 32..o + 40].copy_from_slice(&fsz.to_le_bytes());
        b[o + 40..o + 48].copy_from_slice(&msz.to_le_bytes());
        b[o + 48..o + 56].copy_from_slice(&al.to_le_bytes());
    };
    ph(&mut b, 0, PT_LOAD, 5, 0x1000, 0x1000, 0x100, 0x100, 0x1000);
    for (i, &(t, off, sz, al)) in note_phdrs.iter().enumerate() {
        ph(&mut b, i + 1, t, 4, off, off, sz, sz, al);
    }
    b[NOTE_OFF as usize..NOTE_OFF as usize + notes.len()].copy_from_slice(notes);
    b
}

fn flags_of(blob: &[u8]) -> Result<Option<u32>, ParseError> {
    let img = elf::parse(blob, blob.len() as u64)?;
    elf::prgflags_of(&img, |s| blob.get(s.file_offset as usize..(s.file_offset + s.file_size) as usize))
}

#[test]
fn parse_records_the_note_segment_and_its_value_is_found() {
    let n = prgflags_entry(PF_TTRAMMEM, 4);
    let blob = image(&n, &[(PT_NOTE, NOTE_OFF, n.len() as u64, 4)]);
    let img = elf::parse(&blob, blob.len() as u64).unwrap();
    assert_eq!(img.notes().len(), 1);
    assert_eq!(img.notes()[0].file_offset, NOTE_OFF);
    assert_eq!(img.notes()[0].file_size, 24);
    assert_eq!(img.segments().len(), 1, "a PT_NOTE is not a load segment");
    assert_eq!(flags_of(&blob), Ok(Some(PF_TTRAMMEM)));
}

#[test]
fn no_note_means_no_value_and_the_loader_takes_pf_standard() {
    let blob = image(&[], &[]);
    assert_eq!(flags_of(&blob), Ok(None));
    // The loader's default (`loader.rs` step 1c) is the value every program
    // had before E8b.
    assert_eq!(PF_STANDARD, 7);
}

#[test]
fn a_prgflags_note_in_two_segments_is_refused() {
    let n = prgflags_entry(1, 4);
    let blob = image(&n, &[(PT_NOTE, NOTE_OFF, 24, 4), (PT_NOTE, NOTE_OFF, 24, 4)]);
    assert_eq!(flags_of(&blob), Err(ParseError::BadNote));
}

#[test]
fn note_segments_outside_the_file_too_big_or_too_many_are_refused() {
    let n = prgflags_entry(1, 4);
    let past = image(&n, &[(PT_NOTE, 0x10F0, 0x20, 4)]);
    assert_eq!(elf::parse(&past, past.len() as u64).err(), Some(ParseError::BadNoteSegment));
    let big = image(&n, &[(PT_NOTE, 0, MAX_NOTE_BYTES + 4, 4)]);
    let mut big = big;
    big.resize(0x2000, 0);
    assert_eq!(elf::parse(&big, big.len() as u64).err(), Some(ParseError::BadNoteSegment));
    let many: Vec<_> = (0..=MAX_NOTE_SEGMENTS).map(|_| (PT_NOTE, NOTE_OFF, 24, 4)).collect();
    let many = image(&n, &many);
    assert_eq!(elf::parse(&many, many.len() as u64).err(), Some(ParseError::BadNoteSegment));
}

#[test]
fn a_bad_note_is_an_unloadable_program() {
    for e in [ParseError::BadNote, ParseError::BadNoteSegment] {
        assert_eq!(PexecError::LoadFailed(LoadError::Parse(e)).errno(), EPLFMT);
    }
}

#[test]
fn the_flags_choose_the_pool_like_emutos_alloc_env_and_alloc_tpa() {
    assert_eq!(loader::pool_mode(PF_STANDARD), MX_PREFTTRAM);
    assert_eq!(loader::pool_mode(PF_TTRAMLOAD), MX_PREFTTRAM);
    assert_eq!(loader::pool_mode(0), MX_STRAM);
    assert_eq!(loader::pool_mode(PF_TTRAMMEM | 1), MX_STRAM, "TT-RAM Malloc alone does not move the TPA");
}
