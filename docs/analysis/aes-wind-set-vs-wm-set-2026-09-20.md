# `wind_set` (105) gegen EmuTOS' `wm_set` — Zweig fuer Zweig

Datum: 2026-09-20 · Ticket: [`TASK/A59.md`](../../TASK/A59.md) (P2, Pruefluecke)
· Quittung [`TASK/A59-STATUS-DONE.MD`](../../TASK/A59-STATUS-DONE.MD) · Reine
Pruefung: **kein Produktcode, keine Spec-Aenderung**. Gefundene Abweichungen
haben eigene Tickets **A78–A84**.

Quellenstaende: EmuTOS `references/emutos` @
`1a7484aba1bfba23e2c0899e243b6d8177fe41df` (Zielprofil Falcon030/TOS4,
`CONF_WITH_3D_OBJECTS = 1`, `CONF_WITH_WINDOW_COLOURS = 1`, `AES_VERSION 0x0340`,
`include/config.h:1258-1259,1328-1329,1345-1347`); NewTOS `desk-rebuild` @
`0fa16fc`; Spec `docs/api/aes/window-graphics.md` § `wind_set` (105)
(`:641-719`) und § NewTOS implementation notes (`:1385-1406`); gemlib
`sdk/gemlib/gemlib-0.44.0/` (LP64-Bindung der portierten Apps). Alle
Zeilenangaben auf diese Staende. Aussagen ohne `Datei:Zeile` sind als
**Schluss** gekennzeichnet.

Messbasis: `tosdump/cases/aes-r2-wind.tsv` (Faelle 301–324),
`aes-r4-subfunctions.tsv` (Faelle 462–525), `aes-r3-wind-state.tsv`
(`+105`-Preludes 300–333), verglichen in
`measurement-2026-09-13-regression/aes-r{2,4}*/DIFF-vs-emutos.txt`: **kein**
`wind_set`-Fall weicht dort ausser dem bekannten `AESGLOBAL`-Rauschen ab. Die
Tabellen tragen keine Zeiger-, Farb- oder Handle-0-Faelle und keine
Nachrichten/Pixel (Kopf `aes-r2-wind.tsv:65-70,90-93`) — genau dort liegen die
Befunde unten. Getrieben wurde deshalb per Hosttest ueber den Dispatcher
(Scratch `zz_a59_scratch.rs`/`zz_a59_scratch2.rs`, nicht committet, Ausgabe in
§ 4).

---

## 1. Die beiden Funktionen

**EmuTOS** `aes/gemsuper.c:345-346`: `ret = wm_set(WM_HANDLE, WM_WFIELD,
&WM_IX)` — `int_in[0]`, `int_in[1]`, `pinwds = &int_in[2]`; `addr_in` wird
nicht uebergeben. `aes/gemwmlib.c:1623-1760`:

| Zeile | Schritt |
|---|---|
| `:1630-1636` | `get_pwin(w_handle)` (`:1391-1404`: `0 <= h < NUM_WIN` und `VF_INUSE`, Handle 0 ist legal); NULL → `return FALSE`, **ausser** bei `WF_DCOLOR` |
| `:1638` | `wm_update(BEG_UPDATE)` — `tak_flag(&wind_spb)` (`gemflag.c:71-86`) |
| `:1643-1656` | Eingabeklammer: `WF_HSLSIZ`/`WF_VSLSIZ` lassen `-1` durch, sonst wie `WF_HSLIDE`/`WF_VSLIDE` auf `1..1000` — geschrieben in `pinwds[0]`, das ist `xif`s **lokales** `int_in`, der Aufrufer sieht es nie |
| `:1658-1738` | der `switch` (11 `case`, `default: ret = FALSE`) |
| `:1746-1752` | 3D-Bau: Slider-Gadget → `do_cpwalk = TRUE` (ohne `w_handle == gl_wtop`-Bedingung) |
| `:1754-1755` | `if (do_cpwalk) w_cpwalk(w_handle, gadget, MAX_DEPTH, TRUE)` — **ein** Fenster, **ein** Gadget-Teilbaum, Clip `WS_TRUE`, `do_walk` ueber die Owner-Rechteckliste dieses Fensters (`:363-379`, `:286-312`) |
| `:1757-1759` | `wm_update(END_UPDATE)`; `return ret` |

