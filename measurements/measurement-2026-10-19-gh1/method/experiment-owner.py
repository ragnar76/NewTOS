#!/usr/bin/env python3
"""experiment-owner.py <scratch-tree> -- GH1 EXPERIMENT (scratch copy only, not a fix proposal as code): restrict
post_button to the mouse owner's parked wait, as EmuTOS post_button(gl_mowner, ...) does (aes/geminput.c:463-483),
to measure what an owner-correct delivery alone would do with the current ownership hand-over."""
import pathlib, sys
p = pathlib.Path(sys.argv[1]) / "tos-api/src/aes/input.rs"; s = p.read_text()
old = "    for (w, parm) in BWAIT.iter().zip(BWAIT_PARM.iter()) {\n        if w.load(Ordering::Relaxed) != BW_ARMED {\n"
new = ("    for (gh1_i, (w, parm)) in BWAIT.iter().zip(BWAIT_PARM.iter()).enumerate() {\n"
       "        if gh1_i != GL_MOWNER_PD.load(Ordering::Relaxed) { continue; } // GH1-EXPERIMENT\n"
       "        if w.load(Ordering::Relaxed) != BW_ARMED {\n")
assert s.count(old) == 1, "anchor"
p.write_text(s.replace(old, new)); print("experiment-owner applied", p)
