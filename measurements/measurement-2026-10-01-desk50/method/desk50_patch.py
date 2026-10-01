#!/usr/bin/env python3
"""DESK50-TEMP (scratch copy only) -- make do_wopen's zoom photographable and
trace the window-open tail on the serial line.

1. tos-api/src/aes/graf.rs: while `graf_growbox` (73) animates, three frames
   are HELD for 1.5 s each -- the first (the outline at the source icon), the
   middle one and the last (the outline at the target) -- and announced on the
   serial line as `[d50] ZOOM-HOLD i/steps (x,y,w,h)`, so the QMP driver can
   take a screendump of the XOR outline while it is on screen. Nothing else
   about the animation changes; `form_dial`'s and `graf_movebox`'s animations
   are not held (the flag is set only inside graf_growbox).
   tos-api has no serial-only output, so the kernel installs a say-function
   pointer (`ZOOM_SAY`) at the top of `deskmain`; it writes to the UART only
   (never fbcon, which would paint over the screen being photographed).
2. kernel/src/desk/main.rs: `[d50] …` lines in do_wopen (source/target
   rectangle, current window/root, the curr object's ob_state before and
   after act_chg) and do_diropen (path, new_win, result), plus a dump of the
   window's item objects (name + screen extent) after desk_verify, so click
   points come from the run itself and are not guessed.
3. kernel/src/desk/mod.rs: a dump of the desktop icons at "desktop up".
"""
import sys
scr = sys.argv[1]

def rep(path, a, b, n=1):
    s = open(path).read()
    assert s.count(a) == n, (path, s.count(a), a[:70])
    open(path, 'w').write(s.replace(a, b, n))
    print("patched", path)

g = scr + '/tos-api/src/aes/graf.rs'
rep(g, "pub fn graf_growbox(pb: &Aespb) {\n    let small = (pb.intin(0), pb.intin(1), pb.intin(2), pb.intin(3));\n    let big = (pb.intin(4), pb.intin(5), pb.intin(6), pb.intin(7));\n    animate(small, big);\n",
       "pub fn graf_growbox(pb: &Aespb) {\n    let small = (pb.intin(0), pb.intin(1), pb.intin(2), pb.intin(3));\n    let big = (pb.intin(4), pb.intin(5), pb.intin(6), pb.intin(7));\n    D50_IN_GROW.store(true, core::sync::atomic::Ordering::Relaxed); // DESK50-TEMP\n    animate(small, big);\n    D50_IN_GROW.store(false, core::sync::atomic::Ordering::Relaxed); // DESK50-TEMP\n")
rep(g, "        outline(r.0, r.1, r.2, r.3, BLACK, backend, clip);\n",
       "        outline(r.0, r.1, r.2, r.3, BLACK, backend, clip);\n"
       "        d50_hold(i, steps, r); // DESK50-TEMP\n")
s = open(g).read()
s += '''
// DESK50-TEMP BEGIN -- zoom hold for the screendump (scratch copy only)
static D50_IN_GROW: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// `fn(&str)` installed by the kernel; writes to the UART only.
pub static ZOOM_SAY: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
fn d50_hold(i: i16, steps: i16, r: (i16, i16, i16, i16)) {
    use core::sync::atomic::Ordering;
    if !D50_IN_GROW.load(Ordering::Relaxed) || !(i == 0 || i == steps / 2 || i == steps) {
        return;
    }
    let f = ZOOM_SAY.load(Ordering::Relaxed);
    if f != 0 {
        let say: fn(&str) = unsafe { core::mem::transmute(f) };
        let mut b = [0u8; 96];
        let mut n = 0;
        let mut put = |s: &[u8]| { for &c in s { if n < b.len() { b[n] = c; n += 1; } } };
        put(b"[d50] ZOOM-HOLD ");
        let mut num = |v: i16, put: &mut dyn FnMut(&[u8])| {
            let mut t = [0u8; 8]; let mut k = 8; let neg = v < 0; let mut u = (v as i32).unsigned_abs();
            if u == 0 { k -= 1; t[k] = b'0'; }
            while u > 0 { k -= 1; t[k] = b'0' + (u % 10) as u8; u /= 10; }
            if neg { put(b"-"); }
            put(&t[k..]);
        };
        num(i, &mut put); put(b"/"); num(steps, &mut put); put(b" (");
        num(r.0, &mut put); put(b","); num(r.1, &mut put); put(b","); num(r.2, &mut put); put(b","); num(r.3, &mut put);
        put(b")\\r\\n");
        say(core::str::from_utf8(&b[..n]).unwrap_or("?"));
    }
    if let Some(h) = host() {
        let t = h.monotonic_millis() + 1500;
        while h.monotonic_millis() < t {
            core::hint::spin_loop();
        }
    }
}
// DESK50-TEMP END
'''
open(g, 'w').write(s)
print("patched", g)

