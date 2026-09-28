# PORT07 — Die Ball-Demo auf der Pi konform machen (und damit Dooms Bildweg beweisen)

Status: **OFFEN** · Prioritaet: **P2** · Typ: Portierung / Konformitaet — Anwenderwunsch 2026-09-28

Abhaengigkeiten: — · **Vorbedingung im Sinne eines Beweises** fuer [PORT06](PORT06.md)

## Der Befund

Die Ball-Demo liegt **ausserhalb des Repos** in
`/Volumes/Daten/Development/ball-demo` ([[ball-demo-as-benchmark]]) und hat je
Maschine eine Plattformdatei. Gezaehlt am 2026-09-28:

| Datei | `vro_cpyfm` | `Setscreen` | `Physbase` | `Logbase` | `vs_color` |
|---|---|---|---|---|---|
| **`plat-newtos.c`** | **0** | **8** | **1** | **7** | **0** |
| `plat-tt.c` | 5 | 0 | 1 | 0 | 3 |
| `plat-pistorm.c` | 6 | 0 | 0 | 0 | 5 |

**Unsere Plattformdatei ist die einzige nicht-konforme.** Sie baut den
Doppelpuffer ausschliesslich ueber `Setscreen`/`Logbase`/`Physbase` und benutzt
**kein** `vro_cpyfm` und **kein** `vs_color`. TT und PiStorm gehen den
konformen Weg.

Der Anwender dazu: die Vorgabe war Konformitaet, und hier ist sie verfehlt
worden.

## Warum es sich dreifach lohnt

1. **Es behebt die Abweichung.** Was ueber `Physbase` geht, laeuft nur dort, wo
   ein Shifter sitzt — nicht auf einer Nova, nicht auf einer Grafikkarte, und
   es bekommt keine Hardwarebeschleunigung
   ([PORT06](PORT06.md) §„Das staerkere Argument").
2. **Es beweist Dooms Bildweg empirisch.** PORT06s Entwurf steht auf
   „Truecolor-Speicher-MFDB → `vro_cpyfm`". Laeuft die Ball-Demo so auf der Pi,
   ist das keine Annahme mehr. **Das ist der eigentliche Grund, es vor PORT06
   zu machen.**
3. **Es macht die Demo vergleichbar.** Ihr Zweck war ein kontrolliertes
   Experiment je Maschine — TT gegen PiStorm gegen Pi. Solange die Pi einen
   anderen Weg nimmt, vergleicht die Messung Aepfel mit Birnen.

## Die Arbeit

**Kein Forschungsprojekt: `plat-newtos.c` ist 9,5 KB, und zwei fertige konforme
Fassungen derselben Plattformschicht liegen direkt daneben.** `plat-tt.c` und
`plat-pistorm.c` sind die Vorlage.

Zu beachten: unser Bildschirm ist **Truecolor** (`PLANES = 16`,
`kernel/src/vdi_backend.rs:43`), TT und PiStorm sind es nicht. Der Puffer ist
also in unserer Pixelform anzulegen, nicht in 8 Bit — genau die Frage, die
PORT06 §3a fuer Doom entschieden hat. `vs_color` entfaellt dort entsprechend
oder wird zur Umsetztabelle.

**Kein Kerneleingriff.** Faellt etwas an der VDI auf, wird es **berichtet**,
nicht behoben — elf Tage vor dem Termin wird der Zeichenpfad nicht angefasst.

## Abnahme

* Die Demo laeuft auf der Pi, sichtbar, ohne `Setscreen`/`Physbase`/`Logbase` —
  `/usr/bin/grep` ueber `plat-newtos.c` findet keines der drei.
* **Bildrate vorher und nachher**, auf derselben Maschine gemessen. Wird sie
  schlechter, ist das ein Befund und kein Grund zurueckzurudern — dann wissen
  wir, was unser `vro_cpyfm` kostet, und genau das braucht PORT06.
* Der Quelltext der Demo (`ball.c`, `ball_neu.c`, `ball-tex.c`) bleibt
  **unangetastet**; geaendert wird nur die Plattformdatei.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten. Die Demo liegt ausserhalb des Repos —
Aenderungen dort sind **nicht** Teil eines NewTOS-Commits; der Messbericht
schon.
