# DESK47 — `opn_appl`: der Parameterdialog fehlt, und ohne ihn ist `.NTP` sinnlos

Status: **OFFEN** · Prioritaet: **P2** · Typ: Fehlende EmuTOS-Funktion — Anwenderbefund Pi-Runde 2026-09-28

Abhaengigkeiten: [DESK44](DESK44.md) (dort als „nicht einloesbare Haelfte" vermerkt)

## Der Befund

Anwender, nach dem Doppelklick auf ein `.NTP`: *„da oeffnet sich kein
Argument-Eingabefenster; der Desktop blitzt kurz auf und das war's."*

**Genau so ist es gebaut.** DESK44 hat `.NTP` auf den Konsolenarm gestellt —
das war richtig und ist erledigt. Was fehlt, ist der Schritt davor:
`opn_appl` (`references/emutos/desk/deskpro.c:71-121`) fragt die
Parameterzeile ab, bevor `pro_run` startet. Ohne ihn laeuft ein `.NTP` mit
**leerer** Parameterzeile — und ist damit von einem `.NCP` nicht zu
unterscheiden.

Die Endung `.NTP` heisst „**T**akes params" (ADR-018). Ohne Dialog traegt sie
ihren Namen nicht.

## Was gebaut wird

`opn_appl` nach `deskpro.c:71-121`, in EmuTOS' Form und an EmuTOS' Stelle im
Ablauf: der Dialog kommt aus `pro_run`s Aufrufer, nicht aus dem Startpfad.
Der Baum ist `ADOPENAP` (`desk_rsc.c`), die Vorlage fuer den Nachbau sind
DESK42s `ADDISKIN`/`ADFFINFO` — Objekt fuer Objekt, gleiche Nummern, gleiche
Masse.

**Mitzuklaeren, weil es in derselben Ecke offen steht** (aus dem
U3-Statusbericht): `pro_run`s `(wh, curr)`-Zweig und `do_wopen`
(`desksupp.c:294`) fehlen ebenfalls. Der Zweig war **bis U3 unerreichbar** und
ist es seitdem nicht mehr — das ist eine echte Luecke, keine Formalie.

## Abnahme

* Doppelklick auf ein `.NTP` oeffnet den Parameterdialog; die eingegebene Zeile
  kommt beim Programm an. `C:\\CMDLINE.NTP` gibt sie aus und ist damit der
  Selbstbeleg.
* Screendump des **Konsolenschirms** waehrend des Laufs — der Nachweis, der
  fuer `.NCP` bereits vorliegt (`measurement-2026-09-27-desk46/screendumps/
  a3-3-console.png`) und fuer `.NTP` noch aussteht.
* Gegenprobe: `.NAP` bleibt GEM ohne Dialog, `.NCP` bleibt Konsole ohne Dialog.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
