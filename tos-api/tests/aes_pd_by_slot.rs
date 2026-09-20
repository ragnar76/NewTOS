//! A62 (stage 1) — **the AES process descriptor is keyed by the GEMDOS slot.**
//!
//! Spec: `docs/api/aes/application.md` § "AES process identity and pid",
//! NewTOS contract **(a) Identity**: a client's descriptor (queue, `AP_*`
//! flags, name, wait-key, resource list, window ownership) is keyed by its
//! **GEMDOS process slot**, plus one reserved index for the kernel-context
//! desktop — table size `MAX_PROCESSES + 1`. Because every slot has its own
//! descriptor by construction, **`appl_init` never returns `-1`** for "table
//! full" (EmuTOS `ap_init`, `aes/gemaplib.c:54-63`, three unconditional
//! statements).
//!
//! Stage 1 changes **no reported value**: `ap_id`, `appl_find`, `WF_OWNER` and
//! `msg[1]` must report the same numbers as before the rework (GEMDOS slot + 1;
//! 1 for the kernel-context desktop). Stage 2 (A56, EmuTOS's numbers) is a
//! separate change. The four tests here are, in order:
//!
//! * **(a)** sixteen nested processes, each calling `appl_init` — none gets
//!   `-1` (the measured defect: depth 9 → `-1`,
//!   `measurement-2026-09-20-a62/serial/nest-serial.log`);
//! * **(b)** nine programs one after another in the same slot — `appl_find`
//!   answers for the current occupant only (EmuTOS `p_nameit` overwrites the
//!   one `p_name` in place, `aes/gemshlib.c:633`);
//! * **(c)** a slot's release resets its descriptor — a program that dies
//!   with `AP_OPEN` set and a message unread leaves nothing for the slot's
//!   next tenant (the `handle_pexec` side finding of the A62 report, §8);
//! * **(d)** the reported values, pinned to their pre-rework numbers.
//!
//! The process table, `RUN`, the AES descriptor table and the window table are
//! process-wide statics; the tests serialise on one lock and release every
//! slot they claim.
//!
//! Harness shape (backend + host + parameter block) follows
//! `aes_app_return_descriptor.rs` (silent sink, no pixels); the process
//! fixture follows `aes_rsrc_per_process.rs`.

#![allow(clippy::too_many_arguments)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::process::{self, Abi, MAX_PROCESSES, PROCESS_TABLE};
use tos_api::host::Host;
use tos_api::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect, WriteMode};
use tos_types::BasePage;

const SCREEN_W: u16 = 640;
const SCREEN_H: u16 = 480;

// ── a raster backend that accepts every draw and keeps no pixels ───────────

struct SinkBackend;

impl RasterBackend for SinkBackend {
    fn width(&self) -> u16 {
        SCREEN_W
    }
    fn height(&self) -> u16 {
        SCREEN_H
    }
    fn num_colors(&self) -> u16 {
        256
    }
    fn planes(&self) -> u16 {
        16
    }
    fn set_color(&self, _index: u16, _r: u16, _g: u16, _b: u16) {}
    fn query_color(&self, _index: u16) -> (u16, u16, u16) {
        (0, 0, 0)
    }
    fn set_active_palette(&self, _palette: &[u16; 256]) {}
    fn pel_to_device16(&self, pel: Pel) -> u16 {
        pel as u16
    }
    fn device_to_rgb565(&self, argb: u32) -> u16 {
        argb as u16
    }
    fn rgb565_to_device(&self, px: u16) -> u32 {
        px as u32
    }
    fn read_pixel(&self, _x: i16, _y: i16) -> Pel {
        0
    }
    fn fill_rect(&self, _x: i16, _y: i16, _w: i16, _h: i16, _pel: Pel, _clip: ClipRect) {}
    fn abline(
        &self,
        _x0: i16,
        _y0: i16,
        _x1: i16,
        _y1: i16,
        _pel: Pel,
        _mode: WriteMode,
        _clip: ClipRect,
        mask: u16,
        _last: bool,
    ) -> u16 {
        mask
    }
    fn fill_pattern(&self, _x: i16, _y: i16, _w: i16, _h: i16, _f: PatternFill, _c: ClipRect) {}
    fn draw_glyph_sized(
        &self,
        _x: i16,
        _y: i16,
        _glyph: &[u8],
        _cell_w: i16,
        _cell_h: i16,
        _fg: Pel,
        _mode: WriteMode,
        _clip: ClipRect,
    ) {
    }
    fn blit_mono(&self, _d: Rect, _s: MonoSrc, _fg: Pel, _bg: Pel, _m: WriteMode, _c: ClipRect) {}
    fn copy_rect(&self, _src: Rect, _dx: i16, _dy: i16, _op: u8, _clip: ClipRect) {}
    fn get_device_pixel(&self, _x: i16, _y: i16) -> u32 {
        0
    }
    fn put_device_pixel(&self, _x: i16, _y: i16, _argb: u32) {}
    fn flush_rect(&self, _x0: i16, _y0: i16, _x1: i16, _y1: i16) {}
}

