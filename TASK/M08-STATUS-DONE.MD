# M08 — STATUS: DONE (2026-10-17) — alle gemessenen AES-Tabellen im Dauertest

Rest aus [M08-STATUS-UNFINISHED.MD](M08-STATUS-UNFINISHED.MD): `aes-m07-parameter.tsv`.

- Der Mauszeiger-Befund war schon durch V17 behoben (`tos-api/src/vdi/mouse.rs::park_position`).
- B-2, neu gemessen nach A112/A113: allein 20/20 gruen; im vollen Binary
  6/6 Laeufe genau ein Fall rot (69 oder 70, `int_out[0]` 0x30 statt 0x20).
  Ursache: ein stehengebliebenes `WM_REDRAW` an Fenster 1/2 — **nicht nur m05**,
  auch jeder fensteroeffnende r2-Fall (geschuettelt 5/5 rot neben r2_wind).
  EmuTOS-konform: `wm_open` schickt es (`aes/gemwmlib.c:887-904`), `wm_new`
  leert die Schlange nicht (`:1874-1892`); der Dumper startet m07 auf frischer AES.
- Fix im Harness (`2f3a6608`): `sweep_message_queue` vor und nach jedem Fall
  (Schnellpruefung `evnt_multi(MU_MESAG|MU_TIMER, 0)`, eine Nachricht je Aufruf,
  harter Abbruch nach 64). Preis im Doc-Kommentar: keine Tabelle kann eine
  Nachricht eines Vorgaengerfalls messen.
- Ergebnis: `aes_reference` 224/0 (+20), 12 geschuettelte Laeufe parallel und
  5 seriell gruen, `cargo xtask test` **1411/0/1**, Clippy 0 unter `tests/`.
  Kein Produktcode.

Ueberholt: `M08-STATUS-UNFINISHED.MD` §4 B-2 („Harness kann die Schlange nicht leeren“, „nur m05“).
