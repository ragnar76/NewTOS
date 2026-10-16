# PROC06 — `Pexec`-Exitcode zero-extended auf jedem Terminate-Pfad — Quittung

> Stand 2026-10-16 · Basis `main` @ `43ae3553` · Aenderung **uncommitted** im gemeinsamen Baum
> Spec: `docs/api/gemdos/process/4c-pterm.md` („Exit-code matrix“, Host-Testliste), `4b-pexec.md` (Native ABI: UWORD zero-extended, `0..=0xFFFF`)
> Messung: [`measurements/measurement-2026-10-16-proc06/`](../measurements/measurement-2026-10-16-proc06/RUNINFO.TXT)

Scope in einem Satz: Der `Pexec`-Aufrufer (Modi 0/4/6) bekommt den Exitcode
des Kindes auf jedem Terminate-Pfad als `rc & 0xFFFF` zero-extended — wie
EmuTOS `xterm(UWORD rc)` (`bdos/proc.c:601,616`) —, also auch nach Ctrl-C
(65504 statt −32) und Upcall-Ueberlauf (64538 statt −998).

## Was getan wurde

| Datei | Aenderung |
|---|---|
| `kernel/src/arch/aarch64/exceptions.rs` `finish_terminate` | `Pexec`-Zweig: `frame.gpr[0] = pterm::exit_code_for_caller(rc)` statt `rc as i64 as u64`; Doku des gemeinsamen Endes. Kernelstart-Zweig und `handle_pterm`-Maske unveraendert |
| `tos-api/src/bdos/process/pterm.rs` | `exit_code_for_caller(rc) -> u64` = `parent_exit_code(rc)` zero-extended (host-testbar) |
| `tos-api/src/gemdos/console.rs` | `CTRLC_RC` `pub` (fuer den Hosttest), Doku |
| `tos-api/tests/proc06_pexec_exit_code.rs` | neu, 4 Tests (Spec-Testliste 1–3) |
| `sdk/examples/ptermch.c` | neu: Kind, Exitcode per Kommandozeile (`m` −1, `h` 0x8000, `z` Pterm0, `c` Cconin/Ctrl-C) |
| `sdk/examples/ctrlcpar.c` | neu: `Pexec(PTERMCH "c")`, druckt das Ergebnis (wartet auf Taste — nie in `\AUTO\`) |
| `sdk/apps/apitest/apitest.c` | Fall T33 (T33.1–3: 65535 / 32768 / 0) |
| `tools/make-sd-image.sh` | installiert `C:\PTERMCH.NCP`, `C:\CTRLCPAR.NCP` wie `DTACHILD.NCP` |
| `docs/api/gemdos/process/4c-pterm.md` | „Fixed“-Vermerk bei den Divergenzen, Implementierungsnotiz PROC06 mit sichtbaren Effekten |
| `docs/sdk/06-image-and-testing.md`, `sdk/README.md` | die zwei neuen Beispielprogramme gelistet |

## Pruefung

* Hosttests: rot (Kompilierfehler: `exit_code_for_caller` fehlt, `CTRLC_RC` privat) → gruen 4/4; gesamt 1376/0/1 (= 1372 + 4).
* Warnungen 64/94 (beide Boards, 0 Fehler), Clippy 76/125 (rpi4, qemu-virt, rpi4+selftest-desk; 0 Treffer in den beruehrten Dateien), Drift 4, Unsafe 20, emutos-consts unveraendert, SDK 20/0/0, Programme 0 Warnungen, NAPs unveraendert.
* QEMU raspi4b, Ctrl-C-Kind (Spec-Punkt 8): ohne Fix `CTRLCPAR: Pexec rc=-32 [FAIL]`, mit Fix `rc=65504 [OK]` in on/off/strict.
* APITEST mit T33: 109/0/5 in on/off/strict; Kontrolle auf dem Kernel vor PROC06 ebenfalls 109/0/5 (T33 haelt den schon maskierten `Pterm`-Pfad fest, ist dort nicht rot).
* Regression: 39/45 Dumps bytegleich zu `measurement-2026-10-16-t16-desk60`; die 6 APITEST-Dumps unterscheiden sich nur in den Summenziffern (GEMDOS 10→13, TOTAL 106→109). 0 EXC/PANIC.

## Sichtbare Effekte (in Richtung EmuTOS)

* EmuCON `run` eines per Ctrl-C beendeten Kindes: vorher „Cannot run program (error 32).“, jetzt „[program exited]“ (aus `kernel/src/shell.rs` `cmd_run` abgeleitet, nicht gefahren).
* mintlib `spawn*`/`system`: vorher −1 mit `errno` 32, jetzt 65504 (`sdk/libc/overlay/mintlib/spawn.c:475-493`, abgeleitet).
* AES-Shell (Kernelstart) unveraendert — sah schon 65504 (PROC05).

## Offen

* Echter Pi 4; qemu-virt-Boot; desk=kernel-Pfad.
* Spec-Punkte 6 (`Ptermres(n,-2)`) und 7 (absturzendes Kind) nicht als neue Faelle — beide laufen durch den unveraenderten maskierten `handle_pterm`-Arm.
* Upcall-Ueberlauf (−998) nur hostgetestet (gleicher Weg `terminate_and_redirect` → `finish_terminate` wie Ctrl-C).
* `sdk/apps/apitest/APITEST.NCP` im Baum (gitignored Build-Ausgabe) nicht neu gebaut; neue Karten brauchen `build-apitest.sh` + `PTERMCH.NCP`.
* `docs/STATUS.md` nicht angefasst (Auftrag).
