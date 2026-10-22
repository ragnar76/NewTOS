#!/usr/bin/env python3
"""mk-injector.py <ws> <actions-file> -- patch <ws>/kernel/src/host_env.rs with a scripted IKBD mouse injector
(scratch tree only). Same mechanism as measurements/measurement-2026-10-21-sdk01h/method/scratch-kernel-injector.diff
(hook in the idle pump, armed on the first pump while an EL0 program is the running process), but with the SCRIPT
generated from an action list.
actions:  <tick> parkbr | <tick> move X Y | <tick> down | <tick> up | <tick> dclick
(absolute 640x480 coordinates; one IKBD relative packet per tick, |d|<=127). parkbr sweeps to the bottom-right corner
(crosses no menu title). dclick = down/up/down/up at +0,+3,+8,+11 ticks."""
import sys
ws, acts = sys.argv[1], [l.split() for l in open(sys.argv[2]) if l.strip() and not l.startswith('#')]
px = py = None
out = []
for a in acts:
    t, op = int(a[0]), a[1]
    if op == 'parkbr':
        for k in range(6): out.append((t + k, 'mv(127, 127)'))
        px, py = 639, 479
    elif op == 'move':
        x, y = int(a[2]), int(a[3]); dx, dy = x - px, y - py; k = 0
        while dx or dy:
            sx = max(-127, min(127, dx)); sy = max(-127, min(127, dy))
            out.append((t + k, f'mv({sx}, {sy})')); dx -= sx; dy -= sy; k += 1
        px, py = x, y
    elif op == 'down': out.append((t, 'DN'))
    elif op == 'up': out.append((t, 'UP'))
    elif op == 'dclick':
        for d, n in ((0, 'DN'), (3, 'UP'), (8, 'DN'), (11, 'UP')): out.append((t + d, n))
script = "\n".join(f"        ({t}, &{n})," for t, n in out)
p = ws + '/kernel/src/host_env.rs'
s = open(p).read()
hook = "        pump_devices();\n        core::hint::spin_loop();\n"
assert s.count(hook) == 1, "hook point not found"
s = s.replace(hook, "        pump_devices();\n        con04_inject::step(); // CON04-TEST: scratch injector\n        core::hint::spin_loop();\n")
s += '''
// CON04-TEST BEGIN -- scripted IKBD mouse injector (scratch kernel only, never in the repository tree)
pub mod con04_inject {
    use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    static T0: AtomicU32 = AtomicU32::new(0);
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    const fn mv(dx: i8, dy: i8) -> [u8; 3] { [0xF8, dx as u8, dy as u8] }
    const DN: [u8; 3] = [0xFA, 0, 0];
    const UP: [u8; 3] = [0xF8, 0, 0];
    const SCRIPT: &[(u32, &[u8])] = &[
''' + script + '''
    ];
    pub fn step() {
        if tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null() { return; }
        let now = tos_api::bios::tick::hz_200();
        if T0.load(Ordering::Relaxed) == 0 {
            T0.store(now.max(1), Ordering::Relaxed);
            crate::kprintln!("[con04t] injector armed at tick {}", now);
            return;
        }
        let t0 = T0.load(Ordering::Relaxed);
        let i = NEXT.load(Ordering::Relaxed);
        if i >= SCRIPT.len() { return; }
        let (at, bytes) = SCRIPT[i];
        if now.wrapping_sub(t0) >= at {
            for &b in bytes { crate::ikbd::feed(b); }
            crate::kprintln!("[con04t] step {} at +{} ticks", i, now.wrapping_sub(t0));
            NEXT.store(i + 1, Ordering::Relaxed);
        }
    }
}
// CON04-TEST END
'''
open(p, 'w').write(s)
print(f"injector: {len(out)} steps")
