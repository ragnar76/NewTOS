//! AES Application Manager (`appl_*`, opcodes 10–19). Spec:
//! [`docs/api/aes/application.md`].
//!
//! `appl_init` registers a GEM application (its id = the GEMDOS process
//! id), fills the caller's `AESGLOBAL`, and marks it `AP_OPEN`;
//! `appl_exit` tears it down; `appl_find` looks one up by name;
//! `appl_yield` is the cooperative scheduler hand-off.
//!
//! **What `appl_init` may write, and how far (D-R1-4).** `AESGLOBAL` is
//! EmuTOS's `struct aesglobal` (`aes/gemrslib.h:15-27`) — ten fields,
//! **30 bytes = 15 WORDs** on the m68k. EmuTOS's own AES binding sizes its
//! array to exactly that: `#define G_SIZE 15` / `UWORD global[G_SIZE]`
//! (`desk/gembind.h:27`, `desk/gembind.c:66`). There is no index 15.
//! `appl_init` writes **six** of the ten fields (`aes/gemsuper.c:92-98`) and
//! leaves the other four alone; NewTOS writes the same six and nothing else,
//! so a caller of either declared size gets EmuTOS's result and no write
//! beyond word 14.
//!
//! **ABI note — the pointer fields do not widen (decision of 2026-09-13).**
//! `AESGLOBAL` is a **TOS structure**, so NewTOS lays it out exactly as
//! `gemrslib.h:15-27` does on the m68k: 30 bytes, every `LONG`/pointer field
//! **4 bytes**, on **both** ABIs. It is the one struct in the GEM boundary
//! that is addressed two ways at once — by byte offset *and* by WORD index
//! (`AP_VERSION`..`AP_4RESV`, `include/aesdefs.h:147-157`; every client
//! declares it as `short aes_global[]`, `gemlib globals.c:40`). Widening the
//! pointers would move `ap_planes` off `global[10]` and `ap_rsclen` off
//! `global[9]`, i.e. break the index half of the contract; the two halves are
//! only simultaneously satisfiable at the m68k widths. (This is the named
//! exception to `docs/COMPATIBILITY.md` §"Native vs m68k struct variants":
//! `OBJECT` has no index contract and does follow the caller's width.)
//! Field → gem.h reader → index → byte offset → width (`a_appl_init.c:41`
//! for index 10 = "number of planes of the VDI workstation"):
//!
//!    | field      | gem.h reader    | idx | byte | width | appl_init writes it? |
//!    |------------|-----------------|-----|------|-------|---|
//!    | ap_version | `aes_global[0]` | 0   | 0    | 2     | yes (`gemsuper.c:92`) |
//!    | ap_count   | `aes_global[1]` | 1   | 2    | 2     | yes (`:93`) |
//!    | ap_id      | `aes_global[2]` | 2   | 4    | 2     | yes (`:94`) |
//!    | ap_private | `_AESappglobal` | 3   | 6    | 4     | yes (`:95-96`) |
//!    | ap_ptree   | `_AESrscfile`   | 5   | 10   | 4     | **no** |
//!    | ap_rscmem  | `_AESrscmem`    | 7   | 14   | 4     | **no** |
//!    | ap_rsclen  | `_AESrsclen`    | 9   | 18   | 2     | **no** |
//!    | ap_planes  | `aes_global[10]`| 10  | 20   | 2     | yes (`:97`) |
//!    | ap_3resv   | (no reader)     |11–12| 22   | 4     | yes (`:98`) |
//!    | ap_4resv   | `[13]`/`[14]`   |13–14| 26   | 4     | **no** |
//!
//!    Every field owns its own bytes; no two overlap. **The toolchain, not
//!    the layout, was what had to move**: pristine gemlib's accessors carry
//!    no `__LP64__` arm (`gem.h:41,49,57`), so for an LP64 caller
//!    `*(long*)&aes_global[3]`, `*(OBJECT***)&aes_global[5]` and
//!    `*(void**)&aes_global[7]` are eight bytes each — three pointers inside
//!    fourteen bytes (6..13 / 10..17 / 14..21). NewTOS narrows those three
//!    readers to the TOS widths in `sdk/gemlib/overlay/{gem.h,mt_gem.h.in}`;
//!    the m68k build (`build-gemlib-m68k.sh`) takes the pristine tree, where
//!    `long` is already 4 bytes. Before that overlay an LP64 `_AESrscmem`
//!    read bytes 14..21 and so returned `ap_planes << 48` — a wild pointer
//!    manufactured by the *reader*, not by a second write.
//!
//!    `ap_ptree`/`ap_rscmem`/`ap_rsclen` belong to `rs_load`
//!    (`gemrslib.c:595-597,787-788`), `ap_4resv` to nobody — EmuTOS's
//!    `APPL_INIT` arm never names them, so neither do we. They keep whatever
//!    the caller had; a freshly `.bss`-allocated array therefore still reads
//!    0, which is what the EmuTOS reference dump shows for those slots.
//!    NewTOS's `rsrc_load` fills the first three since 2026-09-14 (A35,
//!    `tos-api/src/aes/resource.rs::report_load`) — at these same TOS widths,
//!    and only when the loaded blob's base fits the four bytes `ap_ptree`
//!    and `ap_rscmem` have.
//!    (The last 4 bytes are EmuTOS's `LONG ap_4resv` "reserved for use in AES
//!    4.00"; gemlib splits the same bytes into two WORD readers
//!    `ap_maxchar`/`ap_minchar` at indices 13/14. Either way `appl_init` at
//!    `ap_version 0x0340` does not produce a value for them.)
//!
//!    `ap_3resv` is EmuTOS's `LONG ap_3resv` (`gemrslib.h:25`, the AES-internal
//!    `&D` pointer, `gemsuper.c:98`). It has **no** gemlib accessor (`gem.h`
//!    defines readers only through index 14). On LP64 the 64-bit AES-state
//!    pointer is truncated to its low 32 bits; harmless, because nothing reads
//!    `ap_3resv` (NewTOS's AES holds its state in kernel statics, not by
//!    reading this field back — the EmuTOS use of `&D` is served here by
//!    [`crate::aes::pd::table_addr`] as a fidelity fill, not a live pointer).
//!
//!    History, so the three earlier defects are not re-introduced: the
//!    original LP64 writer laid the kernel's own *natural-aligned 56-byte*
//!    struct here, overrunning a 32-byte array by 24 bytes on every
//!    `appl_init` — clobbering gemlib's adjacent `vdi_dummy` (`nm QED.NAP`:
//!    `aes_global` `0x70a36`+`0x20`, `vdi_dummy` `0x70a56`). Its replacement
//!    fixed the offsets but still wrote all of `aes_global[0..=15]`, i.e.
//!    gemlib's 32-byte `AES_GLOBMAX = 16` array — two bytes past the 30-byte
//!    `AESGLOBAL` an EmuTOS/Compendium-shaped caller declares (D-R1-4). The
//!    third wrote `ap_private` eight bytes wide at byte 6 and so also wrote
//!    `ap_ptree`'s four bytes, which EmuTOS's `APPL_INIT` arm never touches.

