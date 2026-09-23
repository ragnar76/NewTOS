# DESK08 — STATUS: DONE (2026-09-23)

**Auftrag:** die Kette `form_center` → `form_dial(FMD_START)` → `objc_draw` →
`form_do` → `form_dial(FMD_FINISH)` in den vorhandenen `desk/`-Dialogen
nachziehen (G7 der Lueckenliste).

Basis: Branch `desk-rebuild`, HEAD `326ef4c`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, kein `git checkout/restore/stash/reset`.

## Was EmuTOS an dieser Stelle tut (gepinnt: `references/emutos`)

`desk/deskinf.c:349-357` **`start_dialog(tree)`**

```c
form_center(tree, &t.g_x, &t.g_y, &t.g_w, &t.g_h);
rc_intersect(&gl_rfull, &t);                       /* constrain dialog box */
form_dial(FMD_START, 0,0,0,0, t.g_x, t.g_y, t.g_w, t.g_h);
objc_draw(tree, ROOT, MAX_DEPTH, t.g_x, t.g_y, t.g_w, t.g_h);
```

`desk/deskinf.c:363-390` **`end_dialog(tree)`**

```c
form_center(tree, &xd, &yd, &wd, &hd);
form_dial(FMD_FINISH, 0,0,0,0, xd, yd, wd, hd);
while (evnt_multi(MU_TIMER|MU_MESAG, …) & MU_MESAG) hndl_msg();
```

mit EmuTOS' eigener Begruendung fuer die Schleife (`:371-378`): der Desktop
beantwortet seine Nachrichten selbst, ein nicht geleertes `WM_REDRAW` kaeme
ausser der Reihe und zeigte sich als halber Neuaufbau.

`desk/deskinf.c:396-402` **`inf_show(tree, start)`** = `start_dialog` +
`form_do(tree, start)` + `end_dialog`.
`desk/deskdir.c:79-85` **`draw_dial(tree)`** = `form_center` + `objc_draw`
(der Neuaufbau zwischen zwei `form_do`-Runden).

Aufrufer: `fun_mkdir` (`deskfun.c:697,734`), `fun_mask` (`:655`),
`inf_file_folder`/`inf_disk` (`deskinf.c:574,691`), `inf_backgrounds`
(`:862-896`, mit `draw_fld` in der Schleife), `ins_app_dialog`
(`deskins.c:504,527,597`), `inf_conf` (`:1158,1245`), `deskdir.c:956,1079`,
`desksupp.c:635,659,1637,1646`, `deskrez.c:128,206,286`.

