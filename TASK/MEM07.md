# MEM07 — feste Grenze `MAX_SHARED_BLOCKS = 64`

Status: **OFFEN** · Prioritaet: **P3** · Typ: Grenzwert — Nebenfund MEM05-Entwurf (2026-10-13)

`tos-api/src/bdos/mem/region.rs:45` begrenzt gemeinsam genutzte Bloecke fest
auf 64 — dieselbe Familie fester Grenzen wie der MD-Vorrat (MEM05). Belegen,
wer sie verbraucht und ob sie nach MEM05-Muster wachsen soll.

[Gemeinsame Regeln](README.md) gelten.
