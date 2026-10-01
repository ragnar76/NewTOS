# A94 method (2026-09-21) — the A79 method, re-used

Scripts copied unchanged from `measurement-2026-09-20-a79/method/` (run.sh,
qmp_multi.py, apply_injector.py, a79_patch.py, a78_patch2.py,
make_scratch_final.sh, gen_script.py, sd-put.sh, pixdiff.py, newdesk-tera.inf,
teradesk-window.inf, desk.actions). New: `tera-two.actions` (second
double-click on HARD DISC C after the first window is open) and the
`final_scripts.rs` generated from `tera-two.actions` (SCRIPT_TERA) +
`desk.actions` (SCRIPT_DESK).

Scratch kernel: `make_scratch_final.sh <ws> <scratch>` (rsync of the crates,
injector + serial trace scaffolding — scratch copy only, never the tree),
then `cargo xtask build --board rpi4` inside the scratch dir.
SD images: `tools/make-sd-image.sh <img> 64` (sd-base, our desktop, no #Z);
sd-tera = sd-base + NEWDESK.INF (`newdesk-tera.inf`) + Apps/TERADESK/TERADESK.INF
(`teradesk-window.inf`) via `sd-put.sh`. No `*.img` kept.
Run: `run.sh KERNEL8 IMG OUTDIR TAG 90 MARKER:PNG:SETTLE ...` (QEMU raspi4b,
640x480, -display none, QMP screendump 0.3 s after each serial mark).
