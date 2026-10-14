//! `HELLOGEM.NAP` — DESK43 U6a, the EL0 spike: the smallest complete GEM
//! application in Rust on the `newtos-gem` runtime.
//!
//! What it proves (`docs/analysis/desk43-u5-u10-review-2026-10-14.md`
//! §3.2 U6a, §7.4 point 1/3/7): a Rust `no_std` + `alloc` program, compiled
//! position-independent and linked with `sdk/crt0.S` + `sdk/newtos.ld`, is
//! accepted by the NewTOS loader and runs as an AES client at EL0 —
//! `appl_init`, a virtual VDI workstation, a window with a redraw loop
//! driven by `evnt_multi`, `appl_exit`, the exit code back to the shell.
//! Along the way it exercises the heap (thousands of allocations, `Vec`/
//! `String` growth, frees and reuse) and measures its own stack use; both
//! results are shown in the window and printed once more on the console
//! (serial with `serial=on`) after the window is gone.
//!
//! It ends on a key, a left click, or the window's closer. Built with
//! `--features panic-test` it panics on purpose after its first redraw
//! (the runtime's panic path: the program ends with `PANIC_EXIT`).

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use newtos_gem::aes::{self, Grect};
use newtos_gem::{gemdos, heap::HeapStats, vdi, FixedBuf, HEAP};

/// Window parts: a title bar with closer, movable.
const KIND: i16 = aes::NAME | aes::CLOSER | aes::MOVER;
/// The window title (the AES keeps the pointer, so it is static).
const TITLE: &core::ffi::CStr = c" HELLOGEM (Rust, EL0) ";

/// sdk/crt0.S `MINKEEP`: the stack crt0 keeps above the image.
const CRT0_STACK: usize = 0x10000;
/// The word the unused stack is painted with.
const STACK_PAINT: u64 = 0x5354_4143_4b21_5354; // "ST!KCATS"

extern "C" {
    /// First byte past the image (`sdk/newtos.ld`); crt0's stack ends here.
    static _end: u8;
}

/// What the heap exercise found.
struct HeapRun {
    ok: bool,
    detail: String,
    allocs: u64,
    during: HeapStats,
    after: HeapStats,
}

/// Exercise the allocator: many small objects, `String`/`Vec` growth,
/// frees in the middle, reuse, a `BTreeMap`. Every structure is checked
/// against values computed independently, then everything is dropped.
fn heap_exercise() -> HeapRun {
    let before = HEAP.stats();
    let mut ok = true;
    let mut detail = String::new();

    // 1. 3000 small strings, each built by `format!` (growth inside).
    let strings: Vec<String> = (0..3000u32).map(|i| format!("item {i:05}")).collect();
    let total_len: usize = strings.iter().map(String::len).sum();
    if total_len != 3000 * 10 || strings[1234] != "item 01234" {
        ok = false;
        let _ = write!(detail, "strings({total_len}) ");
    }

    // 2. One vector grown element by element to 20 000 u64 (160 KB):
    //    repeated reallocation, the last ones beyond the hunk schedule.
    let mut squares: Vec<u64> = Vec::new();
    for i in 0..20_000u64 {
        squares.push(i * i);
    }
    // sum_{i<n} i^2 = (n-1) n (2n-1) / 6
    let n = 20_000u64;
    if squares.iter().sum::<u64>() != (n - 1) * n * (2 * n - 1) / 6 {
        ok = false;
        detail.push_str("squares ");
    }

    // 3. 2000 boxes; free every other one, allocate 1000 more into the
    //    holes, check that nothing overlaps (each box holds its index).
    let mut boxes: Vec<Option<alloc::boxed::Box<[u32; 12]>>> =
        (0..2000u32).map(|i| Some(alloc::boxed::Box::new([i; 12]))).collect();
    for b in boxes.iter_mut().step_by(2) {
        *b = None;
    }
    let refill: Vec<alloc::boxed::Box<[u32; 12]>> =
        (0..1000u32).map(|i| alloc::boxed::Box::new([100_000 + i; 12])).collect();
    let boxes_ok = boxes
        .iter()
        .enumerate()
        .all(|(i, b)| b.as_ref().map_or(i % 2 == 0, |b| b.iter().all(|&v| v == i as u32)))
        && refill.iter().enumerate().all(|(i, b)| b.iter().all(|&v| v == 100_000 + i as u32));
    if !boxes_ok {
        ok = false;
        detail.push_str("boxes ");
    }

    // 4. A BTreeMap of 1500 entries (node allocations of several sizes).
    let mut map: BTreeMap<u32, String> = BTreeMap::new();
    for i in 0..1500u32 {
        map.insert(i.wrapping_mul(2_654_435_761) % 100_003, format!("{i}"));
    }
    let map_ok = map.len() == 1500 && map.values().map(|s| s.parse::<u32>().unwrap_or(u32::MAX)).sum::<u32>() == 1499 * 1500 / 2;
    if !map_ok {
        ok = false;
        detail.push_str("map ");
    }

    let during = HEAP.stats();
    drop(map);
    drop(refill);
    drop(boxes);
    drop(squares);
    drop(strings);
    let after = HEAP.stats();
    if after.live != before.live {
        ok = false;
        let _ = write!(detail, "leak({}) ", after.live - before.live);
    }
    HeapRun { ok, detail, allocs: during.allocs - before.allocs, during, after }
}

