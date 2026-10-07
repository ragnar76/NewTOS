# ZVIEW04 — Quittung: File > Quit beendet zView auch bei offenem Browser

Datum 2026-10-07/08 · Basis `desk-rebuild`, HEAD `8435cd2`, Baum vorher sauber
· nicht committet · Belege
[measurements/measurement-2026-10-07-zview04/](../measurements/measurement-2026-10-07-zview04/RUNINFO.TXT)
· Ticket [ZVIEW04](ZVIEW04.md)

## Ursache (belegt) — NewTOS-AES-Abweichung

NewTOS schickte `MN_SELECTED` mit `msg[5..7] = 0`. EmuTOS
(`CONF_WITH_MENU_EXTENSION`, Referenzbau) legt dort HIWORD/LOWORD des
Menübaums und das Elternobjekt des Eintrags ab (`aes/gemctrl.c:353-358,365`);
die Spec (`docs/api/aes/menu.md`, „`MN_SELECTED` message") sagte das schon,
der Code nicht.

WinDom ordnet eine Menüauswahl über genau diese Wörter zu: es vergleicht
`msg[5..6]` mit `menu.root` jedes Fensters
(`sdk/windom/overlay/src/evnt_windom.c:74-83`). Der Browser hat kein
Fenstermenü (`menu.root == NULL`, `wind_create.c:92`), 0 passte also, die
Auswahl landete in der leeren Bindungstabelle des Browserfensters
(`:235-239`), zViews Desktop-Bindung `applexit` (`menu.c:638`) und
`MenuHandle` (`menu.c:687`, setzt den Titel zurück) liefen nie. Ohne Fenster
findet die Schleife nichts → Desktop-Bindung → Quit geht. Das ist genau das
gemeldete Verhalten — und betraf **jede** Desktop-Menüauswahl von zView bei
offenem Fenster (in QEMU gegengeprüft: *zView > About*).

AES-Trace (Injektor, `--features aes-trace`):

```
HEAD  POST/RECV MN_SELECTED msg=[10 1 0 4 30 0 0 0]      -> zurück in die Hauptschleife, "File" bleibt invertiert
Fix   POST/RECV MN_SELECTED msg=[10 1 0 4 30 435 8440 18] -> appl_write(WM_DESTROY) an sich selbst,
      evnt_multi(MU_MESAG) -> wind_close(1), wind_delete(1) ... appl_exit -> Desktop
```

(`0x01B3:0x20F8` = `rsrc_gaddr(0,0)` = die Menüleiste, 18 = die File-Box.)

## Fix (1:1 nach EmuTOS)

* `docs/api/aes/menu.md`: NewTOS-Bindung unter der `MN_SELECTED`-Tabelle
  (welcher Baum, Adresse wie an `menu_bar` übergeben, `get_par`, die
  WinDom-Abhängigkeit).
* `tos-api/src/aes/menu.rs`: `mn_selected_ext(tree, item)` → `[treehi,
  treelo, parent]`, Eltern über den Accessor des Leisten-Besitzers.
* `tos-api/src/aes/object.rs`: `parent_t` (Accessor-Form von `get_par`).
* `tos-api/src/aes/event.rs`: `menu_intercept` füllt `msg[5..7]` für
  `MN_SELECTED`; `AC_OPEN` bleibt 0 (`gemctrl.c:348`).
* Test `mn_selected_carries_menu_tree_and_parent`
  (`tos-api/tests/aes_menu_click_ownership.rs`): gegen HEAD rot
  (`msg[5..6] = (0,0)`), danach grün.

## Prüfungen

| Prüfung | Baseline | Jetzt |
|---|---|---|
| Hosttests | 1248/0/1 | **1249/0/1** (+1) |
| neuer Test gegen HEAD | — | rot (`checks/red-before-fix.txt`) |
| Release rpi4 / qemu-virt (tos-api/kernel) | 64/94 | 64/94 |
| Clippy rpi4 / virt | 76/125 | 76/125 |
| Drift / SDK-Hosttests | 31 / 18/0/0 | 31 / 18/0/0 |
| Abnahme zv-quit (QEMU) | HEAD: Browser bleibt, Titel invertiert | Fix: zView endet (rc=0), Desktop |
| Gegenprobe zv-about | HEAD: nichts | Fix: About-Fenster |
| Regression HEAD vs. Fix (LDGDEMO on/strict, EmuDESK, desk3a, TeraDesk, QED, APITEST, zView Doppelklick + Browser) | — | 60/60 Screendumps bytegleich; APITEST 106/0/5, Serienlogs gleich |

## Pi-Paket (nicht kopiert)

`measurements/measurement-2026-10-07-zview04/bin/kernel8.img` (sha256
`a06d5210…`), `LIESMICH.TXT`, `SHA256SUMS`. ZVIEW.NAP unverändert.

## Offen

* Echte Pi 4: Quit bei offenem Browser, dazu ein paar weitere Menüeinträge.
* Beobachtung, nicht geändert: nach `MN_SELECTED` liefert das nächste
  `evnt_multi` `MU_BUTTON`, solange der auswählende Druck noch gehalten ist;
  nach Quelltext täte EmuTOS' `abutton` dasselbe — nicht gegen einen Lauf
  geprüft.
* Untermenüs aus einem separaten Baum werden weiterhin nicht geliefert
  (menu.md Schritt 8, zurückgestellt).
