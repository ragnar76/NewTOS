# ZVIEW01 — zView browser shows no file or folder names

Status: **OFFEN** · Prioritaet: **P1** (Vorfuehrung) · Typ: Defekt, Ursache unbekannt — Anwender auf der echten Pi 4, 2026-10-07

## Sachverhalt

Kernel HEAD `3895575` (release), `ZVIEW.NAP` sha `9c6497ad…`, Karte nach dem
Aufraeumen vom 2026-10-07. Im Datei-Browser von zView (Baum links, Kacheln
rechts) erscheinen Laufwerke, Ordner und Dateien nur als Symbole; **weder
Datei- noch Ordnernamen werden gezeichnet** (Foto des Anwenders, Baum mit
aufgeklappten Ordnern, Kacheln mit vier Ordnern und zwei Dateien ohne Text).

## Arbeit

Erst die Ursache mit Belegen (welcher VDI/AES-Aufruf zeichnet die Namen in
zView/WinDom — Textausgabe, Schriftwahl, Farbe, Clipping, Schreibmodus — und
was NewTOS dort anders macht als EmuTOS), in QEMU nachstellen, dann Fix
nach EmuTOS.
