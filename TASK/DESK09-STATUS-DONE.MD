# DESK09 — STATUS: DONE (2026-09-23)

**Auftrag:** `cnx_put`/`cnx_get` 1:1 nach der Quelle, Anbindung an
`Save desktop` und an den Startpfad (G2 der Lueckenliste).

Basis: Branch `desk-rebuild`, HEAD `326ef4c`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, kein `git checkout/restore/stash/reset`.

## Was EmuTOS an dieser Stelle tut (gepinnt: `references/emutos`)

`desk/deskapp.h:120-156` — `WSAVE { x_save, y_save, w_save, h_save,
hsl_save, vsl_save, pth_save[LEN_ZPATH] }`, `CSAVE { cs_sort, cs_view,
cs_sizefit, cs_confcpy, cs_confdel, cs_confovwr, cs_dblclick, cs_timefmt,
cs_datefmt, cs_blitter, cs_cache, cs_appdir, cs_fullpath,
cs_wnode[NUM_WNODES] }`, `NUM_WNODES = 7` (`deskconf.h:30`).

Vier Verkehrsrichtungen:

* **Datei → CSAVE:** `app_start`s `#E`-Arm (`deskapp.c:994-1013`) und
  `#W`-Arm (`:889-913`, `pws->x_save *= gl_wchar` usw.).
* **Desktop → CSAVE:** `cnx_put` (`deskmain.c:1371-1427`).
* **CSAVE → Desktop:** `cnx_get` (`:1430-1501`).
* **CSAVE → Datei:** `app_save` (`deskapp.c:1136-1170` fuer `#E`,
  `:1192-1203` fuer **alle** `NUM_WNODES` `#W`-Zeilen).

`cnx_put`s Steckplatzrechnung, woertlich: erst die unbenutzten zaehlen und
vom **Ende** des Feldes rueckwaerts deren `pth_save[0]` nullen, dann von dort
abwaerts die offenen Fenster in Kettenreihenfolge (vorderstes zuerst)
einsetzen — also landet das **oberste** Fenster im hoechsten benutzten Platz
und **Platz 0 haelt das unterste** (`:1367-1369`). `cnx_get` laeuft
`0..NUM_WNODES`, also unterstes zuerst, und die Stapelordnung kommt zurueck.

`cnx_get`s Gueltigkeitspruefung (`:1466-1483`) haengt **an jedem** Platz, nicht
nur am belegten: `x_save >= g_desk.g_w` → `g_desk.g_w/2`, dasselbe fuer `y`;
`w_save <= 0 || > g_desk.g_w` → `g_desk.g_w`, dasselbe fuer `h`. Danach
`do_xyfix` (`desksupp.c:262-272`). Das ist, was einen bei einer anderen
Aufloesung gespeicherten Stand anstaendig degradieren laesst.

`cnx_put`-Aufrufer: `SAVEITEM` (`:889-893`, `cnx_put(); app_save(TRUE);`), das
Ende von `hndl_msg` bei `change` (`:1352-1353`; `change` setzen
`WM_TOPPED`/`WM_FULLED`/`WM_MOVED`/`WM_SIZED`), und der Epilog (`:2154`,
`cnx_put(); app_save(FALSE);`). `cnx_get`-Aufrufer: genau einer, im Startpfad
zwischen `wind_update(BEG_UPDATE)` und `END_UPDATE` (`:2053-2057`).

`deskwin.c:165-177` — `win_alloc` nimmt das Vorgaberechteck eines **neuen**
Fensters aus `G.g_cnxsave->cs_wnode[G.g_wcnt]`.

## Was getan wurde

**`global.rs`:** `WSave` (voller `WSAVE`-Feldsatz) und `CSave
{ cs_view, cs_sort, cs_sizefit, cs_wnode: [WSave; NUM_WNODES] }`, als
`DeskState::g_cnxsave`. Die uebrigen `CSAVE`-Vorliebenfelder spiegeln
`GLOBES`-Felder, die es nur gibt, weil `inf_pref`/`inf_conf` sie bearbeiten —
diese Dialoge existieren nicht, also gibt es nichts zu lesen und nichts
anzuwenden. Ihre Bits ueberleben trotzdem: `desk::inf` bewahrt die geparsten
`#E`-Bytes der Sitzung und gibt jedes Bit wieder aus, das ihm nicht gehoert.
Das steht im Doc-Kommentar der Struktur.

**`inf.rs`:** `apply` (= `app_start`s Verteilerschleife) legt `#E` und `#W`
jetzt in die `CSave` statt direkt in den lebenden Desktop — EmuTOS' Trennung:
`#M`/`#T`/`#O` und `#Q` gehen in `GLOBES`, `#E`/`#W` in den Kontextspeicher
und von dort erst durch `cnx_get` weiter. `build_from_state` (= `app_save`)
liest `cs_view`/`cs_sort`/`cs_sizefit` und `collect_open_windows` liest
`cs_wnode[]` — **alle sieben** Zeilen, mit `hsl`/`vsl`, wie EmuTOS.
`window_rect(slot)` (= `win_alloc`s Vorgabe) liest ebenfalls `cs_wnode[slot]`.

