# A96 — STATUS: DONE (2026-09-21) — Pi-Sichtabnahme offen

**DONE bedeutet hier: Implementierung gelandet, host-verifiziert (rot →
gruen) und in QEMU raspi4b vorher/nachher belegt; die Sichtabnahme auf der
echten Pi 4 steht aus (Tester/Lead; nichts geflasht).** Bearbeitet
2026-09-21 zusammen mit A97 (ein Paket „was einen Prozess ueberlebt“),
Branch `desk-rebuild`, Basis HEAD `1c4414a`, nichts committet, kein
`git checkout/stash/reset` (die HEAD-Kopien fuer den Vorher-Kernel per
`git show` nach `measurement-2026-09-21-a96-a97/method/head-1c4414a/`).

## Auftrag

Sweep der AES-Fenstertabelle wie EmuTOS' `sh_ldapp` (`wm_new()`,
`references/emutos/aes/gemshlib.c:676-677`) an der Stelle, an der der
Desktop die Kontrolle zurueckbekommt; Hosttest vor dem Fix (App-Slot
oeffnet Fenster, `Pterm` ohne `wind_close` → nach dem Resume-Pfad kein
Fenster des Slots mehr, `wind_find` sieht es nicht, die Komposition malt
nichts davon); Entscheidung ueber die Desktop-eigenen Fenster per Code und
Zitat; Sweep auch fuer das verschachtelte `Pexec`-Kind, falls dort derselbe
Haken existiert.

## Was EmuTOS tut (verifiziert an der gepinnten Quelle)

* `sh_ldapp` (`gemshlib.c:645-677`): `dos_exec` → `AP_OPEN`-Cleanup
  (`:650-657`) → Autorun-Rueckfall (`:662-666`) → `if (wind_spb.sy_owner ==
  rlr) unsync(&wind_spb)` (`:668-669`) → `if (gl_shgem) wm_new()`
  (`:676-677`).
* `wm_new` (`gemwmlib.c:1875-1893`): `while (ml_ocnt > 0) wm_update
  (END_MCTRL)`, `while (wind_spb.sy_tas > 0) wm_update(END_UPDATE)`, dann
  fuer `wh = 1..NUM_WIN`: `VF_ISOPEN` → `wm_close(wh)`, `VF_INUSE` →
  `wm_delete(wh)`. Handle 0 bleibt. `gl_newdesk` fasst `wm_new` nicht an;
  das tut `wm_init` (`:1300`) am Schleifenanfang von `sh_main`
  (`gemshlib.c:717`), bevor der Desktop neu startet und seinen Baum wieder
  registriert (`desk/deskmain.c:2049`).
* Die Fenster des Desktops sind in EmuTOS ebenfalls weg: der Desktop ist
  ein Shell-Programm, `shel_write` beendet ihn, `sh_main` startet ihn neu
  (`gemshlib.c:600-612,708-731`), und er baut seine Fenster aus dem eigenen
  Kontextspeicher (`cnx_get`, `deskmain.c:2055`) wieder auf.

## Entscheidung per Code

`desktop_resume` (`kernel/src/desk/main.rs`) verlaesst sich darauf, dass die
Desktop-Fenster in der AES-Tabelle **ueberleben**: `redraw_desktop`
(`desk/redraw.rs`) komposiert sie per `form_dial(FMD_FINISH)` +
`win_shwall_nocursor` aus der Tabelle, `save_windows`/`restore_windows` sind
nicht mehr verdrahtet (G2-Kommentar in `desktop_resume`), einen `cnx_get`
gibt es nicht. Also: `wm_new` **ohne** die Fenster der Desktop-Identitaet
(`w.owner == pd::DESKTOP`; ein EL0-Programm hat als `w_owner` seinen
GEMDOS-Slot, `appl::current_identity`). Wenn der Desktop zurueck ist, lebt
kein EL0-Prozess mehr (Single-Tasking) — jeder Nicht-Desktop-Eigentuemer ist
ein totes Programm (das gestartete oder ein `Pexec`-Nachfahre, der keinen
eigenen Sweep hatte). Der Desktop haelt ueber den Start keine
`wind_update`-Locks (`kernel/src/desk/` ruft `wind_update` nirgends), die
Zaehler gehoeren also dem Programm und werden wie in `wm_new` auf 0 gesetzt.

## Was getan wurde

### `tos-api/src/aes/window.rs`

