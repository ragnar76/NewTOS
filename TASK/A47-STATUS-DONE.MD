# A47 — Quittung

**Status: DONE (Hostbeleg + QEMU-Smoke; echte Pi 4 offen)** · 2026-09-20 ·
uncommitted im Baum (Basis `ecc7ca5`). `TASK/A47.md`s Kopfzeile bleibt
unveraendert.

## Soll (gepinnte Quelle `1a7484ab`, `aes/gemoblib.c`)

`ob_order` (`:1201-1246`) haengt das Objekt mit `ob_delete` (`:1147-1193`)
aus und wieder ein, ohne das Ausgehaengte je zurueckzunehmen. Alleinkind `1`
unter `ROOT`, `objc_order(1, 0)`, Zeile fuer Zeile:

| Zeile | Anweisung | Wirkung |
|---|---|---|
| `:1211` | `parent = get_par(tree, 1)` | `1.ob_next = 0`, `0.ob_tail == 1` → `parent = ROOT` |
| `:1215` → `:1157-1158` | `nextsib = 1.ob_next` (= `ROOT`), `parent = ROOT` | |
| `:1162-1171` | `ROOT.ob_head == 1` **und** `ROOT.ob_tail == 1` → `nextsib = NIL`, `ROOT.ob_tail = NIL` | Tail weg |
| `:1175` | `ROOT.ob_head = nextsib` = `NIL` | Head weg; `1.ob_next` bleibt `ROOT` |
| `:1216` | `chg_obj = ROOT.ob_head` | `NIL` |
| `:1217-1220` | `new_pos == 0`: `1.ob_next = chg_obj` (= **`NIL`**), `ROOT.ob_head = 1` | |
| `:1242-1243` | `if (1.ob_next == parent)` → `NIL == 0`? nein | `ROOT.ob_tail` bleibt **`NIL`** |
| `:1245` | `return TRUE` | `int_out[0] = 1` |

| Feld | vorher | EmuTOS nachher | NewTOS bisher | NewTOS jetzt |
|---|---|---|---|---|
| `ROOT.ob_head` | 1 | 1 | 1 | 1 |
| `ROOT.ob_tail` | 1 | **NIL** | 1 | **NIL** |
| `ROOT.ob_next` | NIL | NIL | NIL | NIL |
| `1.ob_next` | ROOT | **NIL** | ROOT | **NIL** |
| `1.ob_head` / `1.ob_tail` | NIL | NIL | NIL | NIL |
| `int_out[0]` | | 1 | 1 | 1 |

Der Befund des Tickets (`ob_tail = NIL`, `ob_next = NIL`) **ist** EmuTOS'
Ergebnis — ein Baum, dessen Kette nicht mehr auf den Elternknoten
zurueckfuehrt. Projektregel: angleichen; in der Spec benannt.

**Alleinkind mit `new_pos = NIL`** (und jedes `new_pos != 0`): nach dem
Delete ist `chg_obj = ROOT.ob_tail = NIL` (`:1226`), `chgptr = treeptr + NIL`
(`:1237`) — EmuTOS liest `tree[-1].ob_next` in `1.ob_next` und schreibt
`tree[-1].ob_next = 1` (`:1238-1239`). Speicher **vor** dem Array, kein Soll
(so auch das Inventar zu `new_pos = 99`). NewTOS behaelt hier sein definiertes
Ergebnis (Objekt wieder Alleinkind: `1.ob_next = ROOT`, `ROOT.ob_head =
ROOT.ob_tail = 1`) und schreibt nichts ausserhalb des Baums.

**Zwei Kinder** (1, 2): `objc_order(2, 0)` → `ROOT: head 2, tail 1`,
`2.next = 1`, `1.next = ROOT`; `objc_order(1, NIL)` (der `wm_mktop`-Zug,
`gemwmlib.c:1609`) → dasselbe Bild. Beide Seiten geschlossen und gleich.

## Was gebaut wurde

| Teil | Datei / Funktion | Stand |
|---|---|---|
| Fix | `tos-api/src/aes/object.rs` `ob_order`: der fruehe Zweig „parent leer → Alleinkind neu einhaengen" ist weg; der Kopfzweig laeuft wie `:1217-1220` mit `chg_obj = NIL` durch. Fuer `new_pos != 0` mit `chg_obj == NIL` (EmuTOS: `tree[-1]`) bleibt das Alleinkind-Einhaengen, jetzt hinter der Positionswahl; die Klemme fuer `new_pos` jenseits des Endes bleibt | fertig |
| Test | `tos-api/tests/aes_object_order_links.rs` (neu, 5 Faelle ueber den Dispatcher, Baum zwischen zwei Zaunrecords `0x7777`, nach jedem Aufruf Feld fuer Feld `ob_next`/`ob_head`/`ob_tail` aller Beteiligten **und** beide Zaeune): (a) Alleinkind pos 0 → `(NIL,1,NIL)` / `(NIL,NIL,NIL)`; (b) Alleinkind pos −1 → Alleinkind, Zaeune unberuehrt; (c) zwei Kinder, letztes nach vorn; (d) zwei Kinder, erstes nach hinten; (e) ROOT → 0, Baum unveraendert | 5/5 |
| Spec | `docs/api/aes/object-form.md` § `objc_order`: Callout mit Herleitung, Feldtabelle und den beiden Faellen ohne Soll; Edge-Case-Punkt `objc_order(obj, 0)` ergaenzt | nachgezogen |

`ob_order` ist nur ueber Opcode 45 erreichbar (`mod.rs:585`); kein Aufrufer
im Kernel/Desktop.

## Geprueft (Belege in `measurements/measurement-2026-09-20-a47-a52-a55-a58/`, `manifest.sha256`)

* **Rot vor / gruen nach** (`host-tests/a47-red-before.txt`,
  `a47-green-after.txt`): vor dem Hunk faellt genau (a) —
  `ROOT (next, head, tail): left: (-1, 1, 1) right: (-1, 1, -1)`; (b)–(e)
  gruen. Danach 5/5.
* `cargo run -p xtask -- test`: **648 / 0 / 1** (Baseline 637 + 5 + 5 + 1
  ueber A47/A55/A52).
* **Boards** (`build/`): rpi4 / qemu-virt rustc 64/94, Clippy 81/125 — Delta 0;
  Testdatei clippy-sauber.
* **QEMU-Smoke** (`screendumps/smoke-desk-up.png`): Produktkernel, unser
  Desktop, 0 Pixel / bytegleich zu
  `measurements/measurement-2026-09-19-a46/screendumps/smoke-desk-up.png`; Serial ohne
  `EXC`/`panic`.

## Offen

- Tester-Abnahme (Boot + `svc`-Selftests + Screendumps + `fsck_msdos`, beide
  Boards); echte Pi 4. Fuer diese Aenderung ist der Hosttest der Beleg; kein
  Zeichen-/Ladepfad beruehrt, kein neuer Selftest zu verdrahten.
- Nicht committet; `docs/STATUS.md` nicht angefasst; keine Tabelle geaendert.