**NewTOS** `tos-api/src/aes/window.rs:2867-3058` (`wind_set`), Helfer
`slider_size :3062-3068`, `redraw_open :3075-3085`, `set_newdesk :1029-1063`,
`wf_str_ptr :2359-2365`, `copy_cstr :2323-2338`, `top_window :715-767`,
`redraw_vslider :2227-2243`, `redraw_hslider :2303-2317`, `gadget_index
:645-647`, `win :1734-1742`, `set_curr :1745-1748`, `send_redraw :1587-1608`,
`send_redraw_in :1691-1707`, `broadcast_redraw :1721-1727`.

---

## 2. Die Tabelle — Feld fuer Feld

Bauarten laut Ticket: **1** = wir kennen es, `wm_set` nicht · **2** = `wm_set`
kennt es, wir nicht · **3** = beide, verschieden (mit dem Kommentar, wenn er die
Abweichung begruendet). „gleich" = beobachtbar identisch (Rueckgabe, Zustand,
Nachrichten, Pixel).

### 2.1 Prolog

| Schritt | EmuTOS | NewTOS | Spec | Urteil |
|---|---|---|---|---|
| Handle-Pruefung | `:1630-1636` `get_pwin`, NULL → FALSE, `WF_DCOLOR` ausgenommen | `:2874-2889` `WF_DCOLOR` zuerst (ohne Pruefung), dann `win(handle).is_none() → 0`; `win` = `get_pwin` incl. Handle 0 (`:1734-1742`) | `:641-647`, `:680` | **gleich** (gemessen r2 302/308/310, r4 462–516) |
| `wm_update(BEG_UPDATE)`/`END_UPDATE` | `:1638`, `:1757` — `sy_tas` ++/−−, `sy_owner = rlr` | fehlt; `UPDATE_LOCK` wird in `wind_set` nicht angefasst | `:643` nennt den Helfer | **Nebenwirkung nur bei EmuTOS**, bei Single-Tasking unbeobachtbar (Zaehler danach gleich, Owner ist ohnehin der einzige Prozess). Akzeptabel; wird beim kooperativen Kernel relevant |
| Slider-Klammer | `:1643-1656` | `:3007,3019` `clamp(1,1000)`; `:3013,3025` `slider_size` (−1 durch) | `:694-697` | **gleich** (gemessen r2 301–324, r4 480–504; `-1` fuer `WF_HSLIDE` → 1 auf beiden Seiten) |

### 2.2 Die elf Felder des `switch`

