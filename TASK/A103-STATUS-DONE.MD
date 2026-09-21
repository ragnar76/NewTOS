# A103 — STATUS: DONE (Hostbeleg rot/gruen)

Ein Druck auf den **eigenen Bereich** eines Untermenue-Kastens (Rand/Luecke,
von keinem Kind bedeckt) liefert jetzt `smroot` als Item, und ein DISABLED
Item mit angehaengtem Untermenue oeffnet es nicht mehr. Bearbeitet
2026-09-21, Branch `desk-rebuild`, Basis HEAD `a570123`, **nicht committet**,
nichts geflasht. Gemeinsames Paket mit A105/A108
(`measurement-2026-09-21-a103-a108/`).

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemmnlib.c:417` (`cur_submenu = ob_find(smtree, smroot, 1, x, y)`), `:418-422` (`SUBMENU_STATE`), `:446-448` (Un-Hilite beim Verlassen), `:474-475` (`menu_select(smtree, cur_submenu, last_submenu, TRUE)`), `:482-500` (Bereinigung: `:488` `do_chg(smtree, cur_submenu, SELECTED, FALSE, FALSE, TRUE)`, `:493` `do_chg(tree, cur_item, …)` = Gatter + Loeschen des Eltern-SELECTED, `:494-496` `*pitem = cur_submenu; *ptree = smtree`), `:470-475` (`display_submenu` nur in `if (menu_select(...))`), `:158-160` (`do_chg` `chkdisabled`); `aes/gemoblib.c:1034-1113` (`ob_find` gibt `lastfound` = das Elternobjekt zurueck, wenn kein Kind trifft), `:1295-1304` (`ob_change`: XOR des Innenrechtecks) |
| NewTOS (vorher) | `tos-api/src/aes/menu.rs` `track_owned`: `sit = child_at(...)` → `NIL` im Kasten ohne Kind, kein Hilite, Druck ignoriert (die AES wartet weiter); `open_submenu(tree, it)` fuer jedes `it != NIL`, auch DISABLED |
| NewTOS (nachher) | `track_owned`: `sit = if hit == NIL { sub_box } else { hit }`; Elternwahl-Bereinigung `object::change(tree, cur_item, st & !SELECTED, false)`; `open_submenu` und `open_submenu_static` gated auf `!DISABLED` |

## Was gebaut ist

* **`tos-api/src/aes/menu.rs`** — `track_owned`: im Untermenue-Kasten ist
  der Kasten selbst das Item, wenn `child_at` nichts trifft (Kommentar mit
  `gemmnlib.c:417,418-422,474-475,494-496`, `gemoblib.c:1034-1113`); der
  bestehende `rehilite`-Pfad hilitet ihn (Innen-XOR ueber `set_state_bit` →
  `change_t`, `xor_is_ok` fuer eine Schwarz-Weiss-`G_BOX`) und der
  bestehende Gleichbaum-Wahlpfad liefert `(cur_title, sub_box)`; dort neu:
  das Eltern-SELECTED wird ohne Zeichnen geloescht (`:493`). Untermenue-
  Oeffnung nur fuer nicht-DISABLED Items (`:470-475,158-160`), auch in
  `open_submenu_static`.
* **`tos-api/tests/aes_a89_a92_menu_event_neighbours.rs`** — +2 Faelle;
  `Sample` +`dev_a`/`dev_b` (Geraetepixel, weil ein XOR-Weiss kein
  Paletteneintrag ist), `fill_pattern` modusbewusst (XOR) wie die A99/A100-
  Harnesse, `menu_tree_with_same_tree_submenu` (Kasten 17 als viertes Kind
  von `themenus`, Items 18..20 je 2 px innen), `fresh_same_tree_submenu`,
  `menu_ienable`.
* **`docs/api/aes/menu.md`** — Punkt 8 (smroot-Wahl, DISABLED-Item ohne
  Untermenue, Fremdbaum weiter offen).

## Tests: rot vor, gruen nach (`measurement-2026-09-21-a103-a108/host-tests/`)

| Test | vor (HEAD `a570123`) | nach |
|---|---|---|
| `a103_press_on_the_submenu_boxs_own_area_picks_smroot` | rot: „the AES is still waiting after 20000 pumps … pointer (161, 66)" — der Druck auf die Kastenspalte wird nie geliefert | gruen: Probe `smroot.ob_state` SELECTED, Beta nicht, Luecken-Pixel = Komplement von Weiss; `MN_SELECTED (4, 17)`; danach smroot und Open nicht SELECTED |
| `a103_a_disabled_item_with_a_submenu_attached_opens_nothing` | rot: „no submenu may open under a DISABLED item …; pen at (255,85) = 0" (Kasteninneres statt Desktop) | gruen: nur `MU_TIMER`, Probe-Pixel Desktop, Open nicht SELECTED |

Bestehende 8 Faelle der Datei gruen (mit dem modusbewussten `fill_pattern`).

## Nicht angefasst

* Untermenue in einem **Fremdbaum**: die Wahl (auch smroot) braucht den
  `msg[5..7]`-Baumzeiger-Pfad (Modul-Doku von `menu::track`), unveraendert.
* `child_at` laeuft `ob_head`-vorwaerts, EmuTOS `ob_find` ab `ob_tail`
  rueckwaerts — nur bei ueberlappenden Geschwistern verschieden; vorbestehend.
* Ticket-Punkt 3 (veraltetes `cur_menu` nach DISABLED-Titel) wie im Ticket
  nur vermerkt.

## Offen

Nichts auf der Pi sichtbar (TeraDesk/QED ohne Untermenues). Kein Commit.
