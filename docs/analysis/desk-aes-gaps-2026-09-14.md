# Die AES-Lücken, die `kernel/src/desk/` verdeckt hat

**Datum:** 2026-09-14 · **Anlass:** Schritt 1 des DESK-Rückbaus — jede Umgehung
der AES-Trapgrenze aus `kernel/src/desk/` entfernen und benennen, was dabei
zerbricht. Der Wiederaufbau ist ein eigener Auftrag.

**Maßstab.** Erlaubt ist, was `desk/mod.rs::gem_trap` (`svc #2`, Selektor 200)
über einen AESPB schickt — genau der Weg, den EmuTOS' `desk/gembind.c` geht.
Alles andere ist ein Befund.

EmuTOS-Pin: `references/emutos`, Pfade unten relativ dazu.

---

## 1. Die vierzehn gemeldeten Stellen, mit Urteil

### 1.1 Direkte AES-Funktionsaufrufe (4)

| # | Stelle (vorher) | Urteil | Was daraus wurde |
|---|-----------------|--------|------------------|
| 1 | `main.rs:538` `tos_api::aes::window::save_windows()` | **(b)** | entfernt → **G1/G2** |
| 2 | `main.rs:560` `tos_api::aes::shell::sh_draw_clear()` | **(b)** | entfernt → **G1** |
| 3 | `main.rs:583` `tos_api::aes::window::restore_windows()` | **(b)** | entfernt → **G1/G2** |
| 4 | `redraw.rs:209` `tos_api::aes::window::redraw()` | **(a)** | `form_dial(FMD_FINISH, 0,0,0,0, 0,0,scr_w,scr_h)`, Opcode **51** |

Zu (4), der Beleg: EmuTOS' Desktop benutzt für „etwas hat den ganzen Bildschirm
belegt, stell ihn wieder her" genau diesen Aufruf —
`desksupp.c:826`: `form_dial(FMD_FINISH, 0,0,0,0, 0,0,scr_width,scr_height);`,
Gegenstück `:795` mit `FMD_START`. Der AES-Arm (`gemfmlib.c:375-379`) ist
`w_drawdesk(pt)` + `w_update(DESKWH, pt, DESKWH, FALSE)`, also Hintergrund +
Rahmen über `pt` und ein `WM_REDRAW` an jedes berührte Fenster — dasselbe, was
`window::redraw()` tat, nur über die Trapgrenze und mit der Meldungsverteilung,
die vorher fehlte.

### 1.2 Die neun Strukturimporte — **kein Befund, das Briefing lag hier daneben**

`Object`, `TedInfo`, `IconBlk` sind **keine AES-Internas**, sondern die
öffentliche GEM-Datenschnittstelle: `include/obdefs.h` liegt jedem GEM-SDK bei,
und EmuTOS' eigener Desktop schreibt `G.g_screen[i].ob_x = …` durchgehend
(`deskapp.c`, `deskobj.c`, `deskwin.c`). `rsrc_gaddr` liefert einen **typisierten
Zeiger**, den die Anwendung dereferenziert; „nur Bytes im Ressourcenblob" ist
nicht, wie GEM funktioniert.

Der Import ist damit das Äquivalent eines `#include <obdefs.h>` und bleibt. Eine
*Layering*-Anmerkung bleibt trotzdem stehen: die drei Typen wohnen in
`tos_api::aes::object` statt in `tos-types`, wo die geteilten ABI-Strukturen
sonst liegen. Das ist eine Umzugsfrage, keine AES-Lücke — **nicht angefasst**.

**Was hier wirklich ein Befund war** und das `grep` des Briefings nicht fand:
nicht der *Typ*, sondern das **Verketten** der Objekte von Hand (siehe 2.1).

---

## 2. Weitere Umgehungen, die der Befund nicht hatte

### 2.1 `global.rs::ob_add` + `obj_init` — handgeschriebenes `objc_add` → **(a)**

`ob_add` legte `ob_head`/`ob_tail`/`ob_next` selbst, und `obj_init` verdrahtete
ROOT ↔ DROOT ↔ die sieben Fensterboxen von Hand. EmuTOS tut beides über die
Bindung: `deskobj.c:145` (`objc_add(G.g_screen, ROOT, DROOT+ii)` in der Schleife)
und `deskobj.c:232` (in `obj_ialloc`). Bindung: `desk/gembind.c:352-358`,
`AES_CTRL_CODE(OBJC_ADD, 2, 1, 1)`.

