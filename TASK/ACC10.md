# ACC10 — `appl_exit` blockiert headless fuer immer in `wait_for_accs`

Status: **OFFEN** · Prioritaet: **P2** · Typ: Belegter Defekt — aus PORT05 herausgefallen

Abhaengigkeiten: —

## Der Befund, gemessen

`appl_exit()` blockiert in `wait_for_accs(AP_ACCLOSE)`, und das schlaeft auf
der Weckklasse **`WAIT_INPUT`** (`tos-api/src/aes/acc.rs:164`). Die
Accessory-Schranke treibt diese Klasse nicht; `idle_wake_pass` weckt sie nur
bei **Taste oder Zeigerbewegung**. **Headless kommt das Programm nie
zurueck.**

**Unabhaengig von PORT05 gemessen:** ein vom Kernel gestartetes Programm
(kein `Pexec`) bleibt an derselben Stelle stehen, und
`measurement-2026-09-25-port05/serial/control-kernel-launch-unfixed.log`
gegen `...-fixed.log` sind **bis auf die Uhrzeile identisch**.

## Warum es bisher niemand gesehen hat

Interaktiv faellt es nicht auf: eine bewegte Maus weckt die Klasse. Und im
`Pexec`-Fall war der Stillstand vom PORT05-Defekt **verdeckt** — der Elter
wurde ohnehin neu gestartet, bevor jemand auf das Ende des Kindes wartete.

## Warum es zaehlt

Jeder headless-Lauf, der ein Programm bis zu seinem Ende fahren will —
Messungen, Selbsttests, `#Z`-Autorun-Ketten — bleibt hier haengen. Fuer die
Vorfuehrung selbst ist es harmlos, fuer unsere Messwerkzeuge nicht.

## Auftrag

Gegen EmuTOS halten: auf welcher Klasse wartet `wait_for_accs` dort, und wer
weckt sie (`aes/geminit.c:599-630`, `WAIT_TIMEOUT 500`). Der
**Zeitgrenzen**-Anteil ist der verdaechtige Unterschied — EmuTOS wartet
begrenzt, nicht unbegrenzt. Erst lesen, dann entscheiden.

## Abnahme

Ein headless-Lauf, in dem ein Programm `appl_exit` durchlaeuft und
terminiert, ohne dass jemand die Maus bewegt.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