// ── the host ──────────────────────────────────────────────────────────────

struct SilentHost {
    raster: SinkBackend,
    clock: AtomicU64,
}

impl Host for SilentHost {
    fn console_write_byte(&self, _byte: u8) {}
    fn console_try_read_key(&self) -> Option<u16> {
        None
    }
    fn raster(&self) -> Option<&dyn RasterBackend> {
        Some(&self.raster)
    }
    fn monotonic_millis(&self) -> u64 {
        self.clock.load(Ordering::Relaxed)
    }
    fn pump_input(&self) {
        self.clock.fetch_add(1, Ordering::Relaxed);
    }
}

unsafe impl Sync for SilentHost {}
unsafe impl Send for SilentHost {}

static HOST_ONCE: Once = Once::new();

fn install_host() {
    HOST_ONCE.call_once(|| {
        let host: &'static SilentHost = Box::leak(Box::new(SilentHost {
            raster: SinkBackend,
            clock: AtomicU64::new(0),
        }));
        let href: &'static &'static dyn Host = Box::leak(Box::new(host as &'static dyn Host));
        tos_api::host::install_host(href);
    });
}

/// One lock over every process-wide static the tests touch.
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    install_host();
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ── AES parameter block ───────────────────────────────────────────────────

const ABI: Abi = Abi::Lp64;

struct Aespb {
    contrl: Box<[i16; 8]>,
    global: Box<[i16; 16]>,
    intin: Box<[i16; 16]>,
    intout: Box<[i16; 16]>,
    addrin: Box<[u64; 4]>,
    addrout: Box<[u64; 4]>,
    block: Box<[u64; 6]>,
}

impl Aespb {
    fn new() -> Aespb {
        let mut pb = Aespb {
            contrl: Box::new([0; 8]),
            global: Box::new([0; 16]),
            intin: Box::new([0; 16]),
            intout: Box::new([0; 16]),
            addrin: Box::new([0; 4]),
            addrout: Box::new([0; 4]),
            block: Box::new([0; 6]),
        };
        pb.block[0] = pb.contrl.as_ptr() as u64;
        pb.block[1] = pb.global.as_ptr() as u64;
        pb.block[2] = pb.intin.as_ptr() as u64;
        pb.block[3] = pb.intout.as_ptr() as u64;
        pb.block[4] = pb.addrin.as_ptr() as u64;
        pb.block[5] = pb.addrout.as_ptr() as u64;
        pb
    }

    fn call(&mut self, opcode: i16, in_len: i16, out_len: i16, ain_len: i16) -> i16 {
        self.contrl[0] = opcode;
        self.contrl[1] = in_len;
        self.contrl[2] = out_len;
        self.contrl[3] = ain_len;
        self.contrl[4] = 0;
        self.intout.fill(0);
        // SAFETY: all six arrays are live, LP64, and cover the declared lengths.
        assert!(unsafe { tos_api::aes::dispatch(self.block.as_ptr() as u64, ABI) }.is_none());
        self.intout[0]
    }
}

