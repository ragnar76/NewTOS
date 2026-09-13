//! AES — Application Environment Services (GEM trap, selector 200).
//!
//! Spec: [`docs/api/aes/_dispatcher.md`] and the per-manager files under
//! `docs/api/aes/`. The AES shares the GEM trap (`svc #2`) with the VDI;
//! the kernel's exception handler routes selector 200 (`x0`) here with
//! the AES parameter block (`AESPB`) pointer in `x1`.
//!
//! Like the VDI, the AES takes a **parameter block of array pointers**
//! (six of them) rather than a register ABI. [`dispatch`] is EmuTOS's `xif`
//! (`aes/gemsuper.c:428-448`): it parses the block, runs the [`crysbind`]
//! opcode switch against a **local** `int_out[O_SIZE]` — whose slot 0 is
//! `RET_CODE` — and then copies exactly `control[OUT_LEN]` words of it into
//! the caller's array. A caller that declares `OUT_LEN 0` gets nothing
//! written, not even the return code.

mod appl;
mod event;
pub mod form;
pub mod fsel;
mod graf;
pub mod init;
mod input;
pub mod menu;
pub mod object;
pub mod resource;
pub mod shell;
pub mod thread;
pub mod trace;
pub mod window;
mod pd;

use crate::bdos::process::Abi;

/// The AES parameter block: six pointers (four to WORD arrays, two to
/// LONG arrays). The block pointers — and the `addrin`/`addrout` LONG
/// elements — are the caller's pointer width: 8-byte for an LP64 caller,
/// 4-byte for an ILP32 one (ADR-020). `x1` points at the six consecutive
/// pointers; [`Aespb::from_raw`] is told the caller's [`Abi`] so it reads
/// them at the right stride. The WORD arrays (`control`/`global`/`intin`/
/// `intout`) are 16-bit either way, so those stay plain `*mut i16`.
pub(crate) struct Aespb {
    control: *mut i16,
    global: *mut i16,
    intin: *mut i16,
    /// The **dispatch-local** `int_out[O_SIZE]` every manager writes —
    /// EmuTOS `xif`'s own stack array (`gemsuper.c:432`). Nothing a manager
    /// stores here reaches the caller until [`Aespb::copy_intout_back`] runs.
    intout: *mut i16,
    /// The **caller's** `intout` array. Written by exactly one place,
    /// [`Aespb::copy_intout_back`], and only `control[OUT_LEN]` words of it.
    caller_intout: *mut i16,
    /// Base address of the caller's `addrin` LONG array. Element width
    /// tracks [`Self::abi`] (8 bytes LP64, 4 bytes ILP32), so this is
    /// kept as a raw base and indexed at read time, not a typed pointer.
    addrin: u64,
    /// Base address of the caller's `addrout` LONG array — written by the
    /// managers that return addresses (`rsrc_gaddr`, …).
    addrout: u64,
    /// Caller pointer width, threaded from the running process.
    abi: Abi,
}

// CONTRL indices with fixed meaning (`include/aesdefs.h`).
const C_OPCODE: usize = 0;
#[allow(dead_code)]
const C_IN_LEN: usize = 1;
const C_OUT_LEN: usize = 2;
#[allow(dead_code)]
const C_AIN_LEN: usize = 3;

/// `O_SIZE` — the size of the AES's `int_out` array, 7 WORDs
/// (`include/aesdefs.h:123`). `xif` builds its results in a local
/// `WORD int_out[O_SIZE]` (`gemsuper.c:432`); so do we.
const O_SIZE: usize = 7;

