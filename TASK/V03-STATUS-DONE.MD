# V03 — STATUS: DONE (Geräteparameter belegt; `DEV_TAB[39]` aus der Planezahl abgeleitet, getestet)

`INTOUT[39]` (TT 4096, Pi 0) ist EmuTOS' eigener Wert für das jeweilige
Geräteprofil; NewTOS leitet ihn jetzt wie EmuTOS aus der Tiefe ab statt ihn
fest zu verdrahten. Bearbeitet 2026-09-21, Branch `desk-rebuild`, Basis HEAD
`194b057`, **nicht committet**. Paket mit V11/V12
(`measurement-2026-09-21-v11-v12/`).

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `vdi/vdi_control.c:208` (`DEV_TAB[39] = get_palette()`); `bios/screen.c:831-869` `get_palette`: VIDEL (`:840-846`) „we return the same values as Atari TOS 4.04": `(mode & VIDEL_COMPAT) \|\| ((mode & VIDEL_BPPMASK) == VIDEL_4BPP)` → 4096, sonst 0; Shifter-Pfade (`:850-866`): 4096, Mono 2, ST-Farbmodi ohne STe 512. `include/biosdefs.h:148,153,156,158` (`VIDEL_COMPAT 0x80`, `BPPMASK 7`, `4BPP 2`, `TRUECOLOR 4`) |
| Gemessen (Soll) | dasselbe EmuTOS-1.4-ROM: TT-Medium (4 Planes) **4096**, Falcon030 VIDEL-Truecolor (16 Planes) **0** — Fälle 33/274, `tosdump/reference/EMUTOS14-VDI-all{,-falcontc}.BIN`, `measurement-2026-09-21-vdi-qemu/cells.txt` |
| NewTOS (vorher) | `tos-api/src/vdi/workstation.rs:30` `const PALETTE_PRECISION: i16 = 0` — richtiger Wert, aber global fest |
| NewTOS (nachher) | `palette_precision(planes: u16) -> i16`: `planes == 4 → 4096`, sonst `0`; `init_wk` und `vq_extnd(0)` rufen sie; Doc mit der Einschränkung, dass das ST-Kompatibilitätsflag (`:843`) durch eine Planezahl nicht darstellbar ist und NewTOS keinen ST-kompatiblen Modus hat |

## Tests

* Unit-Test `vdi::workstation::tests::dev_tab_39_is_4096_for_four_planes_and_0_for_every_other_videl_depth`
  (16→0, 8→0, 4→4096, 2→0, 1→0) — die Harness hat kein zweites Profil
  (`PLANES` ist eine Konstante), daher die Funktion direkt.
* `tests/vdi_reference/harness.rs::substitute` `(1|100, 39)`: Formel von
  „`PLANES > 8 → 0`, sonst 4096" auf die VIDEL-Regel „`PLANES == 4 → 4096`,
  sonst 0" gebracht (bei 16 Planes derselbe Wert; bei 8 Planes vorher
  falsch).
* Byte-Vergleich gegen die Falcon-TC-Referenz: `cases_v_series.rs`
  `v01_v02_v03_case_0033_…`, `v14_v15_case_0274_…` grün.
* `cargo run -p xtask -- test` 841 / 0 / 1. QEMU raspi4b `#Z`-Dumper:
  `DEV_TAB[39] = 0`, `tosdiff` gegen Falcon-TC 0 von 182.

## Offen

Nichts. Keine Hardwareabnahme nötig: Wert gemessen und formelgleich; die Pi
meldet seit 2026-09-09 denselben Wert 0.