| # | Feld | EmuTOS `case` | NewTOS Arm | Spec | Urteil |
|--:|---|---|---|---|---|
| 2 | `WF_NAME` | `:1660-1667` `w_pname = *(char**)pinwds` (**Zeiger**, kein Kopieren, keine Laengengrenze); nur wenn `VF_ISOPEN`: `w_cpwalk(h, W_NAME)` = nur das Name-Gadget dieses Fensters; TRUE | `:2901-2913` `wf_str_ptr` (`addr_in[0]` bei `AIN_LEN>=1`, sonst gepackt in `int_in[2..]` in Aufruferbreite), `copy_cstr` **kopiert** in `name: [u8; 80]` (`:226`, 79 Zeichen + NUL, NULL → ""); `redraw_open` = **ganzer Rahmen**, wenn offen; 1 | `:668` (Redraw-Spalte **falsch**: „all open windows"), `:1122` ebenso; `:1389-1406` (nur `addr_in[0]`, veraltet) | **Bauart 3** — (a) Kopie statt Zeiger: spaetere In-Place-Aenderung des Puffers erscheint bei EmuTOS beim naechsten Rahmen-Redraw, bei uns nie (**getrieben, T10**); (b) Titel > 79 Zeichen abgeschnitten (Schluss, nur ab ~640 px Gadgetbreite sichtbar); (c) Ganzrahmen statt Gadget: pixelgleich, akzeptabel; (d) `AIN_LEN`-Doppelkonvention: EmuTOS liest nie `addr_in` — Notiz. Kein Kommentar begruendet (a)/(b). → **A83**, Spec → **A82** |
| 3 | `WF_INFO` | `:1668-1675` wie `WF_NAME` mit `W_INFO` | `:2914-2920` wie `WF_NAME` | `:669` (wie oben falsch) | **Bauart 3**, identisch zu `WF_NAME` (a)–(d) → **A83**, **A82** |
| 5 | `WF_CXYWH` | `:1676-1678` `draw_change(h, rect)` (`:1058-1223`): PREV=CURR, CURR=rect, WORK=`wm_calc`; nicht offen → fertig (`:1079-1085`); sonst Rechtecklisten neu, `w_setactive` (`ct_chgown`), Fallunterscheidung: **Top-Fenster reine Verschiebung → `w_move` blittet `WS_TRUE` alt→neu (`:938-996`, `bb_screen :963`), `moved=TRUE`**; **reine Verkleinerung → `stop=h`, `moved=TRUE` (`:1143-1148`)**; dann `w_drawdesk` (nur bei `start==DESKWH`) und `w_update(start, c, stop, moved)` (`:1004-1048`): Fenster **von oben nach unten**, `gl_wtop` uebersprungen wenn `moved`, je Fenster Rahmen + `w_redraw` = `WM_REDRAW(c ∩ hull(rlist))` (`:887-907`); TRUE | `:2921-2983` PREV=CURR, `set_curr`; nicht offen → 1 (`:2936-2939`); sonst Schadensrechteck `d` (Resize: zwei Schatten, sonst Union+Schatten), `redraw_clip(d)` (Desktop-Hintergrund + alle Rahmen), **`send_redraw(h, work)` unbedingt**, `broadcast_redraw(d)` in **Handle-Reihenfolge 1..15**; **kein Blit**; 1 | `:670` „whole frame + uncovered areas" (Blit / Nachrichtenkontrakt fehlt) | **Bauart 3** — Geometrie und Rueckgabe gleich (gemessen r2 313–318, r3-Preludes); **Nachrichten verschieden**: reine Verschiebung/Verkleinerung des Top-Fensters: EmuTOS **0** `WM_REDRAW` an den Verschobenen, wir **1** (getrieben, **T1/T2**); mehrere Fenster: EmuTOS oben→unten, wir Handle-Reihenfolge (**T4**); Vergroesserung: gleich (**T3**). Der Kommentar `:2969-2972` **begruendet** die Abweichung („unlike EmuTOS, which blits … nothing carried its pixels to the new place") → **A78** |
| 10 | `WF_TOP` | `:1679-1684` `if (h != gl_wtop) wm_mktop(h)` (`:1605-1614`): `ob_order` (ROOT wird verweigert, `gemoblib.c:1209-1210`), `draw_change(h, CURR)` Top-Zweig (`:1112-1136`), PREV danach restauriert. **Handle 0**: immer `wm_mktop(0)` (`gl_wtop` ist nie 0) → kein Reorder, aber `WS_WORK(0) := wm_calc(WC_WORK, NONE, gl_rscreen) = (1,1,W−2,H−2)` (`:1073-1077`, `:1807-1866`), dann Rueckkehr vor jedem Zeichnen. **Nicht geoeffnetes Handle ≥ 1**: `get_par` liest `W_TREE[-1]` (`gemobjop.c:145-160`) — undefiniert | `:2984-3002` `if handle != get_top() { top_window(handle) }` (`get_top` = 0 wenn nichts offen). `top_window :715-767`: `old_top == handle` → nichts; sonst `bump_z` (auch fuer geschlossene Slots), **Rahmen des alten Top deaktiviert gezeichnet** (`:741-747`), Rahmen des neuen aktiviert, ggf. `WM_REDRAW` | `:671` | Offenes Fenster: **gleich** (gemessen r2 324, r4 522/525; Hosttest `aes_window_content.rs`). **Bauart 3 fuer Handle 0 und nicht geoeffnete Handles**: wir zeichnen den Rahmen des weiterhin obersten Fensters als **inaktiv** und einen 1-px-Rahmen um den Desktop; EmuTOS zeichnet nichts, aendert aber `wind_get(0, WF_WXYWH)` (getrieben, **T5/T6**) → **A80** |
| 14 | `WF_NEWDESK` | `:1685-1689` `w_owner = rlr`; `gl_newdesk = *(OBJECT**)pinwds` (`int_in[2..3]`); `gl_newroot = pinwds[2]` (`int_in[4]`) — **auch bei NULL**; kein Redraw; TRUE | `:2895-2900` `owner = current_pid()`; `set_newdesk :1029-1063`: Baum via `wf_str_ptr`, Root = `int_in[2]` (nativ) bzw. `int_in[4]` (gepackt); bei Baum 0 wird Root **nicht** gespeichert; zusaetzlich `NEWDESK_APP`/`NEWDESK_ABI`; 1 | `:672`; D-094 `:649-665` (m68k-Packung); `:1389-1406` behauptet **nur `addr_in[0]`** — passt nicht zum Code; Root-Position fuer LP64-Packung **nirgends** festgelegt | Semantik **gleich** (Root-Nichtspeichern bei NULL unbeobachtbar: `w_drawdesk :345-355` liest `gl_newroot` nur bei gesetztem Baum). **Bauart 3 in der Aufrufkonvention**: fuer einen LP64-Aufrufer belegt `intin_ptr(2)` `int_in[2..5]` (`mod.rs:317-338`), der Arm liest den Root aber aus `int_in[4]` — **innerhalb** des Zeigers. gemlibs LP64-Bindung schreibt genau so (`a_wind_set_ptr_int.c:8-13`: Zeiger, dann `aes_intin[4] = W3`) → Root ≠ 0 zerstoert die Bits 32..47 des Zeigers (getrieben, **T9/Scratch 2**: `0x1_0177_E4C0` → `0x2_0177_E4C0`). TeraDesk uebergibt Root 0 (`sdk/apps/teradesk/icon.c:2511`), EmuTOS' eigener Desktop `DROOT = 1` (`desk/deskmain.c:2049`, `desk/deskwin.h:44`) → **A81** |
| 15 | `WF_HSLSIZ` | `:1690-1696` **nur bei Aenderung** speichern + `gadget = W_HSLIDE` → `w_cpwalk(h, W_HSLIDE)` (Slider-Teilbaum, ohne Pfeile); nicht offen: `do_walk` zeichnet nichts (Liste leer / Clip leer); TRUE | `:3023-3028` speichert immer, `redraw_hslider` immer (wenn offen): Pfeile + Track + Thumb | `:673` („only if changed") | **Bauart 3, unbeobachtbar in Pixeln**: Redraw ohne Aenderung (32 Zeichenoperationen statt 0, getrieben **T8** fuer die vertikale Variante); Pfeile mitgezeichnet (pixelgleich) → **A84** (P4) |
| 16 | `WF_VSLSIZ` | `:1697-1703` | `:3011-3016` | `:674` | wie 15 → **A84** |
| 8 | `WF_HSLIDE` | `:1704-1710` | `:3017-3022` | `:675` | wie 15 → **A84**; Wert gemessen gleich (r2 301–306, r4 480–486) |
| 9 | `WF_VSLIDE` | `:1711-1717` | `:3005-3010` | `:676` | wie 15 → **A84** (**T8**); Wert gemessen gleich (r2 307, r4 489) |
| 18 | `WF_COLOR` | `:1719-1726` `gadget = pinwds[0]` (**ungeprueft**), `w_tcolor/w_bcolor[gadget]` ausser −1, `do_cpwalk` → Gadget dieses Fensters **mit der neuen Farbe** neu gezeichnet (`w_setcolor :382-397` setzt `te_color`/`ob_spec` aus `w_tcolor`/`w_bcolor`); TRUE | `:3036-3048` `gadget_index` klammert `0..18`; speichert; `redraw_open` = ganzer Rahmen — **`draw_frame` liest `tcolor`/`bcolor` nie** (einzige Leser: `wind_get :2781-2782,2810-2811`); 1 | `:677` („that gadget, all windows" — falsch, ein Fenster) | **Bauart 3**: Round-Trip gleich (Hosttest), **Rendering fehlt**: 0 Pixel aendern sich (getrieben, **T7**), EmuTOS zeichnet das Gadget in der neuen Farbe. Der Kommentar `:3029-3035` **begruendet** es („our procedural draw_frame renders the fixed TOS4 3D look and does not consume these per-gadget words … pure round-trip state") → **A79**; Spec-Zeile → **A82**. Index-Klammer: EmuTOS-UB, akzeptabel |
| 19 | `WF_DCOLOR` | `:1727-1734` ohne Handle-Pruefung, `gl_wtcolor/gl_wbcolor[gadget]`, `gadget = -1` → kein Redraw; TRUE | `:2874-2885` gleich, Index geklammert | `:678` | **gleich** (Round-Trip per `wind_get` gedeckt); Rendering-Luecke wie `WF_COLOR` (neue Fenster erben die Worte, `:2427-2428`, zeichnen sie aber nicht) → in **A79** mit erfasst |

### 2.3 Felder ohne `case` — auf beiden Seiten `FALSE`

| # | Name (`include/aesdefs.h:534-552`) | EmuTOS | NewTOS | Messung |
|--:|---|---|---|---|
| 1 | `WF_KIND` | `default :1736` | `_ :3052` | r2 306, r4 465: gleich |
| 4 | `WF_WXYWH` | `default` | `_` | r4 468: gleich |
| 6 | `WF_PXYWH` | `default` | `_` | r4 474: gleich |
| 7 | `WF_FXYWH` | `default` | `_` | r4 477: gleich |
| 11 | `WF_FIRSTXYWH` | `default` | `_` | r4 492: gleich |
| 12 | `WF_NEXTXYWH` | `default` | `_` | r4 495: gleich |
| 13 | — (kein Name) | `default` | `_` | nicht gemessen; beide Seiten `default` (Schluss aus dem Code) |
| 17 | `WF_SCREEN` | `default` | `_` | r4 507: gleich |
| 20 | `WF_OWNER` | `default` | `_` | r4 510: gleich |
| 0, 21, 99, negativ | — | `default` | `_` | r4 462/513/516, r2 304: gleich |

Die Handle-Pruefung laeuft **vor** dem `switch`: ein ungueltiges Handle mit
einem unbekannten Feld ergibt auf beiden Seiten `FALSE` aus demselben Grund
(`:1634-1635` / `:2886-2889`).

**Bauart 1 / Bauart 2 (Feldebene): keine.** Beide `switch` beantworten genau
dieselben elf Nummern. Der A42-Fall hat auf der `wind_set`-Seite kein
Gegenstueck — alle Abweichungen sind Bauart 3.

---

## 3. Was unsere Arme tun, was `wm_set` nicht tut — und umgekehrt

**Nur wir:**

1. `WF_CXYWH`: unbedingtes `WM_REDRAW(work)` an den Verschobenen
   (`:2973`) und Fan-out in Handle-Reihenfolge (`:1721-1727`) — A78.
2. `WF_TOP` auf Handle 0 / nicht geoeffnetem Handle: Rahmen des alten Top
   deaktiviert, 1-px-Desktop-Rahmen, `bump_z` fuer einen geschlossenen Slot
   (`:725-747`) — A80.
3. `WF_NAME`/`WF_INFO`/`WF_COLOR`: Ganzrahmen-Repaint statt eines Gadgets
   (`:3075-3085`) — pixelgleich, ohne Ticket.
4. Slider-Setter: Repaint ohne Aenderungswaechter (`:3005-3028`) — A84.
5. `WF_NAME`/`WF_INFO`: Kopie in 80 Byte, NULL → "" (`:2323-2338`) — A83
   (NULL-Fall: EmuTOS speichert NULL und stuerzt beim Zeichnen; akzeptabel).
6. `WF_COLOR`/`WF_DCOLOR`: Index-Klammer (`:645-647`) — EmuTOS-UB, akzeptabel.
7. `WF_NEWDESK`: `NEWDESK_APP`/`NEWDESK_ABI` (`:1046-1048`) — intern,
   unbeobachtbar; `AIN_LEN`-Doppelkonvention (`:2359-2365`) — EmuTOS liest
   nie `addr_in`; nur unser Desktop nutzt sie (Notiz, kein Ticket).

**Nur EmuTOS:**

1. `w_move`-Blit beim Verschieben des Top-Fensters (`:938-996`) — A78.
2. `wm_update(BEG/END_UPDATE)`-Klammer (`:1638,1757`) — unbeobachtbar.
3. `w_setactive` → `ct_chgown` (`:321-337`, aus `draw_change :1095`) —
   Mausbesitz, single-tasking unbeobachtbar.
4. `WS_WORK(0)`-Neuberechnung bei `WF_TOP` auf Handle 0 (`:1073-1077`) —
   beobachtbar via `wind_get(0, WF_WXYWH)`; in A80 als EmuTOS-Seite notiert.
5. `gl_newroot` auch bei NULL-Baum gespeichert (`:1688`) — unbeobachtbar.
6. `WF_COLOR`: Gadget in der neuen Farbe gezeichnet (`:382-397`) — A79.

---

## 4. Getrieben — die Scratch-Ausgabe

Hosttest ueber `tos_api::aes::dispatch` (LP64-Block, 640×480-Raster wie
`aes_window_content.rs`, `wind_new` zwischen den Faellen, Nachrichten per
`evnt_mesag`, Pixel per Snapshot-Diff). Dateien
`tos-api/tests/zz_a59_scratch.rs` / `zz_a59_scratch2.rs` — nach dem Lauf
**entfernt**, Kopien im Job-Scratchpad. `gl_rfull` im Rig: `(0, 19, 640, 461)`.

```
## T1 WF_CXYWH pure move of top window (30,40,320,240)->(60,80,320,240)
   EmuTOS: ret=1; w_move blits WS_TRUE old->new (bb_screen, gemwmlib.c:963), moved=TRUE -> w_update skips gl_wtop (:1027): NO WM_REDRAW to the mover
   NewTOS: ret=1; backend copy_rect (blit) calls=0; messages=[type=20 handle=1 rect=(60,80,322,242)] (work rect now (61, 103, 296, 194))

## T2 WF_CXYWH pure shrink of top window (30,40,320,240)->(30,40,200,160)
   EmuTOS: ret=1; size branch, subset -> stop=w_handle, moved=TRUE (:1143-1148) -> w_update skips gl_wtop: NO WM_REDRAW to the shrunk window
   NewTOS: ret=1; messages=[type=20 handle=1 rect=(30,40,202,162)]

## T3 WF_CXYWH grow of top window (30,40,320,240)->(30,40,400,300)
   EmuTOS: ret=1; one WM_REDRAW, rect = damage ∩ hull(rlist) = WS_TRUE ∩ gl_rfull = (30,40,402,302) (w_redraw :896-905; not the work rect (31, 63, 376, 254))
   NewTOS: ret=1; messages=[type=20 handle=1 rect=(30,40,402,302)]

## T4 WF_CXYWH move of the lower window h1 (30,40,320,240)->(50,60,320,240) under h2 (200,150,300,200)
   EmuTOS: ret=1; w_update top-down (:1022-1044): first WM_REDRAW h2=2 (200,150,172,152), then WM_REDRAW h1=1 (50,60,322,242) = c ∩ hull(rlist)
   NewTOS: ret=1; messages=[type=20 handle=1 rect=(50,60,322,242)] [type=20 handle=2 rect=(200,150,172,152)]

## T5 wind_set(0, WF_TOP) with window h open on top
   EmuTOS: ret=1; wm_mktop(0): ob_order refuses ROOT (gemoblib.c:1210), draw_change(0) recomputes WS_WORK(0)=wm_calc(NONE, gl_rscreen)=(1,1,638,478) and returns before drawing (:1079-1085): 0 pixels change, no message; wind_get(0,WF_WXYWH) afterwards = (1,1,638,478) (was (0, 19, 640, 461))
   NewTOS: ret=1; pixels changed in h's title band (30, 40, 320, 23): 1215 (first Some((55, 43, 0, 16776191))); whole screen: 2775; messages=(none); wind_get(0,WF_WXYWH) after = (0, 19, 640, 461); WF_TOP=1
   (T5 pixels outside the window: 1560 in bbox x 0..639 y 19..479; sample [(0, 19), (639, 19), (0, 20), (639, 20), (0, 21), (639, 21)])

## T6 wind_set(h2, WF_TOP) on a created-but-unopened h2 while h1 is open
   EmuTOS: undefined: ob_order->get_par reads W_TREE[-1] (gemobjop.c:145-160) for an object with ob_next=NIL; no defined pixel effect
   NewTOS: ret=1; h1=1 title band pixels changed: 1215 (first Some((55, 43, 0, 16776191))); whole screen: 1215; messages=(none); WF_TOP=1

## T7 wind_set(h, WF_COLOR, W_NAME=3, tcolor=0x1172 (solid red fill), bcolor=0x1172)
   EmuTOS: ret=1; stores w_tcolor[3]/w_bcolor[3]; do_cpwalk -> w_cpwalk(h, 3, MAX_DEPTH, TRUE): W_NAME re-drawn with te_color=0x1172 (w_setcolor :382-397, :1719-1726): title-band pixels CHANGE (red fill)
   NewTOS: ret=1; title band pixels changed: 0; wind_get(WF_COLOR,3) -> (ret=1, tcolor=0x1172, bcolor=0x1172)

## T8 wind_set(h, WF_VSLIDE, 500) twice — second call with the value unchanged
   EmuTOS: ret=1; `if (pwin->w_vslide != pinwds[0])` (:1712) fails -> gadget stays -1 -> no w_cpwalk: zero drawing ops
   NewTOS: ret=1; backend drawing ops during the 2nd call: 32 (first few: ["fill_rect x=329 y=65 w=19 h=19 pel=8 clip=(30, 40, 351, 281)", ...])

## T9 WF_NEWDESK, LP64 gemlib packing (a_wind_set_ptr_int.c) with root=5 — arithmetic only, not dispatched
   EmuTOS: 32-bit pointer occupies int_in[2..3], root in int_in[4] (pinwds[2], :1688): disjoint. tree=0x123456780, root=5
   NewTOS: intin_ptr(2) reads int_in[2..5] = 8 bytes; gemlib wrote int_in[4]=root over pointer bits 32..47 -> kernel sees tree=0x523456780 (expected 0x123456780), root=int_in[4]=5

## T10 WF_NAME then in-place buffer edit AAAAAAAA->BBBBBBBB, then untop/top (frame repaint)
   EmuTOS: w_pname is the app's POINTER (:1661; w_bldactive reads it at every draw, :501/:737): after the re-top the title shows BBBBBBBB (pixels == 'set to B' reference, != 'A' snapshot)
   NewTOS: copy_cstr stored a copy at set time (window.rs:2910): after the re-top, title-band pixels differing from the 'A' snapshot: 0; differing from the 'B' reference: 168 (sanity: A vs B reference differ in 168 pixels)
```

Scratch 2 (T9 **durch den Dispatcher** gefahren, LP64-Baum auf dem Host,
gemlib-Packung mit Root = 2):

```
tree base = 0x000000010177E4C0 (bits 32..47 = 0x0001)
int_in[2..6] as the kernel's intin_ptr(2) reads them = 0x000000020177E4C0, int_in[4] (root) = 2
wind_set(0, WF_NEWDESK) returned 1
now painting the desktop background through the registered tree ...
survived: the corrupted pointer happened to be readable
```

Der Kernel hat den zerstoerten Zeiger `0x2_0177_E4C0` registriert (Rueckgabe
1); dass der anschliessende Hintergrund-Repaint auf dem Host nicht abstuerzte,
liegt an der macOS-Adressraumbelegung, nicht am Kernel — auf der Pi 4 ist
`0x2_xxxx_xxxx` fuer eine Anwendung mit Baum unter 4 GiB keine gueltige
Anwendungsadresse (`kernel/src/arch/aarch64/el0.rs:48-57`).

### Zu T1/T2 — die Herleitung der EmuTOS-Seite

`draw_change`, reine Verschiebung des Top-Fensters (`:1163-1191`): kein
Superset → „move, close or shrink"; `gl_wtop == w_handle` → `moved =
w_move(h, &stop, &c)`, `start = DESKWH`. `w_move` (`:938-996`): Quelle
vollstaendig auf dem Schirm oder Bewegung vom Rand weg → `*pstop = w_handle`,
`bb_screen` blittet, `prc = s` (das freigewordene Rechteck), Rueckgabe TRUE.
Schwanz: `w_drawdesk(&s+Schatten)`, `w_update(DESKWH, s, h, TRUE)`
(`:1004-1048`): `top = W_TREE[ROOT].ob_tail = h`; erste Runde
`!(moved && top == gl_wtop)` ist falsch → weder `w_cpwalk` noch `w_redraw`;
bei nur einem Fenster `bottom == top` → fertig. **Null `WM_REDRAW`.** Reine
Verkleinerung (`:1141-1157`): Subset → `stop = h`, `w_cpwalk(gl_wtop)`,
`moved = TRUE`; `w_update(DESKWH, c, h, TRUE)` ueberspringt `gl_wtop` genauso.

### Zu T4 — Reihenfolge

`w_update` beginnt bei `top = ob_tail` (= oberstes Fenster) und arbeitet sich
mit `get_prev`-Logik (`:1033-1043`) nach unten: `WM_REDRAW` fuer h2 zuerst,
dann h1. Bei uns: `send_redraw(h1, work)` (`:2973`) zuerst, dann
`broadcast_redraw` ueber Handles 1..15 (`:1723`) — h1 koalesziert per
Rechteck-Union in den vorhandenen Eintrag (`pd.rs`, `doq`-Nachbau), h2 folgt.
Die **Rechtecke** stimmen mit EmuTOS ueberein (die Union `work ∪ (d ∩ hull)`
ist `d ∩ hull`, weil `work ⊂ WS_TRUE`).

---

## 5. Spec gegen Quelle

| Spec-Stelle | Behauptung | Quelle | Befund |
|---|---|---|---|
| `:668-669`, `:677`, `:682-686`, `:1122` | `WF_NAME`/`WF_INFO`/`WF_COLOR` zeichnen das Gadget auf **allen offenen Fenstern** | `:1754-1755` `w_cpwalk(w_handle, gadget, …)` → `do_walk(wh = w_handle)` (`:363-379`, `:286-312`) — **ein** Fenster. EmuTOS' Kommentar `:1665` („update name applies to all open windows") meint „unabhaengig davon, ob getoppt" (Gegensatz zum Slider-Zweig `:1746-1752` im Nicht-3D-Bau) | **Spec falsch** → A82 (Researcher) |
| `:670` | `WF_CXYWH`: „whole frame + uncovered areas" | `draw_change`/`w_move`/`w_update` | unvollstaendig: Blit, `moved`-Ueberspringen, Nachrichtenreihenfolge und -rechteck fehlen → in A78 als Spec-Nachtrag |
| `:1389-1406` | Zeiger fuer `WF_NAME`/`WF_INFO`/`WF_NEWDESK` ist auf NewTOS **`addr_in[0]`** | Code: Doppelkonvention (`:2359-2365`), gemlib LP64 packt in `int_in[2..5]` (`a_wind_set_ptr_int.c:11`) | **Spec veraltet**; Root-Position fuer LP64 nicht festgelegt → in A81 |
| `:641-698` sonst | Handle-Pruefung, `WF_DCOLOR`-Ausnahme, Klammer, `default` | `:1630-1656,1736` | stimmt |

---

## 6. Nicht verifiziert

* **Hardware/Sichtbarkeit**: T1/T2/T4/T5 sind Nachrichten- und
  Pixelbefunde auf dem Host-Raster; der Flacker-Eindruck beim Verschieben
  (kein Blit) und der inaktive Rahmen nach `wind_set(0, WF_TOP)` sind auf der
  Pi 4 nicht angesehen worden. Beides ist auf QEMU per Screendump und auf der
  Pi sichtbar reproduzierbar (Rezept in den Tickets).
* **T9 auf dem Ziel**: der Absturz/Fehlbaum bei Root ≠ 0 ist rechnerisch und
  am Kernel-Eingang belegt (Scratch 2), nicht auf der Pi 4 gefahren — es gibt
  derzeit keine portierte App mit Root ≠ 0.
* **EmuTOS-Seite von T5** (`wind_get(0, WF_WXYWH)` = `(1,1,W−2,H−2)` nach
  `wind_set(0, WF_TOP)`) ist aus der Quelle hergeleitet, nicht in Hatari
  gemessen; die r2-Tabelle verbietet Handle-0-Setter, eine eigene Tabelle
  koennte es messen.
* **Titel > 79 Zeichen** (A83 b): nur hergeleitet; im 640-px-Rig nicht
  darstellbar.
* Feld 13 (namenlos): beide Seiten `default` laut Code, kein Messfall.

---

## 7. Ausserhalb des Auftrags — nur notiert, nicht verfolgt

* `wind_open` (`window.rs:2483`) schickt `WM_REDRAW(work)`; EmuTOS'
  `draw_change`-Open-Zweig schickt `WS_TRUE ∩ gl_rfull` (Rahmen + Schatten
  eingeschlossen, `:1163-1167,1213-1222,896-905`) — **Schluss** aus der
  Quelle, nicht getrieben. Gleiche Klasse wie A77.
* `aes_window_content.rs:748-752` (Doc-Kommentar) beschreibt fuer `WF_TOP`
  noch „WM_REDRAW for its work area"; der Code sendet inzwischen
  `plus_shadow(curr)` (`:756-765`). Kommentar veraltet.
* `aes-r2-wind.tsv:47-50` Kopf: „wind_set(WF_NAME/WF_INFO) is refused … by the
  loader" — Tabellenkommentar, kein Widerspruch zur Quelle.
