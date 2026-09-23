# DESK27 — STATUS: DONE (2026-09-23)

## Auftrag

`docs/api/xbios/26-supexec.md` widersprach sich: der Kopf sagte, der
Dispatcher gebe die Funktionsnummer zurueck, der Abschnitt *Pi / BCM2711
binding* behauptete `EINVFN`. Feststellen, welches stimmt, und die andere
Stelle richtigstellen.

## Befund

Der **Kopf** stimmt. `xbios::dispatch` (`tos-api/src/xbios/mod.rs`) endet auf

```rust
other => other as i64,
```

Ein nicht belegter oder ausserhalb des Bereichs liegender Steckplatz liefert
die Funktionsnummer, zero-extended. `Supexec` (0x26) steht in keinem `match`-
Arm, ein Aufruf bekommt also `38`. `EINVFN` kommt auf diesem Pfad nicht vor —
der Kommentar im Dispatcher nennt `Supexec 0x26` sogar namentlich als einen
der Faelle, die so antworten.

## Aenderung

`docs/api/xbios/26-supexec.md`, vier Stellen im Abschnitt *Pi / BCM2711
binding* und in der Zusammenfassung. Der falsche Wortlaut ist als Zitat
stehengeblieben, damit die Ruecknahme im Text steht. Zwei Saetze ergaenzt:
was ein deferrierter Steckplatz fuer den Aufrufer praktisch heisst (der
Rueckruf laeuft nicht, und die Rueckgabe sagt es nicht), und dass der eine
Anwendungsfall, den der Desktop wirklich brauchte, ueber
`Ssystem(S_GETCOOKIE)` geloest ist (DESK13).

## Pruefung

Keine — reine Dokumentation, kein Code angefasst.

## Offen

Die ADR-Supexec selbst (welcher Mechanismus auf AArch64) bleibt offen; das
war nie Teil dieses Tickets.
