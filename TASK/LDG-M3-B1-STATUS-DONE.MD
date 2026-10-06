# LDG-M3-B1 — Quittung: AES meldet den Tastenzustand des Ereignisses (B1) und setzt die `objc_edit`-Schreibmarke nach `te_just` (O1)

Datum 2026-10-06 · Basis `desk-rebuild`, HEAD `e47bbd2`, Baum vorher sauber
· nicht committet · Belege [measurements/measurement-2026-10-06-aes-evbutton/](../measurements/measurement-2026-10-06-aes-evbutton/RUNINFO.TXT)
· Herkunft der Befunde [TASK/LDG-M3-STATUS-UNFINISHED.MD](LDG-M3-STATUS-UNFINISHED.MD)
(B1, O1; B2 ist **nicht** Teil dieses Auftrags und bleibt offen)

## B1 — `ev_rets` / `mtrans` / `pr_*` wie EmuTOS

EmuTOS meldet in `EV_MX/EV_MY/EV_MB` den Zustand **vor dem letzten
veröffentlichten Tastenwechsel**, sobald seit der letzten Ereignisrückkehr
mehr als ein Wechsel veröffentlicht wurde (`ev_rets`,
`aes/gemevlib.c:42-58`; Buchführung `bchange`, `aes/geminput.c:329-345`).
Ein Klick, dessen Loslassen ins Doppelklickfenster fällt, wird beim
Schließen des Fensters als Druck **und** Loslassen veröffentlicht (`b_delay`,
`:155-171`) → `mtrans == 2` → **`EV_MB` = gedrückt**. NewTOS meldete den
lebenden Zustand (0); WinDom (`evnt.mbut & 1`, `evnt_windom.c:334`) verwarf
dadurch jeden normalen Klick.

Umsetzung (`tos-api/src/aes/input.rs`, `event.rs`):

* `b_click` vergleicht gegen `gl_btrue`, zählt bei offenem Fenster nur
  Eintritte in `gl_bdesired` (`gl_bclick`) und veröffentlicht sonst sofort;
  neues `bchange` (`mtrans++`, `pr_button/pr_mclick/pr_xrat/pr_yrat`,
  `button`, `mclick`); Fensterende `b_expire` (Zählung + zweites
  `bchange(gl_btrue, 1)`), auch für `mchange`s Zwangsablauf (nach dem
  `xrat`-Update, wie die Fork-Reihenfolge).
* `ev_rets()` liefert Aufzeichnung oder Live-Zustand und setzt `mtrans = 0`;
  `fill_rets` nutzt es (`EV_MB` jetzt das AES-`button`, nicht das rohe
  VDI-Byte); `evnt_mouse` überschreibt `EV_MB` mit dem lebenden `button`
  (`gemevlib.c:127`).
* `evnt_multi`-Schnellprüfung prüft zuerst `(mtrans > 1) &&
  downorup(pr_button)` mit `EV_BRET = pr_mclick` (`gemevlib.c:190-194`):
  ein Klick zwischen zwei Aufrufen geht nicht mehr verloren.
* AES-interne Warteschleifen verbrauchen Wechsel an der Stelle, an der ihr
  EmuTOS-Gegenstück `ev_*` zurückkehrt: `gr_stilldn`-Schleifen (`graf.rs`,
  `window.rs` watchbox/Mover/Sizer/Elevator), Control Manager (Druck auf
  Rahmen-Gadget/nicht oberstes Fenster), `fm_do`/`form_alert`/fsel (Taste,
  Druck), `ev_button`-Hochwarten (`wait_up`, `wait_button_up`), Ende der
  Menüverfolgung, Pop-up-Druck. Sonst meldete das nächste Ereignis einen
  Klick, den die AES selbst verarbeitet hat (Wächtertest belegt das).

Abweichung im Aufbau, gleiches Ergebnis: unser Fenster öffnet `count_clicks`
erst *nach* dem Druck; der Druck wird daher sofort mit Zählung 1
veröffentlicht und beim Fensterende nur nachgezählt — dieselben zwei
Wechsel in derselben Reihenfolge.

