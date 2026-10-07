# ZVIEW02 — Quittung: Doppelklick auf eine Ordnerkachel öffnet den Ordner (mit A111 Punkt 1)

Datum 2026-10-07 · Basis `desk-rebuild`, HEAD `b3f642d`, Baum vorher sauber
· nicht committet · Belege
[measurements/measurement-2026-10-07-zview02/](../measurements/measurement-2026-10-07-zview02/RUNINFO.TXT)
· Ticket [ZVIEW02](ZVIEW02.md), zusammen mit [A111](A111.md) Punkt 1

## Ursache (belegt)

Trace der `evnt_multi`-Rückgaben im HEAD-Kernel (Kratzkopie,
`method/apply-trace-zv02.py`), zView-Doppelklick 3/5/3 Ticks:

```
[zv02] count_clicks hz 9337..9338 max 2 cap_ms_left 5 -> got 1
[zv02] evnt_multi ... bclk 0x102 bmask 3 bstate 0 -> fired 0x22 ... EV_MB 1 EV_BRET 1
[zv02] count_clicks hz 9345..9347 max 2 cap_ms_left 15 -> got 1
[zv02] evnt_multi ... -> fired 0x22 ... EV_MB 1 EV_BRET 1
```

zView wartet mit `MU_TIMER` 20 ms (`TimerInit(20)`, `main.c:141`) und
`bclick 258` = Flag 1 + 2 Klicks, Maske 3, Zustand 0 („ein Knopf wird
gedrückt"). `count_clicks` wurde an der Timer-Frist gekappt → jeder Druck kam
als eigener Einfachklick zurück; `nb_click == 2` (`catalog_mouse.c:132`) galt
nie. EmuTOS: das Doppelklickfenster läuft am AES-Tick (`b_delay(1)` aus
`tikcod`, `gemdosif.S:389-411`), unabhängig vom Aufruf; ein abgebrochener
Mehrfachklick-Wartevorgang lässt `gl_bpend` stehen (`acancel`/`takeoff`
dekrementiert nicht, `gemasync.c:96-114,217-243`), der Druck öffnet das
Fenster und wird nicht veröffentlicht (`geminput.c:133-140`), der zweite
Klick verlängert um 3 Ticks (`:121-122`), beim Schließen `bchange(1,2)`
(+`bchange(0,1)`) → `post_button` beendet den gerade geparkten Wartevorgang
mit `min(2,2)` (`:155-171,463-483`) → `EV_BRET 2`, `EV_MB 1`.

## Fix (1:1 nach EmuTOS)

Spec zuerst: `docs/api/aes/event.md` neuer Abschnitt „The double-click window
(`gl_bdely` / `gl_bpend`) — contract", Mapping-Absatz und zwei
Verifikationskriterien. Code (`tos-api`):

* `aes/input.rs`: `gl_bdely` als Tickzähler, `b_delay`/`b_delay_poll`
  (20-ms-Ticks gegen die Host-Uhr, bei jedem Mauspaket und jedem Wartepass),
  `b_click` öffnet/zählt/verlängert wie EmuTOS, `gl_bpend`, je AES-Identität
  ein geparkter Knopf-Wartevorgang (`abutton`/`bwait_take`/`acancel`),
  `post_button` aus `bchange`, `set_mown` mit `post_button`, `downorup` hierher,
  `ev_consume` für AES-interne Knopf-Rückkehrer, `CTL_PRESS` = die Lade-Warte
  des Control Managers (`0x0001ff01`).
* `aes/event.rs`: `evnt_button`/`evnt_multi` auf Schnellprüfung + `abutton` +
  `post_button`; `count_clicks`/`wait_until`/`count_form_clicks` entfernt.
* `aes/form.rs` (`fm_do`-Warte, TOUCHEXIT-Zählung nach Fensterende),
  `window.rs` (Rahmenklick per Pegel oder Latch), `menu.rs`, `graf.rs`,
  `fsel.rs` (`ev_consume`), `trace.rs` (Doku), `vdi/mouse.rs`
  (`TICKCAL_MS` `pub(crate)`).

## Prüfungen

| Prüfung | Baseline | Jetzt |
|---|---|---|
| Hosttests | 1201/0/1 | **1205/0/1** (+4) |
| neue Tests gegen HEAD | — | 4/4 rot (`checks/red-before-HEAD-b3f642d.txt`) |
| Release rpi4 / qemu-virt (tos-api/kernel) | 64/94 | 64/94 |
| Clippy rpi4 / virt | 76/125 | 76/125 |
| Drift | 31 | 31 |
| SDK-Hosttests | 17/0/0 | 17/0/0 |

Geänderte Testvorbedingungen (keine Zusicherung gelockert, Begründung im
RUNINFO): Klick kommt bei armiertem Wartevorgang an; gehaltene Rahmen-/
Doppelklickdrucke werden über das Fenster hinaus gehalten; A92b mit 300-ms-
statt 0-ms-Timer. Ganze Suite 4×, zehn Klick-Testdateien je 5× gemischt: grün.

## QEMU raspi4b (serial=on, memprot on; LDGDEMO auch strict)

* **Abnahme** zView (`ZVIEW.NAP` cebb3b1b, Produkt): Doppelklick „zview"
  (kurz/kurz) → `C:\zview\` (`after-trace-Z06_ZVIEW_DIR.png`); Doppelklick
  „samples" (zweiter Druck 360 ms gehalten) → `C:\zview\samples\` mit
  Vorschaubildern (`after-trace-Z07_SAMPLES.png`). Trace: `fired 0x2 EV_MB 1
  EV_BRET 2`. HEAD: bleibt in `C:\` mit gewählter Kachel.
* **Regression**, jedes Skript gegen HEAD- und neuen Kernel, Bytevergleich:
  EmuDESK 6/6, desk3a 9/9, TeraDesk 4/4, QED 4/4, LDGDEMO on/strict 10/10 +
  10/10, zView-Einfachklick/Baum 7/7 — alle identisch; APITEST 106/0/5, beide
  Bilder identisch zu ZVIEW01.

## Offen

* **A111 Punkt 2** (Schreibmarke `y-3`, `gl_hchar+6`, XOR, `gemobed.c:198-216`)
  nicht gemacht — liegt in `object.rs`, nicht in diesem Änderungssatz. A111
  bleibt offen.
* `graf_mkstate` meldet den rohen VDI-Knopf statt `button`
  (`gemgrlib.c:458-464`); bei offenem Fenster weicht das ab. Vorbestehend, in
  event.md benannt.
* Interne `fm_do`/fsel/Menü-Schleifen handeln weiter sofort auf den rohen
  Druck (EmuTOS: ein Fenster später); sie schließen das Fenster selbst.
* Echte Pi nicht gefahren (keine HW-Aktionen durch Worker).
* `docs/STATUS.md`-Eintrag steht aus (Lead).
