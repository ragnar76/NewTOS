# ARCH02 — `Pexec`-Eltern verlieren ihre FP/SIMD-Register

Status: **OFFEN** · Prioritaet: **P2** · Typ: Belegter Defekt / Architektur — Nebenbefund ACC03

Abhaengigkeiten: [ACC03](ACC03.md) (erledigt)

## Sachverhalt

ACC03 hat einen echten Fehler gefunden und behoben: der **Scheduler**-Wechsel
sicherte `v0..v31`, `FPCR` und `FPSR` nicht. Das Symptom war unmissverstaendlich
— nach einem GEM-Programm trat ein Accessory mit dem `d1` **jenes Programms**
als AES-Opcode in den Trap, 110 484 `appl_exit`-Aufrufe aus einem Binaerstueck,
das die Funktion nie ruft; clang hatte eine Schleifenkonstante in `d1` geparkt.

**Derselbe Mechanismus ist auf dem `Pexec`-Pfad noch offen.** Der Fix deckt den
Schedulerwechsel; der Weg EL0-Elter → EL0-Kind → EL0-Elter
(`Pexec`/`Pterm`) sichert die FP/SIMD-Register weiterhin nicht.

## Warum P2 und nicht P4

Es ist heute latent — die Programme, die wir starten, gehen ueber
`run_program`, nicht ueber `Pexec` aus einem anderen EL0-Prozess. Aber:

* der Mechanismus ist **derselbe**, der gerade zwei Tage Diagnose gekostet
  haette, waere er nicht zufaellig an einer lauten Stelle aufgetreten;
* das Symptom ist Datenverfaelschung ohne Absturz, also die Sorte Fehler, die
  man erst drei Schichten spaeter bemerkt;
* und jede portierte Anwendung, die ein Hilfsprogramm startet, faellt hinein.

## Arbeit

`FpContext` und `save_fp_context`/`restore_fp_context` (ACC03,
`kernel/src/arch/aarch64/exceptions.rs`) auf den `Pexec`/`Pterm`-Pfad ziehen.
Vorher am Quelltext feststellen, **wo genau** dort der Elternrahmen gesichert
und wiederhergestellt wird — es ist nicht derselbe Ort wie beim
Schedulerwechsel.

Abnahme wie bei ACC03: ein Programm, das eine FP-Konstante ueber einen
`Pexec`-Aufruf hinweg haelt und sie danach prueft. Vor dem Fix muss es
scheitern, sonst misst der Test nicht, was er soll.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten. Architekturdatei — Registerverhalten
nur aus der ARM ARM.