Umgestellt auf Opcode **40**. Verhaltensgleich: `tos-api`s `ob_add`
(`aes/object.rs:1112-1124`) legt dieselbe Geschwisterkette.

### 2.2 `menu.rs` `set_disabled`/`set_checked` — direkte `ob_state`-Schreiber → **(a)**

Das war der klarste Fall von „in `desk` eingebaut, weil AES noch nicht so weit
war" — samt schriftlicher Begründung im Modulkopf:

> *„EmuTOS `men_update` calls `menu_ienable`/`menu_icheck`; in `tos-api` those
> aesbind calls **redraw the item unconditionally** … This is the desktop
> managing its own tree, not a workaround around the AES."*

**Die Begründung ist falsch.** `tos-api`s `menu_icheck` ruft
`do_chg(pb, item, CHECKED, …, dodraw = false, …)` (`aes/menu.rs:716-718`) — genau
EmuTOS' `do_chg(…, CHECK_IT, FALSE, FALSE)` (`gemsuper.c:172`). `menu_ienable`
nimmt `dodraw` aus Bit 15 des Item-Worts (`aes/menu.rs:725-730`,
`gemsuper.c:175-176`), das `men_update` nie setzt. Keiner der beiden zeichnet.

EmuTOS' `men_update` benutzt die Bindungen für jedes gesteuerte Item
(`deskmain.c:534-585`, `men_list` `:453-457`) und schreibt das `ob_state`-Bit nur
an **einer** Stelle von Hand: beim Deaktivieren der Trennstriche
(`deskmain.c:475-486`). Genau diese eine Stelle bleibt jetzt auch bei uns direkt
(`menu.rs::build_menu`).

Umgestellt auf Opcode **32** / **31**.

### 2.3 Vier Kopien `screen_dims()` + `global::init(w, h)` — Raster-Backend statt AES → **(a)**

`dlg.rs`, `insapp.rs`, `inf.rs`, `menu.rs` hatten je ein eigenes
`tos_api::host::host().and_then(|h| h.raster())` mit `(640, 480)` als
erfundenem Rückfall; `desk::run()` griff sich den Backend direkt für
`global::init`.

EmuTOS holt die Bildschirmmaße über die AES: `deskmain.c:1959`
`wind_get_grect(DESKWH, WF_WXYWH, &G.g_desk);` — direkt nach `appl_init` und
`graf_handle`. Wo es den **ganzen** Bildschirm braucht, leitet es ihn aus
derselben Größe ab: `desksupp.c:786-787`
`scr_width = G.g_desk.g_w; scr_height = G.g_desk.g_y + G.g_desk.g_h;`.

Umgesetzt: `desk::run()` liest `wind_get(DESKWH, WF_WXYWH)` (Opcode **104**) und
gibt den GRECT an `global::init`; `desk/mod.rs::g_desk()` / `screen_rect()` sind
die einzigen zwei Stellen, die Geometrie kennen, und beide kommen aus dieser
Quelle. `MENU_H` (19) und die AES-`BAR_H` (19) stimmen überein, der Wert ändert
sich also nicht.

### 2.4 `desk::run()`s `tos_api::vdi::mouse::display_at(w/2, h/2, backend)` → **(b), G3**

Ein direkter VDI-Aufruf **mit dem Backend-Zeiger** — etwas, das eine EL0-Anwendung
gar nicht kann. Die Begründung im Quelltext nannte sich selbst beim Namen: ohne
ihn säße der Zeiger auf (0,0) über der Menüleiste und das AES-Hover-Menü klappte
beim Start das Desk-Menü auf.

EmuTOS' Desktop positioniert den Zeiger **nie**. `ratinit` (`gemgsxif.c:205-209`)
ist `gsx_1code(SHOW_CUR, 0); gl_moff = 0;` — nur einblenden; gerufen wird es aus
`sh_main` (`gemshlib.c:716`). Zentriert wird im **VDI**: `vdi_mouse.c:582-583`
`GCURX = xres / 2; GCURY = yres / 2;`. Eine AES-Bindung dafür gibt es nicht.

