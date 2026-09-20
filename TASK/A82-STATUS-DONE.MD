# A82 — STATUS: DONE (Spec-Korrektur, kein Code)

`docs/api/aes/window-graphics.md` behauptete an vier Stellen, `wind_set`
`WF_NAME`/`WF_INFO`/`WF_COLOR` zeichneten das Gadget auf **allen offenen
Fenstern**. Der Researcher hat die Spec am 2026-09-20 richtiggestellt
(uncommitted im Baum; Quittung aus `git diff docs/api/aes/window-graphics.md`
gegen HEAD `a8d3163`). Kein NewTOS-Code betroffen — `redraw_open`
(`tos-api/src/aes/window.rs`) zeichnete immer nur das eine Fenster ueber
dessen Rechteckliste.

## Quelle

`references/emutos/aes/gemwmlib.c:1754-1755` `w_cpwalk(w_handle, gadget,
MAX_DEPTH, TRUE)` → `w_bldactive(w_handle)` + `do_walk(wh = w_handle, …)`
(`:363-379`, `:286-312`): die Owner-Rechteckliste `D.w_win[wh].w_rlist`
**dieses einen** Fensters, ein Gadget-Teilbaum, nur wenn offen
(`if (pwin->w_flags & VF_ISOPEN)`, `:1662`, `:1670`). EmuTOS' Kommentar
`:1665` („update name applies to all open windows“, ebenso `:1673`) ist die
Quelle des Missverstaendnisses: er kontrastiert den Zweig mit dem Fall des
geschlossenen Fensters darueber (Zeiger gespeichert, kein Redraw) und den
Slider-Armen darunter (im Nicht-3D-Bau nur beim getoppten Fenster,
`:1746-1752`).

## Die fuenf korrigierten Stellen (Diff)

| Stelle | vorher | nachher |
|---|---|---|
| Helferzeile unter **Source** (`wm_set`) | „`w_cpwalk` (redraw one gadget on all/topped windows)“ | „redraw one gadget of the **target window alone**, clipped to that window's own rectangle list — corrected 2026-09-20, A82“ |
| Feldtabelle `WF_NAME` (2) | „`W_NAME` gadget, **all open windows** (`do_cpwalk`, `:1665`)“ | „`W_NAME` gadget of **this one window**, if it is open (`w_cpwalk(w_handle,…)`, `:1665,1754-1755`)“ |
| Feldtabelle `WF_INFO` (3) | „`W_INFO` gadget, **all open windows** (`:1673`)“ | „`W_INFO` gadget of **this one window**, if it is open (`:1673,1754-1755`)“ |
| Feldtabelle `WF_COLOR` (18) | „that gadget, all windows `:1719`“ | „that gadget of **this one window** (`w_cpwalk`, `:1719,1754-1755`)“ |
| Blockzitat „Redraw scope“ | „they redraw the one gadget on **all open windows** (name/info apply to every window's frame)“; Slider: „only the slider gadget“ | Herleitung ueber `do_walk` ueber `D.w_win[wh].w_rlist` allein, nur wenn offen; Einordnung des Kommentars `:1665`; Slider-Umfang praezisiert auf den `W_HSLIDE`/`W_VSLIDE`-**Teilbaum** (Spur + Elevator, `w_adjust`, `:430-436,468-474`; Pfeile sind Geschwister unter `W_HBAR`/`W_VBAR`, `ob_draw`s `everyobj(tree, obj, tree[obj].ob_next, …)`, `gemoblib.c:959-977`); `draw_change` als der eine Pfad, der mehrere Fenster neu malt; Waechter + Pfeil-Ausschluss als NewTOS-Luecke A84 vermerkt |
| Verifikationspunkt „`wind_set(WF_NAME)` redraws all windows' name gadget“ | „redraws all windows' name gadget (`do_cpwalk`)“ | „redraws that one window's name gadget, if it is open (`w_cpwalk`, `:1665,1754-1755`)“; `WF_VSLIDE`: „only the slider subtree of that same window“ |

Zusaetzlich (Randfall 5) der Slider-Teilbaum-Umfang und der Verweis auf
A84. Das A83-Blockzitat („store a pointer, not a copy“) im selben Diff
gehoert zu A83, nicht zu A82.

## Pruefung

Zeilen gegen den gepinnten Baum gelesen (`gemwmlib.c:286-312`, `:363-379`,
`:1660-1675`, `:1719`, `:1739-1755`; `gemoblib.c:959-977`); NewTOS
`redraw_open` → `draw_frame_pieces(handle, …)` ueber `walk_rects(handle,
…)` bestaetigt (ein Fenster). Keine Codeaenderung, kein Test noetig; die
A84-Hosttests decken den Slider-Teil der korrigierten Stelle
(Aenderungswaechter) ab.

## Offen

* Spec-Datei bleibt uncommitted (Lead committet).
* Der Pfeil-Ausschluss (Slider-Teilbaum) ist in der Spec als NewTOS-Luecke
  vermerkt und mit A84 **nicht** nachgezogen (`TASK/A84-STATUS-DONE.MD`).