/// Read the stack pointer.
fn sp() -> usize {
    let sp: usize;
    // SAFETY: reads SP only.
    unsafe { core::arch::asm!("mov {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags)) };
    sp
}

/// The stack crt0 gave this program: `[_end, top)` with
/// `top = B + round16(_end - B) + MINKEEP` (`sdk/crt0.S`).
fn stack_bounds(basepage: usize) -> (usize, usize) {
    // Only the address of the linker symbol is taken (no read).
    let end = core::ptr::addr_of!(_end) as usize;
    let top = basepage + ((end - basepage + 15) & !15) + CRT0_STACK;
    ((end + 15) & !15, top)
}

/// Paint the unused part of the stack (below the current SP, minus a
/// margin for this function's own frame).
fn stack_paint(lo: usize) {
    let hi = (sp() - 1024) & !7;
    let mut a = lo;
    while a < hi {
        // SAFETY: `[lo, hi)` is this program's own stack below the live
        // frames (crt0 keeps it, nothing else uses it; exceptions run on
        // SP_EL1).
        unsafe { core::ptr::write_volatile(a as *mut u64, STACK_PAINT) };
        a += 8;
    }
}

/// The lowest stack address that was written since [`stack_paint`].
fn stack_low_water(lo: usize, top: usize) -> usize {
    let mut a = lo;
    while a < top {
        // SAFETY: reads this program's own stack.
        if unsafe { core::ptr::read_volatile(a as *const u64) } != STACK_PAINT {
            return a;
        }
        a += 8;
    }
    top
}

/// The application's state between events.
struct App {
    win: i16,
    vh: i16,
    hchar: i16,
    wchar: i16,
    lines: Vec<String>,
}

impl App {
    /// Paint `area` (a screen rectangle) of the window: every visible piece
    /// of the work area from the AES's rectangle list, clipped (EmuTOS
    /// `do_wredraw` pattern, `desk/desksupp.c`).
    fn redraw(&self, area: Grect) {
        aes::wind_update(aes::BEG_UPDATE);
        aes::graf_mouse(aes::M_OFF);
        let work = aes::wind_get_grect(self.win, aes::WF_WORKXYWH);
        let mut r = aes::wind_get_grect(self.win, aes::WF_FIRSTXYWH);
        while r.w > 0 && r.h > 0 {
            if let Some(c) = r.intersect(&area) {
                self.paint(work, c);
            }
            r = aes::wind_get_grect(self.win, aes::WF_NEXTXYWH);
        }
        vdi::vs_clip(self.vh, None);
        aes::graf_mouse(aes::M_ON);
        aes::wind_update(aes::END_UPDATE);
    }

    fn paint(&self, work: Grect, clip: Grect) {
        let h = self.vh;
        vdi::vs_clip(h, Some(clip));
        vdi::vswr_mode(h, vdi::MD_REPLACE);
        vdi::vsf_interior(h, vdi::FIS_SOLID);
        vdi::vsf_color(h, vdi::WHITE);
        vdi::vr_recfl(h, work);
        vdi::vst_color(h, vdi::BLACK);
        vdi::vst_alignment(h, 0, 5); // left, top line
        for (i, l) in self.lines.iter().enumerate() {
            vdi::v_gtext(h, work.x + self.wchar, work.y + self.hchar / 2 + i as i16 * self.hchar, l.as_bytes());
        }
    }
}

/// crt0 calls `main(BASEPAGE *)`; the return value is the exit code.
///
/// # Safety
///
/// Called once, by `sdk/crt0.S`, with the program's basepage.
#[no_mangle]
pub unsafe extern "C" fn main(basepage: usize) -> i32 {
    let (stack_lo, stack_top) = stack_bounds(basepage);
    stack_paint(stack_lo);

    let ap_id = aes::appl_init();
    if ap_id < 0 {
        gemdos::cconws(c"[HELLOGEM] appl_init failed\r\n");
        return gemdos::ERROR;
    }
    let gh = aes::graf_handle();
    let Some(ws) = vdi::v_opnvwk(gh.handle) else {
        aes::form_alert(1, c"[3][HELLOGEM: no VDI workstation][ Quit ]");
        aes::appl_exit();
        return gemdos::ERROR;
    };

    let run = heap_exercise();

    let st = run.during;
    let mut lines: Vec<String> = Vec::new();
    lines.push(String::from("Hello from Rust at EL0!"));
    lines.push(String::from("HELLOGEM.NAP - DESK43 U6a, runtime newtos-gem"));
    lines.push(String::new());
    lines.push(format!("AES ap_id {} (global {}), VDI handle {} on phys {}", ap_id, aes::ap_id(), ws.handle, gh.handle));
    lines.push(format!("screen {}x{}, char cell {}x{}", ws.xmax as i32 + 1, ws.ymax as i32 + 1, gh.wchar, gh.hchar));
    lines.push(format!(
        "heap test {}: {} allocations {}",
        if run.ok { "OK" } else { "FAILED" },
        run.allocs,
        run.detail
    ));
    lines.push(format!(
        "GEMDOS Malloc {} / Mfree {}, hunks {} (peak {}), {} KiB",
        st.mallocs,
        st.mfrees,
        st.hunks,
        st.peak_hunks,
        st.hunk_bytes / 1024
    ));
    lines.push(format!(
        "after drop: live {}, hunks {}, {} KiB, Mfree {}",
        run.after.live,
        run.after.hunks,
        run.after.hunk_bytes / 1024,
        run.after.mfrees
    ));
    lines.push(String::new());
    lines.push(String::from("Press a key or click to quit."));

    let desk = aes::wind_get_grect(0, aes::WF_WORKXYWH);
    let cols = lines.iter().map(String::len).max().unwrap_or(0) as i16 + 2;
    let mut work = Grect {
        x: 0,
        y: 0,
        w: (cols * gh.wchar).min(desk.w - 4 * gh.wchar),
        h: ((lines.len() as i16) + 1) * gh.hchar,
    };
    work.x = desk.x + (desk.w - work.w) / 2;
    work.y = desk.y + (desk.h - work.h) / 2;
    let outer = aes::wind_calc(aes::WC_BORDER, KIND, work);

    let win = aes::wind_create(KIND, desk);
    if win <= 0 {
        aes::form_alert(1, c"[3][HELLOGEM: no window free][ Quit ]");
        vdi::v_clsvwk(ws.handle);
        aes::appl_exit();
        return gemdos::ERROR;
    }
    aes::wind_set_name(win, TITLE);
    aes::wind_open(win, outer);
    aes::graf_mouse(aes::ARROW);

    let app = App { win, vh: ws.handle, hchar: gh.hchar, wchar: gh.wchar, lines };

    let mut msg = [0i16; 8];
    let mut redraws = 0u32;
    let reason: String = loop {
        let ev = aes::evnt_multi(aes::MU_MESAG | aes::MU_KEYBD | aes::MU_BUTTON, 1, 1, 1, 0, &mut msg);
        if ev.which & aes::MU_MESAG != 0 && msg[3] == win {
            let r = Grect { x: msg[4], y: msg[5], w: msg[6], h: msg[7] };
            match msg[0] {
                aes::WM_REDRAW => {
                    app.redraw(r);
                    redraws += 1;
                    if cfg!(feature = "panic-test") {
                        panic!("deliberate panic (panic-test build) after redraw {}", redraws);
                    }
                }
                aes::WM_TOPPED => {
                    aes::wind_set_top(win);
                }
                aes::WM_MOVED => {
                    aes::wind_set_grect(win, aes::WF_CURRXYWH, r);
                }
                aes::WM_CLOSED => break String::from("closer"),
                _ => {}
            }
        }
        if ev.which & aes::MU_KEYBD != 0 {
            break format!("key 0x{:04x}", ev.key as u16);
        }
        if ev.which & aes::MU_BUTTON != 0 {
            break format!("click at {},{}", ev.mx, ev.my);
        }
    };

    aes::wind_close(win);
    aes::wind_delete(win);
    vdi::v_clsvwk(ws.handle);
    let exit_rc = aes::appl_exit();

    let used = stack_top - stack_low_water(stack_lo, stack_top);
    let fin = HEAP.stats();
    let mut m = FixedBuf::<320>::new();
    let _ = write!(
        m,
        "[HELLOGEM] ap_id={} vwk={} redraws={} quit by {}; heap {} allocs={} Malloc={} Mfree={} peak_hunks={} end: hunks={} live={}; stack used {} of {} bytes; appl_exit={}\r\n",
        ap_id,
        ws.handle,
        redraws,
        reason,
        if run.ok { "OK" } else { "FAILED" },
        fin.allocs,
        fin.mallocs,
        fin.mfrees,
        fin.peak_hunks,
        fin.hunks,
        fin.live,
        used,
        stack_top - stack_lo,
        exit_rc
    );
    drop(reason);
    drop(app);
    gemdos::cconws(m.as_cstr());
    if run.ok {
        0
    } else {
        1
    }
}