// AES opcodes used here.
const APPL_INIT: i16 = 10;
const APPL_READ: i16 = 11;
const APPL_WRITE: i16 = 12;
const APPL_FIND: i16 = 13;
const APPL_EXIT: i16 = 19;
const WIND_CREATE: i16 = 100;
const WIND_OPEN: i16 = 101;
const WIND_CLOSE: i16 = 102;
const WIND_DELETE: i16 = 103;
const WIND_GET: i16 = 104;
const WIND_SET: i16 = 105;

// Window kinds (`aesdefs.h`).
const NAME: i16 = 0x0001;
const CLOSER: i16 = 0x0002;
const MOVER: i16 = 0x0008;

/// `wind_get`/`wind_set` fields (`aesdefs.h`).
const WF_NEWDESK: i16 = 14;
const WF_OWNER: i16 = 20;

/// `WM_REDRAW` (`aesdefs.h:228`).
const WM_REDRAW: i16 = 20;

/// The desktop window handle.
const DESKWH: i16 = 0;

/// Somewhere on the 640×480 screen.
const WR: (i16, i16, i16, i16) = (40, 40, 300, 200);

// ── AES calls ─────────────────────────────────────────────────────────────

/// `appl_init()` — returns the caller's AES id (`ap_id`).
fn appl_init(pb: &mut Aespb) -> i16 {
    pb.intin.fill(0);
    pb.call(APPL_INIT, 0, 1, 0)
}

/// `appl_exit()`.
fn appl_exit(pb: &mut Aespb) -> i16 {
    pb.intin.fill(0);
    pb.call(APPL_EXIT, 0, 1, 0)
}

/// `appl_find(name)` — the 8-char name, space-padded by the caller.
fn appl_find(pb: &mut Aespb, name: &[u8; 8]) -> i16 {
    pb.intin.fill(0);
    pb.addrin[0] = name.as_ptr() as u64;
    let r = pb.call(APPL_FIND, 0, 1, 1);
    pb.addrin[0] = 0;
    r
}

/// `appl_read(pid, 16, buf)` — non-blocking pop of one message; 1 when one
/// was delivered, 0 when the FIFO was empty or the id has no descriptor.
fn appl_read(pb: &mut Aespb, pid: i16) -> (i16, [i16; 8]) {
    let mut buf = [0i16; 8];
    pb.intin.fill(0);
    pb.intin[0] = pid;
    pb.intin[1] = 16;
    pb.addrin[0] = buf.as_mut_ptr() as u64;
    let r = pb.call(APPL_READ, 2, 1, 1);
    pb.addrin[0] = 0;
    (r, buf)
}

/// `appl_write(pid, 16, msg)` — 1 when the message was queued, 0 when not.
fn appl_write(pb: &mut Aespb, pid: i16, msg: &[i16; 8]) -> i16 {
    pb.intin.fill(0);
    pb.intin[0] = pid;
    pb.intin[1] = 16;
    pb.addrin[0] = msg.as_ptr() as u64;
    let r = pb.call(APPL_WRITE, 2, 1, 1);
    pb.addrin[0] = 0;
    r
}

/// `wind_create(kind, full)` → handle.
fn wind_create(pb: &mut Aespb) -> i16 {
    pb.intin.fill(0);
    pb.intin[0] = NAME | CLOSER | MOVER;
    pb.intin[1] = 0;
    pb.intin[2] = 0;
    pb.intin[3] = SCREEN_W as i16;
    pb.intin[4] = SCREEN_H as i16;
    let h = pb.call(WIND_CREATE, 5, 1, 0);
    assert!(h > 0, "wind_create must give a handle, got {h}");
    h
}

/// `wind_open(handle, r)` — posts one `WM_REDRAW` to the window's owner
/// (`gemwmlib.c:1420-1436,1004-1048,887-907`).
fn wind_open(pb: &mut Aespb, h: i16, r: (i16, i16, i16, i16)) {
    pb.intin.fill(0);
    pb.intin[0] = h;
    pb.intin[1] = r.0;
    pb.intin[2] = r.1;
    pb.intin[3] = r.2;
    pb.intin[4] = r.3;
    assert_eq!(pb.call(WIND_OPEN, 5, 1, 0), 1, "wind_open failed");
}

fn wind_close(pb: &mut Aespb, h: i16) {
    pb.intin.fill(0);
    pb.intin[0] = h;
    pb.call(WIND_CLOSE, 1, 1, 0);
}