**`fm_dial` sichert den Hintergrund nicht.** `aes/gemfmlib.c:362-381`: der
`FMD_START`-Arm ist ein nacktes `break` mit dem Kommentar „grab screen sync or
some other mutual exclusion method". Wiederhergestellt wird bei `FMD_FINISH`,
und zwar durch **Neuzeichnen**: `w_drawdesk(pt)` + `w_update(DESKWH, pt,
DESKWH, FALSE)` (`:375-379`). Der Ticket-Text („save/restore the background")
beschreibt nicht, was EmuTOS tut; der A57-Schirmpuffer wird hier nicht
gebraucht und nicht benutzt.

## Was getan wurde

Alles in `kernel/src/desk/` + der Spec + dem neuen Hosttest.

**`mod.rs` (aesbind):** `form_center(tree) -> Grect` (Opcode **54**,
`AES_CTRL_CODE(FORM_CENTER, 0, 5, 1)`), `form_dial_start(r)` /
`form_dial_finish(r)` ueber ein gemeinsames `form_dial(fmd, r)` (Opcode
**51**), `wind_update(mode)` (Opcode **107**, fuer DESK09), `form_do` hat jetzt
wie die EmuTOS-Bindung ein `start`-Argument, `FMD_START` als benannte
Konstante.

**`inf.rs`:** `start_dialog`/`end_dialog`/`inf_show` — an derselben Stelle im
Baum, an der `deskinf.c` sie hat. `redraw::rc_intersect` ist dafuer
`pub(super)`; `main::hndl_msg` ebenfalls (die Leerschleife ruft es, wie
EmuTOS).

**`dir.rs`:** `draw_dial(tree)` (`deskdir.c:79-85`).

**Die vier Dialogerzeuger** bauen ihren Baum jetzt **im Ursprung** (Wurzel
`ob_x = ob_y = 0`, wie ein Ressourcen-Dialog) und gehen durch die Kette:

| Dialog | Datei | Weg |
|---|---|---|
| Name-/Maske-/Sucheingabe | `dlg.rs::edit_name` | `inf_show(tree, ROOT)` |
| Show Info / Umbenennen | `inf.rs::inf_file_folder` | `inf_show(tree, ROOT)` |
| Set background | `inf.rs::set_background` | `start_dialog` / Schleife mit `dir::draw_dial` / `end_dialog` |
| Install application | `insapp.rs::ins_app_dialog` | `start_dialog` / `draw_dial` bei Wiederholung / `end_dialog`, `form_do(tree, O_DOCTYPE)` |

**`main.rs`:** die sechs `redraw::redraw_desktop()` hinter Dialogen sind weg
(`do_optnmenu`s SetBackground, `do_install_app`, `do_new_folder` ×2,
`do_info_rename` ×2, `do_set_mask`, `do_search`) — der Dialog nimmt jetzt sein
eigenes Rechteck zurueck. Die `redraw_desktop()`-Aufrufe in `dir.rs` gehoeren
zu Dateioperationen, nicht zu Dialogen, und sind **unberuehrt**.

## Benannte Abweichung

EmuTOS' `inf_backgrounds` zeichnet zwischen zwei `form_do`-Runden nur das
geaenderte Musterfeld neu (`draw_fld(tree, BGSAMPLE)`, `deskdir.c:127`);
NewTOS zeichnet den ganzen Dialog mit `draw_dial` neu, wie `ins_app_dialog`
auf seinem Wiederholungspfad (`deskins.c:527`). Ein Einzelfeld-Zeichner
braucht `objc_offset` (Opcode 44), das der Desktop nicht bindet.

## Pruefung

**Hosttest (neu): `tos-api/tests/aes_desk_dialog_chain.rs`, 3 Faelle.**
Der Baum ist der des Desktops (`dlg.rs::edit_name`, 290×150 `G_BOX` +
Titel + `G_BOXTEXT` + zwei Knoepfe), gefahren durch den echten
AES-Dispatcher in einen Pixelpuffer.

* `form_center_places_the_desktop_dialog_below_the_menu_bar` — die Wurzel
  traegt nach dem Aufruf `form_center`s Position; sie ist
  `((640-290)/2, gl_hbox + (480-gl_hbox-150)/2)`; und sie ist **nicht** die
  Rechnung des Desktops `(480-150)/2`, sondern genau `gl_hbox/2` tiefer.
* `fmd_finish_restores_only_the_dialog_rectangle` — ausserhalb des
  Dialogrechtecks aendert sich waehrend des Dialogs kein Pixel, und nach
  `FMD_FINISH` ist der ganze Schirm byteweise der von vorher.
* `the_dialog_restore_draws_only_inside_its_own_rectangle` — mit einem
  Sentinelwert gemessen: die Kette **fasst** ausserhalb ihres Rechtecks
  keinen Punkt an, und innerhalb jeden.

**Rot vorher:** `host-tests/desk08-RED-before.txt` (Variante mit der alten
Desktop-Rechnung, `host-tests/desk08_red_before-scratch.rs`, **nicht** im
Build): 1 passed / **2 FAILED** —
`left: (175, 165) right: (175, 174)` (die 9 px) und
`251540 px outside (175,165,290,150) were drawn on`.
Der dritte Fall besteht auf beiden Seiten: auf einem einfarbigen Hintergrund
malt auch der Ganzschirm-`FMD_FINISH` dasselbe Bild wieder hin — deshalb der
dritte Test mit dem Sentinel.
**Gruen nachher:** `host-tests/desk08-GREEN-after.txt`, 3/3.

**QEMU raspi4b** (`method/desk-desk08.actions`, Scratch-Kernel mit dem
H02-Injektor, 640×480, QMP-Screendump je Marke):
`Options` aufklappen → `Set background...` → `Cancel`.

* `dlg-C-dialog.png`: der Kasten steht gemessen bei **(170, 159, 300, 180)** —
  genau `form_center`s Wert (`(640-300)/2 = 170`, `19 + (480-19-180)/2 = 159`).
  Die alte Rechnung haette 150 ergeben.
* `dlg-D-closed.png` gegen `dlg-A-idle.png`: **0 differing pixels**. Der
  Dialog ist rueckstandsfrei verschwunden, ohne dass der Desktop neu
  aufgebaut wurde.
* 0 `[EXC]`/`[PANIC]` im Seriellen.

## Bauen

Frisch je Board (`cargo clean -p kernel -p tos-api` davor):
`cargo run -p xtask -- build --board qemu-virt` und `--board rpi4`, beide
rc=0, **tos-api 64 / kernel 94** Warnungen (Basis 64/94, Delta 0).
`cargo clippy`: tos-api **80**, kernel **125** auf beiden Boards (Basis
80/125), **0** Treffer in `kernel/src/desk/`.
`cargo run -p xtask -- test`: **929 passed / 0 failed / 1 ignored**
(Basis 926/0/1 + 3 neue).

## Offen

* **Pi-4-Sichtabnahme** — HW-Checkliste Punkt **35**, Teil A.
* Die uebrigen Dialoge, die EmuTOS hat und NewTOS nicht (`inf_disk`,
  `inf_pref`, `inf_conf`, `opn_appl`, die Kopier-/Loeschdialoge in
  `deskdir.c`), bekommen die Kette automatisch, sobald sie existieren — sie
  ist nicht mehr pro Dialog zu schreiben.
