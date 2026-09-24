# PORT02 — Welche Anwendungen koennen wir bis zum Termin portieren?

Status: **OFFEN** · Prioritaet: **P2** · Typ: Bewertung / Portierbarkeit — Anwenderwunsch 2026-09-25

Abhaengigkeiten: [PORT01](PORT01.md) (Kommandozeile erreicht die Anwendung)

## Warum

Ohne Anwendungen laesst sich am 9./10. Oktober wenig zeigen. Heute laeuft
**QED**; **TeraDesk** ist portiert. Was sonst schon im Baum liegt, ist
ungeportet und unbewertet.

## Was da ist (Bestandsaufnahme, 2026-09-25)

| Ort | Was |
|---|---|
| `Applications/z-tools/zview` | Bildbetrachter |
| `Applications/z-tools/ztask` | Task-/Programmleiste |
| `Applications/z-tools/zweather` | Wetteranzeige |
| `Applications/z-tools/zcodeclib` | Codec-Bibliothek (Unterbau von zView) |
| `Applications/thing-neo-codeSVN-r62` | Thing — alternativer Desktop |
| `sdk/apps/highwire` | HighWire — Browser |

## Auftrag

Je Kandidat **eine belegte Einschaetzung**, keine Vermutung:

1. **Was braucht er?** Welche Bibliotheken (gemlib, cflib, LDG, eigene), und
   welche davon hat `sdk/` schon. Fehlende benennen, nicht ueberspringen.
2. **Wie viel m68k-Assembler?** Zaehlen, nicht schaetzen. Das ist die
   einzige Stelle, an der die Projektregel „die portierte Anwendung bleibt
   unveraendert" eine Ausnahme kennt.
3. **Welche Systemfunktionen braucht er, die uns fehlen?** Netzwerk faellt
   auf, aber nicht nur — gegen `docs/api/` und die offenen Tickets pruefen.
4. **Lizenz** — was erlaubt sie, was verlangt sie.
5. **Vorfuehrwert** und geschaetzter Aufwand, beides in einem Satz, und
   **getrennt** voneinander: das Beeindruckendste ist nicht zwingend das
   Naechstliegende.

Am Ende eine **Reihenfolge** mit Begruendung — was zuerst, was danach, was
realistisch nicht bis Oktober.

## Wie bewertet wird

Der Massstab ist unsere eigene Erfahrung mit QED, nicht ein Gefuehl: dort hat
sich gezeigt, dass eine Portierung im Kern ein **Neuausrichten der duennen
Trap-/Arch-Grenze plus Uebersetzen-und-Beheben** ist, keine Neuschreibung.
Ein Kandidat, der das nicht zulaesst — weil er zu tief auf Hardware sitzt
oder zu viel Assembler traegt — ist ein schlechter Kandidat, egal wie schoen
er aussieht.

Keine Codeaenderung in diesem Ticket. Ergebnis ist ein Dokument.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