fn wind_delete(pb: &mut Aespb, h: i16) {
    pb.intin.fill(0);
    pb.intin[0] = h;
    pb.call(WIND_DELETE, 1, 1, 0);
}

/// `wind_get(handle, WF_OWNER)` → `int_out[1]`, the owner's reported id.
fn wf_owner(pb: &mut Aespb, h: i16) -> i16 {
    pb.intin.fill(0);
    pb.intin[0] = h;
    pb.intin[1] = WF_OWNER;
    assert_eq!(pb.call(WIND_GET, 2, 5, 0), 1, "wind_get(WF_OWNER) failed");
    pb.intout[1]
}

/// `wind_set(0, WF_NEWDESK, NULL, 0)` — re-owns the desktop window to the
/// caller (`gemwmlib.c:1684-1687`).
fn wind_set_newdesk(pb: &mut Aespb) {
    pb.intin.fill(0);
    pb.intin[0] = DESKWH;
    pb.intin[1] = WF_NEWDESK;
    pb.addrin[0] = 0;
    pb.call(WIND_SET, 6, 1, 1);
}

/// Read everything queued for `pid`.
fn drain(pb: &mut Aespb, pid: i16) -> usize {
    let mut n = 0;
    while appl_read(pb, pid).0 == 1 {
        n += 1;
        assert!(n < 64, "runaway queue");
    }
    n
}

// ── a process slot the host can "run" ─────────────────────────────────────

/// A claimed GEMDOS process slot with a basepage page of its own. Its
/// **reported** AES id is `slot + 1` (unchanged in stage 1). Released on drop
/// through the kernel's own slot release (`pterm::release_slot`), so a failing
/// test leaves neither the slot nor its descriptor behind for the next test.
struct Proc {
    slot: usize,
    page: *mut [u8; 4096],
}

impl Proc {
    fn new() -> Self {
        let slot = process::allocate_slot().expect("a free process slot");
        let page = Box::into_raw(Box::new([0u8; 4096]));
        PROCESS_TABLE[slot]
            .basepage_kva
            .store(page.cast::<BasePage>(), Ordering::Release);
        Proc { slot, page }
    }

    /// The value `appl_init` reports for this slot — today's rule.
    fn reported(&self) -> i16 {
        self.slot as i16 + 1
    }

    fn bp(&self) -> *mut BasePage {
        self.page.cast::<BasePage>()
    }

    /// Make this the running process: every AES call now carries its identity.
    fn run(&self) {
        process::set_current(self.bp());
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        if process::current() == self.bp() {
            process::set_current(std::ptr::null_mut());
        }
        // The slot may already have been released by a `Pterm` in the test;
        // releasing a free slot only re-zeroes it.
        // SAFETY: single-threaded under `LOCK`; no frame snapshot is consumed
        // on the host.
        unsafe { process::pterm::release_slot(self.slot) };
        // SAFETY: `page` came from `Box::into_raw` above and is not aliased
        // once the slot no longer names it.
        unsafe { drop(Box::from_raw(self.page)) };
    }
}

/// The kernel boot context: no running process (`RUN` null) — the built-in
/// desktop's situation.
fn desktop_context() {
    process::set_current(std::ptr::null_mut());
}

/// `Pterm` of the running process — the real `terminate` path followed by the
/// handler's `release_slot`, as `aes_rsrc_per_process.rs` drives it. On the
/// host the pool sweep finds no pool and the parent frame is a marker; what is
/// under test is what the release does to the slot's AES descriptor.
fn pterm(p: &Proc) {
    PROCESS_TABLE[p.slot].parent_frame.elr().store(1, Ordering::Release);
    // SAFETY: single-threaded under `LOCK`; `RUN` is `p`'s basepage.
    let resume = unsafe { process::pterm::terminate(0) }.expect("Pterm of the running process");
    assert_eq!(resume.parent_frame_slot, p.slot);
    // SAFETY: the frame snapshot is not needed on the host.
    unsafe { process::pterm::release_slot(p.slot) };
}

