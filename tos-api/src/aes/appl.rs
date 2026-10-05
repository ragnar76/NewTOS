//! AES Application Manager (`appl_*`, opcodes 10–19). Spec:
//! [`docs/api/aes/application.md`].
//!
//! `appl_init` opens the caller's AES process descriptor, fills the caller's
//! `AESGLOBAL`, and marks it `AP_OPEN`; `appl_exit` closes it; `appl_find`
//! looks one up by name; `appl_yield` is the cooperative scheduler hand-off.
//!
//! **Identity and reported value** (spec § "AES process identity and pid").
//! A client's *identity* — the key of its descriptor, queue, wait-key,
//! resource list and window ownership — is its GEMDOS process slot, or
//! [`pd::DESKTOP`] for the kernel-context desktop ([`current_identity`]).
//! The *value* a program reads back (`ap_id`, `appl_find`, `WF_OWNER`,
//! `msg[1]`) is computed from the identity by [`reported_pid`] and mapped back
//! by [`identity_of`] where a program hands a value in (`appl_write`,
//! `appl_read`). The values are EmuTOS's (contract (b), A56): **0** for every
//! member of the application chain — the desktop, the program it launches,
//! every `Pexec` child at any depth, all on EmuTOS's one `rlr`
//! (`aes/geminit.c:850-877`, `gemshlib.c:572`) — and **1** only as the
//! Screen Manager's sender stamp on `msg[1]` ([`SCRENMGR_PID`]), which no
//! NewTOS process is behind.
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
use core::sync::atomic::{AtomicBool, Ordering};

use crate::bdos::process::sched;

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

/// The AES identity of the current caller — the index of its process
/// descriptor in [`pd`]'s table: its GEMDOS process slot for an EL0 process
/// (`RUN`'s slot), [`pd::DESKTOP`] for the kernel boot context (`RUN` null —
/// the built-in desktop and the boot-time selftests). A non-null `RUN` that
/// names no slot cannot occur (every basepage the loader installs is a
/// slot's); it falls to the desktop's entry, as the old `current_pid` fell to
/// its value 1.
pub(super) fn current_identity() -> usize {
    let bp = crate::bdos::process::current();
    if bp.is_null() {
        return pd::DESKTOP;
    }
    crate::bdos::process::slot_for(bp).unwrap_or(pd::DESKTOP)
}

/// The pid of the application chain — EmuTOS's `D.g_int[0].p_pid`, the one
/// `rlr` the desktop, every launched program and every `Pexec` child run on
/// (`aes/geminit.c:851-877`; never reassigned by `dos_exec`,
/// `gemshlib.c:572`). What `appl_init` returns, `appl_find` answers for any
/// chain name, `WF_OWNER` reports for any application window, and
/// `ap_sendmsg` stamps into `msg[1]` when the application itself is the
/// sender (`WM_REDRAW`, `gemwmlib.c:904`).
pub(super) const AP_CHAIN_PID: i16 = 0;

/// The Screen Manager's pid — `gl_mowner = ctl_pd = iprocess("SCRENMGR",
/// ctlmgr)` takes `curpid` 1 at AES init (`aes/geminit.c:888`, `getpd`,
/// `gempd.c:60-70`). In NewTOS a **stamp**, not a process: the value the
/// control-manager posters write into `msg[1]` (`ap_sendmsg` under `rlr ==
/// ctl_pd`, `gemctrl.c:81-368`), the name `appl_find("SCRENMGR")` answers,
/// and the one `appl_write` target that is not an application.
pub(super) const SCRENMGR_PID: i16 = 1;

/// The pid a program sees for identity `idx` — `ap_id`, `appl_find`'s
/// answer, `WF_OWNER`, `msg[1]` of a message the application itself sends.
/// Every identity NewTOS has — the sixteen GEMDOS slots and the
/// kernel-context desktop — is a member of the application chain, so the
/// value is [`AP_CHAIN_PID`] for all of them (spec § "AES process identity
/// and pid", contract (b)); the identity is not lost, it is simply not what
/// EmuTOS reports. Desk accessories (pid 2+, `D.g_acc[]`) do not exist here;
/// when they do, they are the first identities this map does not send to 0.
pub(super) fn reported_pid(idx: usize) -> i16 {
    debug_assert!(idx < pd::NUM_PDS, "reported_pid of a non-identity {idx}");
    AP_CHAIN_PID
}