Entfernt. **Gemessen:** der Zeiger steht im Screendump trotzdem in der Mitte —
`tos-api/src/vdi/mouse.rs::ensure_init` tut dasselbe wie EmuTOS' VDI. Der
Desk-Aufruf war also überflüssig; die vermutete AES-Reihenfolgenlücke ist damit
für diesen Boot-Pfad **nicht nachweisbar**.

---

## 3. Die Lückenliste

Format: was `desk/` brauchte · welche AES-Funktion das täte · existiert sie ·
was EmuTOS an der Stelle tut.

### G1 — Der Desktop kann kein Programm über die AES starten *(die Wurzel)*

* **Gebraucht:** ein Programm starten und danach wieder Desktop sein.
* **AES-Funktion:** `shel_write(SHW_EXEC, isgraf, 1, cmd, tail)`, Opcode **121**.
* **Existiert:** ja, aber sie **merkt sich den Befehl nur** (`aes/shell.rs:401-444`).
  Ausgeführt wird er von der Shell-Schleife `crate::shell_loop::sh_main` — die es
  **auch schon gibt** und vollständig ist (`wm_init`, `ratinit`, `sh_draw_clear`,
  `sh_ldapp`). Der fehlende Baustein ist ein anderer: **`desk::run()` kehrt nie
  zurück**, also kommt die Schleife nie wieder an die Reihe.
* **EmuTOS:** `deskpro.c:41` `shel_write(SHW_EXEC, …)` → `hndl_button`s
  `do_filemenu(OPENITEM)` liefert TRUE (`deskmain.c:988`) → `done = TRUE` → die
  Ereignisschleife endet → Epilog (`menu_bar(NULL,0)`,
  `wind_set(DESKWH,WF_NEWDESK,NULL)`, `appl_exit()`, `deskmain.c:2159-2166`) →
  **der Desktop-Prozess terminiert** → `sh_ldapp` kehrt zurück → `sh_main`
  (`gemshlib.c:708-731`) macht `wm_init()`, `ratinit()`, `sh_draw(D.s_cmd, TRUE)`
  und lädt erst dann das Kind. Wenn das Kind `Pterm`t, betritt `sh_main` den
  Desktop **von oben** neu.
* **Folge des Rückbaus:** `desk::main::run_gem_app` ruft weiterhin
  `crate::launch::run_program_with_tail` direkt. Das ist die verbliebene
  Umgehung; sie zu schließen heißt, `desk::run()` beendbar zu machen — das ist
  der Wiederaufbau, nicht Schritt 1.

### G2 — Es gibt keinen Kontextspeicher des Desktops (`cnx_put`/`cnx_get`)

* **Gebraucht:** die eigenen Fenster über einen Programmstart hinweg behalten.
* **AES-Funktion:** *keine, und das ist richtig so.* Die AES-Fenstertabelle ist
  **nicht** der Ort dafür; `wm_init()` räumt sie bei jedem Durchlauf ab.
* **Existiert:** nein — und `save_windows`/`restore_windows` in
  `tos-api/src/aes/window.rs:1056-1091` haben **keinen Opcode**, sind also von
  einer EL0-Anwendung nicht erreichbar gewesen.
* **EmuTOS:** der Desktop speichert seinen Zustand in **seinem eigenen**
  Sicherungsbereich: `G.g_cnxsave = dos_alloc_anyram(sizeof(CSAVE))`
  (`deskmain.c:1970`), `cnx_put()` im Epilog (`deskmain.c:2154`), `cnx_get()`
  beim Wiedereintritt. Fenster werden aus **Desktop-Daten** wiederhergestellt,
  nicht aus einem AES-Schnappschuss.
* **Kaputt:** nach einem GEM-Programm bleibt in der geteilten Fenstertabelle
  stehen, was das Kind hinterlassen hat; die Desktop-Fenster sind nicht mehr
  angemeldet. Nur über Doppelklick erreichbar → **Pi-4-HW**.
* **Nebenbefund:** `tos-api/tests/aes_app_return_descriptor.rs:316,325` und
  `aes_launch_boundary_reset.rs:328,335` rufen die beiden Funktionen weiter auf
  und schreiben die alte Aufrufreihenfolge als Vertrag fest. Sie laufen grün
  (nichts an `tos-api` geändert), prüfen aber jetzt Code, den kein
  Produktivpfad mehr benutzt. Der Kommentar in `:397` ist inhaltlich veraltet.
  **Nicht angefasst** — gehört dem Lead.

### G3 — Keine AES-Bindung setzt den Mauszeiger