use super::pd;
use super::Aespb;

/// AES version reported in `ap_version` (`include/config.h` default).
const AES_VERSION: i16 = 0x0340;

/// The six fields `appl_init` reports back through the caller's `AESGLOBAL`
/// (`aes/gemsuper.c:92-98`), held ABI-independently. [`Self::write`] lays them
/// into the caller's array at the **m68k** field widths, the same on every ABI
/// (see the module ABI note) — which is why there is one `write` and not a
/// per-ABI pair. The four fields EmuTOS's `APPL_INIT` arm does not name
/// (`ap_ptree`, `ap_rscmem`, `ap_rsclen`, `ap_4resv`) have no representation
/// here — they are not written, so there is nothing to hold.
struct AesGlobalVals {
    ap_version: i16,
    ap_count: i16,
    ap_id: i16,
    /// LONG global; a native pointer / packed value on LP64, truncated to
    /// 32 bits for an ILP32 caller.
    ap_private: u64,
    /// VDI workstation plane count (index 10, `a_appl_init.c:40`).
    ap_planes: i16,
    /// EmuTOS `ap_3resv = (LONG)&D` (`gemsuper.c:98`, `gemrslib.h:25`): a
    /// pointer to the AES global data area. Written as a **4-byte** field at
    /// byte 22 (see the module ABI note); on LP64 the 64-bit pointer is
    /// truncated (harmless — no reader).
    ap_3resv: u64,
}

