//! Fault-safe kernel access to **user** memory (ARCH03 stage 0c).
//!
//! A TOS syscall argument that is a pointer is a pointer into the *caller's*
//! address space. The kernel reads and writes such memory with the
//! unprivileged load/store instructions `ldtr*`/`sttr*`: executed at EL1 with
//! `PSTATE.UAO = 0` (the Cortex-A72 has no UAO) they are permission-checked
//! "as if executed at EL0" (ARM ARM DDI 0487 M.d C6.2.245; plan
//! `docs/analysis/arch03-implementation-plan.md` Frage 10, 0a/0b notes), so a
//! kernel pointer (`AP = 00`), an address outside the caller's `TTBR0` window
//! and an unmapped user address all fault.
//!
//! What this module adds over the bare instructions is **recovery**. Every
//! `ldtr`/`sttr` below sits in a hand-written leaf routine and is listed in a
//! fixup table `(instruction PC, fixup PC)`. The kernel's synchronous handler
//! consults [`fixup_for`] on a same-EL data abort (`EC 0x25`); if the faulting
//! `ELR_EL1` is one of these instructions it records the fault
//! ([`note_fault`]) and resumes at the fixup, which returns a failure status
//! to the Rust wrapper — instead of halting the machine. The 0b spike measured
//! exactly this picture on QEMU (`docs/analysis/arch03-stage0b-spike.md` §5:
//! `ELR` = the `ldtr`/`sttr` itself, `FAR` = the user VA, DFSC per level).
//! A same-EL abort at any **other** PC is still a kernel fault and halts.
//!
//! ## What a fault means for the caller (EmuTOS behaviour)
//!
//! TOS has no `EFAULT`. On EmuTOS a GEMDOS call that dereferences a bad
//! pointer takes a bus/address error **in supervisor mode**; the default
//! vector (`references/emutos/bios/vectors.S` `any_vec`) runs `dopanic`
//! (`bios/kprint.c:302-596`), which prints the crash report and calls
//! `kill_program` → `Pterm(-1)` (`bios/panicasm.S:96-99`) — and since `run` is
//! still the caller's PD, it is the **calling program** that is terminated
//! (`kprint.c:587-594`). FreeMiNT does the same in substance: a kernel bus
//! error on a user address raises `SIGBUS` on `curproc`
//! (`references/freemint/sys/arch/sig_mach.c:493-502`), whose default action
//! kills it. The recorded fault is therefore not turned into an error code
//! for the program: the kernel's syscall tail picks it up with
//! [`take_pending_fault`] and terminates the caller with the crash report
//! (`docs/api/bios/_exception-crash.md`). The `Err` the wrappers return only
//! lets the Rust call site stop cleanly; its value never reaches the program.
//!
//! On the host (unit tests) there is no EL0/EL1 split and nothing can fault
//! recoverably, so the wrappers use ordinary loads/stores there and the
//! fixup table is empty.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// A user-memory access faulted (the address was not accessible to the
/// calling program). The details are in the record [`note_fault`] stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserFault;

/// One fixup-table entry: a faulting instruction and where to resume.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixupEntry {
    /// Address of an `ldtr*`/`sttr*` instruction that may fault.
    pub insn: u64,
    /// Address execution resumes at when that instruction faults.
    pub fixup: u64,
}

/// Look `pc` up in `table`: the fixup address if `pc` is exactly one of the
/// listed instructions. Exact match only — `ELR_EL1` of a synchronous data
/// abort is the faulting instruction itself (ARM ARM D1, measured in 0b §5),
/// so a PC one instruction off is a different (kernel) fault.
pub fn lookup_fixup(table: &[FixupEntry], pc: u64) -> Option<u64> {
    table.iter().find(|e| e.insn == pc).map(|e| e.fixup)
}

/// The fixup for a same-EL data abort at `pc`, if `pc` is one of this
/// module's user-access instructions.
pub fn fixup_for(pc: u64) -> Option<u64> {
    lookup_fixup(fixup_table(), pc)
}

// ---------------------------------------------------------------------------
// Fault record — written by the kernel's abort handler, consumed by the
// syscall tail.
// ---------------------------------------------------------------------------