* **Gebraucht:** Zeiger beim Start an einen definierten Ort.
* **AES-Funktion:** keine. `graf_mouse` (78) setzt Form und Sichtbarkeit, nicht
  die Position; `graf_mkstate` (79) liest nur.
* **EmuTOS:** macht es im VDI (`vdi_mouse.c:582-583`) und nicht in `desk/`.
* **Status:** nach der Entfernung **gemessen unauffällig** — unser
  `vdi/mouse.rs::ensure_init` zentriert genauso. Die Lücke ist damit theoretisch
  (eine EL0-Anwendung hätte weiterhin keinen Weg), aber sie kostet nichts.

### G4 — `gl_wchar`/`gl_hchar` kommen nicht aus `graf_handle` *(nicht umgestellt)*

* **Gebraucht:** Zeichenzellenmaße für Menübreiten, Textansicht, `#W`-Zeilen
  in `NEWDESK.INF`.
* **Heute:** `use tos_api::vdi::font::{CELL_W, CELL_H}` in `act.rs`, `inf.rs`,
  `menu.rs`, `win.rs` — Konstanten aus dem VDI, am AES vorbei.
* **AES-Funktion:** `graf_handle` (**77**), existiert und liefert genau
  `gl_wchar`/`gl_hchar` (`aes/graf.rs:504-516`).
* **EmuTOS:** `deskmain.c:1956`
  `gl_handle = graf_handle(&gl_wchar, &gl_hchar, &gl_wbox, &gl_hbox);` — eine
  Zeile vor dem `wind_get_grect`, das wir schon umgestellt haben.
