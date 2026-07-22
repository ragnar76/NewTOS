//! AES Application Manager (`appl_*`, opcodes 10–19). Spec:
//! [`docs/api/aes/application.md`].
//!
//! `appl_init` registers a GEM application (its id = the GEMDOS process
//! id), fills the caller's `AESGLOBAL`, and marks it `AP_OPEN`;
//! `appl_exit` tears it down; `appl_find` looks one up by name;
//! `appl_yield` is the cooperative scheduler hand-off.
//!
//! **ABI note (ADR-020).** `appl_init` fills the caller's `AESGLOBAL` at
//! the **caller's** pointer width, exactly as the AESPB block pointers and
//! the `OBJECT` tree do. In **both** ABIs the caller's array is gemlib's
//! `short aes_global[AES_GLOBMAX]` — `AES_GLOBMAX = 16`, so **exactly 32
//! bytes** regardless of pointer width (`short` is always 2 bytes;
//! `globals.c:40`, `mt_gem.h:2471`; confirmed by `nm QED.NAP`:
//! `aes_global` symbol size `0x20`). gemlib's accessor macros (`gem.h:30`,
//! `..:72`) carry **no** `__LP64__`/pointer-size conditional, so the
//! `long`/pointer fields simply widen with the ABI. Field → gem.h reader →
//! index → byte offset → width (`a_appl_init.c:41` for index 10 =
//! "number of planes of the VDI workstation"):
//!
//!    | field      | gem.h reader              | idx | byte | ILP32 | LP64 |
//!    |------------|---------------------------|-----|------|-------|------|
//!    | ap_version | `aes_global[0]`           | 0   | 0    | 2     | 2    |
//!    | ap_count   | `aes_global[1]`           | 1   | 2    | 2     | 2    |
//!    | ap_id      | `aes_global[2]`           | 2   | 4    | 2     | 2    |
//!    | ap_private | `*(long*)&aes_global[3]`  | 3   | 6    | 4     | 8    |
//!    | ap_ptree   | `*(OBJECT***)&[5]`        | 5   | 10   | 4     | 8    |
//!    | ap_rscmem  | `*(void**)&aes_global[7]` | 7   | 14   | 4     | 8    |
//!    | ap_rsclen  | `aes_global[9]`           | 9   | 18   | 2     | 2    |
//!    | ap_planes  | `aes_global[10]`          | 10  | 20   | 2     | 2    |
//!    | ap_3resv   | (no gemlib reader)        |11–12| 22   | 4     | 4    |
//!    | ap_maxchar | `aes_global[13]`          | 13  | 26   | 2     | 2    |
//!    | ap_minchar | `aes_global[14]`          | 14  | 28   | 2     | 2    |
//!    | (reserved) | —                         | 15  | 30   | 2     | 2    |
//!
//!    `ap_3resv` is EmuTOS's `LONG ap_3resv` (`gemrslib.h:25`, the AES-internal
//!    `&D` pointer, `gemsuper.c:98`). It has **no** gemlib accessor (`gem.h`
//!    defines readers only through `ap_minchar`), so it is a **4-byte** field
//!    on both ABIs — kept 4 bytes even at LP64 so the app-readable
//!    `ap_maxchar`/`ap_minchar` (indices 13/14) stay 0 as at EmuTOS
//!    `appl_init`. On LP64 the 64-bit AES-state pointer is therefore truncated
//!    to its low 32 bits; harmless, because nothing reads `ap_3resv`
//!    (no gemlib reader; NewTOS's AES holds its state in kernel statics, not by
//!    reading this field back — the EmuTOS use of `&D` served here by
//!    [`crate::aes::pd::table_addr`] as a fidelity fill, not a live pointer).
//!
//!    On **LP64** the three 8-byte fields at bytes 6/10/14 physically
//!    *overlap* (6–13, 10–17, 14–21) — a gemlib quirk of its m68k-era
//!    (4-byte-`long`) macros. `ap_private` is `rs_trees[DESKTOP][ROOT].ob_spec`
//!    (`gemsuper.c:96`), a ≤32-bit box `gr_crack` word, so its meaningful bits
//!    live in the **low 4 bytes** (6–9); `ap_ptree` (0) written next clears the
//!    overlapping high 4 bytes (10–13, already 0 for a box word), so a native
//!    LP64 reader's `*(long*)&aes_global[3]` reads it back intact. `ap_ptree`
//!    and `ap_rscmem` are 0 at `appl_init`; `ap_planes` (byte 20–21, the high
//!    WORD of the zero `ap_rscmem`) is stored last so it survives. Both
//!    [`AesGlobalVals::write_ilp32`] and [`AesGlobalVals::write_lp64`] touch
//!    **no byte past index 31**. The
//!    previous LP64 writer laid the kernel's own *natural-aligned 56-byte*
//!    struct here, overrunning the 32-byte array by 24 bytes on every
//!    `appl_init` — clobbering gemlib's adjacent `vdi_dummy` (`nm
//!    QED.NAP`: `aes_global` `0x70a36`+`0x20`, `vdi_dummy` `0x70a56`) — and
//!    also placing `ap_ptree`/`ap_rscmem`/`ap_planes` at the wrong offsets
//!    for a native LP64 gemlib reader.