/// Snapshot of a recovered user-access fault: what EmuTOS saves at `0x380`
/// for `dopanic` (`bios/vectors.S` `any_vec`) — the registers at the fault,
/// the fault address and the PC — in AArch64 terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultRecord {
    /// `FAR_EL1` — the user address the kernel tried to access.
    pub far: u64,
    /// `ESR_EL1` of the abort (`EC 0x25`, DFSC, `WnR`).
    pub esr: u64,
    /// `ELR_EL1` — the faulting `ldtr`/`sttr` (a kernel address).
    pub pc: u64,
    /// `SPSR_EL1` of the abort (the EL1 state the access ran in).
    pub pstate: u64,
    /// `x0..x30` at the fault.
    pub regs: [u64; 31],
}

static FAULT_ARMED: AtomicBool = AtomicBool::new(false);
static FAULT_FAR: AtomicU64 = AtomicU64::new(0);
static FAULT_ESR: AtomicU64 = AtomicU64::new(0);
static FAULT_PC: AtomicU64 = AtomicU64::new(0);
static FAULT_PSTATE: AtomicU64 = AtomicU64::new(0);
static FAULT_REGS: [AtomicU64; 31] = [const { AtomicU64::new(0) }; 31];

/// Record a recovered user-access fault. Called by the kernel's synchronous
/// handler right before it redirects `ELR_EL1` to the fixup. The first fault
/// of a syscall wins: a later one (the Rust caller ignoring the `Err` and
/// trying again) does not overwrite the report of the original.
pub fn note_fault(rec: &FaultRecord) {
    if FAULT_ARMED.load(Ordering::Acquire) {
        return;
    }
    FAULT_FAR.store(rec.far, Ordering::Relaxed);
    FAULT_ESR.store(rec.esr, Ordering::Relaxed);
    FAULT_PC.store(rec.pc, Ordering::Relaxed);
    FAULT_PSTATE.store(rec.pstate, Ordering::Relaxed);
    for (dst, &v) in FAULT_REGS.iter().zip(rec.regs.iter()) {
        dst.store(v, Ordering::Relaxed);
    }
    FAULT_ARMED.store(true, Ordering::Release);
}

/// Consume the pending fault record, if a user access faulted since the last
/// call. The kernel calls this when a syscall returns to its caller.
pub fn take_pending_fault() -> Option<FaultRecord> {
    if !FAULT_ARMED.swap(false, Ordering::AcqRel) {
        return None;
    }
    let mut regs = [0u64; 31];
    for (dst, src) in regs.iter_mut().zip(FAULT_REGS.iter()) {
        *dst = src.load(Ordering::Relaxed);
    }
    Some(FaultRecord {
        far: FAULT_FAR.load(Ordering::Relaxed),
        esr: FAULT_ESR.load(Ordering::Relaxed),
        pc: FAULT_PC.load(Ordering::Relaxed),
        pstate: FAULT_PSTATE.load(Ordering::Relaxed),
        regs,
    })
}

// ---------------------------------------------------------------------------
// Public accessors
// ---------------------------------------------------------------------------

/// Read one byte from user address `va`.
pub fn get_user_u8(va: u64) -> Result<u8, UserFault> {
    let mut v = 0u8;
    // SAFETY: `&mut v` is a valid kernel destination of the right size.
    status(unsafe { raw::ld8(va, &mut v) }).map(|()| v)
}

/// Read a 16-bit value from user address `va` (native byte order).
pub fn get_user_u16(va: u64) -> Result<u16, UserFault> {
    let mut v = 0u16;
    // SAFETY: as in [`get_user_u8`].
    status(unsafe { raw::ld16(va, &mut v) }).map(|()| v)
}

/// Read a 32-bit value from user address `va` (native byte order).
pub fn get_user_u32(va: u64) -> Result<u32, UserFault> {
    let mut v = 0u32;
    // SAFETY: as in [`get_user_u8`].
    status(unsafe { raw::ld32(va, &mut v) }).map(|()| v)
}