impl AesGlobalVals {
    /// Lay the six `APPL_INIT` fields into the caller's `global` array —
    /// byte for byte the m68k `AESGLOBAL` (`aes/gemrslib.h:15-27`), on
    /// **both** ABIs: the WORD fields at `short`-array indices 0/1/2/10,
    /// `ap_private` 4 bytes at byte 6, `ap_3resv` 4 bytes at byte 22. See
    /// the module-level ABI note for why the pointer fields do not widen
    /// with the caller's pointer width.
    ///
    /// `ap_ptree` (byte 10), `ap_rscmem` (14), `ap_rsclen` (18) and
    /// `ap_4resv` (26) are **not** written — EmuTOS's `APPL_INIT` arm does
    /// not name them (`gemsuper.c:92-98`); the first three belong to
    /// `rs_load`/`rs_free`. Last byte touched is 25, so a 30-byte
    /// `AESGLOBAL` caller is never overrun.
    ///
    /// # Safety
    /// `global` must address a caller-allocated, writable `AESGLOBAL` of at
    /// least the **30 bytes / 15 WORDs** of EmuTOS's `struct aesglobal`
    /// (`aes/gemrslib.h:15-27`, `desk/gembind.h:27` `G_SIZE 15`).
    unsafe fn write(&self, global: *mut i16) {
        let base = global as *mut u8;
        // SAFETY: every access below is within the 30-byte AESGLOBAL (max
        // byte touched = 25); `base`/`global` are valid and writable per the
        // contract. The 4-byte fields at bytes 6 and 22 are only 2-byte
        // aligned, hence `write_unaligned`.
        unsafe {
            global.add(0).write(self.ap_version); // ap_version [0] @0
            global.add(1).write(self.ap_count); //   ap_count   [1] @2
            global.add(2).write(self.ap_id); //       ap_id      [2] @4
            // ap_private [3–4] @6 — EmuTOS `LONG ap_private`
            // (`gemsuper.c:95-96`), the DESKTOP root's `ob_spec`: a
            // `gr_crack` colour word, so nothing is lost in 32 bits.
            (base.add(6) as *mut u32).write_unaligned(self.ap_private as u32);
            global.add(10).write(self.ap_planes); //  ap_planes  [10] @20
            // ap_3resv [11–12] @22 — EmuTOS `LONG ap_3resv = &D`
            // (`gemsuper.c:98`); the 64-bit AES state pointer is truncated to
            // its low 32 bits (no reader, see the module ABI note).
            (base.add(22) as *mut u32).write_unaligned(self.ap_3resv as u32);
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
/// - `ap_private` (`:95-96`) = `rs_trees[DESKTOP][ROOT].ob_spec` — the ROOT of
///   the AES's **own** built-in `DESKTOP` tree ([`super::init::desktop_tree`],
///   transcribed from `gem_rsc.c:550-570`), i.e. `0x1143` unless the `.INF`
///   file's `#Q` line replaced its low byte at AES init
///   (`super::init::set_aes_background`). Read live via
///   [`super::init::desktop_root_obspec`]. This is **not** the tree a desktop
///   registers with `wind_set(WF_NEWDESK)` — that one lands in `gl_newdesk`
///   (`gemwmlib.c:1687`) and never reaches `ap_private`;
/// - `ap_planes` (`:97`) = `gl_nplanes`, the screen colour-plane count
///   ([`screen_planes`]);
/// - `ap_3resv` (`:98`) = `(LONG)&D`, a pointer to the AES global data area —
///   NewTOS's nearest equivalent is the AES process-descriptor table address
///   ([`pd::table_addr`]).
///
/// `ap_ptree`/`ap_rscmem`/`ap_rsclen` (for `rs_load`/`rs_free`) and
/// `ap_4resv` are **not** named by EmuTOS's `APPL_INIT` arm, so `appl_init`
/// leaves those bytes exactly as the caller had them — and writes nothing
/// past word 14 of the 15-WORD `AESGLOBAL` (D-R1-4; see the module ABI note).
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
        ap_private: super::init::desktop_root_obspec(), // gemsuper.c:95-96
        ap_planes: screen_planes(),                     // gemsuper.c:97
        ap_3resv: pd::table_addr(),                     // gemsuper.c:98
    };

    // Fill the caller's AESGLOBAL — the 15-WORD / 30-byte `struct aesglobal`
    // (`gemrslib.h:15-27`, `gembind.h:27`) — with exactly the six fields
    // `gemsuper.c:92-98` writes, in the m68k field widths that structure
    // keeps on every ABI. No byte past index 12 is touched. See the
    // module-level ABI note.
    // SAFETY: `global` points at a caller-allocated AESGLOBAL (the GEM
    // caller's contract; the AESPB was built for that ABI).
    unsafe { vals.write(pb.global_ptr()) };
    pb.set_ret(pid);
}

/// Opcode 19 — `appl_exit`: deregister the application. Returns 1.
///
/// "Deregister" is EmuTOS's `ap_exit` (`aes/gemaplib.c:251-261`) and is
/// narrower than it sounds: the pending message queue is drained and `AP_OPEN`
/// is cleared, but the process descriptor itself stays — see [`pd::close`].
/// The `APPL_EXIT` arm's `ret` is the enclosing `TRUE` (`gemsuper.c:85,120-123`).
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
/// EmuTOS's `APPL_YIELD` arm (`gemsuper.c:115-119`, `#if CONF_WITH_PCGEM`,
/// default-on per `config.h`) only calls `dsptch()` and `break`s, leaving the
/// enclosing `ret = TRUE` (`gemsuper.c:85`) — so it returns **1**
/// (`application.md:229`). With a single AES application there is nothing to
/// schedule to, so NewTOS returns immediately, but with the same `1`.
pub fn appl_yield(pb: &Aespb) {
    pb.set_ret(1);
}

/// Opcode 12 — `appl_write`: deliver a message into another application's
/// pipe. `intin[0]` = target id, `intin[1]` = byte length (`AP_LENGTH`),
/// `addrin[0]` = message buffer. Returns 1 on success, 0 if the target is
/// unknown or its message queue is full. The message length is taken from
/// `AP_LENGTH` (the standard GEM message is 16 bytes / 8 WORDs, the normal
/// case) and clamped to the FIFO's per-message limit — [`pd::MAX_MSG_WORDS`] =
/// `QUEUE_SIZE / 2`, matching EmuTOS's 128-byte queue (`ap_rdwr(MU_SDMSG)` →
/// `doq`, which memcpys `qpb_cnt` bytes, `gemqueue.c:36,85-125`). The bytes are
/// then appended to the target's FIFO whole.
pub fn appl_write(pb: &Aespb) {
    let target = pb.intin(0);
    let len_bytes = pb.intin(1).max(0) as usize;
    let buf = pb.addrin(0);
    // SAFETY: single trap path. Existence check drops its borrow immediately so
    // the enqueue below can re-borrow the descriptor through `pd::post_words`.
    if unsafe { pd::by_pid(target) }.is_none() {
        pb.set_ret(0);
        return;
    }
    if buf == 0 {
        pb.set_ret(0);
        return;
    }
    // AP_LENGTH is a byte count (EmuTOS `qpb_cnt`); GEM messages are WORD-based,
    // so read whole WORDs, capped at the per-message FIFO limit.
    let words = (len_bytes / 2).min(pd::MAX_MSG_WORDS);
    let mut msg = [0i16; pd::MAX_MSG_WORDS];
    // SAFETY: addrin[0] points at ≥ len_bytes of message data.
    unsafe {
        let p = buf as *const i16;
        for (i, slot) in msg.iter_mut().take(words).enumerate() {
            *slot = p.add(i).read();
        }
    }
    // Enqueue into the target's FIFO via the centralized poster: it appends the
    // message (with the standard redraw/arrow coalescing) AND wakes a peer
    // blocked in `evnt_mesag`/`evnt_multi` on the target's FIFO (WAITING →
    // READY, design §5.2) — no longer bypassing `pd::post_msg`'s wake. Returns 0
    // if the queue is full (AUD-P1-006, a separate finding; EmuTOS blocks the
    // sender instead of returning 0).
    // SAFETY: single trap path.
    pb.set_ret(unsafe { pd::post_words(target, &msg, words) } as i16);
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
    // AP_LENGTH is a byte count; read whole WORDs, capped at the FIFO's
    // per-message limit (EmuTOS `ap_rdwr(MU_MESAG,…,length,…)`).
    let words = (len_bytes / 2).min(pd::MAX_MSG_WORDS);
    let mut buf = [0i16; pd::MAX_MSG_WORDS];
    // Pop the FRONT of the FIFO into the requested-length window (EmuTOS
    // `ap_rdwr(MU_RDMSG)`→`doq` dequeue, `gemqueue.c:76-81`); empty queue → 0.
    let Some(n) = src.queue.pop_into(&mut buf[..words]) else {
        pb.set_ret(0);
        return;
    };
    // SAFETY: addrin[0] points at ≥ len_bytes of destination space.
    unsafe {
        let p = dest as *mut i16;
        for (i, &w) in buf.iter().take(n).enumerate() {
            p.add(i).write(w);
        }
    }
    src.flags |= pd::AP_MESAG;
    pb.set_ret(1);
}

/// Opcode 15 — `appl_trecord`: record AES events into the buffer at
/// `addr_in[0]` (`int_in[0]` = capacity). Returns 0, the number of events
/// recorded, which is literally true: NewTOS records none.
///
/// **Blocked on the AES fork queue, not on this function.** EmuTOS does not
/// record in `ap_trecd` at all — it sets `gl_recd`/`gl_rlen`/`gl_rbuf`
/// (`aes/gemaplib.c:209-213`) and the recording happens inside `forker()`
/// (`aes/gemdisp.c:123-146`), which copies each queued `FPD` into the
/// caller's buffer as events are dispatched; `ap_trecd` then polls
/// `ev_timer(100)` until the queue fills or the tape stops, and converts
/// `FPD` to `EVNTREC` afterwards (`:216-230`). NewTOS' AES has no fork queue
/// — the same gap [`super::chkkbd`] documents for the key path — so there is
/// nothing to tee events out of. The queue is part of the cooperative-kernel
/// work; when it lands, this function and [`appl_tplay`] are its first two
/// consumers. Recorded in `docs/STATUS.md` under *Blocked*, ticket
/// `TASK/A45.md`.
///
/// No error code is invented for the refusal, deliberately: the AES has no
/// convention for one here, and returning a value no TOS ever returns would
/// replace one deviation with another.
pub fn appl_trecord(pb: &Aespb) {
    pb.set_ret(0);
}

/// Opcode 14 — `appl_tplay`: play back the `int_in[0]` recorded events at
/// `addr_in[0]` (time scale `int_in[1]`). EmuTOS's `ap_tplay` is `void` and the
/// `APPL_TPLAY` arm leaves the enclosing `ret = TRUE` (`gemsuper.c:109-110,85`),
/// so it returns **1** (`application.md:273,316`). NewTOS has no path to inject
/// events back into the AES input from here, so it returns at once with that
/// `1`.
///
/// **Blocked on the AES fork queue**, exactly as [`appl_trecord`] is, and the
/// `1` is therefore right for the wrong reason: it is the dispatcher default,
/// not a report that anything played. Playback needs `forkq`/`dsptch` to
/// inject the taped events (`aes/gemaplib.c:126-166`) and, for `MCHNG`, the
/// VDI cursor and motion vectors swapped out so the AES draws the pointer
/// itself (`:139-152`, `gsx_0code(CUR_VECX)`/`MOT_VECX`). Ticket
/// `TASK/A45.md`.
pub fn appl_tplay(pb: &Aespb) {
    pb.set_ret(1);
}