use super::pd;
use super::Aespb;
use crate::bdos::process::Abi;

/// AES version reported in `ap_version` (`include/config.h` default).
const AES_VERSION: i16 = 0x0340;

/// The values `appl_init` reports back through the caller's `AESGLOBAL`,
/// held ABI-independently. [`Self::write`] lays them into the caller's
/// gemlib `short aes_global[16]` (32 bytes either way) at the caller's
/// pointer width: LP64 widens the `long`/pointer fields to 8 bytes
/// ([`Self::write_lp64`]), ILP32 keeps them 4 bytes ([`Self::write_ilp32`]).
struct AesGlobalVals {
    ap_version: i16,
    ap_count: i16,
    ap_id: i16,
    /// LONG global; a native pointer / packed value on LP64, truncated to
    /// 32 bits for an ILP32 caller.
    ap_private: u64,
    /// Pointer to the loaded resource's tree table (0 at `appl_init`).
    ap_ptree: u64,
    /// Pointer to the loaded resource's memory (0 at `appl_init`).
    ap_rscmem: u64,
    /// Loaded-resource length (0 at `appl_init`).
    ap_rsclen: i16,
    /// VDI workstation plane count (ILP32 index 10, `a_appl_init.c:40`).
    ap_planes: i16,
    /// EmuTOS `ap_3resv = (LONG)&D` (`gemsuper.c:98`, `gemrslib.h:25`): a
    /// pointer to the AES global data area. Written as a **4-byte** field at
    /// byte 22 (see the module ABI note); on LP64 the 64-bit pointer is
    /// truncated (harmless — no reader).
    ap_3resv: u64,
}

impl AesGlobalVals {
    /// Lay the values into the caller's `global` array at `abi`'s width.
    ///
    /// # Safety
    /// `global` must address a caller-allocated `AESGLOBAL` — gemlib's
    /// `short aes_global[AES_GLOBMAX]`, ≥ **32 bytes** (`AES_GLOBMAX = 16`
    /// shorts) for **either** [`Abi`]; only the long/pointer field widths
    /// differ (see the module-level ABI note).
    unsafe fn write(&self, global: *mut i16, abi: Abi) {
        match abi {
            // SAFETY: forwarded to this method's width contract.
            Abi::Lp64 => unsafe { self.write_lp64(global) },
            // SAFETY: forwarded to this method's width contract.
            Abi::Ilp32 => unsafe { self.write_ilp32(global) },
        }
    }

