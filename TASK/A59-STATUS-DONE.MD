# A59 — Quittung

**Status: ERLEDIGT · Pruefung abgeschlossen · sieben Tickets angelegt · kein
Produktcode, keine Spec-Aenderung, kein Commit** · 2026-09-20 · Auditor.

Bericht: [`docs/analysis/aes-wind-set-vs-wm-set-2026-09-20.md`](../docs/analysis/aes-wind-set-vs-wm-set-2026-09-20.md).

## Was getan wurde

* EmuTOS `aes/gemwmlib.c` `wm_set` (`:1623-1760`) mit allen elf `case`, dem
  `default`, der Handle-Pruefung, der Slider-Klammer und den Helfern
  (`draw_change`, `w_move`, `w_update`, `w_redraw`, `w_cpwalk`, `do_walk`,
  `wm_mktop`, `ob_order`/`get_par`, `w_setcolor`, `wm_calc`, `wm_update`,
  `get_pwin`) gelesen; `include/aesdefs.h:534-552` und `gemsuper.c:345-346`
  abgeglichen; `CONF_WITH_*`-Gatter fuer das 512er-Profil geprueft
  (`include/config.h`).
* NewTOS `tos-api/src/aes/window.rs` `wind_set` (`:2867-3058`) mit jedem Arm
  und den Helfern gelesen, Kommentare eingeschlossen.
* Spec `docs/api/aes/window-graphics.md` § `wind_set` und § NewTOS
  implementation notes gegen beide Seiten gestellt.
* Messbasis gesichtet: alle `wind_set`-Faelle in r2/r4 stimmen mit EmuTOS
  ueberein (nur `AESGLOBAL`-Rauschen); die Tabellen decken Zeiger-, Farb-,
  Handle-0-, Nachrichten- und Pixelverhalten nicht ab.
* **Getrieben** per Scratch-Hosttest ueber `tos_api::aes::dispatch` (zehn
  Faelle T1–T10 + Scratch 2), Ausgabe im Bericht § 4.

## Ergebnis

Feldebene: beide `switch` beantworten dieselben elf Nummern — **keine Bauart
1, keine Bauart 2**. Sieben Abweichungen der **Bauart 3**, zwei davon mit
begruendendem Kommentar (der A42-Fall):

| Ticket | Prio | Befund | Beleg |
|---|---|---|---|
| [A78](A78.md) | P2 | `WF_CXYWH`: kein `w_move`-Blit, ein `WM_REDRAW` zu viel an den Verschobenen, Fan-out in Handle- statt z-Reihenfolge; Kommentar `window.rs:2969-2972` begruendet | T1/T2/T4 gefahren |
| [A79](A79.md) | P3 | `WF_COLOR`/`WF_DCOLOR`: Farbworte gespeichert, nie gezeichnet; Kommentar `:3029-3035` begruendet | T7 gefahren (0 Pixel) |
| [A80](A80.md) | P3 | `WF_TOP` auf Handle 0 / ungeoeffnetes Handle: Rahmen des obersten Fensters inaktiv + Desktop-Rahmen | T5/T6 gefahren (2775 / 1215 Pixel) |
| [A81](A81.md) | P2 | `WF_NEWDESK`: LP64-gepackter Root ueberlappt den Zeiger (Kernel `int_in[4]` + gemlib-Bindung); Spec `:1385-1406` veraltet | T9 + Scratch 2 gefahren |
| [A82](A82.md) | P3 | Spec: „all open windows" fuer `WF_NAME`/`WF_INFO`/`WF_COLOR` ist eine Fehllesung (ein Fenster) | Quelle `:1754-1755`, `:363-379` |
| [A83](A83.md) | P3 | `WF_NAME`/`WF_INFO`: Kopie statt Zeiger, 79 Zeichen | T10 gefahren |
| [A84](A84.md) | P4 | Slider-Setter ohne Aenderungswaechter (Pixel gleich, 32 Zeichenops statt 0) | T8 gefahren |

Gleich befunden (Rueckgabe, Zustand, gemessen oder gefahren): Handle-Pruefung
incl. `WF_DCOLOR`-Ausnahme, Slider-Klammer und Werte, `WF_CXYWH`-Geometrie
(CURR/PREV/WORK, nicht offen), `WF_TOP` auf offenem Fenster, `WF_NEWDESK`-
Semantik, `WF_DCOLOR`, alle neun namenlosen/verweigerten Felder.

Akzeptabel (EmuTOS undefiniert oder single-tasking unbeobachtbar): Index-
Klammer bei `WF_COLOR`/`WF_DCOLOR`, NULL-Titel → "", fehlende
`BEG/END_UPDATE`-Klammer, fehlendes `w_setactive`, Ganzrahmen- statt
Gadget-Repaint bei `WF_NAME`/`WF_INFO`.

## Nicht verifiziert

* Sichtbarkeit von A78/A80 auf QEMU-Screendump und Pi 4 (nur Host-Raster).
* EmuTOS-Seite von T5 (`wind_get(0, WF_WXYWH)` nach `wind_set(0, WF_TOP)`):
  aus der Quelle hergeleitet, nicht in Hatari gemessen.
* A81 auf dem Ziel: Zeigerzerstoerung am Kernel-Eingang belegt, Absturz auf
  der Pi 4 nicht gefahren (keine portierte App mit Root ≠ 0).
* Titel > 79 Zeichen (A83): hergeleitet.

## Aufraeumung

`tos-api/tests/zz_a59_scratch.rs` und `zz_a59_scratch2.rs` geloescht (Kopien
und Ausgaben im Job-Scratchpad). `git status`: nur die neuen Dateien
`docs/analysis/aes-wind-set-vs-wm-set-2026-09-20.md`, `TASK/A59-STATUS-DONE.MD`,
`TASK/A78.md` … `TASK/A84.md` und die Statuszeile in `TASK/A59.md`; kein
Produkt-, Spec- oder Testcode angefasst.