* **Urteil: (a), bewusst nicht ausgeführt.** Die Umstellung macht aus
  Compile-Zeit-Konstanten Laufzeitwerte und zieht `icons.rs`' `pub const
  IB_WTEXT`/`IB_HTEXT` mit (const → fn), also sechs Dateien für null
  beobachtbaren Unterschied bei 8×16. Gehört in denselben Schritt wie G5.

### G5 — `gl_wschar`/`gl_hschar` haben **keine** AES-Bindung

* **Gebraucht:** die Kleinschrift unter den Icons (`app.rs`, `icons.rs`).
* **AES-Funktion:** **keine.** `graf_handle` liefert nur die vier großen Maße.
* **EmuTOS:** liest die AES-**Globale** direkt — `deskapp.c:519-520,810-811`
  `G.g_iblist[i].ib_wtext = MAX_ICONTEXT_WIDTH * gl_wschar;`. Gesetzt wird sie
  im AES aus dem VDI: `gemgraf.c:407`
  `vst_height(gl_ws.ws_chminh, &gl_wsptschar, &gl_hsptschar, &gl_wschar, &gl_hschar);`.
* **Urteil: (b) — echte Lücke, aber sie trifft EmuTOS genauso.** EmuTOS' Desktop
  ist in die AES **hineingelinkt** und liest deren Globale; eine EL0-Anwendung
  müsste sich eine eigene VDI-Workstation öffnen (`v_opnvwk` + `vst_height`).
  Der `desk/`-Weg ist damit EmuTOS-treu, aber nicht app-fähig. **Nicht entfernt**
  — die Entfernung nähme dem Desktop jede Icon-Beschriftungsgeometrie.

### G6 — `desk/` ruft GEMDOS/BIOS/XBIOS als Rust-Funktion, nicht als Trap

* **Betroffen:** `dir.rs` (9 Funktionen), `model.rs` (`Fsfirst`/`Fsnext`/`Fsetdta`),
  `inf.rs` (5), `app.rs`/`main.rs`/`mod.rs` (`drvbits`), `main.rs`
  (`keymap::to_ascii`, `host().console_read_key_blocking`, `fbcon::*`).
* **Form:** `TrapArgs { regs, func, svc_imm: TRAP_GEMDOS }` wird **von Hand**
  gebaut und die Handler-Funktion direkt gerufen — dieselbe ABI, aber ohne
  `svc`.
* **Urteil: (a), mechanisch, ~25 Stellen.** Beobachtbar identisch (die Handler
  sind dieselben; DTA/Laufwerk sind heute Vordergrund-Globale), also **keine**
  AES-Lücke — eine Ebenenfrage.
* **EmuTOS-Entsprechungen, falls umgestellt wird:** Laufwerksmaske ist dort
  `dos_sdrv(dos_gdrv())` = GEMDOS `Dsetdrv(Dgetdrv())` (`deskapp.c:714`,
  `deskins.c:297`, `desksupp.c:1575,1819`), **nicht** BIOS `Drvmap`; die
  Tastaturtabelle `Keytbl` (XBIOS 0x10); das Tastenwarten `bios_conin()`.
* **Nicht umgestellt** — eine andere Achse als der Auftrag, und halb umgestellt
  wäre schlechter als gar nicht.

### G7 — Dialoge: kein `form_center`, kein `form_dial(FMD_START)`

* **Gebraucht:** einen selbstgebauten Dialogbaum zentrieren und den Bildschirm
  darunter retten.
* **AES-Funktionen:** `form_center` (**54**) und `form_dial(FMD_START/FMD_FINISH)`
  (**51**) — **beide vorhanden**.
* **Heute:** `dlg.rs:65`, `inf.rs:1179`, `inf.rs:1310`, `insapp.rs:149` rechnen
  die Mitte selbst aus `screen_dims()`; danach ruft der Aufrufer
  `redraw::redraw_desktop()` und malt den **ganzen** Bildschirm neu, statt nur
  das Dialogrechteck (`main.rs:845-849,878,944,966,985,1012,1027,1051,1103,1144`).
* **EmuTOS:** `deskinf.c:353,368` und `deskdir.c:83,115,116` —
  `form_center(tree, &x,&y,&w,&h); form_dial(FMD_START,0,0,0,0, x,y,w,h); … ;
  form_dial(FMD_FINISH,0,0,0,0, x,y,w,h);`
* **Urteil: (a), bewusst nicht ausgeführt.** Es ist die richtige Reparatur, aber
  sie baut vier Dialogerzeuger um (erst bauen, dann zentrieren, dann die
  Rechtecke durch den Aufrufer reichen) — Wiederaufbau, nicht Rückbau. Der
  `FMD_FINISH`-Teil ist durch die Umstellung von `redraw_desktop` (1.1 #4)
  bereits an der richtigen AES-Funktion, nur noch nicht am richtigen Rechteck.

---

## 4. Was jetzt kaputt ist

1. **GEM-App-Start aus dem Desktop** (`Doppelklick` / `File → Open` auf `.NAP`
   bzw. `.NTP`): kein Fensterretten (G2), keine Bildschirmlöschung vor dem Kind
   (G1). Das Kind komponiert auf die Pixel des Desktops; nach seinem `Pterm`
   steht in der AES-Fenstertabelle, was es hinterlassen hat. `desktop_resume`
   meldet den Hintergrundbaum wieder an und malt neu, also kommt ein Bild
   zurück — aber nicht der Fensterstand von vorher.
   **Nur interaktiv erreichbar → echte Pi-4-HW.** Der `#Z`-Autorun-Pfad ist
   nicht betroffen (der geht über `shell_loop`, nicht über `run_gem_app`).
2. **Sonst nichts Gemessenes.** Boot, Desktopaufbau, Fensterinhalt, Menüleiste,
   Mauszeigerposition sind im raspi4b-Screendump unverändert
   (`measurement-2026-09-14-desk/screendumps/desk-after.png` gegen
   `measurement-2026-09-12-a37-abnahme/desk.png` — Unterschiede nur im
   Verzeichnisinhalt der Karte).

## 5. Was am Briefing falsch war

* **Die neun Strukturimporte sind kein Befund** (1.2). `OBJECT`/`TEDINFO`/
  `ICONBLK` sind die öffentliche GEM-Schnittstelle, kein AES-Internum.
* **Die interessante Umgehung an derselben Stelle war eine andere:** das
  *Verketten* der Objekte von Hand statt `objc_add` (2.1) — vom `grep` nicht
  erfasst, weil dort kein `tos_api::` steht.
* **„EmuTOS' Desktop ist ein sauberer AES-Client" stimmt nicht.** Er ist in die
  AES hineingelinkt und liest `gl_width`, `gl_height` (`deskobj.c:140`,
  `deskapp.c:1346`), `gl_wschar`, `gl_hschar` (`deskapp.c:519`) und `gl_apid`
  direkt. Der Maßstab „was eine EL0-Anwendung könnte" ist **strenger als
  EmuTOS** — wo beides auseinanderfällt (G5), steht das hier ausdrücklich.
