# A107 — STATUS: DONE (2026-09-22) — Pi-Sichtabnahme offen

**DONE bedeutet hier: Implementierung gelandet, host-verifiziert (rot →
gruen) und in QEMU raspi4b vorher/nachher belegt; die Sichtabnahme auf der
echten Pi 4 steht aus (Tester/Lead; nichts geflasht).** Bearbeitet
2026-09-22 zusammen mit A109 (ein Paket), Branch `desk-rebuild`, Basis HEAD
`de487fe`, nichts committet, kein `git checkout/stash/reset` (die
HEAD-Kopien der fuenf Produktdateien per `git show` nach
`measurement-2026-09-22-a107-a109/method/head-de487fe/`).

## Auftrag

Der Rest aus A96: ein GEM-Kind, das eine **lebende** EL0-Anwendung per
`Pexec` gestartet hat (TeraDesk → QED), bekommt keinen `wm_new`-Sweep.
Slot beim `Pterm` merken, beim naechsten AES-Dispatch des Eltern
`wm_new`-artig fuer diesen Slot ausfuehren, den Ort mit
Sicherheitsargument belegen.

## Was EmuTOS tut (an der gepinnten Quelle geprueft)

* Diese Form existiert dort nicht: ein GEM-Programm startet ein anderes
  ueber die Shell — `shel_write` beendet den Elternprozess, `sh_ldapp`
  fuehrt das Kind aus und sweept danach (`if (gl_shgem) wm_new();`,
  `gemshlib.c:676-677`), `sh_main` startet den Elternprozess neu
  (`:600-612,708-731`).
* `sh_ldapp` gibt die Bildschirmsperre **nur bei Eigentuemergleichheit**
  frei: `if (wind_spb.sy_owner == rlr) unsync(&wind_spb)` (`:668-669`);
  `wm_new` leert danach beide Zaehler (`gemwmlib.c:1879-1883`), was dort
  gefahrlos ist, weil kein anderer Klient mehr lebt.
* **`gl_mntree` wird beim Programmende nirgends zurueckgesetzt.** Geschrieben
  wird es nur in `mn_bar` (`gemmnlib.c:713,726`) und in `fm_own`s
  Dialog-Parken (`gemfmlib.c:89-90,103`). `mn_cleanup` (`gemmnlib.c:740-760`)
  — aus `ap_exit` (`gemaplib.c:254`) und aus `sh_ldapp`s `AP_OPEN`-Nachsorge
  (`gemshlib.c:652`) — schickt nur `AC_CLOSE` an die Accessories und gibt das
  Submenue-Feld frei; `wm_new`/`wm_init` fassen es nicht an. Der Zeiger bleibt
  stehen, bis das naechste Programm `menu_bar(tree, 1)` ruft (der Desktop tut
  genau das, `deskmain.c:1904`). Mit `gl_mntree` steht auch `gl_mnppd`, der
  **Eigentuemer** der Leiste (`gemmnlib.c:707,712-713`).
* `xterm`: `run = run->p_parent` (`bdos/proc.c:609`) — nach dem `Pterm` des
  Kindes ist der Elternprozess wieder der laufende.

## Der gewaehlte Haken und sein Sicherheitsargument

**Markieren** in `aes::release_client(slot)` — das ist bereits die AES-Seite
der Slot-Freigabe und wird aus `bdos::process::pterm::release_slot` gerufen;
`pterm.rs` musste dafuer **nicht** geaendert werden. **Ausfuehren** am
Anfang von `aes::dispatch`, vor dem Opcode.

* `release_slot` laeuft aus dem `Pterm`-SVC-Handler
  (`kernel/src/arch/aarch64/exceptions.rs`) auf dem **Exception-Stack**, mit
  der Abbildung des sterbenden Prozesses und einer moeglicherweise geparkten
  AES-Faser. Der Sweep **zeichnet** (`wm_close` → `draw_change` →
  Hintergrund ueber dem freigewordenen Rechteck + `WM_REDRAW`-Fanout,
  `gemwmlib.c:1453,1216-1222`) und postet in fremde Warteschlangen — dort
  nicht zulaessig.