impl Aespb {
    /// Build from the raw `x1` pointer to the six-pointer block, reading
    /// the block pointers at the caller's pointer width (`abi`): 8-byte
    /// LP64 pointers, or 4-byte (zero-extended) ILP32 pointers.
    ///
    /// `local_intout` is the dispatch-local `int_out[O_SIZE]` the managers
    /// write into (EmuTOS `gemsuper.c:432`); the caller's own `intout`
    /// pointer is kept aside for [`Self::copy_intout_back`].
    ///
    /// # Safety
    /// `ptr` must point at six consecutive valid array pointers of the
    /// caller's width, each to an array large enough for the opcode's
    /// declared lengths. `local_intout` must be a writable `[i16; O_SIZE]`
    /// that outlives this `Aespb`.
    pub(crate) unsafe fn from_raw(ptr: u64, abi: Abi, local_intout: *mut i16) -> Self {
        // SAFETY: caller guarantees the six-pointer block is valid and of
        // `abi`'s width; `read_ptr` reads one such pointer per index.
        unsafe {
            Aespb {
                control: abi.read_ptr(ptr, 0) as *mut i16,
                global: abi.read_ptr(ptr, 1) as *mut i16,
                intin: abi.read_ptr(ptr, 2) as *mut i16,
                intout: local_intout,
                caller_intout: abi.read_ptr(ptr, 3) as *mut i16,
                addrin: abi.read_ptr(ptr, 4),
                addrout: abi.read_ptr(ptr, 5),
                abi,
            }
        }
    }

    /// `xif`'s copy-back rule (`aes/gemsuper.c:443-444`):
    ///
    /// ```c
    /// if (OUT_LEN)
    ///     memcpy(pcrys_blk->intout, int_out, OUT_LEN*sizeof(WORD));
    /// ```
    ///
    /// Exactly `control[OUT_LEN]` WORDs reach the caller — **including**
    /// `RET_CODE`, which is just the local `int_out[0]`
    /// (`include/aesdefs.h:140`). With `OUT_LEN == 0` nothing is written at
    /// all, not even the return code; slots past `OUT_LEN` keep whatever the
    /// caller had there.
    ///
    /// **Boundary, and it is ours, not EmuTOS's.** EmuTOS does not clamp
    /// `OUT_LEN` against `O_SIZE`, so an `OUT_LEN > 7` copies whatever
    /// follows its 7-WORD stack array — an indeterminate value even on
    /// EmuTOS, not a behaviour with a value to reproduce, and an
    /// out-of-bounds read here. The defined range (`OUT_LEN <= O_SIZE`) is
    /// reproduced exactly; beyond it we copy the 7 words that exist and stop.
    /// A negative `OUT_LEN` (EmuTOS: `OUT_LEN*sizeof(WORD)` widening to a
    /// huge `size_t`) copies nothing.
    ///
    /// # Safety
    /// The caller's `intout` must hold at least `min(control[OUT_LEN],
    /// O_SIZE)` WORDs — the caller's own declaration.
    unsafe fn copy_intout_back(&self) {
        let declared = self.control(C_OUT_LEN);
        let n = if declared <= 0 { 0 } else { (declared as usize).min(O_SIZE) };
        for i in 0..n {
            // SAFETY: `i < O_SIZE` bounds the local read; the write is inside
            // the `OUT_LEN` words the caller declared (this method's contract).
            unsafe { self.caller_intout.add(i).write(self.intout.add(i).read()) };
        }
    }