* `pub fn wm_new_keep_desktop()`: `MCTRL_CNT`/`UPDATE_LOCK` auf 0; fuer
  `wh` in `1..NUM_WIN`: Slot in Benutzung und `owner != pd::DESKTOP` →
  `close_window(wh)` falls offen (mit `draw_change`-Repaint und
  `WM_REDRAW`-Fanout wie `wm_close`), dann `wm_delete(wh)`; zuletzt
  `clear_newdesk()`, wenn `NEWDESK_OWNER` ein App-Slot ist (A97).
* `clear_newdesk()` (Baum, Root, Origin, ABI, Owner) — auch von `wm_init`
  benutzt.
* Testhaken `test_window_owner(handle)`, `test_newdesk_tree()`.

### `kernel/src/desk/main.rs`

* `desktop_resume`: `tos_api::aes::window::wm_new_keep_desktop()` als erster
  Schritt, vor `appl_init` — die Reihenfolge von EmuTOS (`sh_ldapp`-Sweep,
  dann `sh_main` → `deskstart` → `appl_init`). Kommentar mit Zitaten und
  der G1/G2-Begruendung.

### Spec

* `docs/api/aes/window-graphics.md` § `wind_new (109)`: neuer Block „After a
  launched application returns, the window table is swept (NewTOS rule,
  A96)“ — EmuTOS-Sequenz, warum die Desktop-Fenster bleiben, der
  beobachtbare Vertrag, der offene Kindpfad.
* `docs/api/aes/shell-loop.md` §8 (`wm_new()`-Punkt): NewTOS-Satz mit
  Verweis.

## Pruefung

