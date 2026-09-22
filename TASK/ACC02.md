# ACC02 — `.ACC` laden: `count_accs`/`load_one_acc` im Startpfad

Status: **OFFEN** · Prioritaet: **P2** · Typ: Funktionsluecke

Abhaengigkeiten: [ACC01](ACC01.md) (Spec), [A106](A106.md) (beruehrt denselben Startpfad)

## Sachverhalt

EmuTOS sucht beim Start bis zu `NUM_ACCS = 6` Dateien `\*.ACC` im Wurzel-
verzeichnis des Bootlaufwerks (`aes/geminit.c:265-284`), laedt jede per
`pgmld` und startet sie als eigenen Prozess (`:236-257`), **vor**
`sh_tographic` und dem Desktop (`:686`), und gibt ihren Speicher nach
`sh_main` wieder frei (`:288-303, 746`). NewTOS hat davon nichts.

## Arbeit

1. Laden in `kernel/src/shell_loop.rs`/`lib.rs` an der Stelle, die EmuTOS'
   `run_accs_and_desktop` entspricht (vor `sh_tographic`; unser Aequivalent
   benennen). Dateisuche ueber unsere GEMDOS-Fsfirst/Fsnext, Ladeweg ueber
   den vorhandenen ELF-Lader (`tos-api/src/bdos/process/loader.rs`) —
   `.ACC` ist bei uns ein natives `.NAP`-artiges Programm; Dateiendung und
   Format im Ticket ACC05 entschieden.
2. Prozess je Accessory ueber den kooperativen Unterbau (`sched.rs`,
   `wait_mesg`), **ohne** `Pterm` am Ende — ein Accessory laeuft weiter.
3. `free_accs`-Aequivalent beim Herunterfahren.
4. Bootflag „Accessories ueberspringen" (EmuTOS `BOOTFLAG_SKIP_AUTO_ACC`,
   Control beim Start) — unsere Entsprechung benennen (Tastatur ueber IKBD
   zu diesem Zeitpunkt verfuegbar? sonst `NEWDESK.INF`-Schalter).

## Abnahme

Hosttest fuer die Suche (bis 6, Reihenfolge, fehlende Dateien); QEMU raspi4b
mit einem Test-`.ACC` auf C:\ — Serial zeigt Laden und Prozessstart, Desktop
kommt hoch, `wait_for_accs`-Aequivalent blockiert nicht ewig.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
