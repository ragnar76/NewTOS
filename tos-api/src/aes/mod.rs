//! AES — Application Environment Services (GEM trap, selector 200).
//!
//! Spec: [`docs/api/aes/_dispatcher.md`] and the per-manager files under
//! `docs/api/aes/`. The AES shares the GEM trap (`svc #2`) with the VDI;
//! the kernel's exception handler routes selector 200 (`x0`) here with
//! the AES parameter block (`AESPB`) pointer in `x1`.
//!
//! Like the VDI, the AES takes a **parameter block of array pointers**
//! (six of them) rather than a register ABI. `dispatch` parses the block
//! and runs the `crysbind`-style opcode switch, leaving the result in
//! `int_out[0]` (`RET_CODE`).

mod appl;
mod event;
pub mod form;
pub mod fsel;
mod graf;
pub mod menu;
pub mod object;
pub mod resource;
pub mod shell;
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
    intout: *mut i16,
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
#[allow(dead_code)]
const C_OUT_LEN: usize = 2;
#[allow(dead_code)]
const C_AIN_LEN: usize = 3;

impl Aespb {
    /// Build from the raw `x1` pointer to the six-pointer block, reading
    /// the block pointers at the caller's pointer width (`abi`): 8-byte
    /// LP64 pointers, or 4-byte (zero-extended) ILP32 pointers.
    ///
    /// # Safety
    /// `ptr` must point at six consecutive valid array pointers of the
    /// caller's width, each to an array large enough for the opcode's
    /// declared lengths.
    pub(crate) unsafe fn from_raw(ptr: u64, abi: Abi) -> Self {
        // SAFETY: caller guarantees the six-pointer block is valid and of
        // `abi`'s width; `read_ptr` reads one such pointer per index.
        unsafe {
            Aespb {
                control: abi.read_ptr(ptr, 0) as *mut i16,
                global: abi.read_ptr(ptr, 1) as *mut i16,
                intin: abi.read_ptr(ptr, 2) as *mut i16,
                intout: abi.read_ptr(ptr, 3) as *mut i16,
                addrin: abi.read_ptr(ptr, 4),
                addrout: abi.read_ptr(ptr, 5),
                abi,
            }
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
    #[inline]
    fn set_intout(&self, i: usize, v: i16) {
        // SAFETY: callers index within `control[OUT_LEN]` output integers.
        unsafe { self.intout.add(i).write(v) }
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
    /// `wind_set`/`wind_get` `WF_NAME`/`WF_INFO` to tell the native
    /// convention (string pointer in `addr_in[0]`, `AIN_LEN >= 1`) from
    /// the classic-AES gemlib convention (pointer packed in the `int_in`
    /// WORDs, `AIN_LEN == 0`).
    #[inline]
    fn ain_len(&self) -> i16 {
        self.control(C_AIN_LEN)
    }
    /// Read a caller-width pointer packed into the `int_in` WORD array
    /// starting at WORD index `word`. This is the classic AES convention
    /// for the `wind_set`/`wind_get` `WF_NAME`/`WF_INFO` string/buffer
    /// pointer: gemlib writes `*(char**)(aes_intin + word) = ptr`
    /// (`a_wind_set_str.c:25`, `a_wind_get.c:344`), i.e. 4 pointer bytes at
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

/// AES dispatch entry. `aespb_ptr` is the `x1` value at the GEM trap
/// (selector 200 in `x0`); `abi` is the calling process's pointer width
/// (the kernel passes [`crate::bdos::process::current_abi`]). Results go
/// into the parameter-block arrays.
///
/// A blocking `evnt_*` wait that found a runnable peer returns the [`Switch`]
/// to yield to (S2 restartable block, design §4.3 + §8.2 sub-step 3); the
/// GEM-trap handler applies it. Every other opcode returns `None`.
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
    // SAFETY: caller guarantees a valid parameter block of `abi`'s width.
    let pb = unsafe { Aespb::from_raw(aespb_ptr, abi) };
    let opcode = pb.control(C_OPCODE);

    match opcode {
        10 => appl::appl_init(&pb),
        19 => appl::appl_exit(&pb),
        11 => appl::appl_read(&pb),
        12 => appl::appl_write(&pb),
        13 => appl::appl_find(&pb),
        14 => appl::appl_tplay(&pb),
        15 => appl::appl_trecord(&pb),
        17 => appl::appl_yield(&pb),
        // The six blocking waits may yield to a peer — propagate the `Switch`.
        20 => return event::evnt_keybd(&pb),
        21 => return event::evnt_button(&pb),
        22 => return event::evnt_mouse(&pb),
        23 => return event::evnt_mesag(&pb),
        24 => return event::evnt_timer(&pb),
        25 => return event::evnt_multi(&pb),
        26 => event::evnt_dclick(&pb),
        70 => graf::graf_rubberbox(&pb),
        71 => graf::graf_dragbox(&pb),
        72 => graf::graf_movebox(&pb),
        73 => graf::graf_growbox(&pb),
        74 => graf::graf_shrinkbox(&pb),
        75 => graf::graf_watchbox(&pb),
        76 => graf::graf_slidebox(&pb),
        77 => graf::graf_handle(&pb),
        78 => graf::graf_mouse(&pb),
        79 => graf::graf_mkstate(&pb),
        30 => menu::menu_bar(&pb),
        31 => menu::menu_icheck(&pb),
        32 => menu::menu_ienable(&pb),
        33 => menu::menu_tnormal(&pb),
        34 => menu::menu_text(&pb),
        35 => menu::menu_register(&pb),
        36 => menu::menu_popup(&pb),
        37 => menu::menu_attach(&pb),
        38 => menu::menu_istart(&pb),
        39 => menu::menu_settings(&pb),
        40 => object::objc_add(&pb),
        41 => object::objc_delete(&pb),
        42 => object::objc_draw(&pb),
        43 => object::objc_find(&pb),
        44 => object::objc_offset(&pb),
        45 => object::objc_order(&pb),
        46 => object::objc_edit(&pb),
        47 => object::objc_change(&pb),
        48 => object::objc_sysvar(&pb),
        50 => form::form_do(&pb),
        51 => form::form_dial(&pb),
        52 => form::form_alert(&pb),
        53 => form::form_error(&pb),
        54 => form::form_center(&pb),
        55 => form::form_keybd(&pb),
        56 => form::form_button(&pb),
        80 => shell::scrp_read(&pb),
        81 => shell::scrp_write(&pb),
        82 => shell::scrp_clear(&pb),
        90 => fsel::fsel_input(&pb),
        91 => fsel::fsel_exinput(&pb),
        100 => window::wind_create(&pb),
        101 => window::wind_open(&pb),
        102 => window::wind_close(&pb),
        103 => window::wind_delete(&pb),
        104 => window::wind_get(&pb),
        105 => window::wind_set(&pb),
        106 => window::wind_find(&pb),
        107 => window::wind_update(&pb),
        108 => window::wind_calc(&pb),
        109 => window::wind_new(&pb),
        110 => resource::rsrc_load(&pb),
        111 => resource::rsrc_free(&pb),
        112 => resource::rsrc_gaddr(&pb),
        113 => resource::rsrc_saddr(&pb),
        114 => resource::rsrc_obfix(&pb),
        120 => shell::shel_read(&pb),
        121 => shell::shel_write(&pb),
        122 => shell::shel_get(&pb),
        123 => shell::shel_put(&pb),
        124 => shell::shel_find(&pb),
        125 => shell::shel_envrn(&pb),
        126 => shell::shel_rdef(&pb),
        127 => shell::shel_wdef(&pb),
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

/// Name a freshly-launched EL0 program's AES process descriptor from its launch
/// `path`, so a later `appl_find(name)` can locate it — the reimplementation of
/// EmuTOS naming a process at start (`pstart` → `p_nameit`, `aes/gempd.c:110-131`).
/// `slot` is the GEMDOS process-table slot the loader placed the program in; its
/// AES id (pid) is `slot + 1`, the same value the app reports at `appl_init`
/// (`appl::current_pid`). Called by the kernel launcher before the program
/// enters EL0, alongside [`resource::set_app_dir`].
pub fn register_process(slot: usize, path: &[u8]) {
    let pid = slot as i16 + 1;
    // SAFETY: single-CPU launcher context; the AES pd table follows the
    // single-trap-path discipline and the launcher runs on that CPU with no
    // concurrent GEM trap in flight.
    unsafe { pd::register(pid, path) };
}
