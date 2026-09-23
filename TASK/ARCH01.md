# ARCH01 — Wettlauf beim EL0-Eintritt: eine IRQ zwischen `msr elr_el1` und `eret` verwirft das Sprungziel

Status: **OFFEN** · Prioritaet: **P1** · Typ: Belegter Defekt / Architektur — gefunden bei DESK19

Abhaengigkeiten: —

## Sachverhalt

`enter_el0_with_resume_capture`
([`kernel/src/arch/aarch64/user_entry.rs`](../kernel/src/arch/aarch64/user_entry.rs))
stellt das Sprungziel in drei Schritten ein und springt dann:

```
msr   elr_el1, {entry}
mov   {tmp}, #0          // SPSR.M = EL0t, DAIF = 0
msr   spsr_el1, {tmp}
eret
```

**Ueber dieses Fenster sind IRQs nicht maskiert.** Trifft eine zwischen dem
Schreiben von `ELR_EL1` und dem von `SPSR_EL1`, ueberschreibt der
Ausnahmeeintritt `ELR_EL1` mit dem unterbrochenen **Kern**-PC. Der Behandler
kehrt in diesen Block zurueck, die naechsten zwei Befehle setzen `SPSR_EL1`
brav auf `0` (EL0t) — und der `eret` springt nach **EL0 auf eine
Kernadresse**.

Genau das zeigt das Protokoll:

```
[launch] EL0: slot 0 entry 0x1000
[EXC] synchronous (non-SVC) exception (Instruction Abort (lower EL)):
      ESR=0x000000008200000e ELR=0xffff0000000c6988 SPSR=0x0000000000000000
[EXC] FAR=0xffff0000000c6988
```

`SPSR = 0` (unser Wert kam durch), `ELR`/`FAR` = Kern-VA, symbolisiert auf
`enter_el0_with_resume_capture + 0x6c`, also **hinter** dem `eret`; IFSC
`0x0e` = Permission Fault Level 2, was EL0 auf einer Kernseite genau ergibt.
Die Registerwerte passen ausschliesslich zu diesem Fenster.

**Haeufigkeit:** einmal in sieben Laeufen desselben Startskripts auf
identischer Kartenkopie (`measurement-2026-09-23-desk19-desk20/serial/`,
`d19-full-serial.log` gegen `d19-full-repeat1..4-serial.log`, RUNINFO §7).

**Wer betroffen ist:** *jeder* Aufrufer des Helfers — Programmstart per
Doppelklick, `\AUTO\`, `Pexec`, acht Selbsttests. Der Fehler ist aelter als
DESK19; DESK19 hat nur eine weitere Aufrufstelle und damit mehr Gelegenheiten
geschaffen.

## Warum P1

Ein Programmstart, der in etwa jedem siebten Versuch mit einer
Ausnahmemeldung endet, ist genau das, was am Vorfuehrtermin nicht passieren
darf — und es trifft den Weg, ueber den **jede** portierte Anwendung startet.

## Arbeit

Quelle: ARM ARM zu `ERET`, `ELR_EL1`, `SPSR_EL1` und dem Ausnahmeeintritt —
welche Register der Eintritt schreibt, und was `eret` aus `SPSR_EL1` in
`PSTATE.DAIF` zurueckholt. Danach das Fenster schliessen (die uebliche Form
ist `msr daifset, …` vor dem ersten Schreiben; der `eret` stellt DAIF aus
`SPSR_EL1` wieder her, das hier `0` ist, also laeuft EL0 mit offenen IRQs
weiter). **Vor dem Fix belegen, dass es der Wettlauf ist** — z. B. durch eine
kuenstlich eingesetzte Verzoegerung zwischen den beiden `msr`, die ihn
zuverlaessig ausloest; sonst ist der Fix eine Vermutung.

Abnahme: derselbe Startlauf **zwanzigmal** hintereinander ohne `[EXC]`, plus
der kuenstlich ausgeloeste Fall vorher/nachher.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten. Architekturdatei — Rule 6 der
CLAUDE.md: Registerverhalten nur aus der ARM ARM, nicht aus dem Gedaechtnis.