| Beleg | Ergebnis |
|---|---|
| `tos-api/tests/aes_a96_zombie_windows.rs` (neu; Prozess-Fixture aus `aes_pd_by_slot.rs`, Pixel-Backend; modelliert `desktop_relinquish` → Programm (`wind_create`/`wind_open`, `wind_update(BEG_UPDATE/BEG_MCTRL)`, `Pterm` ueber `terminate` + `release_slot`) → `close_client_if_open` → `desktop_resume` Schritt fuer Schritt) | **rot** ohne den Sweep-Aufruf (Scratch-Kopie = HEADs `desktop_resume`): 0/2 — `test_window_owner(prog_win) = Some(0)`, `test_newdesk_tree() = <Baum>` (`host-tests/aes_a96_zombie_windows-RED-no-sweep.txt`); **gruen** 2/2 (`…-GREEN.txt`): Handle frei, `wind_find` im Zombie-Rechteck = `DESKWH`, 0 Nicht-Weiss-Pixel im Rechteck, Desktop-Fenster in der Tabelle, gefunden und gezeichnet; App-Baum nach dem Sweep 0 |
| `cargo run -p xtask -- test` | **804 passed / 0 failed / 1 ignored** (Basis 797/0/1: +2 hier, +5 A97), viermal hintereinander — `host-tests/xtask-test-AFTER.log`. Ein frueherer Gesamtlauf hatte den Fall 1 einmal rot („desktop's window must still be painted“): die Pseudo-Palette des Test-Backends war noch die Null-Palette (Stift 0 und 1 derselbe Geraetewert, die Pixelpruefungen damit leer); das Backend seedet die Palette jetzt beim Bau wie `aes_window_content.rs`; die RED-Variante mit erreichter Pixelpruefung (`…-RED-no-sweep-verdict.txt`) zeigt 34725 Nicht-Weiss-Pixel im Zombie-Rechteck ohne Sweep |
| `cargo xtask build --board rpi4` frisch (`cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat`) | rc 0, **tos-api 64 / kernel 94** (Basis 64/94) — `build/xtask-build-rpi4-fresh.log` |
| Clippy rpi4 (`cargo clippy -p kernel --no-default-features --features board-rpi4`) | tos-api 80 / kernel 125 → **80 / 125** (Basis auf einer `git archive HEAD`-Kopie gemessen: `build/clippy-rpi4-baseline-HEAD-1c4414a.txt`); `desk/main.rs` 0 → 0, `window.rs` 11 → 11; `cargo clippy -p tos-api --lib --tests`: 0 Treffer in der neuen Testdatei — `build/clippy-*.txt` |
| qemu-virt | nicht gebaut (Lead-Vorgabe) |

### QEMU raspi4b (`measurement-2026-09-21-a96-a97/`, A87-Methode)

Scratch-Kernel aus einer rsync-Kopie der Crates mit dem H02/A46/A78/A87-
Injector (`method/make_scratch.sh`), Skripte aus `method/*.actions` per
`gen_script.py`; SD-Images `tools/make-sd-image.sh` 64 MiB (`sd-base`) +
`Apps\TERADESK\TERADESK.INF` (`sd-tera`, **ohne** `#Z`) bzw. + `C:\A96ZOMB.NAP`
(`sd-probe`); jeder Lauf auf einer frischen Kopie; 640x480, `-display none`,
QMP-Screendump 0,3 s nach jeder Serial-Marke; keine `*.img` abgelegt. VORHER
= Scratch mit `object.rs`/`window.rs`/`desk/main.rs` von HEAD, NACHHER = Baum.

| Lauf | Ergebnis |
|---|---|
| tera: Kernel-Desktop → APPS (178,104) → TERADESK (202,128) → DESKTOP.NAP (226,152) Doppelklick → TeraDesk → HARD DISC C (195,35) → Fenster → File (70,8) → Quit/Shutdown (130,235) → Dialog → Quit (320,313) → Desktop zurueck | A/D/E/G/H/I **bytegleich** vorher/nachher (TeraDesk schliesst sein Fenster selbst, kein Zombie); H = I; Desktop mit seinen drei Fenstern, gewaehltem DESKTOP.NAP-Icon, gruenem Hintergrund, ohne TeraDesk-Rahmen/-Icons; 0 Exceptions |
| probe: Desktop → A96ZOMB.NAP (94,146) Doppelklick → Sonde oeffnet „A96 ZOMBIE“ (120,80,300,200), `Pterm` nach 3 s ohne `wind_close` → Desktop zurueck | **vorher** `probe-before-C-desk-back.png`: der Rahmen steht ueber dem Desktop (Titel noch lesbar ueber die Blocktabelle des toten Slots, A83); **nachher** `probe-after-C-desk-back.png`: kein Rahmen, C vs A = 1582 px in (59,129)-(131,169) = nur die Icon-Auswahl, C = D; vorher-C vs nachher-C = 6842 px in (73,45)-(566,282) = der Zombie-Rahmen; A und B bytegleich; 0 Exceptions |

`sdk/examples/a96zombie.c` (neu) ist die Sonde; Build `sdk/build.sh`, sha in
`build/a96zomb-nap-sha256.txt`.

## Offen

* Pi-Sichtabnahme (Tester/Lead; nichts geflasht). Kein Commit.
* **Verschachteltes EL0-`Pexec`-Kind:** kein Sweep. Der einzige Haken auf
  dem Pfad ist `aes::release_client(slot)` in `pterm::release_slot`,
  gerufen aus dem `Pterm`-Handler (`exceptions.rs`) auf dem Exception-Stack,
  waehrend der EL0-Elternprozess in einem AES-Wait auf der AES-Faser parken
  kann; `close_window` zeichnet und postet `WM_REDRAW` — dort nicht sicher.
  Vorschlag fuer ein Folgeticket: den Slot als „zu sweepen“ merken und beim
  naechsten AES-Dispatch des Eltern `wm_new`-artig fuer diesen Slot
  ausfuehren. `wm_new_keep_desktop` ist auf „alles ausser Desktop“
  geschnitten; fuer den Kindpfad braeuchte es die Variante „alles dieses
  Slots“.
* **Nebenbefund, unberuehrt:** waehrend das gestartete Programm laeuft,
  stehen die Desktop-Fenster weiter in der Tabelle und werden hinter ihm
  komposiert (`tera-*-E-tera-window.png`: die drei Verzeichnisfenster
  hinter TeraDesk) — G1/G2-Zustand, vorher = nachher.

## Artefakte

`measurement-2026-09-21-a96-a97/` (`RUNINFO.TXT`, `manifest.sha256`,
`method/`, `build/`, `host-tests/`, `screendumps/`, `serial/`). Geaenderte
Dateien im Baum: `tos-api/src/aes/window.rs`, `tos-api/src/aes/object.rs`
(A97), `kernel/src/desk/main.rs`, `tos-api/tests/aes_a96_zombie_windows.rs`
(neu), `sdk/examples/a96zombie.c` (neu), `docs/api/aes/window-graphics.md`,
`docs/api/aes/shell-loop.md`, `docs/STATUS.md`, `TASK/A96.md`, dieses
Ticket.