    #[inline]
    fn control(&self, i: usize) -> i16 {
        // SAFETY: callers index within the control array (≥ 5 WORDs).
        unsafe { self.control.add(i).read() }
    }
    #[inline]
    fn intin(&self, i: usize) -> i16 {
        // SAFETY: callers index within `control[IN_LEN]` input integers.
        unsafe { self.intin.add(i).read() }
    }
    /// Store into the dispatch-local `int_out[O_SIZE]` (EmuTOS
    /// `gemsuper.c:432`). A manager writes freely here; how much of it the
    /// caller ever sees is decided by [`Self::copy_intout_back`] alone.
    /// Indices past `O_SIZE` could never reach the caller (the copy-back is
    /// capped at `O_SIZE`), so they are dropped rather than running off the
    /// local array — `evnt_multi`'s seven words are the AES maximum.
    #[inline]
    fn set_intout(&self, i: usize, v: i16) {
        debug_assert!(i < O_SIZE, "AES int_out index past O_SIZE");
        if i < O_SIZE {
            // SAFETY: `intout` is the dispatch-local `[i16; O_SIZE]` and
            // `i < O_SIZE` was just checked.
            unsafe { self.intout.add(i).write(v) }
        }
    }
    /// The function return code (`RET_CODE = int_out[0]`).
    #[inline]
    fn set_ret(&self, v: i16) {
        self.set_intout(0, v);
    }
    #[inline]
    fn addrin(&self, i: usize) -> u64 {
        // SAFETY: callers index within `control[AIN_LEN]` input addresses;
        // element width tracks the caller's ABI (a `long` is one pointer
        // wide in each). An ILP32 pointer stored here (e.g. an OBJECT tree
        // in the low-VA TPA) zero-extends to a valid VA.
        unsafe { self.abi.read_ptr(self.addrin, i) }
    }
    /// The declared `addrin` length, `control[AIN_LEN]`. Used by
    /// `wind_set` `WF_NAME`/`WF_INFO`/`WF_NEWDESK` to tell the native
    /// convention (string pointer in `addr_in[0]`, `AIN_LEN >= 1`) from
    /// the classic-AES gemlib convention (pointer packed in the `int_in`
    /// WORDs, `AIN_LEN == 0`).
    #[inline]
    fn ain_len(&self) -> i16 {
        self.control(C_AIN_LEN)
    }
    /// Read a caller-width pointer packed into the `int_in` WORD array
    /// starting at WORD index `word`. This is the classic AES convention
    /// for the `wind_set` `WF_NAME`/`WF_INFO` string pointer: gemlib writes
    /// `*(char**)(aes_intin + word) = ptr`
    /// (`a_wind_set_str.c:25`), i.e. 4 pointer bytes at
    /// ILP32 (`int_in[word..word+2]`) or 8 at LP64 (`int_in[word..word+4]`),
    /// stored at `&int_in[word]`. That address is only WORD-aligned, so the
    /// load must be unaligned.
    #[inline]
    fn intin_ptr(&self, word: usize) -> u64 {
        // &int_in[word] — the pointer bytes start `word` WORDs (2 bytes
        // each) into the `int_in` array.
        let base = (self.intin as u64).wrapping_add((word as u64) * 2);
        // SAFETY: the opcode declares enough `int_in` WORDs to hold the
        // packed pointer (gemlib's num_intin = 6); the load width tracks
        // the caller ABI and tolerates the 2-byte alignment of the slot.
        unsafe {
            match self.abi {
                Abi::Lp64 => (base as *const u64).read_unaligned(),
                Abi::Ilp32 => (base as *const u32).read_unaligned() as u64,
            }
        }
    }
    #[inline]
    #[allow(dead_code)] // used by the address-returning managers (later sub-phases)
    fn set_addrout(&self, i: usize, v: u64) {
        // SAFETY: callers index within the addrout array; element width
        // tracks the caller's ABI (ILP32 stores the low 4 bytes).
        unsafe { self.abi.write_ptr(self.addrout, i, v) }
    }
    /// A mutable view of the caller's `global` array (the `AESGLOBAL`).
    #[inline]
    fn global_ptr(&self) -> *mut i16 {
        self.global
    }
}