/// Read a 64-bit value from user address `va` (native byte order).
pub fn get_user_u64(va: u64) -> Result<u64, UserFault> {
    let mut v = 0u64;
    // SAFETY: as in [`get_user_u8`].
    status(unsafe { raw::ld64(va, &mut v) }).map(|()| v)
}

/// Write one byte to user address `va`.
pub fn put_user_u8(va: u64, v: u8) -> Result<(), UserFault> {
    // SAFETY: the store goes through the unprivileged, fault-recovered path.
    status(unsafe { raw::st8(va, u64::from(v)) })
}

/// Write a 16-bit value to user address `va` (native byte order).
pub fn put_user_u16(va: u64, v: u16) -> Result<(), UserFault> {
    // SAFETY: as in [`put_user_u8`].
    status(unsafe { raw::st16(va, u64::from(v)) })
}

/// Write a 32-bit value to user address `va` (native byte order).
pub fn put_user_u32(va: u64, v: u32) -> Result<(), UserFault> {
    // SAFETY: as in [`put_user_u8`].
    status(unsafe { raw::st32(va, u64::from(v)) })
}

/// Write a 64-bit value to user address `va` (native byte order).
pub fn put_user_u64(va: u64, v: u64) -> Result<(), UserFault> {
    // SAFETY: as in [`put_user_u8`].
    status(unsafe { raw::st64(va, v) })
}

/// Copy `dst.len()` bytes from user address `src` into the kernel buffer
/// `dst`. On a fault the bytes before the faulting one have been copied.
pub fn copy_from_user(dst: &mut [u8], src: u64) -> Result<(), UserFault> {
    // SAFETY: `dst` is a valid kernel buffer of `dst.len()` bytes.
    let left = unsafe { raw::copy_from(dst.as_mut_ptr(), src, dst.len() as u64) };
    if left == 0 { Ok(()) } else { Err(UserFault) }
}

/// Copy the kernel buffer `src` to user address `dst`. On a fault the bytes
/// before the faulting one have been stored.
pub fn copy_to_user(dst: u64, src: &[u8]) -> Result<(), UserFault> {
    // SAFETY: `src` is a valid kernel buffer of `src.len()` bytes.
    let left = unsafe { raw::copy_to(dst, src.as_ptr(), src.len() as u64) };
    if left == 0 { Ok(()) } else { Err(UserFault) }
}

/// Copy a NUL-terminated string from user address `src` into `dst`,
/// returning its length (without the NUL). Reading stops at the NUL or after
/// `dst.len()` bytes — a missing terminator truncates (the returned length is
/// then `dst.len()`); the NUL itself is not stored.
pub fn read_user_cstr(src: u64, dst: &mut [u8]) -> Result<usize, UserFault> {
    // SAFETY: `dst` is a valid kernel buffer of `dst.len()` bytes.
    let n = unsafe { raw::strncpy_from(dst.as_mut_ptr(), src, dst.len() as u64) };
    if n < 0 { Err(UserFault) } else { Ok(n as usize) }
}

#[inline]
fn status(rc: u64) -> Result<(), UserFault> {
    if rc == 0 { Ok(()) } else { Err(UserFault) }
}

// ---------------------------------------------------------------------------
// Target implementation: leaf routines + fixup table
// ---------------------------------------------------------------------------

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
mod raw {
    use super::FixupEntry;

