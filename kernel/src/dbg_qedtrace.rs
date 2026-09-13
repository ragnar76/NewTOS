//! **TEMPORARY** kernel-side syscall tracer for an EL0 application (QED).
//!
//! This is a DEBUG tool — it is **not** part of the product and must **not**
//! be committed to product history. It exists solely to root-cause real-HW QED
//! behaviour without a UART: it logs every EL0 (user-app) `svc` call to a RAM
//! buffer and periodically flushes that buffer to `C:\QEDTRACE.LOG` so the
//! trace can be read off the SD card after power-off.
//!
//! ## How it hooks in
//!
//! [`crate::arch::aarch64::exceptions::rust_sync_handler`] calls
//! [`pre_dispatch`] just before, and [`post_dispatch`] just after, the per-class
//! TOS dispatch — but **only** when the trap came from EL0 (`SPSR.M[3:0] == 0`).
//! The desktop runs at EL1 and calls the AES/VDI directly (never through the
//! `svc` path), so hooking the EL0 handler naturally traces **only** the app
//! (QED). Both hooks are guarded by the compile-time [`QED_SVC_TRACE`] const, so
//! with it `false` the tracer vanishes entirely (`if false && …` folds away) and
//! there is zero effect on product behaviour.
//!
//! ## Buffer + flush
//!
//! A single bounded 96 KiB `static` buffer accumulates one compact line per
//! call (plus a few decoded detail lines for the key calls). It is guarded for
//! single-CPU access exactly like the AES statics (an [`UnsafeCell`] touched
//! only from the IRQ-off trap path), plus a re-entrancy flag. When it is nearly
//! full it writes a `TRUNCATED` marker once and stops — it never overflows, and
//! the **early** (startup) trace is what survives.
//!
//! The flush runs at each `evnt_multi` **entry** (the app's idle point — startup
//! is complete by the first one): it rewrites the whole file via the
//! kernel-internal FS API ([`fcreate`](tos_api::bdos::fs::file::fcreate) then
//! [`fwrite`](tos_api::bdos::fs::file::fwrite)), so the file always holds the
//! latest full trace and survives even if the following blocking wait never
//! returns (e.g. no input under QEMU). FS errors are swallowed silently — the
//! flush never panics or hangs the app.

use core::cell::UnsafeCell;
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use tos_api::bdos::process::{current_abi, Abi};
use tos_api::trap::{TrapArgs, TrapClass};

/// Master gate for the QED syscall tracer. Set to `false` to compile the tool
/// out completely (both hook sites fold away, no runtime effect). This is the
/// single obvious switch the task asks for.
pub const QED_SVC_TRACE: bool = false;

/// RAM trace buffer size (96 KiB) — generous for a full QED startup + a few
/// idle-loop iterations; excess is dropped after the truncation marker.
const BUF_LEN: usize = 96 * 1024;

/// Written once when the buffer is nearly full, then all further appends stop.
const TRUNC_MARK: &[u8] = b"\n*** QEDTRACE TRUNCATED ***\n";

/// The bounded trace buffer + write cursor + one-shot truncated flag.
struct TraceBuf {
    data: UnsafeCell<[u8; BUF_LEN]>,
    len: UnsafeCell<usize>,
    truncated: UnsafeCell<bool>,
}
// SAFETY: touched only from the single boot CPU on the IRQ-off `svc` trap path
// (and the re-entrancy flag below serialises the flush), exactly the discipline
// the AES window/process statics use.
unsafe impl Sync for TraceBuf {}

static BUF: TraceBuf = TraceBuf {
    data: UnsafeCell::new([0u8; BUF_LEN]),
    len: UnsafeCell::new(0),
    truncated: UnsafeCell::new(false),
};

/// A second, **never cleared** buffer holding only the window-geometry calls
/// (`wind_create`, `wind_open`, `wind_calc`, `wind_get(WF_WORKXYWH)`).
///
/// Why it has to exist: the question under investigation is whether the work
/// rectangle QED computed for itself with `wind_calc(WC_WORK, …)` is the same,
/// to the pixel, as the first rectangle our `WF_FIRSTXYWH` hands back — QED's
/// `free_for_draw` (`qed/src/window.c:267`) draws directly only when width and
/// height match exactly, and silently defers to a redraw that never comes when
/// they do not. Those calls happen when the window opens, long before anyone
/// types, so the per-cycle buffer has thrown them away by then. This one is
/// written once, kept, and emitted at the top of every flush.
const PRO_LEN: usize = 8 * 1024;
/// Same shape as [`TraceBuf`], its own (much smaller) array size.
struct ProBuf {
    data: UnsafeCell<[u8; PRO_LEN]>,
    len: UnsafeCell<usize>,
    truncated: UnsafeCell<bool>,
}
// SAFETY: identical discipline to `TraceBuf` — single boot CPU, trap path.
unsafe impl Sync for ProBuf {}
static PRO: ProBuf = ProBuf {
    data: UnsafeCell::new([0u8; PRO_LEN]),
    len: UnsafeCell::new(0),
    truncated: UnsafeCell::new(false),
};
/// While set, [`append`] writes into [`PRO`] instead of [`BUF`].
static PRO_MODE: AtomicBool = AtomicBool::new(false);

