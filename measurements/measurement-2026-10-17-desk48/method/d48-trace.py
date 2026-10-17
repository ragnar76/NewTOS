#!/usr/bin/env python3
"""d48-trace.py <scratch-tree> -- DESK48 AES-call trace (SCRATCH COPY ONLY, never the shared tree).

Derived from measurements/measurement-2026-10-16-desk62-fix/method/d62-trace.py. Adds one serial line per AES call of
the desktop program (is_desktop_caller) for the opcodes that show what gets repainted:

  [d48] aes op=<opcode> in=<int_in[0]>,<int_in[1]>,...,<int_in[5]>

  30 menu_bar      redraw_desktop() re-installs the menu bar (menu_bar(tree, 1))
  42 objc_draw     in = start, depth, clip x, y, w, h -- the area each do_wredraw piece paints
  51 form_dial     in0 = 3 (FMD_FINISH) over the whole screen = the redraw_desktop() marker
  52 form_alert    the alerts of the delete paths
  78 graf_mouse    in0 2 = HGLASS (desk_busy_on), 0 = ARROW, 256/257 = M_OFF/M_ON

at the kernel's GEM-trap observation hook (kernel/src/shell_loop.rs observe_desktop_gem_call). Reads control[0] and
int_in[0..6] with the caller's rights; changes nothing the program sees. UART only (kprintln).
"""
import sys
p = sys.argv[1] + "/kernel/src/shell_loop.rs"
s = open(p).read()
anchor = "pub fn observe_desktop_gem_call(args: &TrapArgs) {\n"
assert s.count(anchor) == 1, "anchor"
s = s.replace(anchor, anchor + "    d48_trace(args); // D48-TEMP\n", 1)
s += r'''
// D48-TEMP BEGIN (scratch instrument, measurement-2026-10-17-desk48)
fn d48_trace(args: &TrapArgs) {
    if args.regs[0] != GEM_SEL_AES || args.space != Space::User {
        return;
    }
    if !(DESKTOP_ACTIVE.load(Ordering::Acquire) && is_desktop_caller(args)) {
        return;
    }
    let block = args.regs[1];
    if block == 0 {
        return;
    }
    let ptr = |i: u64| -> Option<u64> {
        match current_abi() {
            Abi::Lp64 => args.space.get_u64(block + 8 * i).ok(),
            Abi::Ilp32 => args.space.get_u32(block + 4 * i).ok().map(u64::from),
        }
    };
    let Some(control) = ptr(0) else { return };
    let Ok(op) = args.space.get_u16(control) else { return };
    if !matches!(op, 30 | 42 | 51 | 52 | 78) {
        return;
    }
    let intin = ptr(2).unwrap_or(0);
    let mut v = [-9999i16; 6];
    if intin != 0 {
        for (i, slot) in v.iter_mut().enumerate() {
            *slot = args.space.get_u16(intin + 2 * i as u64).map(|x| x as i16).unwrap_or(-9999);
        }
    }
    kprintln!("[d48] aes op={} in={},{},{},{},{},{}", op, v[0], v[1], v[2], v[3], v[4], v[5]);
}
// D48-TEMP END
'''
open(p, "w").write(s)
print("patched", p)
