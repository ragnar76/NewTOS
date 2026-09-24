# GUI04 — `CONF_WITH_EXTENDED_MOUSE`: Mausrad und Tasten 3 bis 5

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke IKBD/AES — aus der Schaltertabelle (REF01)

Abhaengigkeiten: —

## Sachverhalt

`CONF_WITH_EXTENDED_MOUSE` ist im Referenzbau **1**
(`references/emutos/include/config.h:1491-1498`): EmuTOS wertet die
IKBD-Scancode-Erweiterung fuer Maustasten 3/4/5 und das **Mausrad** aus
(`references/emutos/bios/aciavecs.S`, `bios/ikbd.c:748`).

`kernel/src/ikbd.rs` entschluesselt nur das Standardpaket (relative Bewegung,
zwei Tasten); `0xF6` gilt dort als Statusbericht-Kopf, eine Rad- oder
Zusatztastenauswertung gibt es nicht.

**Der Punkt, der es interessant macht:** die VDI-Seite ist schon da —
`vex_wheelv` (`tos-api/src/vdi/mouse.rs:710`) kann einen Radvektor
installieren. Es fuettert ihn nur niemand. Ein Programm, das sich dort
eintraegt, wartet bei uns vergeblich.

## Arbeit

Die Scancode-Erweiterung in `ikbd.rs` nachziehen und an `vex_wheelv` binden.
Abnahme braucht echte Hardware mit Rad-Maus; in QEMU ist der Pfad nur bis zur
Dekodierung pruefbar.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
