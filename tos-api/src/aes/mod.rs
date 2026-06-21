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
mod form;
pub mod object;
mod pd;

/// The AES parameter block: six pointers (four to WORD arrays, two to
/// LONG arrays). On native AArch64 each pointer is 64-bit; `x1` points at
/// the six consecutive pointers.
pub(crate) struct Aespb {
    control: *mut i16,
    global: *mut i16,
    intin: *mut i16,
    intout: *mut i16,
    addrin: *mut i64,
    /// `addrout` — written by the managers that return addresses
    /// (`rsrc_gaddr`, `graf_*`, …); unused until those land.
    #[allow(dead_code)]
    addrout: *mut i64,
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
    /// Build from the raw `x1` pointer to the six-pointer block.
    ///
    /// # Safety
    /// `ptr` must point at six consecutive valid array pointers, each to
    /// an array large enough for the opcode's declared lengths.
    pub(crate) unsafe fn from_raw(ptr: u64) -> Self {
        // SAFETY: caller guarantees the six-pointer block is valid.
        let p = ptr as *const u64;
        Aespb {
            control: unsafe { p.read() } as *mut i16,
            global: unsafe { p.add(1).read() } as *mut i16,
            intin: unsafe { p.add(2).read() } as *mut i16,
            intout: unsafe { p.add(3).read() } as *mut i16,
            addrin: unsafe { p.add(4).read() } as *mut i64,
            addrout: unsafe { p.add(5).read() } as *mut i64,
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
        // SAFETY: callers index within `control[AIN_LEN]` input addresses.
        unsafe { self.addrin.add(i).read() as u64 }
    }
    #[inline]
    #[allow(dead_code)] // used by the address-returning managers (later sub-phases)
    fn set_addrout(&self, i: usize, v: u64) {
        // SAFETY: callers index within the addrout array.
        unsafe { self.addrout.add(i).write(v as i64) }
    }
    /// A mutable view of the caller's `global` array (the `AESGLOBAL`).
    #[inline]
    fn global_ptr(&self) -> *mut i16 {
        self.global
    }
}

/// AES dispatch entry. `aespb_ptr` is the `x1` value at the GEM trap
/// (selector 200 in `x0`). Results go into the parameter-block arrays.
///
/// # Safety
/// `aespb_ptr` must be a valid AES parameter block (see [`Aespb`]).
pub unsafe fn dispatch(aespb_ptr: u64) {
    // SAFETY: caller guarantees a valid parameter block.
    let pb = unsafe { Aespb::from_raw(aespb_ptr) };
    let opcode = pb.control(C_OPCODE);

    match opcode {
        10 => appl::appl_init(&pb),
        19 => appl::appl_exit(&pb),
        11 => appl::appl_read(&pb),
        12 => appl::appl_write(&pb),
        13 => appl::appl_find(&pb),
        17 => appl::appl_yield(&pb),
        20 => event::evnt_keybd(&pb),
        21 => event::evnt_button(&pb),
        22 => event::evnt_mouse(&pb),
        23 => event::evnt_mesag(&pb),
        24 => event::evnt_timer(&pb),
        25 => event::evnt_multi(&pb),
        26 => event::evnt_dclick(&pb),
        42 => object::objc_draw(&pb),
        43 => object::objc_find(&pb),
        44 => object::objc_offset(&pb),
        46 => object::objc_edit(&pb),
        47 => object::objc_change(&pb),
        50 => form::form_do(&pb),
        52 => form::form_alert(&pb),
        53 => form::form_error(&pb),
        // Remaining managers (event/object/form/window/menu/resource/
        // scrap/shell) land in later sub-phases. The AES has no invalid-
        // opcode error path (the GEM environment is trusted), so an
        // unimplemented opcode is a no-op until its manager is added.
        _ => {}
    }
}
