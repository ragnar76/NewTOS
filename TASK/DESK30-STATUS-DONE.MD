# DESK30 — STATUS: DONE (2026-09-24) — implementiert, Pi-Abnahme offen

**Auftrag:** `CONF_WITH_DESKTOP_SHORTCUTS` — eine Datei oder ein Ordner als
Symbol auf dem Desktop. Reihenfolge wie bei DESK15: zuerst die Datenebene,
dann `app_blddesk` und `app_afind_by_id`s Konsumenten, dann `ins_shortcut` und
die Dialogzweige.

Basis: Branch `desk-rebuild`, HEAD `fa1d403`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset`.

Messung:
[`measurements/measurement-2026-09-24-desk37-desk36-desk30/`](../measurements/measurement-2026-09-24-desk37-desk36-desk30/)
§4. Spezifikation: `docs/api/desktop/deskins.md`, Abschnitt *`ins_shortcut` —
desktop shortcuts (DESK30)*, und `deskapp.md`. Hardwarepunkt **63** in
`docs/api/desktop/_hw-test-2026-09-17.md`.

---

## 1. Datenebene

`global::Anode` hat drei Felder bekommen: `a_dicon`, `a_flags` und `a_pdata`.

`AF_ISDESK` bleibt **implizit** — jeder Knoten von `G.g_ahead` *ist* ein
Desktop-Knoten; das ist genau die `AF_ISDESK`-Haelfte von EmuTOS' einer
Liste, und die andere Haelfte liegt in `desk::inf`s Sitzungsmodell. Von den
uebrigen `AF_*` traegt der Desktop-Knoten nur `AF_ISEXEC`, und das ist keine
Abkuerzung: `do_aopen` ersetzt die `ANODE` einer Verknuepfung durch die der
Zuordnungsliste, **bevor** es irgendein Flag liest
(`desksupp.c:857-871`) — `AF_ISCRYS`, `AF_ISPARM`, `AF_APPDIR`, `AF_ISFULL`
haette auf dem Verknuepfungsknoten also niemand gelesen.

Fuer eine Verknuepfung ist `a_pappl` der **volle Pfad** und `a_pdata` der
blosse Name (`ins_shortcut`, `deskins.c:1380-1381`); fuer Laufwerk,
Papierkorb und Drucker bleibt `a_pappl` die Beschriftung und `a_pdata` leer.

## 2. `app_blddesk`, `app_parse`, `app_save`

* Beschriftung: `a_pdata` bei `AT_ISFILE`/`AT_ISFOLD`, sonst `a_pappl`
  (`deskapp.c:1392-1396`).
* Bild: `(a_dicon < 0) || (a_type == AT_ISFILE && (a_flags & AF_ISEXEC))`
  → `a_aicon`, sonst `a_dicon` (`:1374-1378`).
* `#X` (Datei) und `#V` (Ordner) lesen (`:333-343`) und schreiben
  (`:1224-1243`).
* Die Vertauschung von `a_pappl`/`a_pdata` unterhalb `inf_rev_level 2`
  (`:404-415`). Dafuer wird `#R` jetzt wirklich gelesen statt verworfen; ein
  von NewTOS geschriebenes `NEWDESK.INF` traegt immer `#R 02`, betroffen sind
  also nur von Hand oder von einer alten EmuTOS-Version geschriebene Dateien.
* `AF_ISEXEC` aus der Endung (`:422-426`), ueber `ins::is_executable`, dessen
  Menge EmuTOS' `exec_ext[]` (`deskins.c:984`) plus die native
  `.NAP`/`.NCP`/`.NTP`-Familie ist (ADR-018).

## 3. `ins_shortcut` und die Verbraucher

`ins_shortcut` (`kernel/src/desk/ins.rs`, `deskins.c:1341-1406`) wird aus
`dir::fun_drag`s Ablegen-auf-die-Flaeche gerufen (`deskfun.c:1312-1319`), wo
bisher der `#else`-Zweig mit `STNODRA1` stand. `fun_drag` nimmt dafuer zwei
Argumente mehr entgegen — den Ablegepunkt, den `act_bdown` zurueckgibt
(`*in_mx = dulx - offx`, `deskact.c:683-684`), den DESK36 ohnehin gerade
verfuegbar gemacht hat.

| Was | Wo | EmuTOS |
|---|---|---|
| Doppelklick auf eine Verknuepfung | `main::open_item`s Desktop-Arm | `do_open`, `desksupp.c:1219-1259` |
| Ordner-Verknuepfung → immer neues Fenster | `main::do_fopen`s `handle == DESKWH`-Arm | `do_fopen`s `WN_DESKTOP`, `:1112-1126` |
| Ziehen **von** einer Verknuepfung | `dir::desktop_selected_sources` | `fun_file2any`s `an_src`-Schalter, `deskfun.c:1149-1160` |
| Ablegen **auf** eine Verknuepfung | `dir::fun_file2desk`s zwei neue Arme | `:1021-1040` |
| Verknuepfung auf ein Programm im Fenster ziehen | `dir::fun_desk2win`s Startzweig | `:1229-1253` |
| Loeschen ueber das Menue | `dir`s Desktop-Loeschzweig | `fun_del`s Schalter, `:1393-1405` |