/// AES dispatch entry — EmuTOS's `xif` (`aes/gemsuper.c:428-448`).
///
/// `aespb_ptr` is the `x1` value at the GEM trap (selector 200 in `x0`);
/// `abi` is the calling process's pointer width (the kernel passes
/// [`crate::bdos::process::current_abi`]).
///
/// Like `xif`, the opcode switch ([`crysbind`]) runs against a **local**
/// `int_out[O_SIZE]`; only `control[OUT_LEN]` words of it are copied into the
/// caller's array afterwards ([`Aespb::copy_intout_back`]). A caller that
/// declares `OUT_LEN 0` therefore gets nothing written — not even `RET_CODE`
/// — and slots past its declaration are never touched.
///
/// A blocking `evnt_*` wait that found a runnable peer returns the [`Switch`]
/// to yield to (S2 restartable block, design §4.3 + §8.2 sub-step 3); the
/// GEM-trap handler applies it. Every other opcode returns `None`.
///
/// That `Switch` is the one exit that does **not** copy back, and the reason
/// is EmuTOS's own control flow: a blocking wait there does not return from
/// `crysbind` at all, it calls `dsptch()` and suspends `xif`'s frame *inside*
/// the wait, so `gemsuper.c:443` has not run yet and only runs once the wait
/// really completes. Our S2 equivalent aborts the trap and re-executes it on
/// resume; no result exists yet, and the caller's `int_out` must be left as it
/// was until the completing pass produces one.
///
/// [`Switch`]: crate::bdos::process::sched::Switch
///
/// # Safety
/// `aespb_ptr` must be a valid AES parameter block of `abi`'s pointer
/// width (see [`Aespb`]).
pub unsafe fn dispatch(
    aespb_ptr: u64,
    abi: Abi,
) -> Option<crate::bdos::process::sched::Switch> {
    // `xif`'s `WORD int_out[O_SIZE]` (`gemsuper.c:432`). EmuTOS leaves it
    // uninitialised — indeterminate C auto storage, i.e. no value to
    // reproduce; zeroed here so an under-writing handler cannot leak stack
    // bytes into a caller that declared more words than the handler filled.
    let mut local_intout = [0i16; O_SIZE];
    // SAFETY: caller guarantees a valid parameter block of `abi`'s width;
    // `local_intout` lives until this function returns, outliving `pb`.
    let pb = unsafe { Aespb::from_raw(aespb_ptr, abi, local_intout.as_mut_ptr()) };

    // AES trap tracer (`trace`): compiled out entirely without the `aes-trace`
    // feature, and even with it the capture is a timestamp plus a fixed-size
    // struct store into a RAM ring — no I/O and no formatting on this path.
    let t0 = trace::call_begin();

    let sw = crysbind(&pb);

    if sw.is_none() {
        // The call completed — `xif`'s copy-back (`gemsuper.c:443-444`).
        // SAFETY: the caller's `intout` holds the `OUT_LEN` words it declared.
        unsafe { pb.copy_intout_back() };
    }
    trace::call_end(&pb, t0, sw.is_some());
    if sw.is_none() {
        // `super()` runs `xif` and then **falls through** to `dsptch()`
        // (`aes/gemsuper.c:452-464`) — every completed AES call ends there.
        // The `Switch` exit is the one case that has not completed: the trap is
        // aborted and re-executed on resume, and the yield itself *is* the
        // dispatch (it came out of `sched::sleep`, which already pumped).
        dsptch();
    }
    sw
}

