# H02 — STATUS: DONE (Ursache festgestellt, kein Fix)

Die Markierung faellt nicht durch die Menuetafel und nicht durch ein `WM_REDRAW`,
sondern weil NewTOS der Anwendung fuer den **Druck, der den Menueeintrag
auswaehlt**, zusaetzlich zum `MN_SELECTED` ein **`MU_BUTTON`** liefert — im selben
`evnt_multi`-Durchlauf, mit den Bildschirmkoordinaten des Eintrags. TeraDesk
behandelt dieses `MU_BUTTON` **vor** der Nachricht als Klick auf den Schirm und
hebt dabei die Auswahl auf; das anschliessende *Info* findet nichts mehr und zeigt
das Laufwerk. EmuTOS liefert diesen Klick nie: der Kontrollmanager besitzt die Maus
waehrend `mn_do`, der Druck wechselt den Besitzer nicht, und die Wartung der App
bleibt unbefriedigt. Bearbeitet 2026-09-17, Branch `desk-rebuild`, Basis HEAD
`f79dfd2`. Messlauf: [`measurement-2026-09-17-h02/`](../measurement-2026-09-17-h02/RUNINFO.TXT).
**Kein Produktcode geaendert. Keine Regression von A65.**

## 1. Reproduziert in QEMU — ja

QEMU `raspi4b` hat keinen Eingabeweg; gemessen wurde nach der H03-Methode mit einer
Scratch-Kopie der Crates, einem IKBD-Injektor in `Host::pump_input` und einem Trace,
der **nur** auf die serielle Leitung schreibt (`method/scratch-*.diff`, je Datei).
Karte: `tools/make-sd-image.sh` + `#Z`-Autorun TeraDesk (`DESKTOP.NAP` `db31aae1…`).

Ablauf (`method/run3.actions`): `HARD DISC` doppelklicken → Fenster `C:\*.*` →
`HELLO TXT` anklicken (220,340) → **A** → *File* hovern (88,8) → **B** → auf
*Info…* fahren (150,43) → **B2** → Druck → **C0** (1 Tick spaeter) → Loslassen →
**C1** → **C2** (0,5 s) → **D** (3 s). (150,43) liegt unter der Tafel auf leerem
Desktop-Hintergrund — die Konstellation des Tickets. `run2` klickt bei (110,43), wo
unter der Tafel das Desktop-Symbol *FLOPPY B* liegt: dort wird stattdessen *FLOPPY B*
selektiert und „Drive does not exist." gemeldet — derselbe Mechanismus, anderes Ziel.

**Pixel, nicht Deutung** (`method/rowstat.py`, Schwarzanteil der Zeile `HELLO TXT`,
x 175..470, y 333..347; `screendumps/run3-*.png`):

| A | B | B2 | C0 | C1 | C2 | D |
|---|---|---|---|---|---|---|
| 0.841 | 0.843 | 0.843 | 0.843 | 0.843 | **0.000** | 0.000 |

- **B/B2:** Tafel offen (*Info…* invertiert, Tafelflaeche 0.496), Zeile **steht**.
- **C0/C1:** Tafel bereits zurueckgeschrieben (Tafelflaeche wieder 0.113 = A-Niveau),
  *File*-Titel noch invertiert, Zeile **steht noch**, Infozeile noch
  „32 bytes used in 1 selected item." → **Spur 1 (Tafel schreibt die Markierung weg)
  ist widerlegt**: das Zurueckschreiben laesst die Markierung stehen.
- **C2:** Zeile weiss, Infozeile „373687 bytes used in 24 items." → die Markierung
  faellt zwischen +3807 und +3900 Ticks, **nach** dem Tafelschluss.
- **D:** Dialog „Disk Volume Information / Disk drive: C" — das Ticketbild.

## 2. Was TeraDesk bekommt und damit tut

Trace `serial/run3-info-click-desktop-bg-serial.log:107-149` (Auszug):

