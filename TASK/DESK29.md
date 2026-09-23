# DESK29 — Eine Auswahl ueberlebt keinen Fensterwechsel

Status: **OFFEN** · Prioritaet: **P3** · Typ: Verhaltensabweichung — Nebenbefund DESK18

Abhaengigkeiten: —

## Sachverhalt

`main::top_msg` ruft `desk_clear(g_cwin)`, wenn ein anderes Fenster nach oben
kommt. Damit ist jede Auswahl beim Fensterwechsel weg, und EmuTOS' Fall
„etwas in einem **anderen** Fenster auswaehlen, dann `File → Open`" ist hier
nicht erreichbar.

Gefunden bei DESK18, wo er die zweite Stellung von *Default dir*
unerreichbar macht: EmuTOS' `do_aopen` waehlt das Verzeichnis des **obersten**
Fensters, wenn `G.g_appdir` aus ist — was voraussetzt, dass die Auswahl aus
einem anderen Fenster stammen kann.

Zu klaeren an der Quelle: wann genau loescht EmuTOS die Auswahl beim Toppen
(`desk/deskmain.c` `hndl_msg`s `WM_TOPPED`-Arm, `desk/desksupp.c`
`desk_clear`), und ob unser unbedingtes Loeschen eine Vereinfachung ist.

## Arbeit

Quelle lesen, die Bedingung angleichen, Abnahme mit zwei offenen Fenstern.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