/// EmuTOS `dsptch()` — the dispatch every AES call ends with.
///
/// `super()`, the AES trap's supervisor entry, runs the opcode switch and then
/// falls through to `dsptch()` for **every** AES call (`aes/gemsuper.c:452-464`);
/// `dsptch` saves the machine state and jumps to `disp` (`aes/gemasm.S:129-167`),
/// and `disp` ends with
///
/// ```text
/// do {
///     if (fpcnt)
///         forker();
///     schedule();
/// } while (fpcnt);
/// ```
///
/// (`aes/gemdisp.c:236-242`). `forker()` drains the fork ring — the FPDs the
/// mouse/keyboard interrupt posted (`aes/gemdisp.c:101-155`) — so `bchange` and
/// `mchange` run *there* and publish the new `button` / `mclick` / `xrat` /
/// `yrat` (`aes/geminput.c:333-343`, `:360-373`), and `schedule()` additionally
/// polls the keyboard through `chkkbd()` (`aes/gemdisp.c:158-177,180-206`).
///
/// **Why this is load-bearing and not bookkeeping.** It is the only reason the
/// input state an application reads is not frozen for the duration of a polling
/// loop. `gr_mkstate` just hands out the globals — `*pmstat = button`
/// (`aes/gemgrlib.c:458-464`) — so an app that does nothing but call
/// `graf_mkstate` in a loop still observes the button change, because each of
/// its traps ends in this dispatch and the drain updates `button` on the way
/// out. TeraDesk depends on exactly that: after a **double** click it spins in
/// `while (xe_button_state()) ;` (`sdk/apps/teradesk/window.c:4406-4412`), and
/// `xe_button_state` is a bare `graf_mkstate`
/// (`sdk/apps/teradesk/library/xdialog/xdevent.c:251-258`).
///
/// **Our equivalent of the drain.** This kernel has no interrupt-driven
/// pointer: the IKBD / USB-HID packets are collected by `Host::pump_input`,
/// which reaches [`crate::vdi::feed_relative`] — our `mouse_int` — and from
/// there [`user_but`] / [`user_mot`], i.e. the `b_click` + `mchange` work
/// EmuTOS does in `forker()`, plus the keyboard poll `schedule()` does in
/// `chkkbd()`. One call is `disp`'s one pass through that loop. It is the same
/// call the blocking `evnt_*` waits already make; what was missing is that the
/// *non-blocking* opcodes made it too, which is what `super()`'s fall-through
/// guarantees in EmuTOS.
fn dsptch() {
    // `schedule()`'s keyboard poll, `chkkbd()` — first thing in every pass of
    // the scheduler loop (`aes/gemdisp.c:180-184`), and it precedes the read
    // because that is where `gsx_char` puts it.
    chkkbd();
    if let Some(h) = crate::host::host() {
        h.pump_input();
    }
}

/// EmuTOS `chkkbd()` (`aes/gemdisp.c:157-177`) — the keyboard poll the
/// scheduler runs on every dispatcher pass.
///
/// EmuTOS reads the key with `gsx_char()`, and `gsx_char` opens with
///
/// ```text
/// intin[0] = 4;
/// intin[1] = 2;
/// gsx_ncode(SET_INPUT_MODE, 0, 2);    /* vsin_mode(4, 2): string, SAMPLE */
/// ```
///
/// (`aes/gemgsxif.c:459-463`) before issuing the `STRING_INPUT`, because a
/// polling AES must not block inside the VDI: REQUEST mode makes `v_string`
/// wait for a key (`vdi/vdi_input.c:88-97`), SAMPLE makes it drain what is
/// pending and return (`:98-105`).
///
/// **Why the mode write belongs here and not only in the VDI.** The four
/// device modes are file-scope VDI globals, not per-workstation fields
/// (`vdi/vdi_defs.h:258`), so the AES's `vsin_mode(4, 2)` is visible to every
/// application through `vqin_mode(4)` — and nothing ever writes it back:
/// `vsin_mode` is the only writer (`vdi_input.c:118-142`), the AES's one
/// other call site sets the *locator* (`gsx_setmousexy`,
/// `aes/gemgsxif.c:484-488`, reached from `appl_tplay` alone), and the only
/// reset is `vdimouse_init` at `v_opnwk` (`vdi/vdi_control.c:563`,
/// `vdi/vdi_mouse.c:575-578`). On any EmuTOS machine `vqin_mode(4)` therefore
/// reports **1** from the first dispatcher pass onward, while devices 1, 2
/// and 3 stay at their default 0. Measured on the reference TT (EmuTOS 1.4):
/// `tosdump/reference/EMUTOS14-VDI-r2.BIN` record 16 and
/// `EMUTOS14-VDI-all.BIN` record 40 both read `INTOUT[0] = 1` for
/// `vqin_mode(4)`; records 6, 38 and 39 of the latter read 0 for devices 1,
/// 2 and 3.
///
/// **What of `chkkbd` this is, and what it is not.** The mode write is
/// reproduced; the key *read* is not routed through `v_string`. EmuTOS'
/// `chkkbd` pulls the key out of the VDI and forks it into the mouse owner's
/// per-process key queue (`forkq(kchange, …)`, `:172-176`); NewTOS' AES has
/// no such queue and leaves the key in the IKBD ring until an `evnt_keybd` /
/// form / dialog consumer takes it — the same delivery difference
/// [`input`](self::input) already documents for the button side. EmuTOS'
/// two guards on the read (`gl_play`, and the owner's queue being full) have
/// no counterpart for that reason; neither ever *clears* the mode, so they
/// can at most delay the first write, and on a normal boot both are false at
/// the first pass.
fn chkkbd() {
    crate::vdi::set_input_mode(crate::vdi::DEV_STRING, crate::vdi::MODE_SAMPLE);
}

