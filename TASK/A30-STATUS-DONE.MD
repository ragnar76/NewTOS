# A30 — Quittung

**Zustand: DONE** · 2026-09-19 · Messdesign, kein Code, keine Tabelle, kein Commit

Entwurf: [`docs/analysis/a30-appl-init-recall-measurement-design-2026-09-19.md`](../docs/analysis/a30-appl-init-recall-measurement-design-2026-09-19.md)

Gepinnte EmuTOS-Quelle `1a7484ab`; NewTOS `desk-rebuild` @ `016cd43`, Baum sauber; nichts
außer dem Entwurf und dieser Quittung geschrieben.

## Die vier Klärungspunkte, je ein Satz

1. **Nicht blockierender Weg:** `evnt_multi(MU_MESAG|MU_TIMER, tmcount 0)` mit gepflanztem
   Puffer — der Timer-Arm setzt `what` vor dem Warteblock (`gemevlib.c:216-220` vor `:232`),
   der Message-Arm entnimmt genau eine Nachricht ohne Warten (`gemqueue.c:98-107`,
   `gemasync.c:40,117-127`; NewTOS `event.rs:455-459,598-619`), im Riegel seit M07 erlaubt
   (`cases.c:1167-1203`) und auf beiden Maschinen gefahren (M07 Fall 69); `appl_write` verrät
   nichts (immer 1 oder Hänger, `gemqueue.c:96-124`) und braucht die maschinenabhängige eigene
   `ap_id` (TT 0, Pi 1/2), `appl_find` liest die Queue nicht.
2. **Zwei-Nachrichten-Konstruktion:** nicht nötig für die Terminierung, aber als Fall M3
   übernommen (`open 1; appl_init; open 2; read; read` → 48 gegen 32), belegt dadurch, dass ein
   Open nur dem geöffneten Handle eine `WM_REDRAW` schickt (`gemwmlib.c:1097-1098,1157-1168,
   1024-1044`; `window.rs:2417`) und `doq` nur gleiche Handles vereinigt (`gemqueue.c:52-56`).
3. **A28-Übergang:** mit dem Dumper **ablesbar** (erster Fall P1 ohne Preludes: 48 = geerbte
   Nachricht), aber **nicht herstellbar** — die Vorbedingung ist eine Bedienergeste vor dem
   Start (A28-Status §7); Referenz 32 aus `deskmain.c:2166` und M07; also kein sicherer
   Dumper-Messfall, sondern eine objektive Ablesung für die vorhandene Handabnahme (Folgeauftrag F3).
4. **`appl_exit`:** nicht nötig — `ap_init` hat keine Vorbedingung (`gemaplib.c:54-63`), der
   Dumper ruft `appl_exit` selbst nach den Fällen (`gem.c:117-127`, `dumpaes.c:157`).

## Fallfolge, Terminierung, Referenz

11 Fälle, 92 Preludes: P1/P2 (Start), R0 (der Wiederaufruf gemessen), C0/C1/C2 (Instrument),
M1/M2/M3/M4/M5 (Messung + Kontrollen); vor jedem konstruierenden Fall acht Drain-Reads, Reset
`wind_new`, nur Handles 1/2. Jeder Baustein terminiert in beiden Ausgängen (Abschnitt 3.2 des
Entwurfs, je `datei:zeile`); der Fehlerfall ist der **leere** Zweig, und der ist der bereits
gemessene. Entwurf mit dem Host-Lader geprüft: `casecheck --aes` Exit 0 (Scratchpad, kein
Artefakt). **Hatari reicht** (bitgleich validiert, relevante EmuTOS-Dateien seit `VERSION_1_4`
unverändert); der TT muss nicht mitlaufen.

## Offen / Folgeaufträge (im Entwurf, Abschnitt 5)

F1 Puffer-Erfassung im AES-Record (Inhalt der gelesenen Nachricht; für die P1-Lesart nötig),
F2 `@apid`-Token (optional), F3 A28-Instrument außerhalb des Dumpers. Nicht auf der echten Pi
gemessen: `evnt_multi` mit 0-ms-Timer (bisher QEMU + `evnt_timer(0)` auf der Pi) — deshalb
Reihenfolge Hatari → QEMU → Pi.