* Der Dispatch-Eingang ist derselbe Kontext wie jeder andere AES-Aufruf und
  wie der inline laufende Control Manager (`menu::track`,
  `window::intercept`, beide aus `evnt_multi` auf genau diesem Pfad): bei
  einem EL0-Aufrufer die AES-Faser (`thread::call`), bei einem
  Kernelaufrufer der Bootstack — und in beiden Faellen ist die Faser am
  Eingang **leer** (ein geparkter `G_USERDEF`-Stall kehrt ueber
  `thread::resume` zurueck, nie ueber `dispatch`).
* `RUN` ist dort bereits der Elternprozess, der Repaint und das
  `WM_REDRAW`-Fanout erreichen also den lebenden Klienten — dasselbe, was
  `sh_ldapp`s Sweep fuer das naechste Programm der Shell-Schleife tut.
* Die Marke kann nur vom **ersten** AES-Aufruf nach dem Tod des Kindes
  verbraucht werden, und ein Nachmieter des Slots kann vor seinem eigenen
  ersten AES-Aufruf kein AES-Fenster besitzen — ein recycelter Slot wird
  also nie unter seinem neuen Mieter weggesweept.

## Was gebaut ist

### `tos-api/src/aes/mod.rs`

* `PENDING_SWEEP: AtomicU32` (ein Bit je GEMDOS-Slot), `mark_for_sweep`
  (aus `release_client`), `sweep_dead_clients` (erste Anweisung in
  `dispatch`, mit dem obigen Argument als Doc-Kommentar), Testhaken
  `test_sweep_pending`.

### `tos-api/src/aes/window.rs`

* `pub fn wm_new_for_slot(slot)`: fuer jedes Fenster dieses Eigentuemers
  `close_window` (falls offen) + `wm_delete`; `clear_newdesk()`, falls
  `newdesk_owner() == Some(slot)`; `menu::release_bar_of(slot)`. Die beiden
  `wind_update`-Zaehler bleiben stehen, mit EmuTOS-Zitat im Doc.

### `tos-api/src/aes/menu.rs`

* `MNPPD` (= `gl_mnppd`) in `install_bar` gesetzt, in `remove_bar`
  geloescht; `release_bar_of(slot)` nimmt die Leiste (und einen von
  `fm_own` geparkten Baum) zurueck, wenn dieser Slot sie installiert hat —
  benannte NewTOS-Regel mit dem EL1-Dereferenz-Argument.

### Sonde

* `sdk/examples/a107parent.c` (neu): oeffnet „A107 PARENT" (360,300,260,160),
  `Pexec`t `C:\A96ZOMB.NAP` (das A96-Kind: Fenster (120,80,300,200), 3 s,
  `Pterm` ohne `wind_close`), wartet dann 2,5 s in `evnt_timer` und beendet
  sich sauber. Per `#Z`-Autorunzeile gestartet — der Lauf braucht **keine**
  Maus.

### Spec

* `docs/api/aes/window-graphics.md` § `wind_new (109)`: der A96-Block
  bekommt die A107-Regel (Ort, Sicherheitsargument, Umfang, die
  ausgenommenen Zaehler).
* `docs/api/aes/shell-loop.md` §8: Satz zu `wm_new_for_slot`.
* `docs/api/aes/menu.md`: neuer Block „A dead client's menu bar" unter
  `menu_bar` (was EmuTOS tut = nichts, warum wir abweichen muessen).

## Pruefung

