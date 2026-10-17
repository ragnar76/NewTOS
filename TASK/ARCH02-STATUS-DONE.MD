# ARCH02 — STATUS: DONE (2026-10-17, Host + QEMU) — `Pexec`-Aufrufer behalten ihre FP/SIMD-Register

**DONE bedeutet hier: gelandet und in QEMU raspi4b belegt; ein Lauf auf der echten Pi 4 steht aus.**

## Was am Quelltext festgestellt wurde

| Stelle | Rolle |
|---|---|
| `handle_pexec`, Startzweig (`child.launch`) | einzige Sicherung des Elternrahmens: Modi 0, 4, 6 (auch eine Basepage aus 5/7, gestartet an `p_tbase`). 3/5/7 starten nichts. Modi 100+ gibt es nicht (`pexec.rs` kennt 0/3/4/5/6/7). |
| `finish_terminate` | einzige Wiederherstellung, gemeinsamer Schluss von `handle_pterm` (0x4C/0x00/0x31), `kill_program` (EL0-Absturz, uaccess-Fehler), `terminate_and_redirect` (Ctrl-C) und `Psigreturn` (`etv_term`-Upcall) |
| Kernel-Start (`launch.rs`) | Elter ist eine EL1-Kernfunktion (AUTO, `sh_ldapp`, NEWDESK, dessen Neustart ist ein frischer Start) — Soft-Float, keine FP-Register |
| `parent_frame_full` | beendetes Accessory setzt den geparkten EL1-Klienten fort — ebenfalls EL1 |

Vor dem Fix sicherte `handle_pexec` nur `x0..x29/lr/elr/spsr/sp_el0`.

## Fix (`8bac146b`)

* `tos-api/src/bdos/process.rs`: `Process::parent_fp: FpContext` (am Kind-Slot, getrennt von `saved_fp`).
* `kernel/src/arch/aarch64/exceptions.rs`: `handle_pexec` → `save_fp_context(parent_fp)`;
  `finish_terminate` → `restore_fp_context(parent_fp)` im `parent_frame_pexec`-Arm.
  Registersemantik: ARM ARM DDI 0487M.d C5.2.8 (FPCR), C5.2.10 (FPSR). `CPACR_EL1.FPEN = 0b11`, also keine FP-Fallen und kein faules Sichern.
* Mitkorrigiert: die NOTE in `boot.rs` (behauptete die Luecke), `FpContext`-Doku, Spec `docs/api/gemdos/process/4b-pexec.md` (Native ABI: Aufrufer kommt mit allen Registern ausser `x0` zurueck, FP/SIMD eingeschlossen).
* `tools/make-sd-image.sh`: `C:\FPPEXEC.NCP` immer, mit `SD_FPPEXEC_AUTO=1` auch in `\AUTO\`.

## Abnahme

| Pruefung | Ergebnis |
|---|---|
| Sonde `sdk/examples/fppexec.c`, Kern vor dem Fix (HEAD `569fb871`) | Runden A `Pexec(0)`, B `3+6`, C `3+4`: je `v=0/32 lost=ffffffff`, FPCR `07c00000/00400000`, FPSR `0800009f/00000000`, **RESULT FAIL** |
| dieselbe Sonde, Kern mit Fix, memprot on/off/strict | je `v=32/32 lost=00000000`, FPCR/FPSR unveraendert, **RESULT PASS**, Desktop danach oben |
| APITEST (neu gebaut, mit PTERMCH/DTACHILD), vor/nach, on/off/strict | **109/0/5** in allen sechs Laeufen, 124 Ergebniszeilen, 0 abweichend |
| qemu-virt | Desktop oben, 0 `[EXC]` |
| Hosttests | **1380/0/1** (+2: `FpContext`-Layout = asm-Offsets; `parent_fp`/`saved_fp` disjunkt) |
| SDK-Hosttests / Bau / Clippy / Driftprobe | 20/0/0 · 64/94 beide Boards frisch · 76/125 (rpi4, qemu-virt, rpi4+selftest-desk) · 0 |

Belege: `measurements/measurement-2026-10-17-arch02/RUNINFO.TXT`.

## Offen

* Lauf auf der echten Pi 4 (Sonde aus `C:\AUTO\`).
* Nicht von der Sonde gefahren, nur gelesen: Kind endet durch Absturz, Ctrl-C oder `Ptermres` (derselbe Arm von `finish_terminate`).
* Bekannt und ausserhalb: Upcalls (`etv_term`, `kbdvec`) sichern die FP-Register des unterbrochenen Codes nicht (`docs/api/xbios/22-kbdvbase.md` C3).

## Pi (2026-10-17)

Kern aus `f2c68a31` (sha256 `bff063c5…`) auf echter Pi 4 ohne Rueckschritt im
Alltag (Anwender). Die Sonde `FPPEXEC` selbst ist auf der Pi **nicht**
gefahren — der Nachweis der FP-Register bleibt QEMU.