/// Re-entrancy guard. The flush calls the FS layer, which are plain kernel
/// functions (not `svc`), so they cannot re-enter this path — the guard is
/// defensive, and also makes a nested trace call a harmless no-op.
static IN_TRACE: AtomicBool = AtomicBool::new(false);

/// Set when an `evnt_multi` returned with MU_KEYBD, i.e. the cycle now being
/// recorded is the application's response to a keystroke.
///
/// Why a latch and not just "keep the last cycle": QED runs a timer (its caret
/// blinks), so it produces an event cycle every few hundred milliseconds
/// whether or not anyone touches the machine. Keeping "the last cycle"
/// therefore always keeps a caret blink and never the keystroke. With this
/// latch the file is written once — for the cycle that followed the key — and
/// then frozen, so the card can be pulled at leisure.
static KEY_SEEN: AtomicBool = AtomicBool::new(false);
/// Set once enough cycles after the keystroke have been written; from then on
/// the file is left exactly as it is.
static FROZEN: AtomicBool = AtomicBool::new(false);
/// How many event cycles to keep after the keystroke, the keystroke's own
/// cycle included.
///
/// One is not enough, and the first run proved it: in the keystroke cycle QED
/// only moved its caret and issued no text-drawing call at all, which leaves
/// open whether it paints the glyph on a LATER cycle (a deferred repaint of
/// the dirty line) or never. These cycles accumulate into one file instead of
/// overwriting each other, so the answer is visible either way.
const CYCLES_AFTER_KEY: u32 = 4;
static CYCLES_LEFT: AtomicU32 = AtomicU32::new(MAX_CYCLES);
/// Set the first time a `v_gtext` is recorded after the keystroke.
///
/// The keystroke cycle on its own turned out not to be enough: QED registers
/// the change (`tasten.c:388`) and calls `restore_edit`, yet the cycle contains
/// no text output and not even the `set_clip(TRUE, work)` that every one of its
/// redraw loops starts with. What is missing is the CONTRAST — a cycle in which
/// the text *does* reach the screen. So recording now runs from the keystroke
/// until the first `v_gtext`, and the file holds both: the cycle that should
/// have drawn and the one that finally did.
static TEXT_DRAWN: AtomicBool = AtomicBool::new(false);
/// Hard cap so a run in which no text is ever drawn still yields a file.
const MAX_CYCLES: u32 = 60;

/// Append raw bytes to the buffer, stopping (with a one-shot marker) before it
/// can overflow. Reserves room for [`TRUNC_MARK`] so the marker always fits.
fn append(bytes: &[u8]) {
    let to_pro = PRO_MODE.load(Ordering::Acquire);
    let (base, len_cell, trunc_cell, cap) = if to_pro {
        (PRO.data.get() as *mut u8, PRO.len.get(), PRO.truncated.get(), PRO_LEN)
    } else {
        (BUF.data.get() as *mut u8, BUF.len.get(), BUF.truncated.get(), BUF_LEN)
    };
    // SAFETY: single-CPU trap path, serialised by `IN_TRACE`; exclusive logical
    // access to the buffer cells, and `cap` is that buffer's own length.
    let (data, len, trunc) = unsafe {
        (
            core::slice::from_raw_parts_mut(base, cap),
            &mut *len_cell,
            &mut *trunc_cell,
        )
    };
    if *trunc {
        return;
    }
    if *len + bytes.len() + TRUNC_MARK.len() > cap {
        // No room for this chunk while keeping space for the marker: mark and
        // stop appending for the rest of the run.
        let end = *len + TRUNC_MARK.len();
        data[*len..end].copy_from_slice(TRUNC_MARK);
        *len = end;
        *trunc = true;
        return;
    }
    data[*len..*len + bytes.len()].copy_from_slice(bytes);
    *len += bytes.len();
}

/// A `core::fmt::Write` sink that appends formatted text to the trace buffer.
struct Line;
impl Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        append(s.as_bytes());
        Ok(())
    }
}

