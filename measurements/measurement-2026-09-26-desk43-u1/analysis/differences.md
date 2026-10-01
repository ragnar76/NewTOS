# U1 — Die zwölf gefundenen Abweichungen, **bewusst nicht behoben**

U1 ändert kein Produktverhalten. Alles hier ist Befund, kein Fix. Die Spalte
*gehört zu* nennt die Arbeitseinheit aus `docs/analysis/desktop-userspace-plan-v2.md`
§5, in der die Stelle fällig ist — soweit der Plan sie schon kennt.

| | Befund | Beleg EmuTOS | Beleg bei uns | gehört zu |
|---|---|---|---|---|
| **D1** | Kein `dos_gdrv()` als erste Handlung. EmuTOS merkt sich das Startlaufwerk in `G.g_stdrv`, bevor die AES überhaupt offen ist; wir haben kein `g_stdrv`. | `deskmain.c:1950` | kein Treffer für `g_stdrv` ausser in Kommentaren (`inf.rs:929-934` beschreibt den Ersatz) | offen — **kein Ticket** |
| **D2** | **`graf_handle` wird nie gerufen.** `gl_wchar`/`gl_hchar` kommen aus `tos_api::vdi::font::{CELL_W, CELL_H}`. | `deskmain.c:1956` | Driftprobe Gruppe A, `tos_api::vdi::font` (8 Codezeilen) | **U7** (AES-Lücke G4) |
| **D3** | Kein `wind_update(BEG); desk_busy_on(); wind_update(END);` nach `wind_get`. Der Sanduhr-Zeiger kommt bei uns schon von `sh_tographic` (`gemshlib.c:237`), die Klammer fehlt trotzdem. | `deskmain.c:1962-1964` | `desk/mod.rs:938-942` begründet nur das *Ende* (`desk_busy_off`), nicht das Fehlen des Anfangs | offen — **kein Ticket** |
| **D4** | `G.g_cnxsave` ist bei uns eine Rust-Struktur, nicht `dos_alloc_anyram(sizeof(CSAVE))`. Kein GEMDOS-Trap, also auch keine `nomem_alert()`-Fehlerbehandlung. | `deskmain.c:1970-1977` | `desk/global.rs` (`g_cnxsave` als Feld) | **U8** (dort braucht der EL0-Prozess einen echten Heap) |
| **D5** | **`app_start()` fragt nicht `shel_get`.** Wir öffnen sofort `C:\NEWDESK.INF`. Im Trace steht kein Opcode 122. | `deskapp.c:832-836` | `desk/inf.rs` `load_and_apply`; Opcodes 122/123 sind serverseitig da (`tos-api/src/aes/mod.rs:837-838`), der Desktop bindet sie nicht (`desk/mod.rs:57-89`) | **U2** |
| **D6** | Reihenfolge: `objc_add`×8 (`obj_init`) und `wind_calc` (`win_view`) laufen bei uns **vor** dem INF-Lesen, bei EmuTOS **danach** (`win_start()` steht hinter `app_start()`). | `deskmain.c:2018` vs. `:2028`, `deskwin.c:95,117` | `desk/global.rs:718-723` (in `global::init`), `desk/mod.rs` `run()`-Reihenfolge | **U3** (der Epilog/Neustart ordnet den Anlauf ohnehin um) |
| **D7** | Die drei `menu_icheck` für View/Sort/Fit stehen bei EmuTOS **vor** `app_blddesk`, bei uns erst in `men_update`. | `deskmain.c:2039-2042` | `desk/menu.rs:1100-1107` | offen — **kein Ticket** |
| **D8** | **`menu_bar` wird zweimal gerufen** — einmal direkt nach `wind_set(WF_NEWDESK)` (`install_menu_bar`), einmal am Ende in `redraw_desktop`. EmuTOS ruft es genau einmal, nach `men_update`. | `deskmain.c:2073` | `desk/mod.rs:902` und `desk/redraw.rs:246` | offen — **kein Ticket** |
| **D9** | Der Abschluss ist bei uns ein **Vollbild-Redraw** (`graf_mouse(M_OFF)`, `form_dial(FMD_FINISH)`, `menu_bar`, `graf_mouse(M_ON)`), den EmuTOS' `deskmain` nicht hat: dort malen `wind_set(WF_NEWDESK)` und `menu_bar` das Bild. | `deskmain.c:2046-2075` (kein Redraw) | `desk/redraw.rs:238-248` | offen — **kein Ticket**; Nachbarschaft zu ADR-022 (`damage_redraw`-Abdrift) |
| **D10** | Die `wind_update(BEG/END)`-Klammer um `menu_bar` + `desk_busy_off` fehlt. | `deskmain.c:2072,2075` | `desk/mod.rs:902,943` | offen — **kein Ticket** |
| **D11** | Nach `desk_busy_off` folgen zwei GEMDOS-Traps, die nur die Konsolenzeile *„desktop up (n drives…)"* füttern (`dos_sdrv(dos_gdrv())`). Fremdkörper in der Aufrufsequenz. | — | `desk/mod.rs:945-949` | **U4** (dort fällt `kprintln` weg) |
| **D12** | `_IDT` wird per `Ssystem(S_GETCOOKIE)` schon im Anlauf gelesen; EmuTOS liest es per `Supexec(get_idt_cookie)` erst in `do_wredraw`. | `desksupp.c:219` | `desk/redraw.rs:136-146` + `desk/mod.rs:856-862` — **im Baum begründet** | bleibt (begründete Abweichung) |

## Einordnung

* **D2 und D5 sind die einzigen zwei, die der Plan schon als Lücke führt**
  (G4 → U7, Zustandskanal → U2). Beide sind durch diese Messung von „im Code
  gesehen" auf „im Trace belegt" gehoben.
* **D1, D3, D7, D8, D9, D10 hat bisher niemand notiert.** Sie sind alle
  Reihenfolge oder fehlende `wind_update`-Klammern, also billig — aber sie
  gehören in eine Einheit, nicht in U1, und sie brauchen je einen Screendump
  vorher/nachher, weil D8/D9 die Zeichenreihenfolge berühren.
* **D9 ist der auffälligste.** Ein Vollbild-Redraw am Ende des Anlaufs ist genau
  die Form von Eigenbau, die ADR-022 als Abdrift 2 benennt
  (`docs/MEMORY.md:1484-1498`). Ob er hier nötig ist, weil `wind_set(WF_NEWDESK)`
  bei uns nicht malt, ist **nicht gemessen** und wäre die erste Frage einer
  Einheit, die ihn anfasst.