rep(scr + '/tos-api/src/aes/mod.rs', "mod graf;\n", "mod graf;\npub use graf::ZOOM_SAY; // DESK50-TEMP\n")

m = scr + '/kernel/src/desk/mod.rs'
rep(m, "fn deskmain() -> bool {\n",
       "fn deskmain() -> bool {\n"
       "    // DESK50-TEMP: serial-only sink for the zoom-hold markers.\n"
       "    fn d50_say(s: &str) {\n"
       "        use crate::hal::uart::Uart as _;\n"
       "        for &b in s.as_bytes() {\n"
       "            crate::console::CONSOLE.write_byte(b);\n"
       "        }\n"
       "    }\n"
       "    tos_api::aes::ZOOM_SAY.store(d50_say as fn(&str) as usize, core::sync::atomic::Ordering::Relaxed);\n")
rep(m, '    kprintln!(\n        "[desk] desktop up ({} drives, {} window(s) from the context)",',
       '    // DESK50-TEMP: where the desktop icons are.\n'
       '    global::with(|g| {\n'
       '        let mut o = g.g_screen[global::DROOT as usize].ob_head;\n'
       '        while o != global::NIL && o != global::DROOT {\n'
       '            if let Some(e) = g.obj_extent(global::DROOT, o) {\n'
       '                kprintln!("[d50] desk icon obj {} ch {:?} at ({},{}) {}x{} state {:#06x}", o, (g.g_screeninfo[o as usize].block.ib_char & 0xff) as u8 as char, e.x, e.y, e.w, e.h, g.g_screen[o as usize].ob_state);\n'
       '            }\n'
       '            o = g.g_screen[o as usize].ob_next;\n'
       '        }\n'
       '    });\n'
       '    kprintln!(\n        "[desk] desktop up ({} drives, {} window(s) from the context)",')

k = scr + '/kernel/src/desk/main.rs'
rep(k, """        if let Some(d) = d {
            super::graf_growbox(d, t);
        }
        act_chg_draw(cwin, croot, curr, g_desk(), false, new_win);
    }
""", """        crate::kprintln!("[d50] do_wopen new_win={} wh={} curr={} cwin={} croot={} src={:?} dst=({},{},{},{})", new_win, wh, curr, cwin, croot, d.map(|d| (d.x, d.y, d.w, d.h)), t.x, t.y, t.w, t.h);
        let st0 = global::with(|g| g.g_screen[curr as usize].ob_state).unwrap_or(0xFFFF);
        if let Some(d) = d {
            super::graf_growbox(d, t);
        }
        act_chg_draw(cwin, croot, curr, g_desk(), false, new_win);
        let st1 = global::with(|g| g.g_screen[curr as usize].ob_state).unwrap_or(0xFFFF);
        crate::kprintln!("[d50] act_chg curr={} ob_state {:#06x} -> {:#06x} dodraw={}", curr, st0, st1, new_win);
    } else {
        crate::kprintln!("[d50] do_wopen new_win={} wh={} curr={} (no source object: no zoom, no deselect)", new_win, wh, curr);
    }
""")
rep(k, """    if redraw && !new_win {
        redraw::do_wredraw(handle, pt);
    }

    pro::desk_busy_off();
    true
}""", """    if redraw && !new_win {
        redraw::do_wredraw(handle, pt);
    }

    // DESK50-TEMP: the window's items, for the click points.
    global::with(|g| {
        let root = g.windows.slots[slot].w_root;
        let mut o = g.g_screen[root as usize].ob_head;
        while o != NIL && o != root {
            let fnode = g.g_screeninfo[o as usize].fnode;
            let name = g.windows.slots[slot].w_pnode.p_flist.get(fnode.max(0) as usize).map(|f| alloc::string::String::from(core::str::from_utf8(f.name_bytes()).unwrap_or("?"))).unwrap_or_default();
            if let Some(e) = g.obj_extent(root, o) {
                crate::kprintln!("[d50] item obj {} {} at ({},{}) {}x{}", o, name, e.x, e.y, e.w, e.h);
            }
            o = g.g_screen[o as usize].ob_next;
        }
    });
    crate::kprintln!("[d50] do_diropen {} new_win={} handle={} -> TRUE", pathname, new_win, handle);
    pro::desk_busy_off();
    true
}""")
rep(k, """    let Some((count, size)) = scanned else {
        pro::desk_busy_off();
        return false;
    };""", """    let Some((count, size)) = scanned else {
        crate::kprintln!("[d50] do_diropen {} new_win={} -> FALSE (pn_active < 0)", pathname, new_win);
        pro::desk_busy_off();
        return false;
    };""")
