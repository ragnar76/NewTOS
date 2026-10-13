#!/usr/bin/env python3
"""emucon-stderr-schedule.py (variant of emucon-schedule.py: runs STDERRQ.NAP (new crt0) then STDERRO.NAP (old crt0, = STDERRQ built with crt0-before-libc-main.S))

emucon-schedule.py <scratch-tree>

On top of measurement-2026-10-04-keys02/method/apply-instrumentation.py
(applied first): replace the KRP injector's desktop tail (Alt-C at +2000)
with an EmuCON session, so one raspi4b run covers KBDVEC (PASS) and the
EL1-Pexec path of EmuCON:

  +400..+920   the KEYS02 schedule for KBDVEC.NAP (unchanged)
  +2000        Ctrl-Z  -> desktop File > Execute EmuCON (CLIITEM shortcut)
  +2400        "run hello2.ncp" Enter   (a PIE: must print its banner)
  +3400        "run oldexec.ncp" Enter  (ET_EXEC: "Cannot run program (error 66)")
  +4400        "exit" Enter             (back to the desktop)
Scancodes: IKBD = PC set 1 (make; break = make | 0x80), 10 ticks apart.
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])
he = t / "kernel/src/host_env.rs"
s = he.read_text()
old = "        (2000, 0x38), (2010, 0x2E), (2020, 0xAE), (2030, 0xB8),\n"
assert s.count(old) == 1, "krp schedule anchor"
SC = {'r':0x13,'u':0x16,'n':0x31,' ':0x39,'h':0x23,'e':0x12,'l':0x26,'o':0x18,
      '2':0x03,'.':0x34,'c':0x2E,'p':0x19,'x':0x2D,'i':0x17,'t':0x14,'d':0x20,'\r':0x1C,'s':0x1F,'q':0x10,'a':0x1E}
ent = [(2000,0x1D),(2010,0x2C),(2020,0xAC),(2030,0x9D)]
def typ(at, text):
    for ch in text:
        ent.append((at, SC[ch])); ent.append((at+5, SC[ch]|0x80)); at += 10
typ(2400, "run stderrq.nap\r")
typ(3400, "run stderro.nap\r")
typ(4400, "exit\r")
new = "".join(f"        ({a}, {b:#04x}),\n" for a,b in ent)
s = s.replace(old, new)
he.write_text(s)
print("emucon schedule:", len(ent), "entries, last at", ent[-1][0])