/// Format one trace line into the buffer (ignores the always-`Ok` result).
macro_rules! trace {
    ($($a:tt)*) => {{
        let _ = core::write!(&mut Line, $($a)*);
    }};
}

/// Called from the EL0 `svc` handler **before** dispatch. Flushes the buffer at
/// each `evnt_multi` entry so the startup trace reaches C: even if the ensuing
/// blocking wait never returns.
pub fn pre_dispatch(class: TrapClass, args: &TrapArgs) {
    if !QED_SVC_TRACE {
        return;
    }
    if IN_TRACE.swap(true, Ordering::AcqRel) {
        return;
    }
    if is_evnt_multi(class, args) && !FROZEN.load(Ordering::Acquire) {
        flush();
        if KEY_SEEN.load(Ordering::Acquire) {
            // Inside the window of interest: keep accumulating rather than
            // clearing, so the file holds the keystroke cycle AND the cycles
            // that follow it, in order.
            let left = CYCLES_LEFT.load(Ordering::Acquire);
            if TEXT_DRAWN.load(Ordering::Acquire) || left <= 1 {
                FROZEN.store(true, Ordering::Release);
                trace!("--- frozen (text drawn: {}) ---\n",
                       TEXT_DRAWN.load(Ordering::Acquire));
            } else {
                CYCLES_LEFT.store(left - 1, Ordering::Release);
                trace!("--- cycle boundary ---\n");
            }
            IN_TRACE.store(false, Ordering::Release);
            return;
        }
        // ...and start the next cycle empty. Without this the 96 KiB buffer
        // fills during QED's start-up, writes its TRUNCATED marker and stops,
        // so the file preserves the launch and never the interaction — the
        // opposite of what an "I typed a key and nothing was drawn" hunt
        // needs. `evnt_multi` entry is the app's idle point, so each written
        // file now holds exactly the calls QED made in response to the LAST
        // thing that happened. Type one character, then touch nothing, and the
        // file on the card is that keystroke's drawing and nothing else.
        // SAFETY: same single-CPU trap-path discipline as `append`/`flush`,
        // and the re-entrancy guard is held by the caller.
        unsafe {
            *BUF.len.get() = 0;
            *BUF.truncated.get() = false;
        }
    }
    IN_TRACE.store(false, Ordering::Release);
}

/// Called from the EL0 `svc` handler **after** dispatch. Appends the one-line
/// record (plus decoded detail for the key calls) for the call just completed.
pub fn post_dispatch(class: TrapClass, args: &TrapArgs, result: i64) {
    if !QED_SVC_TRACE {
        return;
    }
    if IN_TRACE.swap(true, Ordering::AcqRel) {
        return;
    }
    record(class, args, result);
    IN_TRACE.store(false, Ordering::Release);
}

/// Whether this GEM trap is an AES `evnt_multi` (selector 200, opcode 25),
/// peeked from the parameter block before dispatch.
fn is_evnt_multi(class: TrapClass, args: &TrapArgs) -> bool {
    if class != TrapClass::Gem || args.regs[0] != 200 {
        return false;
    }
    let block = args.regs[1];
    if block == 0 {
        return false;
    }
    let abi = current_abi();
    // control = block[0]; opcode = control[0] — the same reads the AES
    // dispatcher performs on this block.
    // SAFETY: QED's TTBR0 is active and the block is a mapped user VA that the
    // AES dispatcher itself reads identically.
    let control = unsafe { abi.read_ptr(block, 0) } as *const i16;
    if control.is_null() {
        return false;
    }
    // SAFETY: control points at the CONTRL WORD array (>= 1 WORD).
    let op = unsafe { control.read() };
    op == 25
}

/// Append the one-line record for a completed EL0 call.
fn record(class: TrapClass, args: &TrapArgs, result: i64) {
    match class {
        TrapClass::Gemdos => {
            let f = args.func_id();
            trace!("GEMDOS #1  fn=0x{:02x} {:<9} -> {}\n", f, gemdos_name(f), result);
            record_gemdos_path(f, args);
        }
        TrapClass::Bios => {
            let f = args.func_id();
            trace!("BIOS   #13 fn=0x{:02x}           -> 0x{:x}\n", f, result);
        }
        TrapClass::Xbios => {
            let f = args.func_id();
            trace!("XBIOS  #14 fn=0x{:02x}           -> 0x{:x}\n", f, result);
        }
        TrapClass::Gem => {
            let sel = args.regs[0];
            let block = args.regs[1];
            match sel {
                200 => {
                    // Window geometry goes to the kept prologue, everything
                    // else to the per-cycle buffer.
                    let op = aes_opcode(block);
                    let geometry = matches!(op, 100 | 101 | 108) || is_work_query(block, op);
                    if geometry {
                        PRO_MODE.store(true, Ordering::Release);
                    }
                    record_aes(block);
                    if geometry {
                        PRO_MODE.store(false, Ordering::Release);
                    }
                }
                115 => record_vdi(block),
                _ => trace!("GEM    #2  sel={} (unknown selector)\n", sel),
            }
        }
    }
}

