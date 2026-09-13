//! Boot-log capture to a file on `C:` — the `usb-diag` instrument.
//!
//! **Why this exists.** The machine under investigation is a real Raspberry
//! Pi 4 with **no serial line attached**, which is also why `APITEST` writes
//! its trace to `C:\APITEST.LOG` rather than to the console
//! (`sdk/apps/apitest/apitest.c:187`). Everything the kernel prints during
//! boot goes through exactly one function — [`crate::hal::uart::UartWriter`]'s
//! `write_str` — so teeing there captures the whole boot log, including every
//! line the xHCI driver already emits, without touching a single call site.
//!
//! **Shape.** A fixed buffer in `.bss` (no allocator at this point in boot),
//! filled from the first `kprint!` onwards, written out with GEMDOS
//! `Fcreate`/`Fwrite`/`Fclose` once the card is mounted. The kernel mounts `C:`
//! at `lib.rs:303`, *before* USB comes up at `lib.rs:327`, so the enumeration
//! trace can be flushed the moment enumeration returns.
//!
//! [`flush`] writes the **whole** buffer each time and truncates the file, so
//! calling it repeatedly just extends the file's contents — no append
//! bookkeeping, and an early flush is never lost if a later one never happens
//! (a hang after the boot screen still leaves the enumeration on the card).
//!
//! Overflow is recorded, never silent: the tail of the file says how many
//! bytes were dropped.

#![cfg(feature = "usb-diag")]

/// 192 KiB. The enumeration trace of a four-device hub chain is a few KiB;
/// the rest is headroom for the whole boot. `.bss` counts against
/// `STRAM_OS_RESERVE` (4 MiB, `layout.rs`) — the kernel image is ~2 MiB, so
/// this fits with room to spare, and the linker `ASSERT` catches it if not.
const CAPACITY: usize = 192 * 1024;

/// SAFETY of the statics below: single CPU, cooperative scheduling, and the
/// only writer is `capture`, called from the console writer. There is no
/// preemption in this kernel, so no lock is needed — the same reasoning the
/// USB driver's `XHCI` static uses.
static mut BUF: [u8; CAPACITY] = [0; CAPACITY];
static mut LEN: usize = 0;
static mut DROPPED: usize = 0;
/// Set while [`flush`] runs, so the GEMDOS path's own console output cannot
/// re-enter the capture and grow the buffer we are in the middle of writing.
static mut FLUSHING: bool = false;

/// Append one console string to the capture buffer. Called from
/// [`crate::hal::uart::UartWriter::write_str`] — i.e. from every `kprint!` and
/// `kprintln!` in the kernel.
pub fn capture(s: &str) {
    // SAFETY: single-CPU, no preemption; see the note on the statics.
    unsafe {
        if FLUSHING {
            return;
        }
        let len = LEN;
        let room = CAPACITY - len;
        let bytes = s.as_bytes();
        // Raw-pointer copy rather than slice indexing: taking a `&mut` to a
        // `static mut` array is what `static_mut_refs` forbids, and the copy is
        // the whole operation anyway.
        let dst = (core::ptr::addr_of_mut!(BUF) as *mut u8).add(len);
        let n = bytes.len().min(room);
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, n);
        LEN = len + n;
        if bytes.len() > room {
            DROPPED += bytes.len() - room;
        }
    }
}

