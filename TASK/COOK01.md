# COOK01 — `_MCH` = 0x00050000 ist die ARAnyM-Kennung

Status: **OFFEN — klaeren** · Prioritaet: **P3** · Typ: Konformitaet — Hinweis von aussen (2026-10-19)

## Befund

`tos-api/src/cookie.rs:223` setzt `_MCH = 0x0005_0000` als vermeintlich
NewTOS-eigene Kennung (Spec `docs/api/bios/_cookie-jar.md:53,136,148`). Der Wert
ist aber belegt: `MCH_ARANYM` (`references/emutos/include/cookie.h:57`;
https://freemint.github.io/tos.hyp/de/bios_cookiejar.html#Cookie_2C_20_MCH).
Ein Programm, das `_MCH` auswertet, haelt NewTOS fuer ARAnyM.

## Moegliche Werte (zu klaeren)

- EmuTOS ohne Atari-Hardware (`CONF_ATARI_HARDWARE` aus): `MCH_NOHARD =
  0xFFFFFFFF` „No ST-compatible hardware“ (`bios/machine.c:400-401`,
  `include/cookie.h:50`) — passt zu NewTOS (kein Shifter/MFP/Videl).
- Was setzen FireBee, Milan (`0x00040000`), Hades, CT60, MiSTer/andere
  Nachbauten? Liste der vergebenen Werte (tos.hyp, FreeMiNT `sys/cookie.h`,
  FireBee-TOS/EmuTOS-ColdFire) zusammentragen; gibt es eine Vergabestelle
  oder Konvention fuer neue Maschinen?
- Wer liest `_MCH` ueberhaupt (SDK-Programme, mintlib, cflib, gemlib, TeraDesk,
  QED, zView)? Anwender: fuer die meisten Anwendungen vermutlich egal.

Nebenbei: `_AKP` steht auf US (`cookie.rs:225`), die Karte ist deutsch
voreingestellt — pruefen, ob `_AKP` der geladenen Tastaturbelegung folgen muss.

## Auftrag

Recherche (Researcher), dann Anwenderentscheidung, dann Spec + Code + Hosttest.

[Gemeinsame Regeln](README.md) gelten.
