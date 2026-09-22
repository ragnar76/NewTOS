# A109 — STATUS: DONE (2026-09-22) — Pi-Sichtabnahme offen

**DONE bedeutet hier: Punkte 1–3 gelandet, host-verifiziert (rot → gruen,
soweit hosttestbar) und in QEMU raspi4b vorher/nachher belegt; Punkte 4 und
5 mit Begruendung unveraendert; die Sichtabnahme auf der echten Pi 4 steht
aus (Tester/Lead; nichts geflasht).** Bearbeitet 2026-09-22 zusammen mit
A107 (ein Paket, ein Messverzeichnis), Branch `desk-rebuild`, Basis HEAD
`de487fe`, nichts committet, kein `git checkout/stash/reset`.

## 1. Dropdown-Eintragshoehe: RSC-Zelle statt `MENU_H`

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `desk/desk_rsc.c:297-341` (Eintraege `{0, row, 20, 1}`), `:291-295,345-349` (Boxen `{x, 0, w, n}` — die File-Box `{8,0,19,16}` = 16 Zellen); `aes/gemrslib.c:53-99` (`fix_chpos`/`rs_obfix`: Zelle × `gl_hchar` fuer die Offsets 1 und 3, Offsetbyte addiert); `desk/deskmain.c:1608-1635` (`adjust_menu` rechnet **nur** Breiten um); `aes/gemgraf.c:412-415` (`gl_hbox = gl_hchar + 3` — die Leistenzeile, nicht die Eintragszeile) |
| NewTOS (vorher) | `kernel/src/desk/menu.rs` `build_menu`: Eintraege `(0, i·MENU_H, iw, MENU_H)`, Boxhoehe `n·MENU_H` — 19 px, File-Box 285 |
| NewTOS (nachher) | `ENTRY_H = CELL_H` (16); Eintraege `(0, i·ENTRY_H, iw, ENTRY_H)`, Boxhoehe `n·ENTRY_H` — File-Box **240** |

`MENU_H` bleibt, wo es hingehoert: Leistenzeile (`THEACTIVE`, Titel — RSC
`769` = `gl_hchar + 3`) und die y-Lage der Boxen (`THEMENUS` `y = 769`).
Die Trefferpruefungen rechnen ohnehin mit den Objektrechtecken
(`menu::track` → `object::child_at`, `open_box` → `sr_rect`,
`rehilite` → `ob_change`), nichts musste dort angepasst werden.

## 2. Druck-Feedback der Scroll-Pfeile

EmuTOS `hctl_window` klammert `handle_arrow_msg` mit `ob_change(gl_awind,
gadget, selected, TRUE)` … `ob_change(gl_awind, gadget, normal, TRUE)`
(`aes/gemctrl.c:271-284`), `normal = ob_state & ~SELECTED`, `selected =
normal | SELECTED` (`:189-194`) — es bewegt sich genau das `SELECTED`-Bit.
Beide Aufrufe stehen unter `if ((gadget != W_HSLIDE) && (gadget !=
W_VSLIDE))`: der **Seitenklick** im Schacht bekommt kein Feedback, weil
`gadget` dort die Schachtbox ist (`ob_find`, `:188`), waehrend `cpt` schon
auf den Fahrstuhl weitergerueckt wurde (`:262-270`).

`tos-api/src/aes/window.rs` `intercept`: `gadget_at` wird einmal gebunden
(`gad`), die vier Pfeilarme klammern `wait_release()` mit
`arrow_change(w, gad, true/false, backend)`. `arrow_change` zeichnet die
Zelle aus `varrow_cells`/`harrow_cells` mit `gadget_sel(..., sel, ...)` —
A105s Modell: `actcolchange = 0` (Standard) zeigt den Druck als gesunkene
Fase (`add_3d_effect`, oben/links BLACK statt WHITE, unten/rechts LWHITE
statt LBLACK, `gemoblib.c:238-276`) und als um (+1,+1) versetzte Glyphe
(`acttxtmove`, `:742-755`). Cursor je Zeichnung aus/an wie `ob_change`s
`gsx_moff`/`gsx_mon` (`:1275,1305`); die Wartezeit dazwischen laeuft mit
sichtbarem Zeiger, wie EmuTOS' `handle_arrow_msg`. Der gedrueckte Pfeil
bleibt ueber die ganze Haltezeit SELECTED — anders als Closer/Fuller, die
`gr_watchbox` beim Verlassen zuruecknimmt. **`WM_ARROWED`-Kadenz
unveraendert** (eine Nachricht, an derselben Stelle gepostet).

## 3. Infozeile

`desk/desk_rsc.c:2468` `STINFOST` = `" %ld bytes used in %d items."`,
Argumentfolge aus `sprintf(pwin->w_info, alert, select_size,
select_count)` (`desk/deskwin.c:905`). `kernel/src/desk/main.rs` `fmt_info`
schreibt jetzt genau das (fuehrendes Leerzeichen, Groesse zuerst, Punkt am
Ende) statt `" N items   M bytes used"`.

