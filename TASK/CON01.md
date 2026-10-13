# CON01 — Spaltenzaehler (`glbcolumn`) und BDOS-Typeahead fuer die Konsole

Status: **OFFEN** · Prioritaet: **P3** · Typ: Konformitaet — Anwenderentscheidung 2026-10-13 bei der FS11-Spec

Abhaengigkeiten: [FS11](FS11.md) (Geraeteweiche)

## Sachverhalt

EmuTOS fuehrt je Geraet einen Spaltenzaehler (`glbcolumn[]`) und einen
64-Tasten-Typeahead (`bdos/console.c`). Darauf beruhen Tab-Expansion in
`tabout`/`cookdout`, das Neueinruecken in `cgets` (`newline(h, stcol)`),
Ctrl-S/Ctrl-Q-Flusskontrolle und die Typeahead-Uebernahme in `conbrk`.
NewTOS hat beides nicht (freigegebene Abweichung, `output_break` in
`tos-api/src/gemdos/console.rs`; `docs/api/gemdos/console/02-cconout.md`,
`09-cconws.md`). Die Abweichung betrifft gemeinsam `Cconout`, `Cconws`,
`Cconrs` und `Fwrite`/`Fread` auf CON und wird deshalb hier fuer alle
zusammen behandelt, nicht einzeln.

Nebenbei: `docs/api/gemdos/console/0a-cconrs.md` ist veraltet („Blocked on
Pterm“), obwohl `edit_line` existiert.

## Arbeit

1. Researcher: Zustand und Ablaeufe aus EmuTOS belegen; Konsequenz fuer den
   SPSC-Eingabering in NewTOS; Spec-Entwurf, Freigabe durch den Anwender.
2. Developer: umsetzen; Hosttests.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
