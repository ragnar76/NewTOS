# PROC05 — Exitcode im Shell-Start — Quittung

> Stand 2026-10-14 · Basis `main` @ `7708a653` · Aenderung **uncommitted** im gemeinsamen Baum
> Umgesetzt als DESK43 **U9-0b** (gemeinsame Quittung: [DESK43-U9-0-STATUS-DONE.MD](DESK43-U9-0-STATUS-DONE.MD))
> Grundlage: `docs/analysis/desk43-u9-prep-2026-10-14.md` §2, Ticket-Korrektur 2026-10-14 in [PROC05.md](PROC05.md)
> Messung: [`measurements/measurement-2026-10-14-desk43-u9-0/`](../measurements/measurement-2026-10-14-desk43-u9-0/RUNINFO.TXT)

Scope in einem Satz: Der Kernel-Programmstart (`launch::run_program_with_tail_rc`)
liefert `dos_exec`s Wert wie EmuTOS — Kind-Exitcode als UWORD zero-extended
`0 … 65535`, Ladefehler negativ wie bisher — statt immer `E_OK`; kein Alert fuer
irgendeinen Exitcode.

## Was getan wurde

| Datei | Aenderung |
|---|---|
| `kernel/src/arch/aarch64/exceptions.rs` `finish_terminate` | Kernelstart-Zweig (weder `pexec_caller` noch `full`): `crate::launch::record_kernel_child_exit(rc)` — vor `release_slot`, wie die Prep §2.4 verlangt |
| `kernel/src/launch.rs` | `KERNEL_CHILD_RC` + `record_kernel_child_exit` (zero-extended); `run_program_with_tail_rc` gibt den Wert zurueck; `run_program_with_tail` = „Kind lief“ (`rc >= 0`; einziger Aufrufer `run_auto_folder` ignoriert das Ergebnis) |
| `tos-api/src/bdos/process/pterm.rs` | `parent_exit_code(rc)` = `xterm(UWORD rc)` (`bdos/proc.c:601,616`) |
| `tos-api/src/aes/shell.rs` | `is_bios_error`, `sh_main_alert`, `sh_ldapp_result` — die `gemshlib.c:681-684,722-723`-Entscheidungen, wortgleich aus `shell_loop.rs` verschoben (Host-testbar) |
| `kernel/src/shell_loop.rs` | benutzt die drei; Logzeile `exited with rc=N` zeigt den echten Wert |
| `docs/api/aes/shell-loop.md` §8 | Abschnitt „NewTOS implementation notes — the child's exit code (PROC05…)“ (Prep §2.3) |
| `tos-api/tests/proc05_shell_exit_code.rs` | 4 Hosttests: Zero-Extension (−1→65535, 0x12345→0x2345, −32→65504), kein Alert fuer jeden Exitcode, Ladefehler-Tabelle −33…−66 → `fm_error` 2/3/4/5/8/15/35, BIOS-Bereich ohne Alert |
| `programs/newtos-gem/src/lib.rs` | `PANIC_EXIT`-Doku: „rc=65535“ statt „rc=-1“ (gleiche Zeilenzahl; NAP byte-gleich) |

**EmuTOS-Treue der Arme:** NORMAL/AUTORUN gibt fuer `ret >= 0` immer 0 zurueck
(`:681-684`) — kein Alert; DESKTOP-Arm unveraendert: der eingebaute Desktop ist
ein Kernelaufruf ohne Exitcode, `wm_new()` bei `rc != 0` (`:606-609`) wird erst
mit dem EL0-Desktop (U9-4) erreichbar.

## Pruefung

* Hosttests 1366/0/1 (= 1360 + 6), Warnungen 64/94, Clippy 76/125 — RUNINFO §1.
* QEMU raspi4b: `HGPANIC` (#Z on/off/strict, F1 aus dem Desktop): `exited with rc=65535`,
  kein Alert, Bildschirm = sauberer Desktop; `HELLOGEM`: `rc=0`, Screendumps 8/8 = U8b —
  RUNINFO §5. Alle uebrigen U8b-Laeufe byte-gleich — RUNINFO §2-4, §7.

## Offen

* `Pterm(1)`, `Pterm(0x12345)` und ein `#Z` auf Nicht-ELF (`EPLFMT` → „TOS error #35“)
  nur im Hosttest, nicht in QEMU (kein passendes Testprogramm auf den Karten).
* Ausserhalb: ein `Pexec`-Aufrufer bekommt nach Ctrl-C weiterhin `-32` sign-extended
  (`frame.gpr[0] = rc as i64`), EmuTOS liefert 65504 — nicht angefasst.
