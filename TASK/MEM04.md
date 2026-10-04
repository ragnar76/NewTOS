# MEM04 — `usermem::free` laesst die Abbildung eines freigegebenen Blocks stehen

Status: **OFFEN** · Prioritaet: **P2** · Typ: Isolationsluecke — Nebenfund aus [ARCH03](ARCH03.md) Schritt 2

Abhaengigkeiten: — (wird mit ARCH03 Stufe 1/3 ohnehin neu gebaut; bis dahin eigenstaendig)

## Sachverhalt (Architekt, `docs/analysis/arch03-implementation-plan.md`)

`tos-api/src/bdos/process/usermem.rs:596-604`: nach `Mfree` bleibt die
EL0-Abbildung des Blocks im Adressraum des bisherigen Besitzers bestehen.
Vergibt der Pool den Block danach an einen anderen Prozess, kann der alte
Prozess weiter hineinschreiben. Unter EmuTOS gibt es keinen Schutz; bei uns
widerspricht es dem eigenen Isolationsversprechen von ADR-021.

## Arbeit

Zuerst gegen den Quelltext belegen (Lead hat den Fund noch nicht selbst
nachgelesen), Wirkung zeigen (Test: Prozess A gibt frei, B bekommt den Block,
A schreibt), dann Abbildung beim Freigeben entfernen (TLB-Pflege nach
QUIRK-001 beachten) oder bewusst bis ARCH03 zurueckstellen.