/// The opcode switch — EmuTOS's `crysbind` (`aes/gemsuper.c:77-421`).
/// Results go into the dispatch-local `int_out`; the caller's array is
/// untouched until [`dispatch`] runs the copy-back.
fn crysbind(pb: &Aespb) -> Option<crate::bdos::process::sched::Switch> {
    let opcode = pb.control(C_OPCODE);

    match opcode {
        10 => appl::appl_init(pb),
        19 => appl::appl_exit(pb),
        11 => appl::appl_read(pb),
        12 => appl::appl_write(pb),
        13 => appl::appl_find(pb),
        14 => appl::appl_tplay(pb),
        15 => appl::appl_trecord(pb),
        17 => appl::appl_yield(pb),
        // The six blocking waits may yield to a peer — propagate the `Switch`.
        20 => return event::evnt_keybd(pb),
        21 => return event::evnt_button(pb),
        22 => return event::evnt_mouse(pb),
        23 => return event::evnt_mesag(pb),
        24 => return event::evnt_timer(pb),
        25 => return event::evnt_multi(pb),
        26 => event::evnt_dclick(pb),
        70 => graf::graf_rubberbox(pb),
        71 => graf::graf_dragbox(pb),
        72 => graf::graf_movebox(pb),
        73 => graf::graf_growbox(pb),
        74 => graf::graf_shrinkbox(pb),
        75 => graf::graf_watchbox(pb),
        76 => graf::graf_slidebox(pb),
        77 => graf::graf_handle(pb),
        78 => graf::graf_mouse(pb),
        79 => graf::graf_mkstate(pb),
        30 => menu::menu_bar(pb),
        31 => menu::menu_icheck(pb),
        32 => menu::menu_ienable(pb),
        33 => menu::menu_tnormal(pb),
        34 => menu::menu_text(pb),
        35 => menu::menu_register(pb),
        36 => menu::menu_popup(pb),
        37 => menu::menu_attach(pb),
        38 => menu::menu_istart(pb),
        39 => menu::menu_settings(pb),
        40 => object::objc_add(pb),
        41 => object::objc_delete(pb),
        42 => object::objc_draw(pb),
        43 => object::objc_find(pb),
        44 => object::objc_offset(pb),
        45 => object::objc_order(pb),
        46 => object::objc_edit(pb),
        47 => object::objc_change(pb),
        48 => object::objc_sysvar(pb),
        50 => form::form_do(pb),
        51 => form::form_dial(pb),
        52 => form::form_alert(pb),
        53 => form::form_error(pb),
        54 => form::form_center(pb),
        55 => form::form_keybd(pb),
        56 => form::form_button(pb),
        80 => shell::scrp_read(pb),
        81 => shell::scrp_write(pb),
        82 => shell::scrp_clear(pb),
        90 => fsel::fsel_input(pb),
        91 => fsel::fsel_exinput(pb),
        100 => window::wind_create(pb),
        101 => window::wind_open(pb),
        102 => window::wind_close(pb),
        103 => window::wind_delete(pb),
        104 => window::wind_get(pb),
        105 => window::wind_set(pb),
        106 => window::wind_find(pb),
        107 => window::wind_update(pb),
        108 => window::wind_calc(pb),
        109 => window::wind_new(pb),
        110 => resource::rsrc_load(pb),
        111 => resource::rsrc_free(pb),
        112 => resource::rsrc_gaddr(pb),
        113 => resource::rsrc_saddr(pb),
        114 => resource::rsrc_obfix(pb),
        120 => shell::shel_read(pb),
        121 => shell::shel_write(pb),
        122 => shell::shel_get(pb),
        123 => shell::shel_put(pb),
        124 => shell::shel_find(pb),
        125 => shell::shel_envrn(pb),
        126 => shell::shel_rdef(pb),
        127 => shell::shel_wdef(pb),
        // Any opcode not matched above — out of range, negative, or a
        // not-compiled-in function — returns the AES unsupported-opcode
        // sentinel `-1` in `int_out[0]` (`RET_CODE`). This mirrors EmuTOS
        // `crysbind`'s `default:` arm (`gemsuper.c:406-417`), which sets
        // `unsupported = TRUE` and then `ret = -1` unconditionally. Opcode 0
        // is included: EmuTOS returns `-1` for it too, only suppressing the
        // on-screen `fm_show(ALNOFUNC)` alert ("since some PRGs make this
        // call"). The alert is reference-level (spec `_dispatcher.md` §2/§3a,
        // D-084); the `-1` return is the contract, so we set it here.
        _ => pb.set_ret(-1),
    }
    // Non-blocking opcodes (and the six waits' non-yield exits) reach here.
    None
}