/// The launcher's pre-run step (`kernel/src/launch.rs`,
/// `register_process(child.slot, path)` — EmuTOS `p_nameit` **and**
/// `rlr->p_flags = 0`, `aes/gemshlib.c:633-636`).
fn launcher_register(slot: usize, path: &[u8]) {
    tos_api::aes::register_process(slot, path);
}

/// The launcher's post-run step (`kernel/src/launch.rs`,
/// `close_client_if_open(child.slot)` — EmuTOS `aes/gemshlib.c:650-657`),
/// addressed by the slot: the identity the descriptor is keyed by.
fn launcher_cleanup(slot: usize) {
    tos_api::aes::close_client_if_open(slot);
}

// ── (a) nesting: one descriptor per process, appl_init never -1 ───────────

/// The measured defect: `NESTTST.NAP` `Pexec`ing itself 16 deep got
/// `appl_init` = 1…8 for depths 1–8 and **−1 from depth 9** (GEMDOS slot 8),
/// because the old descriptor table had 8 entries against 16 process slots.
/// EmuTOS's `ap_init` cannot fail (`aes/gemaplib.c:54-63`); with one
/// descriptor per slot by construction, neither can NewTOS's.
///
/// Every depth's reported id stays what it was (slot + 1) — stage 1 changes
/// no value, only the capacity.
#[test]
fn every_nested_process_gets_a_descriptor_and_appl_init_never_returns_minus_one() {
    let _g = lock();
    let mut pb = Aespb::new();
    desktop_context();
    assert_eq!(appl_init(&mut pb), 1, "the desktop's id is unchanged");

    let chain: Vec<Proc> = (0..MAX_PROCESSES).map(|_| Proc::new()).collect();
    for (depth, p) in chain.iter().enumerate() {
        p.run();
        let id = appl_init(&mut pb);
        assert_ne!(
            id, -1,
            "depth {}: appl_init returned -1 — EmuTOS's ap_init cannot fail \
             (aes/gemaplib.c:54-63); a descriptor per GEMDOS slot leaves no \
             'table full' to report",
            depth + 1
        );
        assert_eq!(id, p.reported(), "depth {}: the reported id is slot + 1", depth + 1);
        // …and the process is not deaf: it can address itself.
        let probe = [0x4711, id, 0, depth as i16, 0, 0, 0, 0];
        assert_eq!(appl_write(&mut pb, id, &probe), 1, "depth {}: appl_write(self)", depth + 1);
        let (got, body) = appl_read(&mut pb, id);
        assert_eq!((got, body), (1, probe), "depth {}: appl_read(self)", depth + 1);
    }

    // Unwind innermost-first, as the Pexec chain does.
    for p in chain.iter().rev() {
        p.run();
        assert_eq!(appl_exit(&mut pb), 1);
        pterm(p);
    }
    desktop_context();
    assert_eq!(appl_init(&mut pb), 1, "the desktop is still there after the chain");
    assert_eq!(appl_exit(&mut pb), 1);
}

// ── (b) nine names in one slot: appl_find answers for the occupant ────────

/// `SEQ1.NAP` … `SEQ9.NAP` one after another all land in GEMDOS slot 0 and
/// rename **one** descriptor (measured: `appl_find(previous) = -1` every
/// time, A62 report §1a) — EmuTOS's `p_nameit` overwrites the one `p_name` in
/// place (`aes/gemshlib.c:633`, `aes/gempd.c:77-86`). One name per slot.
#[test]
fn nine_programs_in_one_slot_rename_one_descriptor() {
    let _g = lock();
    let mut pb = Aespb::new();
    let mut previous: Option<[u8; 8]> = None;
    for n in 1..=9u8 {
        let p = Proc::new();
        assert_eq!(p.slot, 0, "each program takes the first free slot");
        let path = format!("C:\\SEQ{n}.NAP");
        let mut name = *b"        ";
        name[..4].copy_from_slice(b"SEQ ");
        name[3] = b'0' + n;
        launcher_register(p.slot, path.as_bytes());
        p.run();
        assert_eq!(appl_init(&mut pb), 1, "SEQ{n}: appl_init reports slot 0 + 1");
        assert_eq!(appl_find(&mut pb, &name), 1, "SEQ{n}: appl_find(self)");
        if let Some(prev) = previous {
            assert_eq!(
                appl_find(&mut pb, &prev),
                -1,
                "SEQ{n}: the previous occupant's name must be gone — one name per slot"
            );
        }
        assert_eq!(appl_exit(&mut pb), 1);
        pterm(&p);
        previous = Some(name);
    }
    desktop_context();
}