```
[h02] b_click DOWN at (150,43) chk_ctrl=0 -> owner_app=true
[h02] track press edge at (150,43) title=4 box=18 item=20 sub=0
[h02] track box restored, returning (4,20)
[h02] post_msg pid=1 [10, 1, 0, 4, 20, 0, 0, 0] posted=true         MN_SELECTED
[h02] b_click new=0 prev=1 owner_app=true                           Loslassen
[h02] evnt_multi MU_BUTTON fired own=true pass=… clicks=1 mouse=(150,43) mb_now=0
[h02] evnt_multi MU_MESAG deliver [10, 1, 0, 4, 20, 0, 0, 0]
[aes] op 25 in=(19,258,3,0) out=(18,150,43,0,0,0,1)                 evnt_multi → 18 = MU_BUTTON|MU_MESAG
[aes] op 106 in=(150,43,…) out=(0,…)                                wind_find → Desktop
[aes] op 107 (1) · op 78 (256) · op 104 (1,11)→(160,96,320,254) · op 104 (1,12)→(0,0,0,0) · op 78 (257) · op 107 (0)
[aes] op 105 in=(1,3,…)                                             wind_set(1, WF_INFO)
[aes] … op 54 ×2 · op 42                                            form_center, objc_draw: der Dialog
```

TeraDesk (Single-TOS-Bau) nimmt das Ergebnis in `xe_xmulti`
(`sdk/apps/teradesk/library/xdialog/xdevent.c:146-235`): nach `evnt_multi` wird
**zuerst** `MU_BUTTON` bearbeitet (`:213-222`, `xw_hndlbutton(x=150, y=43, …)`),
`MU_MESAG` bleibt gesetzt und kommt **danach** in `evntloop` an
(`main.c:1639-1642` → `hndlmenu`). `xw_hndlbutton` (`xwindow.c:783-800`) sucht das
Fenster unter (150,43) — den Desktop — und ruft `wd_hndlbutton`
(`window.c:4372`): `selection.w != w` → **`desel_old()`** (`:4398-4399` →
`itm_select(selection.w, -1, 0, TRUE)`, `:1292-1296`) — das ist die Neuzeichnung
`wind_update/graf_mouse/WF_FIRSTXYWH…` im Trace und die weisse Zeile in C2; dann
`wd_setselection` → `selection.w = NULL`, `wind_set(WF_INFO)` mit der Zaehlung
„24 items". Laege der Eintrag ueber dem Verzeichnisfenster (wie bei einem gross
gezogenen Fenster auf der Pi), nimmt `wd_hndlbutton` den anderen Zweig:
`in_window` → `itm_select(w, -1, 0, TRUE)` (`:4479-4480`) — gleiche Wirkung.

Erst dann `hndlmenu(4, MSHOWINF)` → `wd_hndlmenu` (`window.c:1353` `w = selection.w`
= NULL) → `case MSHOWINF` (`:1425-1446`): „Nothing is selected, info on the disk
of the top window", `thepath[3] = 0` → `item_showinfo` fuer `C:\` → der Dialog.
**Wer die Auswahl aufhebt: TeraDesk selbst, auf das `MU_BUTTON` hin.** Kein
`WM_REDRAW` im Spiel (kein `post_msg` mit 20 im Trace; Spur 2 widerlegt), kein
Aufheben im `MN_SELECTED`-Behandler (Spur 3 widerlegt).

## 3. Was EmuTOS an derselben Stelle liefert

