# A52 — Quittung

**Status: DONE (Hostbeleg + QEMU-Smoke; echte Pi 4 offen)** · 2026-09-20 ·
uncommitted im Baum (Basis `ecc7ca5`). `TASK/A52.md`s Kopfzeile bleibt
unveraendert.

## Was EmuTOS tut (gepinnte Quelle `1a7484ab`, `aes/gemwmlib.c`)

Es gibt **keinen Zaehler**. Die z-Ordnung ist die Geschwisterkette des
Fensterbaums: `wm_open` haengt das Fenster mit `w_obadd(W_TREE, ROOT,
w_handle)` ans Ende (`:186-201`, Aufruf `:1425`), `wm_mktop` schiebt das
getoppte Fenster mit `ob_order(gl_wtree, w_handle, NIL)` ans Ende (`:1609`),
und „oben" ist `W_TREE[ROOT].ob_tail` (`:1022`, `:1093`). Nichts davon kann
ueberlaufen. Unser monotoner `i16` in `TABLE.next_z` ist damit eine reine
Implementierungsfrage mit einer Pflicht: immer einen Wert oberhalb aller
offenen Fenster liefern — ohne Abbruch.

## Entscheidung: umnummerieren, nicht saettigen, nicht wrappen

| Variante | Wirkung auf die Ordnung |
|---|---|
| saettigen bei `i16::MAX` | jedes weitere Fenster bekommt `MAX`; `get_top` (`>=`), `get_bottom` (`<=`), Zeichenreihenfolge und `wind_find` brechen Gleichstand nach Slot-Index → **falsche** Ordnung |
| wrappen nach `i16::MIN` | das naechste Fenster laege **unter** allen bestehenden |
| **umnummerieren** | offene Fenster nach aufsteigendem `z` in `1..=n`, Zaehler auf `n+1`; Ordnung exakt erhalten |

## Was gebaut wurde

| Teil | Datei / Funktion | Stand |
|---|---|---|
| Fix | `tos-api/src/aes/window.rs` `bump_z`: `if *p == i16::MAX { *p = renumber_z(); }` vor dem Post-Inkrement. Neu `fn renumber_z() -> i16` (Auswahl nach kleinstem `z`, Gleichstand nach Slot-Index wie Zeichenreihenfolge/`get_top`; Zugriff ueber den rohen Tabellenzeiger, weil `wind_open`/`top_window` waehrend `w.z = bump_z()` ein `&mut Window` halten; geschlossene/freie Slots bleiben unberuehrt, ein Wiederoeffnen holt sich einen frischen Wert) | fertig |
| Testhaken | `pub fn test_set_next_z(z) -> i16`, `pub fn test_window_z(handle) -> Option<i16>` (Muster der bestehenden `test_*`-Haken, nicht im Bootpfad) | fertig |
| Test | `tos-api/tests/aes_window_zorder_counter.rs` (neu, 1 Fall ueber den Dispatcher): `wind_new`, drei Fenster, eins offen; Zaehler per Haken auf `i16::MAX − 1`; zwei weitere `wind_open` (das dritte trifft `MAX`), dann zweimal `wind_set(WF_TOP)`; nach jedem Schritt unten/oben ueber `wind_get(0, WF_OWNER)[4]` (`W_TREE[ROOT].ob_head`, `:1588`) und `wind_get(0, WF_TOP)[1]` (`w_top`, `:315-318`, `:1553-1554`); zum Schluss die rohen `z` strikt aufsteigend; `wind_new` hinterher | 1/1 |

Spec: keine — der Zaehler ist kein TOS-Vertrag (kein `docs/api/`-Satz
beschreibt ihn); die Herleitung steht im Doc-Kommentar von `bump_z`.

## Geprueft (Belege in `measurement-2026-09-20-a47-a52-a55-a58/`)

* **Rot vor / gruen nach** (`host-tests/a52-red-before.txt`,
  `a52-green-after.txt`): vor dem Hunk `panicked at
  tos-api/src/aes/window.rs:531:14: attempt to add with overflow` beim dritten
  `wind_open`; danach 1/1, Ordnung nach jedem Schritt richtig.
* `cargo run -p xtask -- test`: **648 / 0 / 1**. Boards rpi4 / qemu-virt
  rustc 64/94, Clippy 81/125 — Delta 0 (ein Zwischenstand hatte 82:
  `needless_range_loop` in `renumber_z`, behoben); Testdatei clippy-sauber.
* **QEMU-Smoke** (`method/run.sh`, raspi4b, Produktkernel `--release` +
  sd-base): `screendumps/smoke-desk-up.png` **0 Pixel** und bytegleich
  (sha256 `dee57cca…`) zu `measurement-2026-09-19-a46/screendumps/smoke-desk-up.png`;
  Serial ohne `EXC`/`panic`. Unterhalb von `i16::MAX` ist `bump_z` unveraendert,
  die Fensterordnung des Desktops damit auch.

## Offen

- Tester-Abnahme; echte Pi 4. Kein neuer Selftest zu verdrahten; der
  Umnummerierungspfad ist im Boot unerreichbar (32767 Oeffnungs-/Top-Vorgaenge).
- Nicht committet; `docs/STATUS.md` nicht angefasst.
