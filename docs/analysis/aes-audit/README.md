# AES 1:1-Divergenz-Audit — NewTOS-Impl vs EmuTOS-Quelle

> Stand: 2026-07-22, Branch `desk-rebuild`. Ziel: **1:1 EmuTOS**, kein Auslassen,
> keine Stubs, keine Abkürzungen. Dieser Ordner ist der Phase-1-Katalog
> (read-only Audit, kein Produkt-Code). Phase 2 = serieller Fix-Loop
> (Developer → Tester → Lead-Commit) gegen diesen Katalog.

Leitprinzip (Memory `no-rationalizing-emutos-deviations`): bei jeder Abweichung
ist der Default **„match it"** — EmuTOS `file:line` zitieren + 1:1-Fix, niemals
„benign/nobody reads it". Soll-Verhalten-Referenz: `docs/api/aes/*.md` und
`docs/api/_discrepancies.md` (D-047..D-054 = AES-Spec-Findings).

## Pakete

| Paket | Katalog | Unsere Datei(en) | EmuTOS-Quelle |
|---|---|---|---|
| P1 appl/pd/dispatch | `p1-appl-pd-dispatch.md` | `aes/appl.rs`, `aes/pd.rs`, `aes/mod.rs` | `gemaplib.c`, `gempd.c`, `geminit.c`, `gemsuper.c` |
| P2 event/queue/ctrl | `p2-event-queue-ctrl.md` | `aes/event.rs` | `gemevlib.c`, `gemdisp.c`, `gemqueue.c`, `geminput.c`, `gemctrl.c`, `gemasync.c`, `gemflag.c` |
| P3 object | `p3-object.md` | `aes/object.rs` | `gemoblib.c`, `gemobjop.c`, `gemobed.c` |
| P4 form | `p4-form.md` | `aes/form.rs` | `gemfmlib.c`, `gemfmalt.c` |
| P5 window/graf | `p5-window-graf.md` | `aes/window.rs`, `aes/graf.rs` | `gemwmlib.c`, `gemwrect.c`, `gemgrlib.c`, `gemgraf.c`, `gemgsxif.c` |
| P6 rsrc/fsel/scrp/shel | `p6-rsrc-fsel-scrp-shel.md` | `aes/resource.rs`, `aes/fsel.rs`, `aes/shell.rs` | `gemrslib.c`, `gemfslib.c`, `gemsclib.c`, `gemshlib.c` |

## Finding-Format (einheitlich über alle Pakete)

Jeder Katalog hat zwei Teile:

**1. Coverage-Tabelle** — *jede* EmuTOS-Public-Funktion im Scope:

| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| … | gemXlib.c:NN | aes/x.rs:MM | impl / partial / **MISSING** / **STUB** | yes/no |

**2. Divergenz-Findings** — pro Abweichung:

```
### AUD-P<n>-NNN: <fn> — <one-line>
- Severity: BLOCKER | HIGH | MEDIUM | LOW
- EmuTOS:  <file>:<line> — <was EmuTOS tut>
- NewTOS:  <file>:<line> — <was unser Code tut / MISSING / STUB>
- Divergenz: <exakter Verhaltens-/ABI-/Return-Code-/Auslassungs-Unterschied>
- 1:1-Fix:  <was der Code werden muss, um EmuTOS zu matchen>
- Runtime-verify: yes/no  (QEMU/HW-Repro in Phase 2 nötig?)
```

Severity-Leitfaden: BLOCKER = falscher Return/ABI/Crash-Pfad oder fehlende
Public-Funktion; HIGH = falsches sichtbares Verhalten/Message-Layout; MEDIUM =
Edge-Case/Fehlercode; LOW = Kosmetik/Kommentar/Reihenfolge ohne Effekt.