| Schritt | EmuTOS |
|---|---|
| Zeiger betritt die Menueleiste (Taste oben) → Maus gehoert dem Kontrollmanager | `aes/geminput.c:388-389` (`mchange`: `gl_mowner = ctl_pd`) |
| Kontrollmanager wacht (`MU_M1`), `hctl_rect` → `mn_do` | `aes/gemctrl.c:427-467`, `:318-371` |
| `mn_do` haelt die Maus fest: `ct_mouse(TRUE)` → `gl_ctmown = TRUE` | `aes/gemmnlib.c:277`; `aes/gemctrl.c:400-408` |
| Hover-Modus: im `INITEM_STATE` wird bei **oben** stehender Taste auf den **Druck** gewartet (`buparm = 0x00010101`) | `aes/gemmnlib.c:314` |
| Der Druck: `bchange` wechselt den Besitzer **nicht** (`!gl_ctmown`), `post_button(gl_mowner=ctl_pd,…)` bedient nur den Besitzer | `aes/geminput.c:334-344`, `:463-483` |
| `mn_do` bricht auf `MU_BUTTON` ab (`menu_state != INTITLE_STATE`), stellt die Tafel wieder her, liefert `(title,item)`, `ct_mouse(FALSE)` | `aes/gemmnlib.c:373-374`, `:490-513` |
| `hctl_rect` → `ct_msgup(MN_SELECTED, gl_mnppd, title, item, …)` | `aes/gemctrl.c:353-367` |
| Die App (blockiert in `ev_multi`) wacht nur fuer `MU_MESAG`; ihre `MU_BUTTON`-Wartung liegt in **ihrer** `c_bsleep`-Liste, die `post_button` nie besucht hat; `acancel` verwirft sie; Rueckgabe = `p_evflg` = `MU_MESAG` allein | `aes/gemevlib.c:231-265`, `aes/geminput.c:545-563` |
| Das Loslassen geht ebenfalls nur an `ctl_pd` (`w_setactive`→`set_mown` ist bei gedrueckter Taste ein No-op, `geminput.c:220-234`), dessen Wartung `0x0001ff01` (Druck) es nicht befriedigt | `aes/gemctrl.c:441-455`, `aes/gemwmlib.c:321-334` |
| Auch ein spaeterer Quick-Check der App ist gesperrt, solange `gl_mowner == ctl_pd` | `aes/gemevlib.c:185` |

**Ergebnis in EmuTOS: die App erhaelt fuer diesen Klick genau ein `MN_SELECTED`
und kein `MU_BUTTON`.** Der Klick ist vom Kontrollmanager verbraucht.

Einzige Ausnahme, die EmuTOS selbst kennt: haelt der Anwender die Taste noch
gedrueckt, wenn die App nach `MN_SELECTED` erneut `ev_multi` ruft, testet `abutton`
den Pegel einmal unbedingt (`geminput.c:549-552`). Das ist der lange Druck, nicht der
normale Klick — und nicht der gemessene Fall (Loslassen 30 ms nach dem Druck,
`MU_BUTTON` trotzdem im **selben** Durchlauf wie die Nachricht).

## 4. Die Abweichung in NewTOS

Zwei Stellen, dieselbe Ursache — der Kontrollmanager haelt bei uns die Maus
**nicht** ueber die Menueverfolgung:

1. **`tos-api/src/aes/input.rs:90-101` (`b_click`)** wendet `chkown()` auf jeden
   0→Druck an, **ohne** EmuTOS' Klammer `!gl_ctmown` (`geminput.c:334`). Der
   Kommentar `:87-89` begruendet das damit, „our menu tracker runs synchronously
   inside the trap, so no event loop can observe the interval" — das ist die
   falsche Annahme: `b_click` **laeuft in genau diesem Intervall**, aus
   `menu::track`s `h.pump_input()` (`menu.rs:365`) heraus, und schreibt
   `GL_MOWNER_APP = chk_ctrl(150,43) >= 0` = **true** (Trace: `chk_ctrl=0 ->
   owner_app=true`), waehrend EmuTOS den Besitzer bei `ctl_pd` laesst.
2. **`tos-api/src/aes/event.rs:475-520` (`evnt_multi`)**: nach
   `menu_intercept(pid) == true` (`:475`, Auswahl gepostet) laeuft **derselbe
   Durchlauf** weiter in den `MU_BUTTON`-Zweig (`:501-520`); mit `own == true`
   erfuellt `downorup(mb=1, 0x0102, 3, 0)` TeraDesks Wartung auf einen Druck, und
   der vom Menue verbrauchte Druck wird als Klick geliefert (`fired |= MU_BUTTON`,
   `:520`), Koordinaten = Eintrag, `EV_BRET = 1`.

