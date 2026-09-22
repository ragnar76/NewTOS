# ACC05 — Format und Endung: was ist ein NewTOS-Accessory?

Status: **OFFEN** · Prioritaet: **P2** · Typ: Entscheidung + Werkzeugkette

Abhaengigkeiten: [ACC01](ACC01.md)

## Sachverhalt

Unsere eigenen Programme tragen `.NAP` (GEM) und `.NCP` (Konsole) mit
Grund: der spaetere m68k-Emulator soll `.PRG`/`.TOS`/`.TTP` uebernehmen
(Memory `own-extensions-separate-aarch64-from-m68k`). Fuer Accessories
gilt dasselbe: `.ACC` waere die m68k-Endung, ein natives AArch64-Accessory
braucht eine eigene (`.NAC`?). EmuTOS sucht `\*.ACC` im Wurzelverzeichnis
(`aes/geminit.c:272`).

## Arbeit

1. Endung entscheiden (Vorschlag `.NAC`) und in ACC02s Suche eintragen;
   `.ACC` bleibt dem Emulator vorbehalten.
2. SDK: Bauskript fuer ein Accessory (Muster `sdk/build.sh`/`build-libc.sh`),
   Beispiel `sdk/examples/clockacc.c` — `appl_init`, `menu_register`,
   `evnt_multi`-Schleife, `AC_OPEN` oeffnet ein Fenster, `AC_CLOSE`
   schliesst es, **kein** `appl_exit`.
3. `tools/make-sd-image.sh`: Installationsblock fuer Accessories im
   Wurzelverzeichnis von C:.
4. `docs/sdk/`: Seite oder Abschnitt „Ein Accessory portieren/schreiben".

## Abnahme

Das Beispielaccessory laeuft in QEMU, erscheint unter *Desk*, oeffnet und
schliesst sein Fenster; Dokumentation vorhanden.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
