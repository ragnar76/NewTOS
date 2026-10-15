# DESK59 — `act_chg` prueft `ob_flags & DISABLED` statt `ob_state`

Status: **OFFEN** · Prioritaet: **P3** · Typ: Konformitaet — Nebenfund DESK43 U5b (2026-10-14)

`kernel/src/desk/act.rs:75` bricht ab, wenn `ob_flags & DISABLED` gesetzt ist.
EmuTOS `act_chg` (`desk/deskact.c:428-430`) prueft `ob_state & DISABLED`.
`DISABLED` (0x0008) ist ein State-Bit; im Flag-Wort ist 0x0008 `EDITABLE`.
Folge: ein Objekt mit `EDITABLE` wird faelschlich uebersprungen, ein wirklich
deaktiviertes nicht. Red-Test, dann Fix; Driftprobe/Orakel vorher-nachher.

[Gemeinsame Regeln](README.md) gelten.

## Abschluss — 2026-10-16

Status: **ERLEDIGT (QEMU), nicht committet.** `act_chg` prueft jetzt `ob_state & DISABLED`. Test: Desktop-Selbsttest
`[desk-act]` 0/3 → 3/3. Keine sichtbare Aenderung (45/45 Dumps byte-gleich zur Abnahme 2026-10-15). Quittung:
[DESK59-STATUS-DONE.MD](DESK59-STATUS-DONE.MD); Beleg `measurements/measurement-2026-10-16-desk59/RUNINFO.TXT`.