/// The identity a program addresses with the reported value `pid` — the
/// inverse of [`reported_pid`] for `appl_write`/`appl_read` targets and the
/// test hooks. `None` when no client reports that value.
///
/// Every chain member reports 0, and at most one of them is running — a
/// `Pexec` parent is blocked while its child runs, the desktop while a
/// launched program runs — so 0 resolves to the **currently-scheduled
/// client**, the caller ([`current_identity`]). That is `fpdnm(NULL, 0)`
/// always finding `D.g_int[0]` (`gemsuper.c:102-105`, `gempd.c:38-57`): the
/// queue the caller itself reads. 1 is the Screen Manager ([`SCRENMGR_PID`])
/// and names no NewTOS client; 2+ would be accessories, of which there are
/// none (`totpds = num_accs + 2 = 2`, `geminit.c:819`).
pub(super) fn identity_of(pid: i16) -> Option<usize> {
    (pid == AP_CHAIN_PID).then(current_identity)
}

/// The reported pid of the current caller ([`reported_pid`] of
/// [`current_identity`]) — what `appl_init` returns and what a poster that
/// runs *as the application* stamps into `msg[1]` (`ap_sendmsg`'s
/// `rlr->p_pid`, `gemwmlib.c:855`, for `w_redraw`'s `WM_REDRAW` inside the
/// application's own `wind_*` trap). The control-manager posters do not use
/// it: they stamp [`SCRENMGR_PID`].
pub(super) fn current_pid() -> i16 {
    reported_pid(current_identity())
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

/// Opcode 10 — `appl_init`: open the caller's process descriptor, fill
/// `AESGLOBAL`, return the application id (`ap_id`). Never −1: EmuTOS's
/// `ap_init` (`aes/gemaplib.c:54-63`) has no failure path, and neither has
/// [`pd::open`] — the descriptor exists per identity from boot (A62).
///
/// Fills the caller's `AESGLOBAL` **1:1 with EmuTOS's `APPL_INIT` arm**
/// (`gemsuper.c:92-98`), field by field, at the caller's pointer width:
///
/// - `ap_version` (`:92`) = [`AES_VERSION`] (`0x0340`);
/// - `ap_count` (`:93`) = 1;
/// - `ap_id` (`:94`) = the caller's reported pid ([`current_pid`]);
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
    let idx = current_identity();
    let pid = reported_pid(idx);
    // SAFETY: single trap path.
    unsafe { pd::open(idx) };

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
    // The caller's AESGLOBAL, checked writable with the program's rights and
    // reached through its kernel alias (ARCH03 3a, `Aespb::global_kptr`); a
    // `global` the program may not write ends the call (and the program).
    let Some(global) = pb.global_kptr() else {
        return;
    };
    // SAFETY: `global` is the checked kernel address of the caller's 30-byte
    // AESGLOBAL (the GEM caller's contract; the AESPB was built for that ABI).
    unsafe { vals.write(global) };
    pb.set_ret(pid);
}

/// Opcode 19 — `appl_exit`: deregister the application. Returns 1.
///
/// "Deregister" is EmuTOS's `ap_exit` (`aes/gemaplib.c:251-261`) and is
/// narrower than it sounds: the pending message queue is drained and `AP_OPEN`
/// is cleared, but the process descriptor itself stays — see [`pd::close`].
/// The application's loaded resources stay too: `ap_exit` does not touch
/// them, they are freed by `rsrc_free` or at the process's end
/// (`resource::on_process_exit`, A39 R6). XaAES frees a client's whole list
/// here (`xa_appl.c:745-750`); not adopted.
/// The `APPL_EXIT` arm's `ret` is the enclosing `TRUE` (`gemsuper.c:85,120-123`).
pub fn appl_exit(pb: &Aespb) -> Option<crate::bdos::process::sched::Switch> {
    // `mn_cleanup();` (`aes/gemaplib.c:253`) — AC_CLOSE to every registered
    // desk accessory, so one with state tied to *this* application can drop it
    // before the application's memory goes. Posted **once** per `appl_exit`:
    // the wait below can abort and re-execute this whole call (the S2
    // restartable block), and a second round of messages would be observable
    // as duplicate `AC_CLOSE`s. The round flag is NewTOS mechanism — EmuTOS
    // needs none because its `ap_exit` never leaves and re-enters.
    if !ACCLOSE_ROUND.swap(true, Ordering::AcqRel) {
        super::menu::mn_cleanup();
    }
    // `wait_for_accs(AP_ACCLOSE);` (`:255`) — "block until all DAs have seen
    // AC_CLOSE". Returns a `Switch` when an accessory is runnable: this call
    // aborts, the accessory runs, and the re-executed `svc` re-enters here.
    if let Some(sw) = super::acc::wait_for_accs(pd::AP_ACCLOSE) {
        return Some(sw);
    }
    ACCLOSE_ROUND.store(false, Ordering::Release);
    // SAFETY: single trap path.
    unsafe { pd::close(current_identity()) };
    pb.set_ret(1);
    None
}

