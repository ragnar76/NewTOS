# DESK36 — STATUS: DONE (2026-09-24) — implementiert, Pi-Abnahme offen

**Auftrag:** `gr_drgplns`' Schleife nachziehen, damit das Ziel unter dem
Zeiger waehrend des Ziehens hervorgehoben wird. Vorher am Quelltext
feststellen, wo unsere Ziehschleife liegt und ob sie pro Bewegung laeuft —
davon haengt ab, ob das eine Ergaenzung oder ein Umbau ist.

Basis: Branch `desk-rebuild`, HEAD `fa1d403`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset`.

Messung:
[`measurements/measurement-2026-09-24-desk37-desk36-desk30/`](../measurements/measurement-2026-09-24-desk37-desk36-desk30/)
§3. Spezifikation: `docs/api/desktop/deskact.md`, Abschnitt *`gr_drgplns` —
the drag loop, and the target highlight (DESK36)*. Hardwarepunkt **62** in
`docs/api/desktop/_hw-test-2026-09-17.md`.

---

## 1. Wo unsere Ziehschleife lag — Umbau, nicht Ergaenzung

Sie lag **nicht** im Desktop. `act_bdown` rief fuer den Ziehfall die AES
`graf_dragbox` (71). Die fuehrt die Schleife selbst
(`tos-api/src/aes/graf.rs:221-260`: `loop { mouse_position(); outline(...);
if !mouse_down() { break } host.pump_input() }`), zeichnet **einen** Kasten
und gibt nur die Endposition zurueck. Der Desktop sieht also pro Ziehvorgang
genau ein Ergebnis und keine einzige Bewegung — `act_chg` kann dort gar nicht
gerufen werden, und die AES kennt keine `ANODE`s, koennte die Regel also auch
nicht selbst anwenden.

EmuTOS' Desktop ruft `graf_dragbox` an dieser Stelle nie; seine Bindung
enthaelt die Funktion nicht einmal (`aesbind.h:103-105`, festgehalten in
`docs/api/desktop/gembind.md:85`). Die Schleife gehoert in den Desktop, und
dorthin ist sie jetzt gezogen.

## 2. Was gebaut wurde

`kernel/src/desk/main.rs`, neben `gr_obfind`/`act_bdown` (wo `deskact.c`'
trap-benutzende Haelfte in NewTOS schon lag):

* `gr_drgplns` (`deskact.c:248-355`) — die Schleife samt der drei Regeln;
* `gr_bwait` (`:209-232`), `gr_isdown` (`:66-77`), `gr_plns` (`:195-207`),
  `gr_extent` (`:169-188`), `gr_obalign` (`:234-242`),
  `rc_constrain` (`util/rectfunc.c:32-42`);
* `act_chg_draw` — `act_chg`s `dodraw`-Haelfte, `do_wredraw(wh,
  rc_intersect(pc, extent))` (`:453-460`).

`kernel/src/desk/act.rs` (rein, ohne Traps): `gr_accobs` (`:87-127`) und
`DeskState::obj_extent`.
`kernel/src/desk/app.rs`: `G.g_xyicon`/`G.g_xytext` (`deskapp.c:988-1011`).
`kernel/src/desk/dir.rs`: `i_find_atype` — `i_find`s beide Arme
(`desksupp.c:1746-1767`), reduziert auf `(a_type, isapp)`.
`kernel/src/desk/mod.rs`: `graf_handle` (77), `evnt_multi_m1`, `FLATHAND`;
`graf_dragbox` ist raus, weil sie keinen Rufer mehr hat.

Dazu die benannte Abweichung: **`kernel/src/desk/vdibind.rs`**, neu — siehe
§4.

## 3. Die vier Faelle des Tickets, gemessen

Eine Geste (`method/d36.actions`): `HELLO.TXT` anfassen und ohne Loslassen
ueber vier Ziele ziehen, dann ueber dem Ausgangssymbol loslassen (Selbstabwurf,
den EmuTOS abbricht — die Karte wird nicht angefasst).

| Bild | Ziel | Hervorhebung | Regel |
|---|---|---|---|
| `d36-after-C-prog.png` | `QED.NAP` (`*.NAP` → `isapp`) | **ja** | 1 |
| `d36-after-D-folder.png` | Ordner `SUB` (`AT_ISFOLD`) | **ja** | 1 |
| `d36-after-E-file.png` | `README.TXT` (`AT_ISFILE`, `!isapp`) | **nein** | 1 |
| `d36-after-F-trash.png` | Papierkorb (`AT_ISTRSH`, Desktop-Arm) | **ja** | 1 |
| `d36-after-D/E/F` | das vorige Ziel jeweils | zurueckgenommen | 2 |
| `d36-after-G-back.png` | das gezogene Symbol (schon `SELECTED`) | unberuehrt | 2 |
| `d36-after-H-released.png` | nach dem Loslassen | ueberall zurueck | 3 |

Gezaehlt statt angesehen — Pixel, die sich von derselben Zelle in
`d36-after-B-win1.png` (dem nicht hervorgehobenen Grundbild) unterscheiden,
je Zielzelle aus dem Probe-Kasten:

| Zelle | Kasten | abweichende Pixel | |
|---|---|---|---|
| `QED.NAP` | (141,255) 76x40 | 1446 | invertiert |
| `SUB` | (477,87) 76x40 | 1333 | invertiert |
| `README.TXT` | (309,255) 76x40 | **143** | **nicht** invertiert (nur Umriss + Zeiger) |
| `TRASH` | (0,433) 76x40 | 1130 | invertiert |

Eine invertierte Zelle liegt eine Groessenordnung von einer nicht invertierten
entfernt; die Regel fuer die gewoehnliche Datei haengt also nicht daran, wie
man ein Bild liest. Nach dem Loslassen sind alle vier Zellen **bitgleich** mit
dem Grundbild (leeres Differenzrechteck).

Jedes `d36-before-*` zeigt denselben Augenblick auf HEAD: ein Kasten, keine
Hervorhebung, kein Handzeiger.

**Das XOR-Loeschen ist exakt.** `d36-after-H-released.png` unterscheidet sich
von `d36-after-B-win1.png` (aufgenommen, bevor das Ziehen begann) nur im
Rechteck `(58,70)-(383,211)` — Infozeile und die Zelle des Quellsymbols, beides
echte Zustandsaenderungen. Auf dem Hintergrundraster und auf jedem anderen
Symbol steht Pixel fuer Pixel dasselbe.

## 4. Die benannte Abweichung: eine eigene VDI-Arbeitsstation

`gr_plns` zeichnet ueber `gsx_pline` → `gsx_xline`, also `vsl_udsty` +
`v_pline` je Segment in `MD_XOR`, mit einer Strichphase aus der
Koordinatenparitaet (`aes/gemgraf.c:140-182`) — genau das macht den Umriss
positionsunabhaengig, was das zweimalige Zeichnen erst zum Loeschen macht.
EmuTOS' Desktop zeichnet auf der Arbeitsstation, die er mit der AES **teilt**;
er darf das, weil der ROM-Desktop in die AES gelinkt ist.

Unser Desktop erreicht das System nur ueber Traps, und unsere AES hat keine
VDI-Arbeitsstation zum Teilen: sie malt direkt ueber das Raster-Backend,
`v_opnwk` wird nirgends gerufen, Handle 1 ist nicht offen. Deshalb oeffnet
`vdibind.rs` die **eigene** virtuelle Arbeitsstation des Desktops, so wie es
jede NewTOS-GEM-Anwendung tut (`graf_handle` (AES 77) → `v_opnvwk`), und setzt
darauf `vsl_type(7)` + `vsl_udsty(0xffff)` — was in EmuTOS zur Einrichtung der
AES-Arbeitsstation gehoert (`gsx_start`, `aes/gemgraf.c:420-422`). `gr_bwait`
setzt Clip und Schreibmodus in **jedem** Durchlauf neu; genau das verlangt
EmuTOS' eigener Kommentar an `desk/deskact.c:211-214`.

Zwei kleinere, ebenfalls benannt:

* `gsx_attr`s `gl_mode`/`gl_lcolor`-Cache (`aes/gemgraf.c:205-240`) ist nicht
  nachgebaut — er spart VDI-Aufrufe und laesst die Arbeitsstation im selben
  Zustand zurueck.
* EmuTOS liest bei `*pdobj == NIL` und `curr_sel == 0` `tree[-1]`
  (`deskact.c:334-335`). Unser `gr_obfind` liefert an dieser Stelle `root`
  statt `NIL`, der Fall ist von der ersten Abfrage mit abgedeckt.

Neu genommen wurde auch `act_bdown`s abschliessendes
`evnt_button(0x01, 0x01, 0x00, …)` (`:681`), das die Freigabe abraeumt.

## 5. Pruefung

* `cargo xtask build --board rpi4` / `--board qemu-virt`, je nach
  `cargo clean -p kernel -p tos-api`: `tos-api` 64 / `kernel` 94 — Basis
  64/94, Delta 0.
* `cargo clippy --features board-rpi4` / `board-qemu-virt`: 80/125 — Basis
  80/125, Delta 0; null Treffer in `kernel/src/desk/`.
* `cargo run -p xtask -- test`: 939 / 0 / 1 — Basis 939/0/1.
* Zwei QEMU-Laeufe mit je sieben Aufnahmen, 0 `[EXC]`/`[PANIC]`,
  `fsck_msdos -n` sauber.

## 6. Offen

Die echte Pi 4, Punkt 62. Zwei Dinge sagt der Injektor nicht: ob beim
**langsamen** Ziehen eine Spur zurueckbleibt (er springt in Schritten von bis
zu 127 Pixeln), und wie ein Block aus **mehreren** Umrissen aussieht.