/// For the GEMDOS file/dir calls that carry a path string, append a detail
/// line decoding the actual path QED passed. For every one of these calls the
/// string pointer is the **first** argument register (native `x0`, mapped to
/// `regs[0]`); the pointer is a full register value in either ABI (LP64 stores
/// the whole 8-byte pointer, ILP32 a zero-extended 4-byte pointer), so no
/// param-block widening is needed here. `Dgetpath`'s `regs[0]` is an *output*
/// buffer, so after dispatch it holds the resolved current path.
fn record_gemdos_path(f: u16, args: &TrapArgs) {
    let ptr = args.arg_u64(0);
    let label: &str = match f {
        0x43 => "    Fattrib  path=",
        0x39 => "    Dcreate  path=",
        0x3D => "    Fopen    path=",
        0x3C => "    Fcreate  path=",
        0x3B => "    Dsetpath path=",
        0x47 => "    Dgetpath buf=",
        0x4E => "    Fsfirst  spec=",
        _ => return,
    };
    trace!("{}", label);
    trace_path(ptr);
    trace!("\n");
}

/// Append the C-string at the user pointer `ptr`, bounded to 128 bytes and
/// sanitised to printable ASCII (any control/high byte → `.`), wrapped in
/// quotes. A null pointer prints `(null)`.
///
/// This reads the very pointer the just-dispatched GEMDOS call already walked
/// (via `super::cstr`) in this same trap context with QED's `TTBR0` active, so
/// it is exactly as safe as that call: it stops at the NUL or the 128-byte cap
/// and never scans unbounded memory.
fn trace_path(ptr: u64) {
    if ptr == 0 {
        append(b"(null)");
        return;
    }
    append(b"\"");
    let p = ptr as *const u8;
    let mut i = 0usize;
    while i < 128 {
        // SAFETY: `p` is the same user string pointer the dispatched GEMDOS
        // call read through `super::cstr`; QED's TTBR0 is active, the scan is
        // bounded to 128 bytes and stops at the first NUL.
        let b = unsafe { p.add(i).read() };
        if b == 0 {
            break;
        }
        let c = if (0x20..0x7f).contains(&b) { b } else { b'.' };
        append(&[c]);
        i += 1;
    }
    append(b"\"");
}

/// Read WORD `i` of an `i16` array, tolerating a null base.
#[inline]
fn wrd(base: *const i16, i: usize) -> i16 {
    if base.is_null() {
        return 0;
    }
    // SAFETY: callers index within the opcode's declared array length; the
    // array is a mapped param-block VA (read identically by the dispatcher).
    unsafe { base.add(i).read() }
}

/// Decode + append an AES call (selector 200) with its important values.
/// The AES opcode inside a parameter block, without recording anything.
fn aes_opcode(block: u64) -> i16 {
    if block == 0 {
        return -1;
    }
    let abi = current_abi();
    // SAFETY: same `control` pointer the AES dispatcher reads.
    let control = unsafe { abi.read_ptr(block, 0) } as *const i16;
    wrd(control, 0)
}

/// True for `wind_get(handle, WF_WORKXYWH)` — the other half of the pixel
/// comparison QED's `free_for_draw` makes.
fn is_work_query(block: u64, op: i16) -> bool {
    const WF_WORKXYWH: i16 = 4;
    if op != 104 || block == 0 {
        return false;
    }
    let abi = current_abi();
    // SAFETY: same `int_in` pointer the AES dispatcher reads.
    let intin = unsafe { abi.read_ptr(block, 2) } as *const i16;
    wrd(intin, 1) == WF_WORKXYWH
}

