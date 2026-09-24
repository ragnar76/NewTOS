# DESK22 — STATUS: DONE (2026-09-24)

**Auftrag:** `launch_native` auf denselben Weg legen, den DESK06 gebaut hat
(`sh_toalpha`/`sh_tographic` ueber `shell_loop`), statt den Schirm nur zu
loeschen. Screendump vor/nach.

Basis: Branch `desk-rebuild`, HEAD `3f6b6ef`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset`.

Messung:
[`measurement-2026-09-24-desk22-desk23-desk28-desk29/`](../measurement-2026-09-24-desk22-desk23-desk28-desk29/)
§3. Spezifikation: `docs/api/aes/shell-loop.md` §9.

---

## 1. Der Defekt

`main::launch_native`s `ftype::Flavor::Console`-Arm rief
`crate::fbcon::clear_and_home()`, lud das Programm und zeichnete danach den
Desktop neu. Auf dem Weg steht keine Modusumschaltung, also auch kein
`ratexit()` — der Mauszeiger blieb als schwarzer Fleck im Textschirm stehen,
an der Stelle, an der der Doppelklick ihn zurueckgelassen hatte.

Beleg vorher, neu aufgenommen:
`screendumps/d22-before-C-program.png`, Fleck bei (179,187) — dem Klickpunkt
von `HELLO.NCP` auf dieser Karte (`serial/probe-serial.log`, `[probe] obj 25
'HELLO.NCP' … click (179,187)`).

## 2. Der Fix

EmuTOS' Weg fuer ein zeichenorientiertes Programm ist
`pro_run(isgraf = FALSE, …)` → `shel_write(SHW_EXEC, 0, 1, …)` (setzt
`sh_isgem = FALSE`, `aes/gemshlib.c:173`) → `deskmain` kehrt zurueck →
`sh_main`s naechster Durchlauf ruft `sh_chgrf` (`:513-523`), das wegen des
Moduswechsels `sh_toalpha` (`:246-260`) ausfuehrt: Pfeilform, Fehlerbehandler
zurueck, `ratexit()` **nimmt den Zeiger weg**, Workstation zu. Erst dann
laedt `sh_ldapp`. Nach dem `Pterm` findet der naechste Durchlauf `sh_isgem`
wieder wahr und schaltet mit `sh_tographic` (`:223-239`) zurueck.

NewTOS' eingebauter Desktop kann nicht nach `sh_main` zurueckkehren (AES-Luecke
G1), also klammert er den Start selbst:

* neu `crate::shell_loop::sh_alpha_enter` / `sh_alpha_leave` — jeweils
  `shell::set_sh_isgem(…)` wie es `shel_write` taete, dann das **echte**
  `sh_chgrf`. Dadurch bleibt `gl_shgem` der Zustand, gegen den der naechste
  Vergleich laeuft; `sh_toalpha`/`sh_tographic` werden nicht am AES vorbei
  gerufen.
* `main::launch_native` klammert damit den Lauf, in `sh_main`s Reihenfolge:
  Umschaltung → Programm → `console_pause_for_key` → Ruecksschaltung →
  Warnung bei fehlgeschlagenem Start (EmuTOS' `fm_error`, `:722-723`, laeuft
  ebenfalls **nach** der Umschaltung) → `redraw_desktop` →
  `pro::desk_busy_off` (`deskmain.c:2071`), das die Sanduhr von
  `sh_tographic` zuruecknimmt — dieselbe Stelle wie nach dem eingebetteten
  EmuCON-Durchlauf in `desk::run`s Schleife.

Das Loeschen des Schirms geht nicht verloren: `sh_toalpha` endet in unserer
Fassung mit `fbcon::clear_and_home()`.

## 3. Beleg

Gleiche Karte, gleiche Klickfolge (`method/d22.actions`), zwei Kerne:

| | Textschirm waehrend `console_pause_for_key` |
|---|---|
| vorher (HEAD) | `d22-before-C-program.png` — schwarzer Fleck bei (179,187) |
| nachher | `d22-after-C-program.png` — zeigerfrei |

`d22-before-D-after.png` und `d22-after-D-after.png` (der zurueckgekehrte
Desktop) sind gleich — die Umschaltung kostet auf dem Rueckweg nichts.
0 `[EXC]`/`[PANIC]` in beiden Protokollen.

## 4. Pruefung

* `cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat`
  vor **jedem** Board.
* `cargo xtask build --board rpi4` → tos-api 64 / kernel 94 (Basis 64/94).
* `cargo xtask build --board qemu-virt` → tos-api 64 / kernel 94 (Basis 64/94).
* `cargo clippy --features board-rpi4` → 80 / 125 (Basis 80/125), 0 Treffer
  `desk/`.
* `cargo clippy --features board-qemu-virt` → 80 / 125 (Basis 80/125), 0
  Treffer `desk/`.
* `cargo run -p xtask -- test` → 938 passed / 0 failed / 1 ignored (Basis
  938/0/1). Kein Hosttest hinzugefuegt (siehe DESK28-Quittung §4).
* `fsck_msdos -n` sauber auf allen beschriebenen Abbildern.

## 5. Offen

* Hardwareabnahme: Punkt **56** in
  `docs/api/desktop/_hw-test-2026-09-17.md`.
* Nicht gefahren: ein `.NCP`, das **nicht** geladen werden kann — der
  `alert_launch_failed`-Zweig, der jetzt erst im Grafikmodus warnt.
