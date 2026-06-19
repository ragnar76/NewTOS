//! AES Application Manager (`appl_*`, opcodes 10–19). Spec:
//! [`docs/api/aes/application.md`].
//!
//! `appl_init` registers a GEM application (its id = the GEMDOS process
//! id), fills the caller's `AESGLOBAL`, and marks it `AP_OPEN`;
//! `appl_exit` tears it down; `appl_find` looks one up by name;
//! `appl_yield` is the cooperative scheduler hand-off.
//!
//! **ABI note.** Like the rest of NewTOS, the `AESGLOBAL` uses the
//! **native** AArch64 layout — 64-bit pointers, natural alignment — not
//! the m68k packed offsets, exactly as the Pexec basepage and the MFDB
//! do. Recompiled GEM programs use the NewTOS TOS headers, which declare
//! this layout.

use super::pd;
use super::Aespb;

/// AES version reported in `ap_version` (`include/config.h` default).
const AES_VERSION: i16 = 0x0340;

/// The application's AES state, as the `global` array is interpreted.
/// Native layout (`#[repr(C)]`): 64-bit pointers, natural alignment.
#[repr(C)]
struct AesGlobal {
    ap_version: i16,
    ap_count: i16,
    ap_id: i16,
    _pad0: i16,
    ap_private: u64,
    ap_ptree: u64,
    ap_rscmem: u64,
    ap_rsclen: u16,
    ap_planes: i16,
    _pad1: i32,
    ap_3resv: u64,
    ap_4resv: u64,
}

/// The id of the currently-running process, used as the AES application
/// id. From a real EL0 process this is its slot-derived pid; from the
/// kernel boot context (no process yet) it defaults to 1 — the single
/// boot application.
pub(super) fn current_pid() -> i16 {
    let bp = crate::bdos::process::current();
    if bp.is_null() {
        return 1;
    }
    crate::bdos::process::slot_for(bp)
        .map(|s| s as i16 + 1)
        .unwrap_or(1)
}

/// Screen bit-plane count (`ap_planes`) = log2 of the device colour
/// count (16 colours → 4 planes); 4 when no display is up.
fn screen_planes() -> i16 {
    crate::host::host()
        .and_then(|h| h.raster())
        .map(|r| r.num_colors().trailing_zeros() as i16)
        .unwrap_or(4)
}

/// Opcode 10 — `appl_init`: register the application, fill `AESGLOBAL`,
/// return the application id (`ap_id` = pid) or −1 on failure.
pub fn appl_init(pb: &Aespb) {
    let pid = current_pid();
    // SAFETY: single trap path.
    let ok = unsafe { pd::open(pid) }.is_some();
    if !ok {
        pb.set_ret(-1);
        return;
    }

    // Fill the caller's AESGLOBAL (native layout).
    // SAFETY: `global` points at a caller-allocated AESGLOBAL.
    let g = pb.global_ptr() as *mut AesGlobal;
    unsafe {
        (*g).ap_version = AES_VERSION;
        (*g).ap_count = 1;
        (*g).ap_id = pid;
        (*g).ap_private = 0;
        (*g).ap_ptree = 0;
        (*g).ap_rscmem = 0;
        (*g).ap_rsclen = 0;
        (*g).ap_planes = screen_planes();
        (*g).ap_3resv = 0;
        (*g).ap_4resv = 0;
    }
    pb.set_ret(pid);
}

/// Opcode 19 — `appl_exit`: deregister the application. Returns 1.
pub fn appl_exit(pb: &Aespb) {
    let pid = current_pid();
    // SAFETY: single trap path.
    unsafe { pd::close(pid) };
    pb.set_ret(1);
}

/// Opcode 13 — `appl_find`: find an application by its 8-char name in
/// `addrin[0]`. Returns the pid, or −1 if not found (or name is NULL).
pub fn appl_find(pb: &Aespb) {
    let name_ptr = pb.addrin(0);
    if name_ptr == 0 {
        pb.set_ret(-1);
        return;
    }
    let mut name = [b' '; 8];
    // SAFETY: addrin[0] is a caller-supplied pointer to ≥ 8 name bytes.
    unsafe {
        let p = name_ptr as *const u8;
        for (i, slot) in name.iter_mut().enumerate() {
            *slot = p.add(i).read();
        }
    }
    // SAFETY: single trap path.
    let pid = unsafe { pd::find_by_name(&name) };
    pb.set_ret(pid);
}

/// Opcode 17 — `appl_yield`: cooperative yield to the AES scheduler.
/// With a single AES application there is nothing to schedule to, so this
/// returns immediately (the documented single-app behaviour).
pub fn appl_yield(pb: &Aespb) {
    pb.set_ret(0);
}

/// Opcode 12 — `appl_write`: deliver a message into another application's
/// pipe. `intin[0]` = target id, `intin[1]` = byte length, `addrin[0]` =
/// message buffer. Returns 1 on success, 0 if the target is unknown.
/// The standard GEM message is 16 bytes (8 WORDs); longer writes are
/// truncated to the single pending slot (bring-up limitation).
pub fn appl_write(pb: &Aespb) {
    let target = pb.intin(0);
    let len_bytes = pb.intin(1).max(0) as usize;
    let buf = pb.addrin(0);
    // SAFETY: single trap path.
    let Some(dst) = (unsafe { pd::by_pid(target) }) else {
        pb.set_ret(0);
        return;
    };
    if buf == 0 {
        pb.set_ret(0);
        return;
    }
    let words = (len_bytes / 2).min(pd::MSG_WORDS);
    // SAFETY: addrin[0] points at ≥ len_bytes of message data.
    unsafe {
        let p = buf as *const i16;
        for i in 0..words {
            dst.msg[i] = p.add(i).read();
        }
    }
    dst.msg_pending = true;
    pb.set_ret(1);
}

/// Opcode 11 — `appl_read`: read a message from the calling application's
/// pipe. `intin[0]` = own id, `intin[1]` = byte length, `addrin[0]` =
/// destination buffer. Returns 1 when a message was delivered, 0 when the
/// pipe is empty.
///
/// Real GEM blocks here until a message arrives; with no AES scheduler
/// yet, the bring-up returns the pending message if present and 0
/// otherwise (messages are posted synchronously by the menu/window code).
pub fn appl_read(pb: &Aespb) {
    let id = pb.intin(0);
    let len_bytes = pb.intin(1).max(0) as usize;
    let dest = pb.addrin(0);
    // SAFETY: single trap path.
    let Some(src) = (unsafe { pd::by_pid(id) }) else {
        pb.set_ret(0);
        return;
    };
    if dest == 0 || !src.msg_pending {
        pb.set_ret(0);
        return;
    }
    let words = (len_bytes / 2).min(pd::MSG_WORDS);
    // SAFETY: addrin[0] points at ≥ len_bytes of destination space.
    unsafe {
        let p = dest as *mut i16;
        for i in 0..words {
            p.add(i).write(src.msg[i]);
        }
    }
    src.msg_pending = false;
    src.flags |= pd::AP_MESAG;
    pb.set_ret(1);
}