## O1 — `pxl_rect`

`tos-api/src/aes/object.rs` `pxl_rect`: `gr_just(te_just, te_font,
te_ptmplt, w, h)` (`gemgraf.c:567-589`, Schablone im TEDINFO-Font gemessen,
halbe Ränder aufgerundet) + `ch_pos * gl_wchar` (`gemobed.c:180-190`).

## Spec

* `docs/api/aes/event.md`: neuer Abschnitt „The button state an event
  reports (`mtrans` / `pr_*`) — contract“ + zwei Verifikationskriterien.
* `docs/api/aes/object-form.md` `objc_edit`: „Where the caret is drawn —
  `pxl_rect`“.

## Prüfungen

| Prüfung | Baseline | Jetzt |
|---|---|---|
| Hosttests | 1165/0/1 | **1172/0/1** (+7, `tos-api/tests/aes_ev_rets_state_at_event.rs`) |
| neue Tests gegen HEAD | — | 5/7 rot (`checks/red-before-HEAD-e47bbd2.txt`); 2 Wächter (auch an HEAD grün; der graf-Wächter wird rot, wenn der `ev_rets`-Aufruf in `graf.rs` fehlt) |
| Release-Bau rpi4 / qemu-virt (tos-api/kernel) | 64/94 | 64/94 |
| Clippy rpi4 / virt | 76/125 | 76/125 |
| Drift | 31 | 31 |
| SDK-Hosttests | 15/0/0 | 15/0/0 |

## QEMU raspi4b (serial=on, memprot on; Abnahme auch strict)

* **Abnahme** (`LDGDEMO.NAP`, Schritte wie `m3c`, Schritt 3 neu):
  `acc-on-S05_SHORTCLICK.png` — kurzer Klick (30 ms) auf *Compute* öffnet
  jetzt „Example of math library!!!“ (M3: nichts). Schreibmarke im
  Wert-Feld direkt hinter dem Text (`acc-on-S06_VALUE_TYPED.png`: „123|“),
  18 px rechts der M3-Lage, einziger Pixelunterschied zu M3. on = strict
  (9/9). Client endet rc=0. POPUP-Pfad (B2) nicht berührt.
* **Regression**, je gleiches Skript gegen einen HEAD-Kernel gefahren, alle
  Bilder pixelgleich: EmuDESK (Doppelklick Laufwerk C, Doppelklick Ordner,
  Menü *View → Show as text*), arch03-4-Desktoplauf desk3a (Ordner-
  Doppelklick, Closer, Auswahl, Ctrl-I-Dialog mit Eingabe, Umbenennen,
  Alt-C), TeraDesk (Einfachklick wählt TRASH, Doppelklick öffnet Laufwerk),
  QED (Menüklick *Datei → Neuer Text* öffnet „Namenlos“) — Ausnahme
  `qed-END.png`: nur die Zelle der QED-Textschreibmarke (Blinkphase,
  PLAUSIBEL). APITEST 106/0/5, beide Bilder gleich M3.
  Doppelklicks bleiben Doppelklicks (EmuDESK, TeraDesk).

## Offen

* **B2** (Modul-`malloc` nach Modul-`exit`) unverändert offen — POPUP.NLD.
* `curfld`-Ausdehnung der Schreibmarke (`y-3`, `gl_hchar+6` Zeilen, XOR,
  `gemobed.c:198-216`) nicht nachgebaut — nur ihre Lage.
* `count_clicks` gibt jedem weiteren Klick ein volles Fenster und kehrt beim
  Erreichen der Zielzahl sofort zurück; EmuTOS verlängert das offene Fenster
  um 3 Ticks je Klick und meldet erst an seinem Ende (`geminput.c:122`,
  `:155-171`). Vorbestehend, nicht geändert.
* Echte Pi nicht gefahren (keine HW-Aktionen durch Worker).
* `docs/STATUS.md`-Eintrag steht aus (Lead).
