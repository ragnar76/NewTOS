# BOOT01 — Bootschirm: Layout auf hohen Aufloesungen, Ausrichtung, Tippfehler

Status: **OFFEN** · Prioritaet: **P3** · Typ: Optik / Feinschliff — Anwenderbefund am Geraet (2026-09-23)

Abhaengigkeiten: —

## Herkunft

Fotos vom laufenden System (Pi 4, 1920×1080). Das Bild ist im Kern richtig —
Rahmen, Logo, Trennlinien und Ausrichtung des Infoblocks stimmen; eine
fruehere Lesart „Linien laufen ins Leere" war ein Ausschnittsfehler und ist
hiermit zurueckgenommen.

## Die Stellen (`kernel/src/initinfo.rs`)

1. **Der Inhalt sitzt im oberen Drittel.** Die Layoutschritte sind feste
   Pixelwerte um den 8×16-Font (`y += 18/22/24/26`, `:219-321`), der Rahmen
   spannt dagegen den ganzen Schirm (`rect(16, 12, sw-17, sh-13)`, `:195-197`).
   Bei 640×480 fuellt das; bei 1920×1080 bleiben zwei Drittel leer.
   Zu entscheiden: Block vertikal zentrieren **oder** Text auf hohen
   Aufloesungen doppelt skalieren (EmuTOS behaelt 8×16, faehrt aber 640×480).
2. **Der USB-Block ist linksbuendig** (`tx = fx0 + 40`, `:259`), alles andere
   zentriert auf `cx = sw / 2` (`:192`).
3. **Alle vier USB-Geraete melden „Port 1"** (`:277`, `d.port`). Bei einem Hub
   waere das der Downstream-Port; vier Geraete auf demselben ist verdaechtig.
   Gegen die enumerierte Topologie pruefen (`kernel/src/drivers/usb/`), ggf.
   Hub-Adresse mit ausgeben.
4. **Tippfehler**: „This operating system isnt Original" (`:222`) → „isn't".
5. Die beiden *Hold*-Zeilen: „Hold \<Shift\> to pause this screen" ist
   invertiert, „Hold \<Control\> to skip AUTO/ACC" nicht (`:307-317`) —
   pruefen, ob beabsichtigt; sonst vereinheitlichen.

## Abnahme

Screendump in QEMU (640×480) **und** auf der Pi (1920×1080): Block sinnvoll
platziert, alle Zeilen gleich ausgerichtet, Portnummern belegt, Text fehlerfrei.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten. Sichtbefunde erst am Pixel belegen,
dann benennen.
