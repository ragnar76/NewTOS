#!/usr/bin/env python3
"""d62-trace.py <scratch-tree> -- DESK62 call-path instrument (SCRATCH COPY ONLY, never the shared tree).

Adds one serial line per AES call of interest, from any EL0 caller, at the kernel's
GEM-trap observation hook (kernel/src/shell_loop.rs observe_desktop_gem_call, which the
svc handler calls for every GEM trap before the dispatch):

  [d62] slot=<running slot> desk=<0|1> aes op=<opcode> in0=<int_in[0]> in1=<int_in[1]>

for opcodes 10 appl_init, 19 appl_exit, 78 graf_mouse, 121 shel_write (D62_ALL=1 in the
environment: every AES opcode). desk=1 means the
caller is the desktop program the shell's DESKTOP arm started (is_desktop_caller).
It reads control[0] and int_in[0..2] with the caller's rights, exactly the words the AES
dispatcher reads right after; it changes nothing the program sees. Output is UART only
(kprintln), never the framebuffer console.
"""
import sys
p = sys.argv[1] + "/kernel/src/shell_loop.rs"
s = open(p).read()
anchor = "pub fn observe_desktop_gem_call(args: &TrapArgs) {\n"
assert s.count(anchor) == 1, "anchor"
s = s.replace(anchor, anchor + "    d62_trace(args); // D62-TEMP\n", 1)
s += r'''
// D62-TEMP BEGIN (scratch instrument, measurement-2026-10-16-desk62)
const D62_ALL: bool = @ALL@;
fn d62_trace(args: &TrapArgs) {
    if args.regs[0] != GEM_SEL_AES || args.space != Space::User {
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
    // D62_ALL=1 at patch time: every AES opcode; otherwise 10/19/78/121 only.
    if !D62_ALL && !matches!(op, 10 | 19 | 78 | 121) {
        return;
    }
    let intin = ptr(2).unwrap_or(0);
    let in0 = if intin != 0 { args.space.get_u16(intin).map(|v| v as i16).unwrap_or(-9999) } else { -9999 };
    let in1 = if intin != 0 { args.space.get_u16(intin + 2).map(|v| v as i16).unwrap_or(-9999) } else { -9999 };
    let desk = DESKTOP_ACTIVE.load(Ordering::Acquire) && is_desktop_caller(args);
    kprintln!(
        "[d62] slot={:?} desk={} aes op={} in0={} in1={}",
        running_slot(),
        desk as u8,
        op,
        in0,
        in1
    );
}
// D62-TEMP END
'''
import os
s = s.replace("@ALL@", "true" if os.environ.get("D62_ALL") == "1" else "false")
open(p, "w").write(s)
print("patched", p)
