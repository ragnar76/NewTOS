# A07 — Quittung

**Status: ERLEDIGT — Konfigurationsunterschied, kein Defekt** · 2026-09-19

## Was getan wurde

Feld `global[3..4]` (`ap_private`), TT `0x1143`, Pi `0x117B`, Delta `0x38`.
Die Pruefung vom 2026-09-10 hatte Wortreihenfolge und Werte bestaetigt und
nur den **Konfigurationsbeleg** offen gelassen, weil die `NEWDESK.INF` des
Laufs nicht archiviert war.

Beleg heute von der Karte (`/Volumes/BOOT/NEWDESK.INF`, sha256
`457a70e75015…`, mtime 2026-09-17, Kopie in
`measurement-2026-09-09/run1-pi4/NEWDESK.INF.card-as-of-2026-09-19`):

```
#Q 41 40 43 40 7B 40
```

EmuTOS-Regel (`aes/geminit.c:384-388` liest die drei Paare; `:724-726`
waehlt `n = (gl_nplanes > 2) ? 2 : gl_nplanes-1`; `set_aes_background`
`:634-641` ersetzt das **Low-Byte** des DESKTOP-ROOT-`ob_spec` `0x1143`):
Pi mit 16 Planes → `n = 2` → `bg[2] = 0x7B` → `0x1100 | 0x7B` = **`0x117B`**.
NewTOS rechnet identisch (`tos-api/src/aes/init.rs:305-313`). Der TT-Wert
`0x1143` ist das kompilierte `4419` ohne wirksame `#Q`-Aenderung.

## Einordnung

Das Delta ist die vom Anwender im Desktop gesetzte Hintergrundfarbe
(Set-Background-Dialog schreibt `#Q`), kein Defekt. Die Datei ist die
**heutige** Kartendatei, nicht die des Laufs vom 2026-09-09 — eine
archivierte Kopie gab es nie. Dass die Zeile damals denselben Wert trug,
folgt daraus, dass die Messung genau `0x117B` zeigt und nach der Regel kein
anderer Weg zu diesem Wert fuehrt.

## Pruefung

Keine Codeaenderung; `tos-api/src/aes/init.rs` Test `:429-437` prueft beide
`#Q`-Varianten weiterhin. Keine Hardwareabnahme noetig.
