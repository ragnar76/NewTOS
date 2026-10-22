# CON04 — `Bconout(2)` wertet keine VT52-Folgen aus

Status: **OFFEN** · Prioritaet: **P2** · Typ: Abweichung von der Spec / TOS-API-Luecke — Befund CON03-Spec (2026-10-22)

## Befund

`docs/api/bios/03-bconout.md` (Handle 2 = CON mit VT52, Handle 5 = roh ohne
VT52, EmuTOS `bios/chardev.c:104`) verlangt fuer Handle 2 das Auswerten der
VT52-Escape-Folgen. Die Umsetzung `tos-api/src/vdi/escape.rs` `conout`
(`:122-166`) ignoriert `ESC` und gibt den Folgebuchstaben aus („NewTOS' VT-52
console does not parse ESC sequences from the character stream“). Jedes
Textprogramm mit `ESC E` (Bildschirm loeschen), Cursorsetzen (`ESC Y`),
Inversschrift usw. zeigt Zeichensalat. EmuCON braucht `ESC E e D C K l v w b c`
(`docs/api/emucon/`).

## Auftrag

1. Spec der VT52-Konsole aus `references/emutos/bios/vt52.c` (und
   `conout.c`, Line-A-Zustand `docs/api/_line-a.md`) nach
   `SOURCE-ANALYSIS-PROTOCOL.md`: alle Folgen, Zustandsautomat, Farben
   (`ESC b/c`), Wrap (`ESC v/w`), Cursor ein/aus, Speichern/Wiederherstellen,
   Verhalten von Handle 5. Freigabe durch den Anwender.
2. Vollstaendiger Parser nach `vt52.c`, auf dem bestehenden `escape.rs`-Zustand
   (derselbe Cursor wie `v_escape`).
3. Tests: Hosttest je Folge; QEMU-Sonde mit Screendump; Vergleich gegen
   Hatari-Falcon (EmuTOS) mit derselben Ausgabe.

Voraussetzung fuer [CON03](CON03.md).

[Gemeinsame Regeln](README.md) gelten.