/// Write the captured log to `C:\USBDIAG.LOG`, truncating whatever is there.
///
/// `tag` names the flush point and is written as the last line, so a file that
/// stops early tells you *where* the boot stopped. Safe to call more than
/// once; each call rewrites the file with everything captured so far.
///
/// Must be called from ordinary kernel context (the boot path), **not** from
/// inside an exception handler: it issues `svc #1`, and a nested exception at
/// EL1 would overwrite the outer handler's `ELR_EL1`/`SPSR_EL1`.
pub fn flush(tag: &str) {
    // SAFETY: single-CPU, no preemption; see the note on the statics.
    unsafe {
        if FLUSHING {
            return;
        }
        FLUSHING = true;
    }
    let tail = tail_line(tag);
    let (ptr, len) = unsafe { ((*core::ptr::addr_of!(BUF)).as_ptr(), LEN) };

    let path = c"C:\\USBDIAG.LOG";
    let h = gemdos_fcreate(path.as_ptr() as u64, 0);
    if h >= 0 {
        let handle = h as u16;
        // Chunked so one Fwrite never spans an implausible length; the file
        // system takes a long write fine, but a short chunk makes a partial
        // failure visible in the return value rather than all-or-nothing.
        let mut off = 0usize;
        while off < len {
            let n = (len - off).min(16 * 1024);
            let w = gemdos_fwrite(handle, n as i64, ptr as u64 + off as u64);
            if w != n as i64 {
                break;
            }
            off += n;
        }
        let _ = gemdos_fwrite(handle, tail.1 as i64, tail.0.as_ptr() as u64);
        let _ = gemdos_fclose(handle);
        crate::kprintln!("[diag] wrote C:\\USBDIAG.LOG ({} bytes, {})", off, tag);
    } else {
        crate::kprintln!("[diag] Fcreate(C:\\USBDIAG.LOG) failed -> {}", h);
    }

    // SAFETY: single-CPU; clearing the re-entry guard we set above.
    unsafe { FLUSHING = false };
}

/// Build the trailing line: the flush point, the byte count, and — the part
/// that matters — how much was dropped, so a truncated log says so itself.
fn tail_line(tag: &str) -> ([u8; 128], usize) {
    let mut b = [0u8; 128];
    let mut n = 0usize;
    let mut put = |s: &str, b: &mut [u8; 128], n: &mut usize| {
        for &c in s.as_bytes() {
            if *n < b.len() {
                b[*n] = c;
                *n += 1;
            }
        }
    };
    let mut put_num = |mut v: usize, b: &mut [u8; 128], n: &mut usize| {
        let mut d = [0u8; 20];
        let mut i = 0;
        if v == 0 {
            d[0] = b'0';
            i = 1;
        }
        while v > 0 {
            d[i] = b'0' + (v % 10) as u8;
            v /= 10;
            i += 1;
        }
        while i > 0 {
            i -= 1;
            if *n < b.len() {
                b[*n] = d[i];
                *n += 1;
            }
        }
    };
    put("\r\n[diag] end of capture at '", &mut b, &mut n);
    put(tag, &mut b, &mut n);
    put("', dropped ", &mut b, &mut n);
    // SAFETY: single-CPU read of the drop counter.
    put_num(unsafe { DROPPED }, &mut b, &mut n);
    put(" bytes\r\n", &mut b, &mut n);
    (b, n)
}

/// GEMDOS `Fcreate` (0x3C): x0 = name, x1 = attr. Same shape as the EMMC
/// self-test's helper (`selftest_emmc.rs:650`) — the trap path, not a private
/// back door into the file system.
fn gemdos_fcreate(name: u64, attr: u16) -> i64 {
    let r: i64;
    // SAFETY: svc #1, GEMDOS Fcreate ABI; `name` is a kernel string.
    unsafe {
        core::arch::asm!("svc #1", in("x8") 0x3Cu64, inout("x0") name => r, in("x1") attr as u64, options(nostack));
    }
    r
}

/// GEMDOS `Fwrite` (0x40): x0 = handle, x1 = len, x2 = buf.
fn gemdos_fwrite(handle: u16, len: i64, buf: u64) -> i64 {
    let r: i64;
    // SAFETY: svc #1, GEMDOS Fwrite ABI; `buf` is a kernel buffer.
    unsafe {
        core::arch::asm!("svc #1", in("x8") 0x40u64, inout("x0") handle as u64 => r, in("x1") len, in("x2") buf, options(nostack));
    }
    r
}

/// GEMDOS `Fclose` (0x3E): x0 = handle.
fn gemdos_fclose(handle: u16) -> i64 {
    let r: i64;
    // SAFETY: svc #1, GEMDOS Fclose ABI.
    unsafe {
        core::arch::asm!("svc #1", in("x8") 0x3Eu64, inout("x0") handle as u64 => r, options(nostack));
    }
    r
}
