# DESK48 — STATUS: DONE (2026-10-17, QEMU) — kein `redraw_desktop()` mehr nach Dateioperationen

**Auftrag:** Die Stellen, an denen der Desktop `redraw_desktop()` ruft und EmuTOS nicht, auf EmuTOS' eigenen Ersatz
umstellen — Ersatz vor Entfernung.

## Aenderung (`f2f127a7`)

Erst die zwei Werkzeuge, die EmuTOS statt eines Vollbilds benutzt:

| | EmuTOS | NewTOS |
|---|---|---|
| Zeichenhaelfte von `desk_clear` | `act_allchg(wh, root, 0, &gl_rfull, &c, FALSE)` → `do_wredraw` ueber die abgewaehlten Objekte, `desksupp.c:122-153`, `deskact.c:480-555` | `main::desk_clear` → `DeskState::act_allchg_clear` (`act.rs`) → `do_wredraw` |
| `desk_clear_all` | Schleife ueber `G.g_wfirst`, dann `desk_clear(DESKWH)`, `desksupp.c:160-169` | dieselbe Schleife; `win_shwall` + `do_wredraw(DESKWH, …)` entfallen |
| `fun_rebld(path)` | jedes Fenster mit diesem Pfad, `deskfun.c:173-191` | `dir::fun_rebld` (Lauf in Z-Ordnung, Sanduhr), jetzt an allen EmuTOS-Aufrufstellen |

Dann die Stellen:

| Stelle | EmuTOS | vorher | jetzt |
|---|---|---|---|
| 1 `fun_desk2desk` | `fun_file2any` → `fun_file2desk` → `fun_rebld(pathname)`, `desk_clear(DESKWH)`, `deskfun.c:1085-1091,1191` | `redraw_desktop()`, Fenster nicht neu eingelesen | `fun_rebld` des Ziels, `desk_clear(DESKWH)` |
| 2 `desk_clear_all` | s. o. | eigene Zeichnung aller Fenster | erledigt mit der Zeichenhaelfte (Review hatte recht: kein `redraw_desktop` mehr, aber ein Ersatz an falscher Stelle) |
| 3 `fun_selectall` | `win_sinfo`; `fun_msg(WM_REDRAW, …)` ueber `WF_WXYWH`, `deskfun.c:627-630` | `redraw_desktop()` | `do_wredraw(cwin, WF_WXYWH)` |
| 4 `fun_rebld`-Semantik | `fun_rebld(p_spec)` / `fun_rebld(destpath)`, `deskfun.c:991-998,1129-1130,1253-1254,1421-1422`, und `fun_file2desk` `:1085-1090` | `rebuild_window(handle)`, ein Fenster | `fun_rebld(pfad)`, alle Fenster; neu auch der Ordner-Fall und der Zielpfad in `fun_file2desk` |
| 5 `fun_del` Desktopfall (Review, ungeprueft) | `refresh_drive(path[0])` je Laufwerk, `desk_clear(DESKWH)`, `deskfun.c:1404-1413`; `refresh_drive` `desksupp.c:1696-1715` | `redraw_desktop()` | `main::refresh_drive` (Fenster des Laufwerks zur Wurzel, `fun_close(CLOSE_TO_ROOT)` + `refresh_window`), `desk_clear(DESKWH)` |

Nebenwirkung, gewollt: jeder bisherige Aufrufer von `desk_clear` ohne eigenes Neuzeichnen (`WM_TOPPED`, nach jedem Ziehen,
Pfeiltasten) liess die Markierung stehen; jetzt verschwindet sie wie bei EmuTOS.

## Belege (`measurements/measurement-2026-10-17-desk48/RUNINFO.TXT`)

QEMU raspi4b, IKBD-Injektor, AES-Spur je Aktion, Vergleich gegen HEAD `d35da475`:

* Fensterwechsel: HEAD laesst drei Zeilen invertiert stehen (`sel-red-d-topped.png`), neu sauber, ein `objc_draw` ueber genau die drei Zeilen.
* Klick auf den Desktop (`desk_clear_all`): HEAD drei ganze Arbeitsflaechen, neu nur die eine Zeile; Pixel gleich.
* *Select all*: HEAD `form_dial(FMD_FINISH)` ueber den Bildschirm + `menu_bar` + 6 `objc_draw`; neu ein `objc_draw` ueber das Fenster; Pixel gleich.
* Zwei Fenster auf `C:\`, Kopie aus `C:\SUB\` hinein: beide zeigen die neue Datei (HEAD nur eines); Kopie auf einen Ordner frischt das Fenster `C:\SUB\` auf (HEAD nicht).
* Verknuepfung auf Laufwerkssymbol, Loeschen vom Desktop: kein Vollbild mehr, Fenster neu eingelesen, Symbol abgewaehlt (HEAD: veraltete Fenster, Symbol bleibt invertiert); Kartenstand danach in beiden gleich.
* Grundfunktionen (Laufwerk oeffnen, Ordner oeffnen, Auswahl, Kopierdialoge) pixelgleich zu HEAD.
* Selbsttest neu `[desk-act] act_allchg(FALSE)` 4/0; sonst wie HEAD (94/1, kartenabhaengiger FAIL).
* Basislinien: Hosttests 1380/0/1, SDK 20/0/0, Bau 64/94 beide Boards, Clippy 76/125, Programme-Clippy 0, Driftprobe 0.

## Offen

* Abnahme auf echter Pi 4.
* **Entscheidung fuer den Anwender:** EmuTOS ruft `desk_clear(DESKWH)` in `fun_file2any`, also innerhalb der
  `win_isel`-Schleife von `fun_desk2desk`/`fun_desk2win`; danach ist nichts mehr ausgewaehlt, und EmuTOS verarbeitet von
  mehreren gezogenen Desktopsymbolen nur das **erste**. NewTOS verarbeitet weiter alle und raeumt einmal nach der Schleife.
  1:1 hiesse: nur das erste.
* Bekannt, nicht angefasst: `fun_rebld`/`fun_mark_for_rebld` vergleichen das Verzeichnis, nicht das ganze `p_spec` (NewTOS haelt die
  Maske in `p_mask`) — ein maskiertes Fenster auf demselben Verzeichnis wird mit aufgefrischt; `fun_rebld_marked` laeuft noch in
  Slot- statt Z-Ordnung; `do_new_folder` ruft `rebuild_window(cwin)` statt `fun_rebld`; `search_current_window` und
  `set_window_mask` rufen weiter `redraw_desktop()`; `act_bsclick`/Rubber-Band zeichnen noch das ganze Fenster statt der
  geaenderten Objekte.
* Vorbestehend, in HEAD gleich: nach dem Toppen eines Fensters ueber einem frisch geoeffneten bleibt dessen freigelegter Teil
  ungezeichnet (`open-b-folderopen.png`, AES-Luecke #123).