// ── (c) the slot's release resets its descriptor ──────────────────────────

/// A program that called `appl_init`, has a message unread, and dies without
/// `appl_exit` (a crashed GEM app, or a `Pexec` child that never bothered).
/// The A62 report (§8) found that `handle_pexec` runs neither `register` nor
/// `close_client_if_open`, so the slot's next tenant inherited `AP_OPEN` and
/// the queue. With the descriptor keyed by the slot, the slot's release is the
/// one place the process model knows the tenant is gone — and it does what
/// `sh_ldapp` does at its boundary (`aes/gemshlib.c:636,650-657`): flags
/// cleared, queue drained.
#[test]
fn releasing_a_slot_resets_its_descriptor_and_queue() {
    let _g = lock();
    let mut pb = Aespb::new();

    let slot = {
        let dying = Proc::new();
        dying.run();
        let id = appl_init(&mut pb);
        assert_eq!(id, dying.reported());
        let stale = [0x0BAD, id, 0, 0, 0, 0, 0, 0];
        assert_eq!(appl_write(&mut pb, id, &stale), 1);
        // No appl_exit: AP_OPEN stays set, the message stays queued.
        pterm(&dying);
        dying.slot
    };

    // The next tenant of the same slot.
    let tenant = Proc::new();
    assert_eq!(tenant.slot, slot, "the freed slot is the first free one again");
    tenant.run();
    let id = appl_init(&mut pb);
    assert_eq!(id, tenant.reported());
    let (got, body) = appl_read(&mut pb, id);
    assert_eq!(
        (got, body[0]),
        (0, 0),
        "the new tenant read the dead tenant's message (type {:#x}) — the \
         slot's release did not drain the descriptor's queue",
        body[0]
    );

    // The flag reset, seen through its one consumer (A28's argument): the
    // launcher's "appl_init without appl_exit" cleanup must judge the program
    // that just ran. A console program (never `appl_init`) in this slot, with a
    // message posted to it meanwhile, must keep that message — a stale AP_OPEN
    // would have drained it.
    assert_eq!(appl_exit(&mut pb), 1);
    pterm(&tenant);
    drop(tenant);

    let console = Proc::new();
    assert_eq!(console.slot, slot);
    launcher_register(console.slot, b"C:\\HELLO.NCP");
    console.run();
    let posted = [0x4711, console.reported(), 0, 0, 0, 0, 0, 0];
    assert_eq!(appl_write(&mut pb, console.reported(), &posted), 1);
    launcher_cleanup(console.slot);
    let (got, body) = appl_read(&mut pb, console.reported());
    assert_eq!(
        (got, body),
        (1, posted),
        "close_client_if_open fired for a program that never called appl_init \
         — a stale AP_OPEN survived the slot's release"
    );
    drop(console);
    desktop_context();
}

// ── (d) the reported values, unchanged ────────────────────────────────────