fn record_aes(block: u64) {
    if block == 0 {
        trace!("AES    op=?   (null param block)\n");
        return;
    }
    let abi = current_abi();
    // SAFETY: same six-pointer AESPB read the AES dispatcher performs.
    let control = unsafe { abi.read_ptr(block, 0) } as *const i16;
    let intin = unsafe { abi.read_ptr(block, 2) } as *const i16;
    let intout = unsafe { abi.read_ptr(block, 3) } as *const i16;
    let addrin_base = unsafe { abi.read_ptr(block, 4) };
    let addrout_base = unsafe { abi.read_ptr(block, 5) };

    let op = wrd(control, 0);
    let ret = wrd(intout, 0);
    trace!("AES    op={:<3} {:<12} -> {}\n", op, aes_name(op), ret);

    let ii = |i: usize| wrd(intin, i);
    let io = |i: usize| wrd(intout, i);

    match op {
        // graf_handle — returned VDI handle + system-font cell metrics.
        77 => trace!(
            "    graf_handle: handle={} wchar={} hchar={} wbox={} hbox={}\n",
            io(0), io(1), io(2), io(3), io(4)
        ),
        // wind_create — kind flags + max extent; RET_CODE is the handle.
        100 => trace!(
            "    wind_create: kind=0x{:04x} full=({},{},{},{}) -> handle {}\n",
            ii(0) as u16, ii(1), ii(2), ii(3), ii(4), io(0)
        ),
        // wind_open — the geometry the window is displayed at (the spurious
        // wide-short window shows up here).
        101 => trace!(
            "    wind_open:  handle={} xywh=({},{},{},{})\n",
            ii(0), ii(1), ii(2), ii(3), ii(4)
        ),
        // wind_get — decode the desk work area read (handle 0, WF_WORKXYWH=4).
        104 => {
            let handle = ii(0);
            let field = ii(1);
            if handle == 0 && field == 4 {
                trace!(
                    "    wind_get(desk, WF_WORKXYWH): xywh=({},{},{},{})\n",
                    io(1), io(2), io(3), io(4)
                );
            } else {
                trace!(
                    "    wind_get: handle={} field={} -> ({},{},{},{})\n",
                    handle, field, io(1), io(2), io(3), io(4)
                );
            }
        }
        // wind_calc — in rect + out rect (border<->work conversion).
        108 => trace!(
            "    wind_calc: wtype={} kind=0x{:04x} in=({},{},{},{}) out=({},{},{},{})\n",
            ii(0), ii(1) as u16, ii(2), ii(3), ii(4), ii(5), io(1), io(2), io(3), io(4)
        ),
        // evnt_button — resolved click count + mouse/keyboard state.
        21 => trace!(
            "    evnt_button: clicks={} mx={} my={} btn=0x{:x} ks=0x{:x}\n",
            io(0), io(1), io(2), io(3) as u16, io(4) as u16
        ),
        // evnt_multi — fired flags + mouse/button/key state; message on MU_MESAG.
        25 => {
            let fired = io(0) as u16;
            trace!(
                "    evnt_multi: fired=0x{:04x} mx={} my={} btn=0x{:x} ks=0x{:x} key=0x{:04x} clicks={}\n",
                fired, io(1), io(2), io(3) as u16, io(4) as u16, io(5) as u16, io(6)
            );
            const MU_KEYBD: u16 = 0x0001;
            if fired & MU_KEYBD != 0 {
                KEY_SEEN.store(true, Ordering::Release);
                trace!("    *** keystroke -- this cycle is the one being hunted ***\n");
            }
            const MU_MESAG: u16 = 0x0010;
            if fired & MU_MESAG != 0 {
                // Message buffer pointer is addr_in[0] (caller-width).
                // SAFETY: addrin_base points at the caller's addr_in array.
                let pbuff = if addrin_base == 0 {
                    0
                } else {
                    unsafe { abi.read_ptr(addrin_base, 0) }
                };
                if pbuff != 0 {
                    let mp = pbuff as *const i16;
                    let m = |i: usize| wrd(mp, i);
                    trace!(
                        "      msg=[{},{},{},{},{},{},{},{}]\n",
                        m(0), m(1), m(2), m(3), m(4), m(5), m(6), m(7)
                    );
                }
            }
        }
        // form_center — the AKTION-window bug lives here: log the ROOT object's
        // (ob_x,ob_y,ob_width,ob_height) the app passed in `addr_in[0]`, plus the
        // centred rect the call returns (`int_out[1..4]`). If the ROOT width is
        // already the ~1239 off-screen value here, it was baked in by load, not
        // computed by form_center (which only sets ob_x/ob_y).
        54 => {
            let tree = read_addr(abi, addrin_base, 0);
            let (x, y, w, h) = obj_dims(tree, 0);
            trace!(
                "    form_center tree=0x{:x} ROOT(x,y,w,h)=({},{},{},{}) -> centred=({},{},{},{})\n",
                tree, x, y, w, h, io(1), io(2), io(3), io(4)
            );
            // Bounded linear walk of the first objects, so the dialog's own
            // coordinates are visible: if children carry raw char-grid packed
            // values (e.g. 0x0401=1025) the inline resource was never
            // char->pixel converted (our rsrc_obfix is a no-op identity).
            for i in 0..8i16 {
                let (t, ox, oy, ow, oh) = obj_full(tree, i);
                trace!("      obj[{}] type={} (x,y,w,h)=({},{},{},{})\n", i, t, ox, oy, ow, oh);
            }
        }
        // rsrc_load — which `.RSC` file the app asked for (`addr_in[0]`); the
        // return in `int_out[0]` tells success (1) / failure (0).
        110 => {
            let fname = read_addr(abi, addrin_base, 0);
            trace!("    rsrc_load fname=");
            trace_path(fname);
            trace!("\n");
        }
        // rsrc_gaddr — for an R_TREE (type 0) request, log the returned tree
        // address (`addr_out[0]`) and its ROOT (ob_x,y,w,h) *straight out of
        // load/obfix*. This is the direct read of where a baked-in ob_width
        // (e.g. AKTION = 1239) would already be visible before any form_center.
        112 => {
            let rtype = ii(0);
            let index = ii(1);
            let addr = read_addr(abi, addrout_base, 0);
            if rtype == 0 && addr != 0 {
                let (x, y, w, h) = obj_dims(addr, 0);
                trace!(
                    "    rsrc_gaddr(R_TREE idx={}) tree=0x{:x} ROOT(x,y,w,h)=({},{},{},{})\n",
                    index, addr, x, y, w, h
                );
            }
        }
        // rsrc_obfix — our impl is an identity (native NRSC already stores pixel
        // coords), so this logs the obfix'd object's coords unchanged: the
        // input == output. Confirms whether obfix is (not) the 1239 source.
        114 => {
            let tree = read_addr(abi, addrin_base, 0);
            let obj = ii(1);
            let (x, y, w, h) = obj_dims(tree, obj);
            trace!(
                "    rsrc_obfix tree=0x{:x} obj={} (x,y,w,h)=({},{},{},{}) [identity]\n",
                tree, obj, x, y, w, h
            );
        }
        _ => {}
    }
}