    /// gemlib's `short aes_global[16]` (32 bytes) at **LP64** widths: the
    /// `long`/pointer fields are 8 bytes on 16-bit-word boundaries (indices
    /// 3/5/7 → bytes 6/10/14, none 8-aligned → unaligned stores). gemlib
    /// has no `__LP64__` conditional (`gem.h:30`, `..:72`), so these are
    /// exactly `_AESappglobal`/`_AESrscfile`/`_AESrscmem`. On LP64 they
    /// overlap (bytes 6–13, 10–17, 14–21); `ap_private` is a ≤32-bit box word
    /// (its meaningful bits in bytes 6–9) and the zero `ap_ptree` written next
    /// clears the overlapping high bytes (10–13), so storing them in ascending
    /// order and writing `ap_planes` (byte 20–21) last leaves the array
    /// correct. `ap_3resv` is a 4-byte field at byte 22 (indices 11–12);
    /// `ap_maxchar`/`ap_minchar`/reserved (indices 13–15) are cleared so they
    /// stay 0 as at EmuTOS `appl_init`. Writes stay within the 32-byte array
    /// (last byte touched = index 31); no overrun.
    ///
    /// # Safety
    /// `global` addresses a writable 32-byte `short[16]` array.
    unsafe fn write_lp64(&self, global: *mut i16) {
        let base = global as *mut u8;
        // SAFETY: every access below is within the 32-byte array (max byte
        // index 31, written at index 15); `base`/`global` are valid and
        // writable per the contract. The 8-byte fields at bytes 6/10/14 are
        // only 2-byte aligned, hence `write_unaligned`. Ascending offset
        // order makes the `ap_planes` short (byte 20) win its overlap with
        // the high WORD of the zero `ap_rscmem` (bytes 14–21).
        unsafe {
            global.add(0).write(self.ap_version); // ap_version [0]    @0
            global.add(1).write(self.ap_count); //   ap_count   [1]    @2
            global.add(2).write(self.ap_id); //       ap_id      [2]    @4
            (base.add(6) as *mut u64).write_unaligned(self.ap_private); // [3–6]  @6
            (base.add(10) as *mut u64).write_unaligned(self.ap_ptree); // [5–8]  @10
            (base.add(14) as *mut u64).write_unaligned(self.ap_rscmem); // [7–10] @14
            global.add(9).write(self.ap_rsclen); //   ap_rsclen  [9]    @18
            global.add(10).write(self.ap_planes); //  ap_planes  [10]   @20
            // ap_3resv [11–12] @22 — EmuTOS `LONG ap_3resv = &D` (`gemsuper.c:98`).
            // A 4-byte field (no gemlib reader; see the module ABI note) so the
            // app-readable ap_maxchar/ap_minchar below stay 0; the 64-bit AES
            // state pointer is truncated to its low 32 bits.
            (base.add(22) as *mut u32).write_unaligned(self.ap_3resv as u32);
            // ap_maxchar [13] / ap_minchar [14] (AES 0x0400+, not reported at
            // version 0x0340) and reserved [15]. Cleared so no stale value
            // leaks — matching EmuTOS's 0 for these at appl_init.
            for i in 13..16 {
                global.add(i).write(0);
            }
        }
    }

