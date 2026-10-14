# DESK59 — `act_chg` prueft `ob_flags & DISABLED` statt `ob_state`

Status: **OFFEN** · Prioritaet: **P3** · Typ: Konformitaet — Nebenfund DESK43 U5b (2026-10-14)

`kernel/src/desk/act.rs:75` bricht ab, wenn `ob_flags & DISABLED` gesetzt ist.
EmuTOS `act_chg` (`desk/deskact.c:428-430`) prueft `ob_state & DISABLED`.
`DISABLED` (0x0008) ist ein State-Bit; im Flag-Wort ist 0x0008 `EDITABLE`.
Folge: ein Objekt mit `EDITABLE` wird faelschlich uebersprungen, ein wirklich
deaktiviertes nicht. Red-Test, dann Fix; Driftprobe/Orakel vorher-nachher.

[Gemeinsame Regeln](README.md) gelten.
