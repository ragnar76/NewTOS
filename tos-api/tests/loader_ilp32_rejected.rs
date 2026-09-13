//! NewTOS runs native 64-bit applications only: an `ELFCLASS32` (ILP32)
//! program is refused at load time with `EPLFMT`.
//!
//! **Where this comes from.** ADR-020's addendum (2026-09-04) retired the
//! ILP32 *build* path — the seven `*-ilp32.sh` scripts and `crt0-ilp32.S`
//! are gone, LP64 is the only application ABI the SDK can produce. What
//! survived is the *runtime* width switch (`Abi::Ilp32`, read by the
//! AES/VDI parameter-block readers at 65 sites), kept on purpose as the
//! substrate for the future m68k thunk.
//!
//! That left a gap: the ELF loader would still mark a process ILP32 from
//! a `ELFCLASS32` header, feeding 4-byte-word branches that nothing builds
//! and nothing measures. `load_to_slot` now rejects such an image outright
//! (`loader.rs`, step 1b), which makes every downstream 4-byte branch
//! unreachable by construction instead of merely untested.
//!
//! The honest-unsupported shape matters: this is a refusal with a real TOS
//! error code, not a silent fallback that pretends to run the program.
//! `EPLFMT` (-66, "invalid program load format") is what EmuTOS answers for
//! a program header it will not load (`bdos/kpgmld.c:47-63`).
//!
//! What is asserted here:
//!
//! 1. The parser still *understands* ELF32 — `is_ilp32()` is true for a
//!    well-formed ELFCLASS32 AArch64 executable. Without this the rejection
//!    test below would be vacuous (it would also pass if the parser had
//!    simply stopped recognising the class).
//! 2. The ELF64 control parses as LP64.
//! 3. `Ilp32Unsupported` reports `EPLFMT` through `Pexec`.

use tos_api::bdos::process::elf;
use tos_api::bdos::process::loader::LoadError;
use tos_api::bdos::process::pexec::PexecError;
use tos_types::error::EPLFMT;

const ET_EXEC: u16 = 2;
const EM_AARCH64: u16 = 0xb7;
const PT_LOAD: u32 = 1;
const PF_R: u32 = 0x4;
const PF_X: u32 = 0x1;

/// Entry VA every synthesized image claims. Above the reserved low page
/// (basepage + sysvars), so the header is plausible as well as well-formed.
const ENTRY_VA: u64 = 0x1000;

fn put16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}
fn put64(buf: &mut [u8], off: usize, v: u64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

/// A minimal but genuinely well-formed ELF32 AArch64 executable: header
/// plus one `PT_LOAD` segment. Note the ELF32 phdr field order — `p_flags`
/// is the second-to-last field (offset 24), not right after `p_type` as in
/// ELF64. Getting that wrong would mis-parse rather than fail loudly.
fn elf32_exec() -> Vec<u8> {
    const EHDR: usize = 52;
    const PHDR: usize = 32;
    let mut b = vec![0u8; EHDR + PHDR];

    b[0..4].copy_from_slice(b"\x7fELF");
    b[4] = 1; // EI_CLASS = ELFCLASS32
    b[5] = 1; // EI_DATA  = ELFDATA2LSB
    b[6] = 1; // EI_VERSION = EV_CURRENT

    put16(&mut b, 16, ET_EXEC);
    put16(&mut b, 18, EM_AARCH64);
    put32(&mut b, 20, 1); // e_version
    put32(&mut b, 24, ENTRY_VA as u32); // e_entry  (4-byte in ELF32)
    put32(&mut b, 28, EHDR as u32); // e_phoff
    put16(&mut b, 42, PHDR as u16); // e_phentsize
    put16(&mut b, 44, 1); // e_phnum

    let p = EHDR;
    put32(&mut b, p, PT_LOAD); // p_type
    put32(&mut b, p + 4, EHDR as u32 + PHDR as u32); // p_offset
    put32(&mut b, p + 8, ENTRY_VA as u32); // p_vaddr
    put32(&mut b, p + 16, 0); // p_filesz
    put32(&mut b, p + 20, 0); // p_memsz
    put32(&mut b, p + 24, PF_R | PF_X); // p_flags  (ELF32 order!)
    b
}

/// The ELF64 control, same program shape.
fn elf64_exec() -> Vec<u8> {
    const EHDR: usize = 64;
    const PHDR: usize = 56;
    let mut b = vec![0u8; EHDR + PHDR];

    b[0..4].copy_from_slice(b"\x7fELF");
    b[4] = 2; // EI_CLASS = ELFCLASS64
    b[5] = 1;
    b[6] = 1;

    put16(&mut b, 16, ET_EXEC);
    put16(&mut b, 18, EM_AARCH64);
    put32(&mut b, 20, 1);
    put64(&mut b, 24, ENTRY_VA); // e_entry
    put64(&mut b, 32, EHDR as u64); // e_phoff
    put16(&mut b, 54, PHDR as u16); // e_phentsize
    put16(&mut b, 56, 1); // e_phnum

    let p = EHDR;
    put32(&mut b, p, PT_LOAD); // p_type
    put32(&mut b, p + 4, PF_R | PF_X); // p_flags (right after p_type in ELF64)
    put64(&mut b, p + 8, (EHDR + PHDR) as u64); // p_offset
    put64(&mut b, p + 16, ENTRY_VA); // p_vaddr
    put64(&mut b, p + 32, 0); // p_filesz
    put64(&mut b, p + 40, 0); // p_memsz
    b
}

/// Guards against a vacuous rejection test: the parser must still classify
/// a well-formed ELFCLASS32 image as ILP32, so the loader's gate has a real
/// condition to fire on. This is the substrate ADR-020's addendum keeps for
/// the m68k thunk — parsing ELF32 stays, *running* it does not.
#[test]
fn parser_still_recognises_elf32_as_ilp32() {
    let blob = elf32_exec();
    let img = elf::parse(&blob, blob.len() as u64).expect("well-formed ELF32 must parse");
    assert!(
        img.is_ilp32(),
        "ELFCLASS32 image must still be classified ILP32 by the parser"
    );
    assert_eq!(img.entry_va, ENTRY_VA);
    assert_eq!(img.segments().len(), 1, "the single PT_LOAD must be seen");
}

/// The control: an ELF64 image is LP64, which is the only class that may
/// actually reach EL0.
#[test]
fn elf64_parses_as_lp64() {
    let blob = elf64_exec();
    let img = elf::parse(&blob, blob.len() as u64).expect("well-formed ELF64 must parse");
    assert!(!img.is_ilp32(), "ELFCLASS64 image must be LP64");
    assert_eq!(img.entry_va, ENTRY_VA);
    assert_eq!(img.segments().len(), 1);
}

/// The documented error code for the documented error input: refusing an
/// ILP32 program is `EPLFMT` (-66) at the `Pexec` boundary, the same answer
/// EmuTOS gives a header it will not load (`bdos/kpgmld.c:47-63`) — not
/// `EINVFN` (that is a bad *mode*) and not `ENSMEM` (that is a shortfall).
#[test]
fn ilp32_load_failure_reports_eplfmt() {
    let err = PexecError::LoadFailed(LoadError::Ilp32Unsupported);
    assert_eq!(
        err.errno(),
        EPLFMT,
        "an ILP32 image is a format we refuse, so Pexec answers EPLFMT"
    );
}