/// Read pointer element `i` from an `addr_in`/`addr_out` array at `base`, at the
/// caller's ABI width. `base == 0` (unset array) reads as null.
#[inline]
fn read_addr(abi: Abi, base: u64, i: usize) -> u64 {
    if base == 0 {
        return 0;
    }
    // SAFETY: `base` is the caller's addr array VA (read identically by the AES
    // dispatcher); element `i` is in range for the ops decoded here.
    unsafe { abi.read_ptr(base, i) }
}

/// Read object `obj`'s (ob_x, ob_y, ob_width, ob_height) from the OBJECT tree at
/// `tree`, at the caller's ABI width — the exact 32-byte (LP64) / 24-byte
/// (ILP32) record layout the AES `ObjTree` accessor uses (coord field at offset
/// 24 / 16). Returns all-zero for a null tree or an implausible object index
/// (bounded to keep the read in a sane dialog-tree span, since our identity
/// `rsrc_obfix` never validated the index itself).
fn obj_dims(tree: u64, obj: i16) -> (i16, i16, i16, i16) {
    if tree == 0 || !(0..256).contains(&obj) {
        return (0, 0, 0, 0);
    }
    let (stride, coord_off): (u64, u64) = match current_abi() {
        Abi::Lp64 => (32, 24),
        Abi::Ilp32 => (24, 16),
    };
    let p = (tree + obj as u64 * stride + coord_off) as *const i16;
    // SAFETY: `tree` is a mapped OBJECT-array VA the AES dereferences identically;
    // `obj` is bounded < 256 so the four ROOT/obj coord WORDs stay in a sane span.
    unsafe {
        (
            p.read_unaligned(),
            p.add(1).read_unaligned(),
            p.add(2).read_unaligned(),
            p.add(3).read_unaligned(),
        )
    }
}

/// Like [`obj_dims`] but also returns the object's `ob_type` (the WORD at record
/// offset 6, ABI-independent). Used for the bounded AKTION-tree walk so each
/// object's graphic type is visible alongside its coordinates.
fn obj_full(tree: u64, obj: i16) -> (u16, i16, i16, i16, i16) {
    if tree == 0 || !(0..256).contains(&obj) {
        return (0, 0, 0, 0, 0);
    }
    let stride: u64 = match current_abi() {
        Abi::Lp64 => 32,
        Abi::Ilp32 => 24,
    };
    let base = tree + obj as u64 * stride;
    // SAFETY: as `obj_dims`; `ob_type` is the WORD at offset 6 in every record.
    let ty = unsafe { ((base + 6) as *const u16).read_unaligned() };
    let (x, y, w, h) = obj_dims(tree, obj);
    (ty, x, y, w, h)
}

