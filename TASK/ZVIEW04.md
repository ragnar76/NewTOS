# ZVIEW04 — File > Quit does not quit zView while the browser window is open

Status: **OFFEN** · Prioritaet: **P2** · Typ: Defekt, Ursache unbekannt

## Sachverhalt

Anwender auf der echten Pi 4, Kernel `55eddb8f` (= HEAD `8435cd2`),
`ZVIEW.NAP` aus `sdk/zview/build-zview.sh`: Ist das Datei-Browserfenster von
zView offen, beendet *File > Quit* zView nicht. zView beendet sich erst, wenn
vorher das Browserfenster geschlossen wurde.

## Arbeit

Ursache mit Belegen klaeren, bevor gefixt wird: zViews Quit-Pfad lesen
(Menuehandler, WinDom `ApplExit`/`WindClose`/`WM_CLOSED`, Schliesslogik des
Katalog-/Browserfensters) und in QEMU mit AES-Trace und IKBD-Injektor
(`measurements/measurement-2026-10-07-zview02/method`) nachverfolgen, was beim
Quit mit offenem Browser passiert; gegen EmuTOS halten
(`references/emutos/aes`). Entscheiden: NewTOS-AES-Abweichung (dann 1:1 nach
EmuTOS fixen, Spec zuerst), Toolchain/LP64-Problem von zView/WinDom (dann
Overlay, Quellen unberuehrt) oder zViews eigenes Verhalten auch auf dem Atari
(dann berichten, nichts aendern).
