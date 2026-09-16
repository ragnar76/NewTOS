# Referenzbilder: EmuTOS-AES-Dialoge in 3D, deutsch

Stand 2026-09-17. Diese Bilder sind das **Soll** fuer unsere AES-Dialoge.

## Woher

`https://tho-otto.m68k.eu/rscview/de/aes.php?aes3d=1` — Thorsten Ottos
`rscview` rendert EmuTOS' eigene AES-Ressource, in der gewaehlten Sprache und
**mit eingeschaltetem 3D**. Das ist genau unser Zielprofil
(`CONF_WITH_3D_OBJECTS`, Falcon030/TOS4), und damit **besser als ein
Hatari-Lauf**: das Hatari-Buendel baut flach, dort ist die 3D-Optik gar nicht
zu sehen.

Bildmuster: `aes/<nnn>_de_<name>_<zeitstempel>_3d.png`. Der Zeitstempel im
Namen wandert mit dem Bau der Seite — beim naechsten Holen die Uebersichtsseite
neu lesen, nicht den hier abgelegten Namen raten.

## Was hier liegt

| Datei | Inhalt |
|---|---|
| `fselectr.png` | Dateiauswaehler — das Soll fuer [A63](../../../TASK/A63.md) und [A64](../../../TASK/A64.md) |
| `dialert.png` | Alarmbox |
| `desktop.png` | Desktop-Hintergrundbaum |
| `al00crt.png` | eine der Fehlerboxen |
| `newtos-pi4-fsel.jpg` | **unser** Stand, Foto vom Schirm einer echten Pi 4 |

Nicht geholt (auf der Uebersichtsseite vorhanden): `AL01CRT`–`AL05CRT`,
`AL18ERR`, `AL04ERR`, `AL05ERR`, `AL15ERR`, `AL08ERR`, `ALXXERR`, `ALNOFUNC`,
`ALFSMEM`, `ITEMSLCT`, `NOTEBB`, `QUESTBB`, `STOPBB`.

## Der zweite Referenzweg

`tools/hatari-qed.sh` startet EmuTOS 1.4 auf einem emulierten Atari TT mit der
m68k-Fassung von QED. Der taugt fuer **Verhalten und Anwendungsdialoge** (QEDs
eigene Fenster, die `rscview` nicht kennt), nicht fuer die 3D-Optik.

## Wie zu vergleichen ist

**Nicht nach Augenschein.** In diesem Projekt sind Bildschirmfotos schon
dreimal ueberinterpretiert worden — zuletzt am 2026-09-17, als der Lead das
korrekte kursiv-schraffierte Ausgrauen gesperrter Laufwerksbuchstaben fuer
Zeichenmuell hielt. Kanten punktweise vergleichen, und einen Unterschied erst
benennen, wenn die Quellzeile dazu gefunden ist.
