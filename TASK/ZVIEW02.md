# ZVIEW02 — double-click on a folder tile in zView's browser only selects

Status: **OFFEN** · Prioritaet: **P2** · Typ: Defekt, Ursache unbekannt — aus [ZVIEW01](ZVIEW01-STATUS-DONE.MD)

## Sachverhalt

In QEMU (Injektor, zwei Druecke mit 3 bzw. 72 Ticks Abstand) waehlt ein
Doppelklick auf eine Ordnerkachel nur aus, der Ordner oeffnet sich nicht.
zView verlangt `nb_click == 2 && evnt.mbut == 1` (`catalog/catalog_mouse.c:132`,
`evnt_multi` mit `bclick 258`, `main.c:143`). Auf der echten Pi nicht geprueft.

Verdacht, nicht belegt: Klickzaehlung bzw. gemeldeter Tastenzustand am Ende des
Doppelklick-Fensters weicht von EmuTOS ab — siehe [A111](A111.md) Punkt 1
(`count_clicks` kehrt sofort zurueck, EmuTOS meldet erst am Fensterende,
`geminput.c:122`) und das `pr_button`-Modell aus LDG-M3-B1.

## Arbeit

Ursache mit Trace der `evnt_multi`-Rueckgaben belegen, gegen EmuTOS halten,
dann Fix (vermutlich zusammen mit A111).