**`STINFST2`** (`" %ld bytes used in %d selected items."`, `:2469`) hat
**keinen Aufrufer**: `set_window_strings` laeuft nur beim (Neu-)Einlesen
eines Verzeichnisses und immer mit `p_count`/`p_size` des ganzen
Verzeichnisses; nichts ruft `win_sinfo(pw, TRUE)` nach einer
Auswahlaenderung wie EmuTOS (`deskmain.c:982-983`). Fehlender Aufrufer,
benannt — nicht durch einen Stub verdeckt.

## 4 / 5 — ohne belegte Wirkung, unveraendert

* **`child_at`-Reihenfolge**: `child_at` laeuft `ob_head` vorwaerts,
  `ob_find` ab `ob_tail` (`gemoblib.c:1034-1113`). Unterschiedlich nur bei
  **ueberlappenden** Geschwistern; im Desktop-Menue sind die Titel
  zusammenhaengend und gleich hoch, die Eintraege disjunkte Zeilen — kein
  Testfall kann einen Unterschied zeigen.
* **`THEBAR`-Objekttest in `menu::track`**: `in_bar = my < gl_hbox()` statt
  `rect_change(tree, &p2mor, THEBAR, TRUE)` (`gemmnlib.c:286-292`). Gelesen
  wird es ausschliesslich, solange `cur_title == NIL` (`START_STATE`), und
  der Track wird nur ueber `gl_ctwait` = `THEACTIVE` betreten, dessen
  x-Spanne genau die zusammenhaengenden Titel sind — nach dem ersten
  Durchlauf ist `cur_title` nie mehr `NIL`. Die Differenzzeile 18 (THEBAR 18
  vs. `gl_hbox` 19) ist damit unbeobachtbar. Das praezisiert A108s Notiz.

## Pruefung

| Beleg | Ergebnis |
|---|---|
| `tos-api/tests/aes_a105_frame_remnants.rs` (+2) | an HEAD **1 rot** („held: the bevel sinks, left line BLACK" — vorgefunden `0xFFFBFF` = WHITE), im Baum **7/7**; der Seitenklick-Fall ist auf beiden Seiten gruen (er pinnt die Wache) |
| `tos-api/tests/aes_a100_a102_desk_menu_geometry.rs` (+2, Literale nachgezogen) | **10/10**; der Kernel-Crate ist `forced-target` AArch64 — die Hostdatei traegt eine **Replik**, die Kernelgeometrie belegt der Serial-Dump |
| `cargo run -p xtask -- test` | **916 / 0 / 1** (Basis 909/0/1; +4 hier, +3 A107) |
| `cargo xtask build` frisch fuer **beide** Boards | tos-api **64** / kernel **94**, rc 0 — Delta 0 |
| Clippy rpi4 | tos-api **80** / kernel **125** → unveraendert; `desk/menu.rs` 0→0, `desk/main.rs` 0→0, `window.rs` 11→11 |

### QEMU raspi4b (`measurement-2026-09-22-a107-a109/`)

Kernel-Desktop (`sd-base`, ohne `#Z`), 5 Marken, 0 `EXC`/`PANIC`:

* Serial vorher `[a108] BOX1#11=(64,19,160,285) ENTRY0=(0,0,160,19)`,
  nachher `BOX1#11=(64,19,160,240) ENTRY0=(0,0,160,16)` (Desk 32, View 160,
  Options 192; `THEBAR`/`THEACTIVE`/Titel unveraendert).
* F (File offen): 6986 px Unterschied, Bbox (58,21)-(293,304) = die
  gekuerzte Box + die Infozeile.
* A / H / I: 970 px, Bbox (58,70)-(293,83) = **nur** die Infozeile:
  `26 items   418693 bytes used` → ` 418693 bytes used in 26 items.`
* G (Pfeil gedrueckt, Zeiger mit gehaltener Taste weggezogen, damit der
  Cursor nicht auf der Zelle liegt): die Pfeilzelle (567,82,24,24)
  unterscheidet sich um **102 px** (Fase gesunken, Glyphe versetzt) —
  dieselbe Zelle **0 px** in A (vor dem Druck) und in I (nach dem
  Loslassen).
* TeraDesk-Kontrolle (3 Marken): **0 px** auf allen dreien.

## Offen

Pi-Sichtabnahme (Tester/Lead; nichts geflasht), HW-Checkliste Punkt 32.
Kein Commit.

## Artefakte

`measurement-2026-09-22-a107-a109/`. Geaenderte Dateien im Baum:
`kernel/src/desk/{menu,main}.rs`, `tos-api/src/aes/window.rs`,
`tos-api/tests/{aes_a105_frame_remnants,aes_a100_a102_desk_menu_geometry}.rs`,
`docs/api/desktop/{deskmain,deskwin}.md`,
`docs/api/aes/window-graphics.md`, `docs/STATUS.md`, `TASK/A109.md`, dieses
Ticket.