/// Decode + append a VDI call (selector 115) with its important values.
fn record_vdi(block: u64) {
    if block == 0 {
        trace!("VDI    op=?   (null param block)\n");
        return;
    }
    let abi = current_abi();
    // SAFETY: same five-pointer VDIPB read the VDI dispatcher performs.
    let contrl = unsafe { abi.read_ptr(block, 0) } as *const i16;
    let intin = unsafe { abi.read_ptr(block, 1) } as *const i16;
    let ptsin = unsafe { abi.read_ptr(block, 2) } as *const i16;
    let intout = unsafe { abi.read_ptr(block, 3) } as *const i16;

    let op = wrd(contrl, 0);
    trace!("VDI    op={:<3} {:<12}\n", op, vdi_name(op));

    // The calls that lie between a keystroke and a glyph on screen. QED
    // advances its own caret but the cell stays blank, so the character is
    // produced and lost on the way out; these five say where.
    let ii = |i: usize| wrd(intin, i);
    let pi = |i: usize| wrd(ptsin, i);
    match op {
        // v_gtext — where, and what. contrl[1] = number of ptsin points,
        // contrl[3] = number of intin entries = the string's length; the
        // characters are one per intin word (VDI passes text expanded).
        8 => {
            TEXT_DRAWN.store(true, Ordering::Release);
            let n = wrd(contrl, 3).clamp(0, 8) as usize;
            trace!("    v_gtext at ({},{}) len={} chars=", pi(0), pi(1), wrd(contrl, 3));
            for i in 0..n {
                let c = ii(i);
                if (0x20..0x7f).contains(&c) {
                    trace!("{}", c as u8 as char);
                } else {
                    trace!("<{:02x}>", c as u16);
                }
            }
            trace!("\n");
        }
        // vs_clip — the rectangle a draw is confined to. A clip that is empty,
        // or one whose corners are read in the wrong order, swallows a
        // single-character draw while leaving a full-window redraw mostly
        // intact — which is exactly the shape of the reported symptom.
        129 => trace!(
            "    vs_clip flag={} rect=({},{})-({},{})\n",
            ii(0), pi(0), pi(1), pi(2), pi(3)
        ),
        // vr_recfl — the filled rectangle itself (the caret is one of these).
        114 => trace!("    vr_recfl rect=({},{})-({},{})\n", pi(0), pi(1), pi(2), pi(3)),
        // vswr_mode — 1 replace, 2 transparent, 3 XOR, 4 reverse-transparent.
        // A glyph drawn twice in XOR is invisible.
        32 => trace!("    vswr_mode -> {} (out {})\n", ii(0), wrd(intout, 0)),
        // vst_color — the text colour actually set (out = what we granted).
        22 => trace!("    vst_color -> {} (out {})\n", ii(0), wrd(intout, 0)),
        // The cursor semaphore around a draw: a stale save-under repaints over
        // freshly drawn pixels.
        122 => trace!("    v_show_c reset={}\n", ii(0)),
        123 => trace!("    v_hide_c\n"),
        _ => {}
    }

    // v_opnvwk — the screen workstation attributes QED reads (work_out[0..10]).
    if op == 100 {
        let io = |i: usize| wrd(intout, i);
        trace!(
            "    v_opnvwk work_out[0..10]=[{},{},{},{},{},{},{},{},{},{}]\n",
            io(0), io(1), io(2), io(3), io(4), io(5), io(6), io(7), io(8), io(9)
        );
    }
}

/// Rewrite `C:\QEDTRACE.LOG` with the whole buffer so far, via the kernel FS
/// API (not a nested `svc`). Errors are swallowed silently.
fn flush() {
    // SAFETY: single-CPU trap path; read-only snapshot of the buffer cells.
    let len = unsafe { *BUF.len.get() };
    let pro_len = unsafe { *PRO.len.get() };
    if len == 0 && pro_len == 0 {
        return;
    }
    let data_ptr = BUF.data.get() as *const u8 as u64;
    let pro_ptr = PRO.data.get() as *const u8 as u64;

    // Fcreate("C:\QEDTRACE.LOG", 0) — create/truncate for writing.
    const PATH: &[u8] = b"C:\\QEDTRACE.LOG\0";
    let cargs = TrapArgs {
        regs: [PATH.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0],
        func: 0x3C,
        svc_imm: 1,
    };
    let h = tos_api::bdos::fs::file::fcreate(&cargs);
    if h < 0 {
        return; // C: not writable / create failed — skip quietly
    }
    let handle = h as u64;

    // The kept window-geometry prologue first, then this cycle's calls.
    if pro_len > 0 {
        let pargs = TrapArgs {
            regs: [handle, pro_len as u64, pro_ptr, 0, 0, 0, 0, 0],
            func: 0x40,
            svc_imm: 1,
        };
        let _ = tos_api::bdos::fs::file::fwrite(&pargs);
    }
    // Fwrite(handle, len, data).
    let wargs = TrapArgs {
        regs: [handle, len as u64, data_ptr, 0, 0, 0, 0, 0],
        func: 0x40,
        svc_imm: 1,
    };
    let _ = tos_api::bdos::fs::file::fwrite(&wargs);

    // Fclose(handle).
    let clargs = TrapArgs {
        regs: [handle, 0, 0, 0, 0, 0, 0, 0],
        func: 0x3E,
        svc_imm: 1,
    };
    let _ = tos_api::bdos::fs::file::fclose(&clargs);
}