    /// gemlib's packed `short aes_global[16]` (32 bytes). Long/pointer
    /// fields are 4 bytes on 16-bit-word boundaries (indices 3/5/7 →
    /// bytes 6/10/14, none 4-aligned → unaligned stores), truncated to
    /// 32 bits. `ap_3resv` is a 4-byte field at byte 22 (indices 11–12);
    /// `ap_maxchar`/`ap_minchar`/reserved (indices 13–15) are cleared.
    /// Writes stay within the 32-byte array.
    ///
    /// # Safety
    /// `global` addresses a writable 32-byte `short[16]` array.
    unsafe fn write_ilp32(&self, global: *mut i16) {
        let base = global as *mut u8;
        // SAFETY: every access below is within the 32-byte array (max
        // byte index 31, written at index 15); `base`/`global` are valid
        // and writable per the contract. The 4-byte fields at bytes
        // 6/10/14/22 are unaligned, hence `write_unaligned`.
        unsafe {
            global.add(0).write(self.ap_version); // ap_version [0] @0
            global.add(1).write(self.ap_count); //   ap_count   [1] @2
            global.add(2).write(self.ap_id); //       ap_id      [2] @4
            (base.add(6) as *mut u32).write_unaligned(self.ap_private as u32); // [3–4] @6
            (base.add(10) as *mut u32).write_unaligned(self.ap_ptree as u32); // [5–6] @10
            (base.add(14) as *mut u32).write_unaligned(self.ap_rscmem as u32); // [7–8] @14
            global.add(9).write(self.ap_rsclen); //   ap_rsclen  [9] @18
            global.add(10).write(self.ap_planes); //  ap_planes  [10] @20
            // ap_3resv [11–12] @22 — EmuTOS `LONG ap_3resv = &D` (`gemsuper.c:98`).
            (base.add(22) as *mut u32).write_unaligned(self.ap_3resv as u32);
            // ap_maxchar [13] / ap_minchar [14] (AES 0x0400+, not reported at
            // version 0x0340) and reserved [15]. Cleared so no stale value
            // leaks — matching EmuTOS's 0 for these at appl_init.
            for i in 13..16 {
                global.add(i).write(0);
            }
        }
    }
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

/// Screen bit-plane count (`ap_planes`), the VDI's `INQ_TAB[4] = v_planes`.
/// EmuTOS' AES reads exactly this: `ap_planes = gl_nplanes`
/// (`gemsuper.c:97`), `gl_nplanes = gsx_nplanes()` (`gemgraf.c:395`), and
/// `gsx_nplanes()` returns `intout[4]` of an `EXTENDED_INQUIRE`
/// (`gemgsxif.c:497-501`) — i.e. `INQ_TAB[4]`. This is **not**
/// `log2(num_colors)`: under Falcon truecolor `v_planes == 16` while the
/// pen count is 256 (they only coincide in the palette modes, 16 colours =
/// 4 planes). 4 when no display is up.
fn screen_planes() -> i16 {
    crate::host::host()
        .and_then(|h| h.raster())
        .map(|r| r.planes() as i16)
        .unwrap_or(4)
}

/// Opcode 10 — `appl_init`: register the application, fill `AESGLOBAL`,
/// return the application id (`ap_id` = pid) or −1 on failure.
///
/// Fills the caller's `AESGLOBAL` **1:1 with EmuTOS's `APPL_INIT` arm**
/// (`gemsuper.c:92-98`), field by field, at the caller's pointer width:
///
/// - `ap_version` (`:92`) = [`AES_VERSION`] (`0x0340`);
/// - `ap_count` (`:93`) = 1;
/// - `ap_id` (`:94`) = the caller pid ([`current_pid`]);
/// - `ap_private` (`:95-96`) = `rs_trees[DESKTOP][ROOT].ob_spec` — the desktop
///   background pattern/colour word. NewTOS's equivalent is the WF_NEWDESK
///   registered desktop-background tree's root `ob_spec`, read live via
///   [`super::window::desktop_bg_obspec`] (see its doc for the pre-registration
///   fallback);
/// - `ap_planes` (`:97`) = `gl_nplanes`, the screen colour-plane count
///   ([`screen_planes`]);
/// - `ap_3resv` (`:98`) = `(LONG)&D`, a pointer to the AES global data area —
///   NewTOS's nearest equivalent is the AES process-descriptor table address
///   ([`pd::table_addr`]).
///
/// `ap_ptree`/`ap_rscmem`/`ap_rsclen` are **not** set by `appl_init` (EmuTOS
/// leaves them for `rsrc_load`), so they are 0 here.
pub fn appl_init(pb: &Aespb) {
    let pid = current_pid();
    // SAFETY: single trap path.
    let ok = unsafe { pd::open(pid) }.is_some();
    if !ok {
        pb.set_ret(-1);
        return;
    }

    let vals = AesGlobalVals {
        ap_version: AES_VERSION,                        // gemsuper.c:92
        ap_count: 1,                                    // gemsuper.c:93
        ap_id: pid,                                     // gemsuper.c:94
        ap_private: super::window::desktop_bg_obspec(), // gemsuper.c:95-96
        ap_ptree: 0,                                    // set later by rsrc_load
        ap_rscmem: 0,                                   // set later by rsrc_load
        ap_rsclen: 0,                                   // set later by rsrc_load
        ap_planes: screen_planes(),                     // gemsuper.c:97
        ap_3resv: pd::table_addr(),                     // gemsuper.c:98
    };

    // Fill the caller's AESGLOBAL — gemlib's 32-byte `short aes_global[16]`
    // either way — at the caller's pointer width: 8-byte long/pointer
    // fields for an LP64 caller, 4-byte for an ILP32 one (no byte past
    // index 31 in either case). See the module-level ABI note.
    // SAFETY: `global` points at a caller-allocated AESGLOBAL of `pb.abi`'s
    // width (the GEM caller's contract; the AESPB was built for that ABI).
    unsafe { vals.write(pb.global_ptr(), pb.abi) };
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
/// message buffer. Returns 1 on success, 0 if the target is unknown or its
/// message queue is full. The standard GEM message is 16 bytes (8 WORDs);
/// longer writes are truncated to that length, then appended to the target's
/// FIFO queue (EmuTOS `ap_rdwr(MU_SDMSG)`→`doq`, `gemqueue.c:85-125,31-74`).
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
    let mut msg = [0i16; pd::MSG_WORDS];
    // SAFETY: addrin[0] points at ≥ len_bytes of message data.
    unsafe {
        let p = buf as *const i16;
        for (i, slot) in msg.iter_mut().take(words).enumerate() {
            *slot = p.add(i).read();
        }
    }
    // Enqueue into the target's FIFO (0 if the queue is full).
    pb.set_ret(dst.queue.push(msg) as i16);
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
    if dest == 0 {
        pb.set_ret(0);
        return;
    }
    // Pop the FRONT of the FIFO (EmuTOS `ap_rdwr(MU_RDMSG)`→`doq` dequeue,
    // `gemqueue.c:76-81`); empty queue → 0.
    let Some(msg) = src.queue.pop() else {
        pb.set_ret(0);
        return;
    };
    let words = (len_bytes / 2).min(pd::MSG_WORDS);
    // SAFETY: addrin[0] points at ≥ len_bytes of destination space.
    unsafe {
        let p = dest as *mut i16;
        for (i, &w) in msg.iter().take(words).enumerate() {
            p.add(i).write(w);
        }
    }
    src.flags |= pd::AP_MESAG;
    pb.set_ret(1);
}

/// Opcode 15 — `appl_trecord`: record AES events into the buffer at
/// `addr_in[0]` (`int_in[0]` = capacity). NewTOS has no event-tape recorder,
/// so nothing is captured and it returns 0 (the number of events recorded) —
/// an honest report of the capability, not a faked success. A documented
/// follow-up (event recording needs the input pipeline to tee events).
pub fn appl_trecord(pb: &Aespb) {
    pb.set_ret(0);
}

/// Opcode 14 — `appl_tplay`: play back the `int_in[0]` recorded events at
/// `addr_in[0]` (time scale `int_in[1]`). NewTOS has no path to inject events
/// back into the AES input from here, so this returns at once — a documented
/// follow-up (pairs with `appl_trecord`).
pub fn appl_tplay(pb: &Aespb) {
    pb.set_ret(0);
}