/// `true` between [`appl_exit`] posting its round of `AC_CLOSE` messages and
/// the matching `wait_for_accs(AP_ACCLOSE)` completing — see [`appl_exit`].
static ACCLOSE_ROUND: AtomicBool =
    AtomicBool::new(false);

/// Opcode 13 — `appl_find`: find an application by its 8-char name in
/// `addrin[0]`. Returns its reported pid ([`reported_pid`] of the identity
/// [`pd::find_by_name`] locates — 0 for any chain name), or −1 if not found
/// (or name is NULL). `"SCRENMGR"` is the one name outside the chain that
/// EmuTOS's `fpdnm` walk finds: the Screen Manager's descriptor, named by
/// `iprocess("SCRENMGR", ctlmgr)` → `pstart` → `p_nameit`
/// (`aes/geminit.c:888`, `gempd.c:117-121`), pid [`SCRENMGR_PID`].
pub fn appl_find(pb: &Aespb) {
    let name_ptr = pb.addrin(0);
    if name_ptr == 0 {
        pb.set_ret(-1);
        return;
    }
    let mut name = [b' '; 8];
    // addrin[0] is a caller-supplied pointer to ≥ 8 name bytes, read in the
    // caller's space (ARCH03 3a); one the program cannot read ends the call
    // and the program.
    if pb.space().copy_from(&mut name, name_ptr).is_err() {
        return;
    }
    // SAFETY: single trap path.
    let pid = match unsafe { pd::find_by_name(&name) } {
        Some(idx) => reported_pid(idx),
        None if &name == b"SCRENMGR" => SCRENMGR_PID,
        None => -1,
    };
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
///
/// **Full queue: 0, not a block — a sanctioned deviation.** EmuTOS parks the
/// sender (`aqueue`, `gemqueue.c:96-97,120-124`; `mwait`, `gemasync.c:117-127`)
/// until a read of the target's *own* queue drains it; NewTOS has no second
/// independently-scheduled AES client to perform that read and no "queue
/// gained room" wake key, so blocking here would hang the one execution
/// context — `docs/COMPATIBILITY.md` "`appl_write` on a full
/// application-message queue returns 0" (A75, 2026-09-20).
pub fn appl_write(pb: &Aespb) {
    let len_bytes = pb.intin(1).max(0) as usize;
    let buf = pb.addrin(0);
    if buf == 0 {
        pb.set_ret(0);
        return;
    }
    // The target arrives as a reported pid. 0 is the caller (`fpdnm(NULL, 0)`
    // = `D.g_int[0]`, the chain's one queue). 1 is the Screen Manager: in
    // EmuTOS the write lands in `ctl_pd`'s queue and succeeds, and `ctlmgr`
    // reads it on its next turn (`ev_multi(… | MU_MESAG)`, `gemctrl.c:448-481`)
    // — no application ever sees it. NewTOS has no queue there; the write
    // succeeds and the message goes nowhere. (What `ctlmgr` does with a
    // `WM_REDRAW` it reads — `hctl_msg` → `w_redraw_desktop`, `:380-384` — has
    // no spec entry yet and is not reproduced here.) Any other value names
    // no descriptor within `totpds` (no accessories): 0.
    if pb.intin(0) == SCRENMGR_PID {
        pb.set_ret(1);
        return;
    }
    let Some(target) = identity_of(pb.intin(0)) else {
        pb.set_ret(0);
        return;
    };
    // AP_LENGTH is a byte count (EmuTOS `qpb_cnt`); GEM messages are WORD-based,
    // so read whole WORDs, capped at the per-message FIFO limit.
    let words = (len_bytes / 2).min(pd::MAX_MSG_WORDS);
    let mut msg = [0i16; pd::MAX_MSG_WORDS];
    // addrin[0] points at ≥ len_bytes of message data, read in the caller's
    // space (ARCH03 3a); a buffer the program cannot read ends the call.
    for (i, slot) in msg.iter_mut().take(words).enumerate() {
        match pb.space().get_u16(buf + 2 * i as u64) {
            Ok(w) => *slot = w as i16,
            Err(_) => return,
        }
    }
    // Enqueue into the target's FIFO via the centralized poster: it appends the
    // message (with the standard redraw/arrow coalescing) AND wakes a peer
    // blocked in `evnt_mesag`/`evnt_multi`/`appl_read` on the target's FIFO
    // (WAITING → READY, design §5.2) — no longer bypassing `pd::post_msg`'s
    // wake. Returns 0 if the queue is full: the sanctioned deviation named
    // above (`docs/COMPATIBILITY.md`, A75; EmuTOS blocks the sender).
    // SAFETY: single trap path.
    pb.set_ret(unsafe { pd::post_words(target, &msg, words) } as i16);
}

/// Opcode 11 — `appl_read`: read a message from the calling application's
/// pipe. `intin[0]` = own id, `intin[1]` = byte length, `addrin[0]` =
/// destination buffer. Returns 1 with the message; **blocks while the pipe
/// is empty** (`application.md` "Blocking behaviour (A75)").
///
/// EmuTOS `ap_rdwr(MU_MESAG, …)` (`aes/gemaplib.c:69-89`) hands the read to
/// `aqueue` (`aes/gemqueue.c:85-125`), whose read readiness is `p_qindex > 0`
/// (`:98-99`) — the queue is **non-empty**; it is not "`length` bytes are
/// buffered" (`doq` then copies `qpb_cnt` bytes unguarded, `:76-81`). A ready
/// read is serviced at once (`doq` + `azombie(e, 1)`, `:104-107`) and returns
/// 1; an empty queue parks the caller on `p_qdq` (`evinsert`, `:120-124`) in
/// `dsptch()` (`mwait`, `aes/gemasync.c:117-127`) until a later write into
/// this same queue services it (`:108-118`). There is no `0` return for
/// "empty".
///
/// NewTOS blocks the way its sibling `evnt_mesag` does on the very same pipe
/// (`event.rs`): pop the front message if one is present, else
/// `sched::sleep(wait_mesg(idx))` — the key every poster wakes
/// (`pd::post_msg`/`post_words` → `wake_mesg_waiter`) — and re-check on
/// resume. A sleep that finds a runnable peer yields the `Switch` up through
/// the dispatcher (S2 restartable block): nothing has been committed, and the
/// re-executed trap starts over. The pop runs only on the delivering pass, so
/// a restart never double-pops.
pub fn appl_read(pb: &Aespb) -> Option<sched::Switch> {
    let len_bytes = pb.intin(1).max(0) as usize;
    let dest = pb.addrin(0);
    // `intin[0]` is the caller's own reported id, 0; resolve it to an
    // identity — the caller's. 1 (the Screen Manager's queue, which an
    // application reading it would block on in EmuTOS) and 2+ (no
    // accessories) name no queue an application can read here: 0.
    let Some(idx) = identity_of(pb.intin(0)) else {
        pb.set_ret(0);
        return None;
    };
    if dest == 0 {
        pb.set_ret(0);
        return None;
    }
    // AP_LENGTH is a byte count; read whole WORDs, capped at the FIFO's
    // per-message limit (EmuTOS `ap_rdwr(MU_MESAG,…,length,…)`).
    let words = (len_bytes / 2).min(pd::MAX_MSG_WORDS);
    let mut buf = [0i16; pd::MAX_MSG_WORDS];
    loop {
        // SAFETY: single trap path; `by_index` hands out the live slot.
        let Some(src) = (unsafe { pd::by_index(idx) }) else {
            // No descriptor — only an identity outside the table, which
            // `identity_of` never produces: nothing to wait on.
            pb.set_ret(0);
            return None;
        };
        // The destination, checked writable with the caller's rights once a
        // message is there to copy — where EmuTOS's `doq` writes it — and
        // reached through its kernel alias (ARCH03 3a). One the program may
        // not write ends the call (and the program) with the message still
        // queued.
        if src.queue.pending() > 0 && pb.ubuf(dest, 2 * words as u64, true).is_none() {
            return None;
        }
        // Queue non-empty (`p_qindex > 0`, `gemqueue.c:99`): pop the FRONT of
        // the FIFO into the requested-length window (`doq` dequeue,
        // `gemqueue.c:76-81`) and complete with 1 (`azombie(e, 1)`, `:107`).
        if let Some(n) = src.queue.pop_into(&mut buf[..words]) {
            // `dest` was checked writable for `words` WORDs just above; `n <=
            // words`. Written in the caller's space.
            for (i, &w) in buf.iter().take(n).enumerate() {
                if pb.space().put_u16(dest + 2 * i as u64, w as u16).is_err() {
                    return None;
                }
            }
            src.flags |= pd::AP_MESAG;
            pb.set_ret(1);
            return None;
        }
        // Empty: block on this identity's message wake key until a write lands
        // (`evinsert(e, &p_qdq)` + `mwait`, `gemqueue.c:120-124`,
        // `gemasync.c:117-127`); the poster wakes `WAIT_MESG(idx)`.
        if let sched::SleepOutcome::Yield(sw) = sched::sleep(sched::wait_mesg(idx)) {
            return Some(sw);
        }
    }
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
