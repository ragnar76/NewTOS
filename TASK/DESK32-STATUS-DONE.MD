# DESK32 — STATUS: DONE (2026-09-25) — nachgemessen und behoben

**Auftrag:** Desk misst 22 Zeichenzellen statt 21, File 23 statt 22. Das
Ticket nennt sich selbst eine Messbehauptung, die vor jeder Aenderung
nachzumessen ist.

Basis: Branch `desk-rebuild`, HEAD `12fddc1`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset` auf dem gemeinsamen Baum.

Messung:
[`measurement-2026-09-25-desk-rest/`](../measurement-2026-09-25-desk-rest/) §2.
Spezifikation:
[`docs/api/desktop/deskmain.md`](../docs/api/desktop/deskmain.md).
Hardwarepunkt **72**.

---

## 1. Nachgemessen — das Ticket stimmt

Zwei Kernel, die sich in `kernel/src/desk/menu.rs` **allein** unterscheiden
(`method/make_scratch_before.sh` nimmt diese eine Datei aus HEAD), dieselbe
Aktionsliste, dieselbe Karte. `[probe] box N abs (x,y) WxH`, 8 px je
Zeichenzelle:

| Kasten | vorher | nachher | EmuTOS | Ticket |
|---|---|---|---|---|
| Desk | 176 px = **22** | 168 px = **21** | 21 | 22 → 21 ✔ |
| File | 184 px = **23** | 176 px = **22** | 22 | 23 → 22 ✔ |
| View | 184 px = 23 | 184 px = 23 | 23 | unveraendert ✔ |
| Options | 240 px = 30 | 240 px = 30 | 30 | unveraendert ✔ |

EmuTOS' Spalte kommt aus der Quelle, nicht aus einem Bild: laengste
Nicht-Trenner-Ressourcenzeichenkette + `1 + SHORTCUT_SIZE`
(`copy_menu_items`, `deskmain.c:1776-1781`), minus NUL, plus `adjust_menu`s
`m++` (`:1620`).

* `"  Desktop info..."` = 17 → 17+3+1 = **21**
* `"  Close top window"` = 18 → **22**
* `"  Set background..."` = 19 → **23**
* `"  Desktop configuration..."` = 26 → **30**

Der Startwert `MIN_DESKMENU_WIDTH - 1` (19) des Desk-Kastens bindet in keinem
Fall.

## 2. Die Ursache, genau

Nicht in `adjust_menu`, sondern in dem, was es zu messen bekam. EmuTOS' Kette
hat zwei Glieder:

1. `copy_menu_items` misst die **Ressourcen**-Zeichenketten — ungepolstert —,
   nimmt die laengste, addiert `1 + SHORTCUT_SIZE` und kopiert jede in den RAM,
   auf `width - 1` mit Leerzeichen gepolstert.
2. `adjust_menu` misst **diese** RAM-Zeichenketten; die Abkuerzungsspalte
   steckt damit schon in `maxlen`, und `m++` gibt die letzte Zelle.

`build_menu` hat Glied 1 immer getan (die RAM-Kopie in `MENU.labels`) — aber
die Quellzeichenketten in `MENUS` waren **ebenfalls** vorgepolstert, also
zaehlte `adjust_menu` die Fuellzeichen ein zweites Mal.

## 3. Was geaendert wurde

* Die Polsterung ist aus den `MENUS`-Zeichenketten heraus; sie tragen jetzt
  genau EmuTOS' Ressourcentext (die zwei fuehrenden Leerzeichen der
  Haekchenspalte bleiben — die sind Text, keine Polsterung).
* Die Trenner stehen wieder auf `"-"`, der Ressourcenform. `build_menu` fuellt
  sie ohnehin auf die fertige Breite (`separator+MAXLEN_SEPARATOR-m`,
  `:1621-1624`); die alten Strichfolgen waren tot und mussten von Hand
  mitgepflegt werden.
* `adjust_menu` misst jetzt **alles ausser Trennern** statt nur der
  Aktionseintraege — `copy_menu_items`' eigene Regel (`if (*q != '-')`,
  `:1773`). Das schliesst die sechs Accessory-Platzhalter ein, die darum
  ebenfalls auf ihrem Ressourcentext `"1"`…`"6"` stehen. Ein Zeichen lang,
  also nie entscheidend — aber die Regel ist die der Quelle, und ein
  Platzhalter, der eines Tages laenger wuerde, wuerde jetzt mitgezaehlt.
  Sichtbar sind sie ohnehin nur, wenn kein Accessory registriert ist, und dann
  verknuepft `menu_fixup` sie gar nicht erst (`aes/gemmnlib.c:89-125`).

`install_shortcuts`/`recover_shortcuts` sind nicht betroffen: sie rechnen aus
`ob_width` und der RAM-Zeichenkette, und beide bleiben konsistent
(`cells - SHORTCUT_SIZE` zeigt weiter auf die letzten zwei Zeichen vor dem
NUL).

---

## Geaenderte Dateien

* `kernel/src/desk/menu.rs` — `MENUS` (alle Beschriftungen, alle Trenner),
  `adjust_menu` (Messfilter + Kommentar)
* `docs/api/desktop/deskmain.md`,
  `docs/api/desktop/_hw-test-2026-09-17.md` (Punkt 72)

## Geprueft

* `cargo run -p xtask -- test` — **944 passed / 0 failed / 1 ignored**.
* Frischer Build je Board: **tos-api 64 / kernel 94**, Delta 0.
* `clippy` beide Boards: **tos-api 80 / kernel 125**, Delta 0, null Treffer in
  `kernel/src/desk/`.
* QEMU raspi4b, zwei Laeufe (`probe-before`, `probe`): alle vier Klappmenues
  je einmal, plus die Zahlen aus `probe_dump()`. Null `[EXC]`, null `[PANIC]`,
  `fsck_msdos -n` sauber.

## Offen

* **Hardwareabnahme**, Punkt 72 — dieselbe Sichtprobe in 1920×1080, wo die
  Zellenbreite dieselbe ist, aber der Kasten im Verhaeltnis viel schmaler
  wirkt.