/// Stage 1's regression condition: `ap_id`, `appl_find`, `WF_OWNER` and
/// `msg[1]` report exactly the numbers they reported before the rework —
/// GEMDOS slot + 1 for a process, 1 for the kernel-context desktop, 0 for the
/// unowned desktop window. These are **not** EmuTOS's numbers (that is stage
/// 2, A56); they are pinned here so the identity rework cannot move them.
#[test]
fn reported_values_are_unchanged_by_the_identity_rework() {
    let _g = lock();
    let mut pb = Aespb::new();

    // The desktop: id 1, and its window's owner reports 1.
    desktop_context();
    assert_eq!(appl_init(&mut pb), 1, "desktop ap_id");
    assert_eq!(wf_owner(&mut pb, DESKWH), 0, "the desktop window has no owner yet");
    let dw = wind_create(&mut pb);
    assert_eq!(wf_owner(&mut pb, dw), 1, "WF_OWNER of a desktop-created window");
    wind_open(&mut pb, dw, WR);
    let (got, body) = appl_read(&mut pb, 1);
    assert_eq!((got, body[0], body[1], body[3]), (1, WM_REDRAW, 1, dw), "desktop WM_REDRAW msg[1]");
    drain(&mut pb, 1);
    wind_set_newdesk(&mut pb);
    assert_eq!(wf_owner(&mut pb, DESKWH), 1, "WF_NEWDESK re-owns the desktop window to the caller");
    wind_close(&mut pb, dw);
    wind_delete(&mut pb, dw);
    drain(&mut pb, 1);

    // A launched program in slot 0 and its Pexec child in slot 1.
    let parent = Proc::new();
    let child = Proc::new();
    assert_eq!((parent.slot, child.slot), (0, 1));
    launcher_register(parent.slot, b"C:\\TERADESK\\DESKTOP.NAP");

    parent.run();
    assert_eq!(appl_init(&mut pb), 1, "slot 0 reports 1 — the same number as the desktop");
    assert_eq!(appl_find(&mut pb, b"DESKTOP "), 1, "appl_find by the launch name");
    let pw = wind_create(&mut pb);
    assert_eq!(wf_owner(&mut pb, pw), 1, "WF_OWNER, slot 0");
    wind_open(&mut pb, pw, WR);
    let (got, body) = appl_read(&mut pb, 1);
    assert_eq!((got, body[0], body[1], body[3]), (1, WM_REDRAW, 1, pw), "slot 0 WM_REDRAW msg[1]");
    // A message the parent sends itself by its own number reaches the parent.
    let own = [0x4711, 1, 0, 7, 0, 0, 0, 0];
    assert_eq!(appl_write(&mut pb, 1, &own), 1);
    assert_eq!(appl_read(&mut pb, 1), (1, own), "appl_write(1) from slot 0 reaches slot 0");

    child.run();
    assert_eq!(appl_init(&mut pb), 2, "slot 1 reports 2");
    assert_eq!(appl_find(&mut pb, b"DESKTOP "), 1, "appl_find still names the parent's slot");
    let cw = wind_create(&mut pb);
    assert_eq!(wf_owner(&mut pb, cw), 2, "WF_OWNER, slot 1");
    wind_open(&mut pb, cw, WR);
    let (got, body) = appl_read(&mut pb, 2);
    assert_eq!((got, body[0], body[1], body[3]), (1, WM_REDRAW, 2, cw), "slot 1 WM_REDRAW msg[1]");
    // The parent's window still reports the parent, and a message to 1 from
    // the child lands in the parent's queue, not the child's.
    assert_eq!(wf_owner(&mut pb, pw), 1);
    let to_parent = [0x4712, 2, 0, 0, 0, 0, 0, 0];
    assert_eq!(appl_write(&mut pb, 1, &to_parent), 1);
    assert_eq!(appl_read(&mut pb, 2).0, 0, "nothing for the child");
    assert_eq!(appl_write(&mut pb, 17, &to_parent), 0, "no process reports 17");
    assert_eq!(appl_write(&mut pb, 0, &to_parent), 0, "no process reports 0 in stage 1");
    wind_close(&mut pb, cw);
    wind_delete(&mut pb, cw);
    assert_eq!(appl_exit(&mut pb), 1);
    pterm(&child);

    parent.run();
    assert_eq!(appl_read(&mut pb, 1), (1, to_parent), "the parent reads what the child sent it");
    wind_close(&mut pb, pw);
    wind_delete(&mut pb, pw);
    drain(&mut pb, 1);
    assert_eq!(appl_exit(&mut pb), 1);
    pterm(&parent);
    drop(child);
    drop(parent);

    // Back in the desktop: still 1, still addressable (A29's effect).
    desktop_context();
    assert_eq!(appl_init(&mut pb), 1, "desktop ap_id after the programs");
    let probe = [0x4713, 1, 0, 0, 0, 0, 0, 0];
    assert_eq!(appl_write(&mut pb, 1, &probe), 1);
    assert_eq!(appl_read(&mut pb, 1), (1, probe));
    assert_eq!(appl_exit(&mut pb), 1);
}
