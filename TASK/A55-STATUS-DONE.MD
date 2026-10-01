# A55 — Quittung

**Status: DONE (Hostbeleg + QEMU-Smoke; echte Pi 4 offen)** · 2026-09-20 ·
uncommitted im Baum (Basis `ecc7ca5`). `TASK/A55.md`s Kopfzeile bleibt
unveraendert.

## Soll (gepinnte Quelle `1a7484ab`)

| Zeile | Befund |
|---|---|
| `aes/gemsuper.c:85` | `ret = TRUE;` vor dem `switch` (das Ticket nennt `:84`; am gepinnten Stand ist es `:85`) |
| `:204-206` | `case OBJC_ADD: ob_add(...); break;` — `ret` wird nicht angefasst |
| `aes/gemoblib.c:1121-1141` | `void ob_add`; der Waechter `if ((parent != NIL) && (child != NIL))` (`:1127`) entscheidet nur, ob geschrieben wird |
| `tosdump/cases/aes-r4-subfunctions.tsv` 550–552 (CRC `227422B2`) | `objc_add(NIL, NIL)`, `(NIL, ROOT)`, `(ROOT, NIL)` mit `AIN_LEN 1` und **ohne Baum** (Zeigerslot = Runner-Poison): TT `[0] = 1`, NewTOS `[0] = 0` — D-3 |

Entscheidung: **Rueckgabe 1 immer.** Die NewTOS-Waechter (Baum null,
`parent < 0`, `child < 0` — EmuTOS' Waechter, auf jeden negativen Index und
den Nullzeiger erweitert; EmuTOS wuerde dort mit dem negativen Wert
indizieren bzw. durch den Nullzeiger schreiben) bleiben als Schutz und
entscheiden nur noch, ob `ob_add` laeuft. Der Desktop-Wrapper
`kernel/src/desk/mod.rs::objc_add` liest die Rueckgabe nicht.

## Was gebaut wurde

| Teil | Datei / Funktion | Stand |
|---|---|---|
| Fix | `tos-api/src/aes/object.rs` `objc_add`: `if !tree.is_null() && parent >= 0 && child >= 0 { ob_add(...) } pb.set_ret(1);` | fertig |
| Test | `tos-api/tests/aes_object_add_return.rs` (neu, 5 Faelle ueber den Dispatcher, `AES_PARAMS(40,2,1,1,0)`): gueltiger Fall (zwei Kinder nacheinander, Links Feld fuer Feld, 1 beide Male); `parent = NIL` und `child = NIL` mit echtem Baum → 1, alle Links unveraendert; die drei r4-Faelle 550–552 exakt nachgestellt (Zeigerslot = Poison) → 1, `int_out[1]` Poison; Nullbaum → 1 | 5/5 |
| Spec | `docs/api/aes/object-form.md` § `objc_add`: Absatz „The answer is `1` in the no-op cases too" mit Quellzeilen, r4-Faellen und NewTOS-Waechtern; Zeile in der Fehlertabelle | nachgezogen |

## Geprueft (Belege in `measurements/measurement-2026-09-20-a47-a52-a55-a58/`)

* **Rot vor / gruen nach** (`host-tests/a55-red-before.txt`,
  `a55-green-after.txt`): vor dem Hunk fallen genau die vier Waechterfaelle
  (`left: 0 right: 1`, Fall 550 als erster), der gueltige Fall ist gruen.
  Danach 5/5.
* `cargo run -p xtask -- test`: **648 / 0 / 1**. Boards rpi4 / qemu-virt
  rustc 64/94, Clippy 81/125 — Delta 0; Testdatei clippy-sauber.
* QEMU-Smoke unser Desktop bytegleich zu a46 (`screendumps/smoke-desk-up.png`)
  — der Desktop baut seine Fensterboxen ueber diesen Aufruf auf
  (`desk/global.rs:418`), die Rueckgabe wird dort nicht gelesen.

## Offen

- Tester-Abnahme; echte Pi 4. Gegenmessung mit der r4-Tabelle auf der
  Hardware (Faelle 550–552 muessen jetzt `1` liefern) — der Hosttest stellt
  die drei Faelle wortgleich nach.
- Nicht committet; `docs/STATUS.md` nicht angefasst; keine Tabelle geaendert.