    // Every routine is a **leaf** (no frame, `lr` untouched), so a fixup can
    // simply set the return value and `ret` to the Rust caller. Calling
    // convention AAPCS64; the scratch registers used are caller-saved.
    //
    // Status routines return `x0 = 0` on success and `1` after a fault.
    // `copy_*` return the number of bytes **not** copied (0 = all done);
    // `strncpy_from` returns the string length or `-1` after a fault.
    core::arch::global_asm!(
        r#"
    .pushsection .text.newtos_uaccess, "ax", %progbits
    .balign 4

    .global __uaccess_ld8
    .type   __uaccess_ld8, %function
__uaccess_ld8:
.Luacc_ld8:
    ldtrb   w2, [x0]
    strb    w2, [x1]
    mov     x0, #0
    ret

    .global __uaccess_ld16
    .type   __uaccess_ld16, %function
__uaccess_ld16:
.Luacc_ld16:
    ldtrh   w2, [x0]
    strh    w2, [x1]
    mov     x0, #0
    ret

    .global __uaccess_ld32
    .type   __uaccess_ld32, %function
__uaccess_ld32:
.Luacc_ld32:
    ldtr    w2, [x0]
    str     w2, [x1]
    mov     x0, #0
    ret

    .global __uaccess_ld64
    .type   __uaccess_ld64, %function
__uaccess_ld64:
.Luacc_ld64:
    ldtr    x2, [x0]
    str     x2, [x1]
    mov     x0, #0
    ret

    .global __uaccess_st8
    .type   __uaccess_st8, %function
__uaccess_st8:
.Luacc_st8:
    sttrb   w1, [x0]
    mov     x0, #0
    ret

    .global __uaccess_st16
    .type   __uaccess_st16, %function
__uaccess_st16:
.Luacc_st16:
    sttrh   w1, [x0]
    mov     x0, #0
    ret

    .global __uaccess_st32
    .type   __uaccess_st32, %function
__uaccess_st32:
.Luacc_st32:
    sttr    w1, [x0]
    mov     x0, #0
    ret

    .global __uaccess_st64
    .type   __uaccess_st64, %function
__uaccess_st64:
.Luacc_st64:
    sttr    x1, [x0]
    mov     x0, #0
    ret

    // Shared fixup of the status routines: report the fault.
.Luacc_fault_status:
    mov     x0, #1
    ret

    // x0 = dst (kernel), x1 = src (user), x2 = count
    .global __uaccess_copy_from
    .type   __uaccess_copy_from, %function
__uaccess_copy_from:
    cbz     x2, .Luacc_cf_done
.Luacc_cf_loop:
.Luacc_cf_insn:
    ldtrb   w3, [x1]
    strb    w3, [x0], #1
    add     x1, x1, #1
    subs    x2, x2, #1
    b.ne    .Luacc_cf_loop
.Luacc_cf_done:
    // Also the fixup: x2 still counts the faulting byte as not copied.
    mov     x0, x2
    ret

    // x0 = dst (user), x1 = src (kernel), x2 = count
    .global __uaccess_copy_to
    .type   __uaccess_copy_to, %function
__uaccess_copy_to:
    cbz     x2, .Luacc_ct_done
.Luacc_ct_loop:
    ldrb    w3, [x1], #1
.Luacc_ct_insn:
    sttrb   w3, [x0]
    add     x0, x0, #1
    subs    x2, x2, #1
    b.ne    .Luacc_ct_loop
.Luacc_ct_done:
    mov     x0, x2
    ret

    // x0 = dst (kernel), x1 = src (user), x2 = max; returns length or -1
    .global __uaccess_strncpy_from
    .type   __uaccess_strncpy_from, %function
__uaccess_strncpy_from:
    mov     x4, #0
.Luacc_sc_loop:
    cmp     x4, x2
    b.hs    .Luacc_sc_done
.Luacc_sc_insn:
    ldtrb   w3, [x1, #0]
    cbz     w3, .Luacc_sc_done
    strb    w3, [x0, x4]
    add     x1, x1, #1
    add     x4, x4, #1
    b       .Luacc_sc_loop
.Luacc_sc_done:
    mov     x0, x4
    ret
.Luacc_sc_fault:
    mov     x0, #-1
    ret
    .popsection

    .pushsection .rodata.newtos_uaccess_fixups, "a", %progbits
    .balign 8
    .global __uaccess_fixups_start
__uaccess_fixups_start:
    .quad .Luacc_ld8,  .Luacc_fault_status
    .quad .Luacc_ld16, .Luacc_fault_status
    .quad .Luacc_ld32, .Luacc_fault_status
    .quad .Luacc_ld64, .Luacc_fault_status
    .quad .Luacc_st8,  .Luacc_fault_status
    .quad .Luacc_st16, .Luacc_fault_status
    .quad .Luacc_st32, .Luacc_fault_status
    .quad .Luacc_st64, .Luacc_fault_status
    .quad .Luacc_cf_insn, .Luacc_cf_done
    .quad .Luacc_ct_insn, .Luacc_ct_done
    .quad .Luacc_sc_insn, .Luacc_sc_fault
    .global __uaccess_fixups_end
__uaccess_fixups_end:
    .popsection
"#
    );

    unsafe extern "C" {
        #[link_name = "__uaccess_ld8"]
        pub fn ld8(src: u64, dst: *mut u8) -> u64;
        #[link_name = "__uaccess_ld16"]
        pub fn ld16(src: u64, dst: *mut u16) -> u64;
        #[link_name = "__uaccess_ld32"]
        pub fn ld32(src: u64, dst: *mut u32) -> u64;
        #[link_name = "__uaccess_ld64"]
        pub fn ld64(src: u64, dst: *mut u64) -> u64;
        #[link_name = "__uaccess_st8"]
        pub fn st8(dst: u64, v: u64) -> u64;
        #[link_name = "__uaccess_st16"]
        pub fn st16(dst: u64, v: u64) -> u64;
        #[link_name = "__uaccess_st32"]
        pub fn st32(dst: u64, v: u64) -> u64;
        #[link_name = "__uaccess_st64"]
        pub fn st64(dst: u64, v: u64) -> u64;
        #[link_name = "__uaccess_copy_from"]
        pub fn copy_from(dst: *mut u8, src: u64, n: u64) -> u64;
        #[link_name = "__uaccess_copy_to"]
        pub fn copy_to(dst: u64, src: *const u8, n: u64) -> u64;
        #[link_name = "__uaccess_strncpy_from"]
        pub fn strncpy_from(dst: *mut u8, src: u64, n: u64) -> i64;

        static __uaccess_fixups_start: FixupEntry;
        static __uaccess_fixups_end: FixupEntry;
    }

    /// The fixup table emitted by the assembly above.
    pub fn table() -> &'static [FixupEntry] {
        let start = &raw const __uaccess_fixups_start;
        let end = &raw const __uaccess_fixups_end;
        let len = (end as usize - start as usize) / core::mem::size_of::<FixupEntry>();
        // SAFETY: the assembly emits `len` contiguous, 8-byte aligned
        // `FixupEntry` records between the two symbols, in read-only data
        // that lives for the whole kernel lifetime.
        unsafe { core::slice::from_raw_parts(start, len) }
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
#[inline]
fn fixup_table() -> &'static [FixupEntry] {
    raw::table()
}

/// Host build: no EL0/EL1 split, no recoverable faults — plain accesses.
#[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
mod raw {
    pub unsafe fn ld8(src: u64, dst: *mut u8) -> u64 {
        // SAFETY: host tests pass valid host addresses.
        unsafe { dst.write((src as *const u8).read_volatile()) };
        0
    }
    pub unsafe fn ld16(src: u64, dst: *mut u16) -> u64 {
        // SAFETY: as above.
        unsafe { dst.write((src as *const u16).read_unaligned()) };
        0
    }
    pub unsafe fn ld32(src: u64, dst: *mut u32) -> u64 {
        // SAFETY: as above.
        unsafe { dst.write((src as *const u32).read_unaligned()) };
        0
    }
    pub unsafe fn ld64(src: u64, dst: *mut u64) -> u64 {
        // SAFETY: as above.
        unsafe { dst.write((src as *const u64).read_unaligned()) };
        0
    }
    pub unsafe fn st8(dst: u64, v: u64) -> u64 {
        // SAFETY: as above.
        unsafe { (dst as *mut u8).write_volatile(v as u8) };
        0
    }
    pub unsafe fn st16(dst: u64, v: u64) -> u64 {
        // SAFETY: as above.
        unsafe { (dst as *mut u16).write_unaligned(v as u16) };
        0
    }
    pub unsafe fn st32(dst: u64, v: u64) -> u64 {
        // SAFETY: as above.
        unsafe { (dst as *mut u32).write_unaligned(v as u32) };
        0
    }
    pub unsafe fn st64(dst: u64, v: u64) -> u64 {
        // SAFETY: as above.
        unsafe { (dst as *mut u64).write_unaligned(v) };
        0
    }
    pub unsafe fn copy_from(dst: *mut u8, src: u64, n: u64) -> u64 {
        // SAFETY: as above; `dst` holds `n` bytes.
        unsafe { core::ptr::copy(src as *const u8, dst, n as usize) };
        0
    }
    pub unsafe fn copy_to(dst: u64, src: *const u8, n: u64) -> u64 {
        // SAFETY: as above; `src` holds `n` bytes.
        unsafe { core::ptr::copy(src, dst as *mut u8, n as usize) };
        0
    }
    pub unsafe fn strncpy_from(dst: *mut u8, src: u64, n: u64) -> i64 {
        let mut i = 0u64;
        while i < n {
            // SAFETY: as above; bounded by `n`.
            let b = unsafe { ((src + i) as *const u8).read_volatile() };
            if b == 0 {
                break;
            }
            // SAFETY: `dst` holds `n` bytes and `i < n`.
            unsafe { dst.add(i as usize).write(b) };
            i += 1;
        }
        i as i64
    }
}

#[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
#[inline]
fn fixup_table() -> &'static [FixupEntry] {
    &[]
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: [FixupEntry; 3] = [
        FixupEntry { insn: 0xFFFF_0000_0010_0000, fixup: 0xFFFF_0000_0010_0100 },
        FixupEntry { insn: 0xFFFF_0000_0010_0010, fixup: 0xFFFF_0000_0010_0100 },
        FixupEntry { insn: 0xFFFF_0000_0010_0040, fixup: 0xFFFF_0000_0010_0050 },
    ];