/// `sh_ldapp`'s `AP_OPEN` cleanup (`aes/gemshlib.c:650-657`) for the program
/// that has just terminated — see `pd::close_if_open`. Called from the
/// program launcher at the point EmuTOS calls it: immediately after
/// `dos_exec(PE_LOADGO, …)` returns.
pub fn close_client_if_open(pid: i16) {
    // SAFETY: single-CPU launcher context; the AES pd table follows the
    // single-trap-path discipline and the launcher runs on that CPU with no
    // concurrent GEM trap in flight.
    unsafe { pd::close_if_open(pid) };
}

/// The AES id (pid) NewTOS gives the program in GEMDOS process-table slot
/// `slot` — `slot + 1`, the value that program reports at `appl_init`
/// (`appl::current_pid`).
pub fn pid_of_slot(slot: usize) -> i16 {
    slot as i16 + 1
}

/// Name a freshly-launched EL0 program's AES process descriptor from its launch
/// `path`, so a later `appl_find(name)` can locate it — the reimplementation of
/// EmuTOS naming a process at start (`pstart` → `p_nameit`, `aes/gempd.c:110-131`).
/// `slot` is the GEMDOS process-table slot the loader placed the program in; its
/// AES id (pid) is [`pid_of_slot`]. Called by the kernel launcher before the
/// program enters EL0, alongside [`resource::set_app_dir`].
pub fn register_process(slot: usize, path: &[u8]) {
    let pid = pid_of_slot(slot);
    // SAFETY: single-CPU launcher context; the AES pd table follows the
    // single-trap-path discipline and the launcher runs on that CPU with no
    // concurrent GEM trap in flight.
    unsafe { pd::register(pid, path) };
}

/// The AES's `user_but` vector — call it from the VDI mouse interrupt when the
/// button state changed. EmuTOS installs `far_bcha` here with
/// `gsx_setmb(far_bcha, far_mcha, &drwaddr)` (`aes/gemgsxif.c:271`, i.e.
/// `vex_butv`/`vex_motv`), and `far_bcha` calls `b_click`
/// (`aes/gemdosif.S:306-325`). `buttons` is the new mask in VDI convention
/// (bit 0 = left), the `d0` the VDI passes (`vdi/vdi_asm.S:270-284`).
pub fn user_but(buttons: u8) {
    input::b_click(buttons);
}

/// The AES's `user_mot` vector — call it from the VDI mouse interrupt for a
/// packet that carried movement, with the **clipped** new position
/// (`vdi/vdi_asm.S:355-357`). EmuTOS's `far_mcha` forks `mchange`
/// (`aes/gemdosif.S:327-347`).
pub fn user_mot(x: i16, y: i16) {
    input::mchange(x, y);
}
