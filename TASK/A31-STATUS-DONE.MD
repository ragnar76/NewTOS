# A31 — Quittung

**Status: GEGENSTANDSLOS** · 2026-09-19 · kein Test geschrieben, mit Grund

## Sachverhalt

Das Ticket (2026-09-10) verlangt einen Test, dass Rechteckliste und Cursor
den `save_windows`/`restore_windows`-Zyklus ueberstehen. Seitdem:

- `343b3c5` (2026-09-15, „remove what was built around the AES") hat beide
  Aufrufe aus `kernel/src/desk/main.rs` entfernt; Gap G2 dort
  (`:586-590`): EmuTOS stellt Fenster nicht wieder her, es betritt
  `deskmain` neu (`gemshlib.c:600-612`) und baut aus `cnx_get`
  (`deskmain.c:2055`).
- `/usr/bin/grep -rn "save_windows\|restore_windows" tos-api/src kernel/src`:
  nur die Definitionen (`window.rs:1049`, `:1074`) und Kommentare. Aufrufer
  im Produkt: **keine**. Nutzer: `tos-api/tests/aes_app_return_descriptor.rs:316-325`,
  `aes_launch_boundary_reset.rs:328` — dort als Stellvertreter fuer die
  Startgrenze.

## Entscheidung

Ein Test ueber einen Pfad, den kein Produktcode faehrt, sichert nichts und
wuerde einen Nicht-EmuTOS-Mechanismus festschreiben. Ticket gegenstandslos.
Beim Wiederaufbau des Rueckwegs nach EmuTOS (G1/G2) ist die Fensterliste
Teil der Abnahme; `save_windows`/`restore_windows` sind dann zu entfernen
(die zwei Tests auf den echten Grenzpfad umzustellen).

## Pruefung

Nur Quellenlesen; kein Code, keine Tests, keine Hardware.