**`main.rs`:** `cnx_put()`, `cnx_get() -> usize`, `cnx_get_prefs()`;
`open_dir_window_at(spec, title, rect, cvcol, cvrow)` (die Form von
`do_diropen(pw, TRUE, obid, path, pt, TRUE)`), `open_dir_window` ist der
Aufsatz ohne Rechteck. `hndl_msg` fuehrt wie die Quelle ein `change` und ruft
am Ende `cnx_put()`; `top_msg`/`full_msg`/`moved_msg`/`sized_msg` melden es.
`do_save_desktop` ist `cnx_put(); app_save(TRUE)`. `desktop_relinquish` (der
Epilog) beginnt mit `cnx_put()`.

**`mod.rs`:** `run()` ruft `wind_update(BEG_UPDATE); cnx_get();
wind_update(END_UPDATE);` an EmuTOS' Stelle — nach `app_blddesk` und
`wind_set(DESKWH, WF_NEWDESK, …)`, vor `men_update`.

**`inf.rs::set_background`** ruft `cnx_put()` vor seinem `save_desktop()`:
NewTOS sichert einen Hintergrundwechsel sofort, wo EmuTOS ihn dem naechsten
`Save desktop` ueberlaesst, und behaelt so die Invariante, die jeder
`app_save`-Aufruf in EmuTOS hat.

## Vier benannte Abweichungen

1. **`app_save(FALSE)` im Epilog hat kein Gegenstueck.** EmuTOS parkt das
   ganze INF-Abbild im AES-Shell-Puffer (`shel_get`/`shel_put`), damit ein
   neu gestarteter Desktop es ohne Platte findet; NewTOS liest und schreibt
   die Datei direkt. Folge: ein Fensterstand ueberlebt einen **Kaltstart**
   nach `Save desktop`, aber nicht den A106-AES-Neustart fuer sich.
2. **`cnx_get()` laeuft nicht im Desktop-*Prolog*** (`desktop_resume`).
   Dieser Desktop kehrt an Ort und Stelle zurueck, mit seinen Fenstern noch
   angemeldet (`wm_new_keep_desktop`) — ein `cnx_get` wuerde jedes doppelt
   oeffnen. Sie im Epilog zu schliessen und im Prolog neu zu bauen ist die
   Startpfad-Haelfte von G1/G2 und gehoert dorthin.
3. **Kein `obj_get_obid(pth_save[0])`.** EmuTOS loest den Laufwerksbuchstaben
   des gespeicherten Pfades in eine Desktop-Objektnummer auf und gibt sie
   `win_alloc` als `w_obid`; NewTOS' Oeffnenpfad fuehrt durchweg
   `win_alloc(handle, 0)`. EmuTOS' `win_free` bei fehlgeschlagenem
   `do_diropen` hat ebenfalls kein Gegenstueck, weil der NewTOS-Pfad kein
   Fehlersignal hat: ein unerreichbarer Pfad gibt ein leeres Fenster, wie
   jeder andere unerreichbare Pfad auch.
4. **Ein ungenutzter Steckplatz liefert kein Vorgaberechteck.** EmuTOS'
   `win_alloc` nimmt `cs_wnode[g_wcnt]` bedingungslos — das geht, weil sein
   eingebautes `EMUDESK.INF` **immer** vier `#W`-Vorgabezeilen mit leerem
   Pfad mitbringt (`desk_inf_data1`, `deskapp.c:177-180`). NewTOS liefert
   keine, und `cnx_get`s Pruefung blaest einen Nullsteckplatz auf den
   **ganzen** Schreibtisch auf (`w_save <= 0` → `g_desk.g_w`) — bedingungslos
   genommen, oeffnete jedes frische Fenster bildschirmfuellend. **Im ersten
   Lauf genau so gemessen** (`control-desktop.png` vor der Korrektur: Fenster
   (0,19,640,461)), deshalb liefert `window_rect` nur fuer einen Steckplatz
   mit gespeichertem Fenster ein Rechteck; sonst bleibt die gestaffelte
   Vorgabe. **Empfehlung an den Lead:** `desk_inf_data1`s vier `#W`-Zeilen
   uebernehmen; das raeumt die Abweichung weg und aendert das Startbild
   (304×192 bei (16,96) statt 544×400 bei (48,43)) — eine eigene
   Entscheidung, deshalb hier nicht getroffen.

