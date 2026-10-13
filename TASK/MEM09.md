# MEM09 — `memprot::on_alloc` ignoriert den Rueckgabewert von `record`

Status: **OFFEN** · Prioritaet: **P3** · Typ: Robustheit — Nebenfund MEM05-Entwurf (2026-10-13)

Schlaegt `record` fehl (Tabelle voll), faellt der Block still auf `PROT_P`
zurueck. MEM05 reserviert die Kapazitaet kuenftig vor `ffit`; dieser Fall
soll trotzdem nicht still bleiben. Pruefen, ob nach MEM05 noch erreichbar;
sonst als Invariante absichern.

[Gemeinsame Regeln](README.md) gelten.