| Beleg | Ergebnis |
|---|---|
| `tos-api/tests/aes_a96_zombie_windows.rs` (+3 Faelle `a107_*`) | an HEAD **3/3 rot** (Kindfenster `owner = Some(1)` in der Tabelle, `WF_NEWDESK`-Baum registriert, Menuebaum installiert), im Baum **5/5 gruen**; die zwei A96-Faelle auf beiden Seiten gruen (`host-tests/aes_a96_zombie_windows-A107-RED-no-sweep.txt`, `…-GREEN-after.txt`) |
| `cargo run -p xtask -- test` | **916 passed / 0 failed / 1 ignored** (Basis 909/0/1; +3 hier, +4 A109) |
| Build beide Boards, frisch | tos-api **64** / kernel **94**, rc 0 — Delta 0 |
| Clippy rpi4 (Basis auf der Scratch-Kopie mit HEAD-Dateien gemessen) | tos-api **80** / kernel **125** → unveraendert; `window.rs` 11→11, `menu.rs` 10→10, `aes/mod.rs` 54→54 |

### QEMU raspi4b (`measurement-2026-09-22-a107-a109/`, A108-Methode)

Sondenlauf `probe` (`sd-probe` = `sd-base` + `C:\A107PAR.NAP` +
`C:\A96ZOMB.NAP` + `#Z` fuer den Elternprozess), 640x480, `-display none`,
QMP-Screendump 0,3 s nach jeder Serial-Marke, frische Imagekopie je Lauf,
0 `EXC`/`PANIC`:

| Marke | Ergebnis |
|---|---|
| A (nur das Elternfenster), B (Kindfenster darueber) | **0 px** Unterschied — solange beide leben, aendert der Sweep nichts |
| C (nach dem `Pterm` des Kindes und dem naechsten AES-Aufruf des Eltern), D (spaeter in derselben Wartezeit) | **6779 px**, Bbox (120,80)-(617,319) = Kindfenster + Schatten. **Vorher:** der Rahmen „A96 ZOMBIE" steht fuer den Rest des Elternlebens und behaelt den Top-Platz (das Elternfenster ist untopped gezeichnet). **Nachher:** Rahmen weg, Hintergrund von `draw_change` neu gemalt, „A107 PARENT" wieder oben (Stipple im Titel) |
| E (Elternprozess beendet, Kernel-Desktop zurueck) | 912 px = nur die Infozeile (A109.3) |

TeraDesk-Kontrolllauf (3 Marken): **0 px** auf allen dreien.

## Offen / Rest

* **Pi-Sichtabnahme** (Tester/Lead; nichts geflasht). Kein Commit.
* **Die beiden `wind_update`-Zaehler** werden fuer das tote Kind **nicht**
  zurueckgesetzt: der lebende Elternprozess kann sie halten, EmuTOS gibt die
  Sperre nur bei Eigentuemergleichheit frei (`gemshlib.c:668-669`) und unsere
  Zaehler tragen keinen Eigentuemer. Ein Kind, das mit `BEG_UPDATE` /
  `BEG_MCTRL` stirbt, laesst den Stand stehen. Eigener Task, falls es je
  auffaellt (`UPDATE_LOCK`-Doc nennt die Stelle).
* Die Menuezeilen-**Pixel** raeumt der Sweep nicht auf (nur den Zustand) —
  wie `mn_bar(…, FALSE)`s Zustandshaelfte; das Bild gehoert dem, der als
  naechstes malt (TeraDesks `regen_desktop`).

## Artefakte

`measurement-2026-09-22-a107-a109/` (`RUNINFO.TXT`, `manifest.sha256`,
`method/`, `build/`, `host-tests/`, `screendumps/`, `serial/`; keine
`*.img`). Geaenderte Dateien im Baum: `tos-api/src/aes/{mod,window,menu}.rs`,
`tos-api/tests/aes_a96_zombie_windows.rs`, `sdk/examples/a107parent.c` (neu),
`docs/api/aes/{window-graphics,shell-loop,menu}.md`, `docs/STATUS.md`,
`TASK/A107.md`, dieses Ticket.