Ausserdem: `Options → Read .INF file` ist ein NewTOS-Menuepunkt, der an Ort
und Stelle neu anwendet, wo EmuTOS den Desktop neu startet
(`READITEM` → `restart_desktop`, `deskmain.c:919-924`). Er nimmt deshalb nur
die Vorliebenhaelfte von `cnx_get` (`cnx_get_prefs`, `:1437-1461`).

## Pruefung

**Hosttest: keiner moeglich, und das ist ein Befund, keine Auslassung.** Die
gesamte Logik liegt in `kernel/`, und `kernel` traegt
`forced-target = "aarch64-unknown-none-softfloat"`; `cargo run -p xtask --
test` faehrt nur `tos-api`, `tos-types`, `xtask`
(`tools/xtask/src/main.rs:582`). Ein `.INF`-Rundlauf als Hosttest setzt
voraus, dass der Textkodierer aus `kernel/src/desk/inf.rs` herauswandert —
ein eigener Auftrag, kein Nebeneffekt dieses Tickets. Der Nachweis ist
deshalb vollstaendig in QEMU gefuehrt.

**QEMU raspi4b, vier Laeufe** (640×480, `-display none`, QMP-Screendump,
frische Bildkopie je Lauf; alle 0 `[EXC]`/`[PANIC]`):

1. **`control`** — Serienabbild (`NEWDESK.INF` = Platzhalter), Produktkernel.
   `control-desktop.png` gegen
   `measurement-2026-09-23-a104-a110/screendumps/desk-after-A-idle.png`:
   **50 px**, alle im Mauszeiger (bbox (319,240)-(640,480); die Referenz hatte
   den Zeiger unten rechts geparkt). Keine Regression am Startbild.
2. **`seeded`** — `NEWDESK.INF` mit `#E 9A …` (Textansicht) und zwei
   `#W`-Zeilen (`C:\SUB\*.*` bei 4/5/1E/0A Zellen, `C:\*.*` bei
   18/0F/28/0C), Produktkernel. `seeded-desktop.png`: **beide** Fenster
   stehen wieder da, mit dem Rahmen gemessen genau bei **(32,80,240,160)** und
   **(192,240,320,192)**, das zuletzt geoeffnete (`C:\`, Platz 1) oben, und
   **beide in Textansicht** — die `#E`-Ansicht ist also durch `cnx_get` bis in
   `do_viewmenu` durchgelaufen.
3. **`save`** — Scratch-Kernel mit dem H02-Injektor
   (`method/desk-desk09.actions`), Serienabbild: Titelzeile fassen und das
   `C:\`-Fenster von (48,43) nach **(144,141)** ziehen (`save-B-moved.png`),
   `Options` aufklappen (`save-C-options.png`), `Save desktop...` klicken
   (`save-D-saved.png`). Danach das Abbild gemountet und gelesen
   (`method/NEWDESK.INF.after-save`):

   ```
   #E 1A E0 00 00 60
   #W 00 00 12 08 44 19 00 C:\*.*@
   #W 00 00 00 01 50 1C 00 @      (×6)
   ```

   `12 08 44 19` = 18/8/68/25 Zellen = **(144, 128, 544, 400)** px; die sechs
   leeren Zeilen sind genau `cnx_get`s Pruefung, zurueckgeschrieben
   (`00 01 50 1C` = x 0, y 16 — `do_xyfix` hob 0 auf `g_desk.y` = 19 —,
   w 640, h 448 ← 461).
4. **`restart`** — **dasselbe** Abbild, **Produktkernel**, Kaltstart.
   `restart-desktop.png`: das Fenster steht wieder bei **(144, 128)**, Groesse
   unveraendert. Die 13 px gegen (144,141) sind die Zeichenzellen-Speicherung
   der `#W`-Zeile (`app_save`: `y_save/gl_hchar`, `app_start`: `* gl_hchar`) —
   TOS-treu, kein Fehler.

## Bauen

Frisch je Board (`cargo clean -p kernel -p tos-api` davor):
`--board qemu-virt` und `--board rpi4`, beide rc=0, **tos-api 64 / kernel 94**
Warnungen (Basis 64/94, Delta 0). `cargo clippy`: tos-api **80**, kernel
**125** auf beiden Boards (Basis 80/125), **0** Treffer in
`kernel/src/desk/`. `cargo run -p xtask -- test`: **929 passed / 0 failed /
1 ignored** (Basis 926/0/1 + 3 aus DESK08).

## Offen

* **Pi-4-Sichtabnahme** — HW-Checkliste Punkt **35**, Teil B.
* Die vier benannten Abweichungen oben; besonders Nummer 4 braucht eine
  Lead-Entscheidung.
* Ein Hosttest fuer den `.INF`-Rundlauf setzt voraus, dass der Textkodierer
  aus `kernel/` herauswandert.