Sekundaer, nicht ursaechlich: `event.rs:462` ruft `input::w_setactive()` in
**jedem** Poll-Durchlauf der App, also auch unmittelbar nach `mchange`s Uebergabe
an `ctl_pd` und **vor** `menu_intercept` (Trace: `mchange (88,8) -> owner ctl_pd`,
direkt danach `w_setactive -> owner app`). Bei EmuTOS laeuft `w_setactive` nur am
Kopf der Kontrollmanager-Schleife, die waehrend des Hovers blockiert ist
(`gemctrl.c:441`). Es aendert am gemessenen Ausgang nichts (Punkt 1 setzt den
Besitzer beim Druck ohnehin), gehoert aber zum selben Modell.

Spec-Stand: `docs/api/aes/event.md:169-173` nennt die Quick-Check-Sperre
`rlr == gl_mowner`, aber weder `event.md` noch `menu.md` tragen die Zusage, dass
der Kontrollmanager die Maus ueber `mn_do` haelt (`gl_ctmown`, `bchange`-Klammer)
und der auswaehlende Klick die App **nie** als `MU_BUTTON` erreicht.
`menu.md:572-583` beschreibt zudem nur den Halte-Modus („while the button is held
… On button release"); `gemmnlib.c:314` waehlt je nach Tastenlage Druck **oder**
Loslassen. Spec-Luecke → Researcher.

## 5. Regressionsfrage: nein

- `run4` = `run3` auf dem Baum **`52a969b`** (Stand vor A65/A63/A64/T01; `object.rs`
  `63dce27b…` statt `e067ded9…`), mit denselben Instrumenten (die sechs
  instrumentierten Dateien sind zwischen `52a969b` und HEAD unveraendert, `git diff
  --quiet`): **identischer Trace** (`serial/run4-preA65-52a969b-info-click-serial.log:107-129`
  = `run3:107-129` Zeile fuer Zeile), **identische Bildpunktwerte** (0.843 → 0.000
  zwischen C1 und C2), identischer Dialog (`screendumps/run4-preA65-D.png`).
- `git diff --stat 52a969b HEAD -- kernel tos-api tos-types` beruehrt nur
  `fsel.rs`, `object.rs` und den A65-Test; `event.rs`/`input.rs`/`menu.rs` wurden
  zuletzt am 2026-09-01/03 geaendert (`e1d2436`, `1a8f147`, `49c3938`).
- Der Kartenstand `scratchpad/karte-vor-2026-09-17b/` aus dem Ticket **existiert
  nirgends** (weder im Projekt noch in den Job-Scratchpads); der Vergleich lief
  deshalb ueber den rekonstruierten Quellstand `52a969b` plus dem heutigen
  `DESKTOP.NAP` (`db31aae1…`; der Unterschied zu `450ad5c0…`/`d957d765…` liegt in
  `_DISKINFO`/`a_evnt_timer.c`, nicht im Ereignis-/Menueweg). A65 ist unschuldig.

## 6. Ursache in einem Satz — und die Schicht

**NewTOS' AES gibt beim Druck auf einen Menueeintrag den Mausbesitz an die
Anwendung (`input.rs:100`, ohne EmuTOS' `gl_ctmown`-Klammer) und liefert ihr daraufhin
im selben `evnt_multi`-Durchlauf (`event.rs:501-520`) den vom Menue verbrauchten
Druck als `MU_BUTTON` neben dem `MN_SELECTED`; TeraDesk deutet ihn als Klick auf den
Schirm und hebt die Auswahl auf, bevor es *Info* ausfuehrt.** Schicht: **AES
Ereignis-/Eingabemanager in `tos-api`** (`aes/input.rs`, `aes/event.rs` — das
Modell des Kontrollmanager-Mausbesitzes ueber `menu::track`). Kein TeraDesk-, kein
gemlib-, kein `desk/`-, kein `object.rs`-Defekt. Kein Fix in diesem Auftrag.

## Was gefahren wurde / was offen bleibt

Gefahren: Auswahl → Tafel → Druck auf den Eintrag ueber leerem Desktop (run3) und
ueber einem Desktop-Symbol (run2); Bildfolge A–D mit Pixelwerten; Ereignis-Trace;
Gegenprobe auf `52a969b`. Nicht gefahren: Eintrag ueber dem Verzeichnisfenster
(Zweig `itm_select(w,-1)` statt `desel_old()` — aus der Quelle belegt, nicht
gemessen), echte USB-Maus auf der Pi (Zeit zwischen Druck und Loslassen ist dort
menschlich; `count_clicks` wartet das Loslassen ab, die Reihenfolge bleibt), langer
Druck (EmuTOS' `abutton`-Ausnahme). Hardware-Nachmessung nach einem Fix: Datei
markieren, *File → Info*, Markierung muss stehen bleiben und der Dateidialog kommen.

## Beobachtungen ausserhalb des Auftrags — nicht verfolgt

- `w_setactive()` je App-Poll-Durchlauf (`event.rs:462`) statt je Kontrollmanager-
  Wakeup — siehe Punkt 4, sekundaer.
- Der H03-Injektor-Diff (`measurement-2026-09-17-h03/method/`) schreibt seinen Trace
  ueber `console_write_bytes` auf die fbcon; die hier benutzte Fassung geht ueber eine
  zusaetzliche `Host::debug_write` nur auf die PL011 — falls der Injektor
  wiederverwendet wird, diese Fassung nehmen.

## Aufraeumen

Projektbaum: `git status --short` zeigt nur `measurement-2026-09-17-h02/` und diese
Quittung als neu; `kernel/src/host_env.rs` sha256 `287ff741…` vor und nach der
Messung. Injektor und Trace existieren nur in der Scratch-Kopie
(`…/scratchpad/h02/ws`, `…/ws-old`); `method/scratch-*.diff` dokumentieren sie.
Keine QEMU-Prozesse, keine QMP-Sockets, kein gemountetes Scratch-Image, Karte nicht
angefasst, nichts geflasht, nichts committet.

## Verifikation (Tester, 2026-09-17)

Unabhaengig nachgemessen, Fix lag uncommitted in `tos-api/src/aes/{event,input,menu}.rs`
(+`tos-api/tests/aes_menu_click_ownership.rs`), Basis HEAD `2d41261`. Dem
Developer-Selbstreport oben ("kein Fix in diesem Auftrag") NICHT getraut --
der Fix war da, wurde real gebaut und in QEMU gefahren. Volle Belege, Hashes,
Trace-Auszuege und Screendumps: [`measurement-2026-09-17-h02/verify/RUNINFO.TXT`](../measurement-2026-09-17-h02/verify/RUNINFO.TXT).

**PASS (Kernziel):** 3/3 neue Hosttests, 5/5 Regressionstests (`aes_button_dispatch`,
`aes_window_drag`), `cargo run -p xtask -- test` = 560 gruen/0 rot/1 ignoriert
(exakt wie erwartet). Beide Boards bauen sauber, Clippy-Delta 0 gegen die
genannte Baseline (tos-api 81, kernel 125). Vier unabhaengig gefahrene QEMU-
raspi4b-Sequenzen (H02 selbst, H03-Regression, Ziehen mit Auswahl auf
Loslassen, Druck-ausserhalb-in-die-Leiste-gezogen) bestaetigen per Trace UND
Screendump: die App bekommt fuer den auswaehlenden Klick genau EIN
`MN_SELECTED`, nie ein `MU_BUTTON` -- sowohl fuer Auswahl auf Druck als auch
auf Loslassen (EmuTOS `gemmnlib.c:314`), sowohl bei TeraDesk als auch am
eigenen Desktop nach dem Quit-Ruecksprung. Der Datei-Dialog fuer HELLO.TXT
erscheint korrekt (nicht mehr die Laufwerksinfo).

**FAIL (Nebenbefund, ausserhalb des Auftragstexts, aber real und mit
Kontrollbau belegt):** Der woertliche `method/run3.actions`-Ablauf (Park
verweilt ~1s auf (0,0), das liegt INNERHALB der Menueleiste) oeffnet auf dem
GEFIXTEN Baum das `C:\`-Fenster nicht mehr per Doppelklick auf HARD DISC --
der erste Klick wird vom beim Verweilen bereits offenen Menue-Tracker als
Pseudo-Edge verschluckt und als `None` verworfen (Item ueber die
Tafel-Geometrie an einer davon weit entfernten Bildschirmposition
aufgeloest), der Doppelklick degradiert zum Einzelklick. Mit `git checkout
HEAD` auf exakt dieselben drei Dateien (Kontrollbau, sonst identisches
Skript/Karte/Timing) oeffnet sich das Fenster zuverlaessig -- die Regression
liegt eindeutig an der neuen `!gl_ctmown`-Klammer in `b_click`, die die
Besitzuebergabe an die App vom Druck auf das Loslassen verschiebt; die alte,
eigentlich fehlerhafte sofortige Uebergabe hat diesen Fall zufaellig richtig
behandelt und damit verdeckt. Schmaler, vorbedingungsgebundener Fall
(laengeres unbeabsichtigtes Verweilen in der Leiste vor einem entfernten
Klick); auf echter Hardware seltener, aber nicht ausgeschlossen. Reproduktion,
Trace und beide Kernel-Belege (fix vs. Kontrollbau) in RUNINFO.TXT Abschnitt 4.
Dem Developer zur Bewertung uebergeben, kein Fix durch den Tester.

**Nicht gefahren (Zeitbudget):** Szenario "eigener Desktop" (Desk-Menue) als
eigener Lauf -- der Mechanismus ist bereits durch die vier gefahrenen
Sequenzen plus den Quit-Ruecksprung auf den eigenen Desktop in Szenario b
abgedeckt (derselbe pid-unabhaengige AES-Kernkode), aber nicht am Desk-Menue
selbst Klick-fuer-Klick durchlaufen. Echte USB-Maus-Zeitabstaende und ein
Nachtest auf echter Pi 4 (Datei markieren, File->Info, Markierung bleibt
stehen) gehoeren auf die HW-Checkliste.

Baum: `git status --short` zeigt vor/nach der Messung nur die vier
Fix-Dateien plus das neue `measurement-2026-09-17-h02/verify/`-Verzeichnis.
`kernel/src/host_env.rs` sha256 identisch vor/nach (`287ff741...`). Injektor
und Kontrollbau nur in Scratch-Kopien, keine laufenden QEMU-Prozesse, keine
QMP-Sockets uebrig, nichts geflasht, nichts committet.

### Einordnung des Nebenbefunds durch den Lead (2026-09-17)

**Kein Defekt des Fixes — das Skript klickt in ein offenes Menue.** Der
Kontrollbau-Trace des Testers zeigt es selbst
(`verify/serial/control-prefix-trace-serial.log:44-72`): der Zeiger faehrt
auf dem Weg zum Parkplatz ueber `(65,0)` — den Titel *File* — und die Tafel
`title=4 box=18` oeffnet sich; beim Parken auf `(0,0)` bleibt sie offen
(`track iter … title=4 box=18 item=-1`, Zeiger in der Leiste, aber ueber
keinem Titel). Der Doppelklick auf `(195,35)` landet damit **innerhalb der
offenen File-Tafel** — ein Menueklick, kein Desktopklick. Dass der alte Bau
das Fenster trotzdem oeffnete, ist genau H02: der erste Klick leckte als
`MU_BUTTON` mit Klickzaehler 2 an die App. EmuTOS verhaelt sich wie der
gefixte Bau: `OUTSIDE_STATE` haelt die Tafel offen, bis ein Klick faellt
(`gemmnlib.c`, siehe Spec `menu.md` „The drop-down tracking loop"), und ein
Klick in die Tafel ist eine Auswahl, keine Anwendungseingabe.

Folge fuer die Methode, nicht fuer den Code: der Parkplatz `(0,0)` liegt in
der Leiste; `verify/method/run3-safepark2.actions` (Park ausserhalb) ist die
Fassung fuer kuenftige Laeufe. Szenario e (eigenes Desk-Menue) bleibt als
HW-Punkt auf der Liste.