    #[test]
    fn lookup_hits_exact_instruction() {
        assert_eq!(lookup_fixup(&TABLE, 0xFFFF_0000_0010_0000), Some(0xFFFF_0000_0010_0100));
        assert_eq!(lookup_fixup(&TABLE, 0xFFFF_0000_0010_0010), Some(0xFFFF_0000_0010_0100));
        assert_eq!(lookup_fixup(&TABLE, 0xFFFF_0000_0010_0040), Some(0xFFFF_0000_0010_0050));
    }

    #[test]
    fn lookup_misses_neighbours_and_unrelated_pcs() {
        // One instruction before/after a listed one is a different PC: a
        // kernel fault there must stay fatal.
        assert_eq!(lookup_fixup(&TABLE, 0xFFFF_0000_0010_0004), None);
        assert_eq!(lookup_fixup(&TABLE, 0xFFFF_0000_0010_003C), None);
        assert_eq!(lookup_fixup(&TABLE, 0), None);
        assert_eq!(lookup_fixup(&[], 0xFFFF_0000_0010_0000), None);
    }

    #[test]
    fn host_table_is_empty() {
        // Nothing can fault recoverably on the host.
        assert_eq!(fixup_for(0xFFFF_0000_0010_0000), None);
    }

    #[test]
    fn fault_record_round_trip_and_first_fault_wins() {
        let mut regs = [0u64; 31];
        regs[0] = 0x1111;
        regs[30] = 0x3030;
        let first = FaultRecord { far: 0xDEAD_0000, esr: 0x9600_0047, pc: 0x10, pstate: 0x3C5, regs };
        let second = FaultRecord { far: 0xBEEF, ..first };
        // Drain anything a previous test left (tests share the statics).
        let _ = take_pending_fault();
        note_fault(&first);
        note_fault(&second);
        assert_eq!(take_pending_fault(), Some(first));
        assert_eq!(take_pending_fault(), None);
    }

    #[test]
    fn host_accessors_read_and_write() {
        let s = b"PATH\0junk";
        let mut buf = [0u8; 16];
        assert_eq!(read_user_cstr(s.as_ptr() as u64, &mut buf), Ok(4));
        assert_eq!(&buf[..4], b"PATH");
        // Truncation without a NUL inside the window.
        let mut small = [0u8; 2];
        assert_eq!(read_user_cstr(s.as_ptr() as u64, &mut small), Ok(2));
        let mut w = 0u32;
        put_user_u32(&mut w as *mut u32 as u64, 0xA5A5_5A5A).unwrap();
        assert_eq!(get_user_u32(&w as *const u32 as u64), Ok(0xA5A5_5A5A));
    }
}