`move_drvicon` und `rmv_icon` waren schon typunabhaengig und brauchten nichts.

## 4. Waisen

`remove_locate_shortcut` (`desksupp.c:376-412`) liegt in
`kernel/src/desk/dir.rs`: der `STRMVLOC`-Kasten („… not found. Do you want to
remove the icon or locate the item?", Remove | Locate | Cancel,
`desk_rsc.c:2531-2533`), *Locate* mit `fsel_exinput` ab der Wurzel des
Startlaufwerks; fuer eine Datei wird der gewaehlte **Name** an das Verzeichnis
gehaengt, fuer einen Ordner das Verzeichnis **ohne** den abschliessenden
Trenner genommen (`:405-409`).

Gerufen wird er an EmuTOS' drei Stellen: vor dem Start (`file_exists`,
`:861-868`), vor dem Ordnerfenster (`set_default_path == EPTHNF`,
`:1119-1123`) und beim Ziehen einer Verknuepfung, hinter der keine Datei mehr
steht (`deskfun.c:1183-1188`).

## 5. Eine zusammengefasste Stelle, benannt

`ins_shortcut` nimmt die Iconnummern von der `ANODE`, die `i_find` liefert,
und **ueberschreibt** sie mit einer zweiten Suche,
`app_afind_by_name(…, AF_ISDESK, …)` (`:1394-1400`). Die beiden unterscheiden
sich nur in der Ignoriermaske: `i_find` ueberspringt zusaetzlich `AF_WINDOW`
(`desksupp.c:1755`), die Ueberschreibung laesst genau diese Knoten also
wieder zu — damit ein ueber *Install icon → Window* gesetztes Bild gewinnt.
NewTOS' `app_afind_by_name` ueberspringt nur Desktop-Knoten, ist also schon
die Variante des zweiten Aufrufs; die erste Suche liefert denselben Knoten.
Ergebnis gleich, eine Suche statt zwei.

## 6. Gemessen

| Bild | Was |
|---|---|
| `d30-after-C-drag.png` | `QED.NAP` ueber der freien Flaeche, Umriss am Zeiger |
| `d30-after-D-shortcut.png` | losgelassen: Symbol `QED.NAP` mit dem Programmbild |
| `d30-before-D-shortcut.png` | HEAD: der Kasten „Can't drop items here." |
| `d30-after-E-saved.png` + Karte | `Options → Save desktop` schreibt `#X 02 09 06 FF   C:\QED.NAP@ QED.NAP@` |
| `d30b-A-coldstart.png` | dieselbe Karte kalt gebootet: die Verknuepfung ist zurueck, gleiche Zelle, gleiche Beschriftung, gleiches Bild |
| `d30c-D-qed.png` | Doppelklick darauf startet QED |
| `d30d-C-shortcut.png` | der Ordner `SUB` als Verknuepfung |
| `d30d-D-opened.png` | Doppelklick darauf: **neues** Fenster `C:\SUB\`, drei Eintraege |

Der vollstaendige `NEWDESK.INF`-Auszug nach dem Speichern steht in
`RUNINFO.TXT` §4.

## 7. Pruefung

* `cargo xtask build --board rpi4` / `--board qemu-virt`, je nach
  `cargo clean -p kernel -p tos-api`: `tos-api` 64 / `kernel` 94 — Basis
  64/94, Delta 0.
* `cargo clippy --features board-rpi4` / `board-qemu-virt`: 80/125 — Basis
  80/125, Delta 0; null Treffer in `kernel/src/desk/`.
* `cargo run -p xtask -- test`: 939 / 0 / 1 — Basis 939/0/1.
* Fuenf QEMU-Laeufe (`d30-after`, `d30-before`, `d30b`, `d30c`, `d30d`),
  0 `[EXC]`/`[PANIC]`, `fsck_msdos -n` sauber auf jedem geschriebenen Abbild —
  einschliesslich der Karte, auf die der Desktop die `#X`-Zeile geschrieben
  hat.

## 8. Offen

* **Der Waisenfall ist gebaut, aber nicht gefahren.** Er verlangt, die
  Zieldatei zwischen zwei Sitzungen zu verschieben; im Injektor waere das eine
  lange Klickfolge ohne Mehrwert gegenueber der Hardware. Punkt 63.5 der
  Checkliste.
* ***Show info* auf einem Desktop-Symbol** gibt es in NewTOS ueberhaupt nicht
  (`main::do_info_rename` kehrt fuer `G.g_cwin == DESKWH` sofort um), also
  hat `do_info`s Verknuepfungsarm (`desksupp.c:1300-1330`) nichts, woran er
  haengen kann. Die Luecke ist aelter als DESK30 und betrifft Laufwerke,
  Papierkorb und Drucker genauso — gehoert in einen eigenen Task.
* Der Nebenbefund aus DESK18 (die beiden Stellungen von *Default dir* werden
  erst ueber Verknuepfungen sichtbar) ist damit **pruefbar** geworden, aber
  nicht geprueft: er braucht zwei Laeufe mit unterschiedlicher Einstellung und
  gehoert zu DESK18, nicht hierher.
