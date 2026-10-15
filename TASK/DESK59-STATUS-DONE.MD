# DESK59 — STATUS: DONE (2026-10-16) — behoben, in QEMU belegt; nicht committet

**Auftrag:** `act_chg` prueft `ob_flags & DISABLED` statt `ob_state & DISABLED` (`TASK/DESK59.md`).

## Defekt

`programs/desktop/src/act.rs` `DeskState::act_chg` (vormals `kernel/src/desk/act.rs:75`):
`if self.g_screen[obid].ob_flags & DISABLED != 0 { return false; }`.
Soll: `references/emutos/desk/deskact.c:428-430` — `old_state = curr_state = tree[obj].ob_state; if (curr_state & DISABLED) return FALSE;`;
`docs/api/desktop/deskact.md` §`act_chg` (Zeile 196) und Fehlertabelle (Zeile 414) sagen dasselbe (Spec war richtig).
`DISABLED` = 0x0008 im State-Wort (`obdefs.h:104`), im Flag-Wort ist 0x0008 `EDITABLE` (`obdefs.h:86`).
Fix: eine Zeile, `ob_flags` → `ob_state`, Doc-Kommentar mit Zitat.

**Weitere Stellen gleicher Art:** keine. Grep ueber `programs/desktop/src` nach jedem State-Bit an `ob_flags`/`flags`, jedem
Flag-Bit an `ob_state`/`state` und nach Zahlenmasken an beiden Woertern: einziger Treffer `act.rs:73`.

## Test rot → gruen

Desktop-Crate ist `no_std` mit `test = false` → kein Hosttest moeglich ohne Cargo-Umbau; daher ein Desktop-Selbsttest
`act::selftest()` (selftest-desk-Build, QEMU), von `selftest::run_once` zuerst gerufen; aendert nur Flags/State eines
Desktopsymbols und stellt sie zurueck. Drei Faelle: EDITABLE im Flag-Wort waehlt aus; DISABLED im State verweigert Auswahl;
DISABLED|SELECTED verweigert Abwahl.

| Kernel | Ergebnis |
|---|---|
| `kernel8-kt-red.img` (Test, ohne Fix) | `[desk-act] 0 passed, 3 failed` |
| `kernel8-kt.img` (mit Fix) | `[desk-act] 3 passed, 0 failed`; gesamt 56 PASS / 1 FAIL (= 53/1 + 3), uebrige Zeilen = Abnahme 2026-10-15 |

## Sichtbare Wirkung

Keine — erwartet und gemessen. Kein `g_screen`-Objekt traegt je `EDITABLE` (obj_init/obj_ialloc: 0 oder `LASTOB`) oder
`DISABLED` im State; auch EmuTOS setzt `DISABLED` nur in Dialogbaeumen. 8 Ansichten + Serie (Desktop, APITEST, TeraDesk, QED),
EL0-Standarddesktop, on/off/strict: **45/45 Dumps byte-gleich** zur Abnahme `measurement-2026-10-15-desk43-acceptance`;
APITEST 106/0/5, 0 EXC/PANIC.

## Guards (docs/api/desktop/_rebuild-plan.md §12)

```
  DRIFT-PROBE TOTAL: 4 (before: 4)   DRIFT-PROBE UNSAFE: 20 (baseline 20)
  EMUTOS-CONSTS: total=466 matched=332 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=132
  DESK-CITE: fns=477 cited=316 newtos=5 uncited=156            (whole desktop; vorher 476/315/5/156)
  DESK-CITE --since 4d1919b3: rects_overlap (act.rs, unveraenderter Helfer, nur durch angehaengten Block im Hunk);
                              run_once (selftest.rs, bestehender NEWTOS-Marker); keine neuen NEWTOS-fns
  Oracle: not required: the desktop's svc calls are unchanged (one comparison in act_chg)
```

## Zahlen

| | Soll | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1370/0/1 | 1370/0/1 (kein neuer Hosttest, s.o.) |
| Warnungen rpi4 / qemu-virt | 64/94 | 64/94 beide |
| Clippy Kernel rpi4 / qemu-virt / +selftest-desk | 76/125 | 76/125 alle |
| Programme (build + clippy) | 0 | 0 |
| Drift / unsafe | 4 / 20 | 4 / 20 |
| emutos-consts | gruen | gruen (s.o.) |
| SDK | 20/0/0 | 20/0/0 |

Beleg: `measurements/measurement-2026-10-16-desk59/RUNINFO.TXT`.

## Offen

* Echte Pi 4 (keine HW-spezifische Wirkung erwartet: reine Zustandslogik). `desk=kernel`, qemu-virt, on-asid/verify nicht gefahren.

## Ausserhalb des Auftrags (je eine Zeile, nicht angefasst)

* `act_chg` kehrt bei unveraendertem Zustand frueh zurueck und setzt dann `f_selected` nicht; EmuTOS setzt `fn->f_selected` immer (`deskact.c:447-449`) — beobachtbar nur, wenn die beiden je auseinanderlaufen.
* `tools/inject-arm.py` T16 weiterhin offen (korrigierte Kopie im Messordner verwendet).

## Selbstpruefung

Aufgabe in einem Satz: `act_chg` prueft `DISABLED` im `ob_state` wie `deskact.c:428-430`, mit Rot-Gruen-Test.
Geaendert: `programs/desktop/src/act.rs`, `programs/desktop/src/selftest.rs`; diese Quittung, `TASK/DESK59.md` (Abschluss),
das Messverzeichnis. Beide Boards bauen (64/94, Clippy 76/125). Nicht committet.