/// Short name for a GEMDOS function id (the common QED calls); `""` otherwise.
fn gemdos_name(f: u16) -> &'static str {
    match f {
        0x02 => "Cconout",
        0x09 => "Cconws",
        0x0E => "Dsetdrv",
        0x19 => "Dgetdrv",
        0x1A => "Fsetdta",
        0x20 => "Super",
        0x2A => "Tgetdate",
        0x2C => "Tgettime",
        0x2F => "Fgetdta",
        0x30 => "Sversion",
        0x36 => "Dfree",
        0x39 => "Dcreate",
        0x3B => "Dsetpath",
        0x3C => "Fcreate",
        0x3D => "Fopen",
        0x3E => "Fclose",
        0x3F => "Fread",
        0x40 => "Fwrite",
        0x41 => "Fdelete",
        0x42 => "Fseek",
        0x43 => "Fattrib",
        0x44 => "Mxalloc",
        0x47 => "Dgetpath",
        0x48 => "Malloc",
        0x49 => "Mfree",
        0x4A => "Mshrink",
        0x4B => "Pexec",
        0x4C => "Pterm",
        0x4E => "Fsfirst",
        0x4F => "Fsnext",
        0x56 => "Frename",
        0x57 => "Fdatime",
        _ => "",
    }
}

/// Short name for an AES opcode; `"?"` for the ones the tracer doesn't label.
fn aes_name(op: i16) -> &'static str {
    match op {
        10 => "appl_init",
        11 => "appl_read",
        12 => "appl_write",
        13 => "appl_find",
        17 => "appl_yield",
        19 => "appl_exit",
        20 => "evnt_keybd",
        21 => "evnt_button",
        22 => "evnt_mouse",
        23 => "evnt_mesag",
        24 => "evnt_timer",
        25 => "evnt_multi",
        26 => "evnt_dclick",
        70 => "graf_rubberbx",
        71 => "graf_dragbox",
        77 => "graf_handle",
        78 => "graf_mouse",
        79 => "graf_mkstate",
        30 => "menu_bar",
        40 => "objc_add",
        42 => "objc_draw",
        43 => "objc_find",
        47 => "objc_change",
        48 => "objc_sysvar",
        50 => "form_do",
        51 => "form_dial",
        52 => "form_alert",
        54 => "form_center",
        100 => "wind_create",
        101 => "wind_open",
        102 => "wind_close",
        103 => "wind_delete",
        104 => "wind_get",
        105 => "wind_set",
        106 => "wind_find",
        107 => "wind_update",
        108 => "wind_calc",
        109 => "wind_new",
        110 => "rsrc_load",
        111 => "rsrc_free",
        112 => "rsrc_gaddr",
        114 => "rsrc_obfix",
        120 => "shel_read",
        121 => "shel_write",
        125 => "shel_envrn",
        _ => "?",
    }
}

/// Short name for a VDI opcode; `"?"` for the ones the tracer doesn't label.
fn vdi_name(op: i16) -> &'static str {
    match op {
        1 => "v_opnwk",
        2 => "v_clswk",
        3 => "v_clrwk",
        6 => "v_pline",
        8 => "v_gtext",
        12 => "vst_height",
        21 => "vst_font",
        22 => "vst_color",
        32 => "vswr_mode",
        39 => "vst_alignment",
        100 => "v_opnvwk",
        101 => "v_clsvwk",
        105 => "v_get_pixel",
        106 => "vst_effects",
        107 => "vst_point",
        109 => "vro_cpyfm",
        111 => "vsc_form",
        114 => "vr_recfl",
        116 => "vqt_extent",
        117 => "vqt_width",
        122 => "v_show_c",
        123 => "v_hide_c",
        124 => "vq_mouse",
        129 => "vs_clip",
        _ => "?",
    }
}
