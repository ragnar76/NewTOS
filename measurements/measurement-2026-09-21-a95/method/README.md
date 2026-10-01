# A95 method (2026-09-21) — the A94/A79 method, re-used, plus the corner residual

Scripts copied unchanged from `measurement-2026-09-21-a94/method/` (run.sh,
qmp_multi.py, apply_injector.py, a79_patch.py, a78_patch2.py,
make_scratch_final.sh, gen_script.py, sd-put.sh, pixdiff.py, newdesk-tera.inf,
teradesk-window.inf, desk.actions, tera-two.actions, final_scripts.rs).

New:

* `dumpwin.py PNG x y w h [x0 y0 x1 y1]` — ASCII class map of a window's
  frame-relative region (`#` black, `.` white, `g` EmuTOS LBLACK
  `(140,138,140)`, `w` LWHITE, `?` other); negative coordinates count from
  the right/bottom border pixel. Used to read the reference pixels the host
  tests encode (`tos-api/tests/aes_window_frame_reference.rs`).
* `corner_compare.py REF rx ry rw rh OURS ox oy ow oh OUT.png` — the A79
  three-corner composite (TL `(0,0)+120x48`, TR `(w-72,0)+72x48`, BR
  `(w-72,h-72)+72x72`, 4x, reference above ours) and the raw per-pixel class
  diff per crop.
* `corner_residual.py REF rx ry rw rh OURS ox oy ow oh [MAXROW]` — the same
  crops minus what is not the frame: work area and info-line interior
  (application content), the pointer sprite on the reference (col `w-1`, rows
  `h-50..h-36`), and — because the VDI aligns fill patterns to absolute screen
  coordinates (`vdi/vdi_fill.c`) and the two windows sit at different screen
  positions — a stipple pixel of the name box (`IP_2PATT`) or a slider track
  (`IP_1PATT`) is compared against its own screen position's expectation in
  each image ("pattern phase"); a track pixel that is not phase-explained is
  the elevator length (an application's `WF_?SLSIZE`). Prints every remaining
  differing pixel with frame-relative coordinates. `MAXROW` restricts the two
  top crops to the title band (for a window whose lower part lies under
  another one).

Scratch kernel: `make_scratch_final.sh <ws> <scratch>` (rsync of the crates +
injector/serial-trace scaffolding — scratch copy only, never the tree; the
scratch `window.rs` was diffed against the tree's: identical), then
`cargo xtask build --board rpi4` inside the scratch dir.
SD images: `tools/make-sd-image.sh <img> 64` (sd-base, our desktop, no #Z);
sd-tera = sd-base + NEWDESK.INF (`newdesk-tera.inf`) + Apps/TERADESK/TERADESK.INF
(`teradesk-window.inf`) via `sd-put.sh`. No `*.img` kept (`../manifest-scratch.sha256`).
Run: `run.sh KERNEL8 IMG OUTDIR TAG 90|120 "MARK <NAME>:<png>:0.3" ...` (QEMU
raspi4b, 640x480, `-display none`, QMP screendump 0.3 s after each serial mark;
the pointer is parked bottom-right (639,479) at every dump).
