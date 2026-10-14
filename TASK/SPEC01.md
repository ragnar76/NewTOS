# SPEC01 — `docs/api/structures/object.md`: falsche Offsets des nativen OBJECT

Status: **OFFEN** · Prioritaet: **P3** · Typ: Spec-Korrektur — Nebenfund DESK43 U5b (2026-10-14)

`docs/api/structures/object.md:47-50` gibt fuer das native (LP64) OBJECT
`ob_spec` bei 0x0C und `ob_x` bei 0x14 an. Der Code liegt bei 0x10 bzw. 0x18
und ist seit U5b per Compile-Zeit-Assertion festgeschrieben
(`tos-types/src/aes.rs`). Researcher: Spec gegen die Assertions korrigieren,
auch `TedInfo` (40 B) und `IconBlk` (48 B) gegenpruefen.

[Gemeinsame Regeln](README.md) gelten.
