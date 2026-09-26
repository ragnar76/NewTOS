# U1 — Der gemessene Desktop-Boot gegen `desk/deskmain.c:1950-2075`

Quelle links: `references/emutos` @ `1a7484ab` (`VERSION_1_4-71-g1a7484ab`).
Quelle rechts: `trace/desktop-section-annotated.txt`, erzeugt am 2026-09-26 aus
einem headless-Lauf in QEMU `raspi4b` (640×480, Dev-Bau, SD-Image aus
`tools/make-sd-image.sh`). Die Roh-Aufzeichnung ist
`trace/QEDTRACE-widened-gate.log`; der Desktop-Abschnitt beginnt beim dritten
`appl_init` (die beiden davor sind die zwei Accessories aus `C:\ACC\`).

Aufgezeichnet wird jeder `svc` — GEM-Trap (`svc #2`, AES + VDI) und GEMDOS-Trap
(`svc #1`). Kein `Supexec`, kein Direktaufruf: was hier **nicht** steht, ist auch
nicht durch den Trap gegangen.

## Die Gegenüberstellung

| # | EmuTOS `deskmain.c` | Trap | Unser Lauf | Urteil |
|---|---|---|---|---|
| 1 | `:1950` `G.g_stdrv = dos_gdrv();` | GEMDOS 0x19 | — | **fehlt** (D1) |
| 2 | `:1953` `gl_apid = appl_init();` | AES 10 | `AES op=10 appl_init -> 0` | **gleich** |
| 3 | `:1956` `gl_handle = graf_handle(&gl_wchar,&gl_hchar,&gl_wbox,&gl_hbox);` | AES 77 | — | **fehlt** (D2 = AES-Lücke **G4**) |
| 4 | `:1959` `wind_get_grect(DESKWH, WF_WXYWH, &G.g_desk);` | AES 104 | `AES op=104 wind_get -> (0,19,640,461)` | **gleich** |
| 5 | `:1962-1964` `wind_update(BEG); desk_busy_on(); wind_update(END);` | AES 107, 78, 107 | — | **fehlt** (D3) |
| 6 | `:1967` `detect_features();` | (Cookies) | — | — |
| 7 | `:1970` `G.g_cnxsave = dos_alloc_anyram(sizeof(CSAVE));` | GEMDOS 0x44 | — | **abweichend, unkritisch** (D4) |
| 8 | `:1979/:1984/:1992/:1996` `desk_rs_init` / `desk_xlate_fix` / `align_title` / `app_tran` | — | — | kein Trap, beidseitig |
| 9 | `:2018` `app_start();` → `shel_get(buf, SIZE_SHELBUF)` (`deskapp.c:832`), erst danach die Datei | AES 122, GEMDOS 0x3d/0x3f/0x3e | `Fopen "C:\NEWDESK.INF"`, `Fread`, `Fclose` — **kein `shel_get`** | **fehlt** (D5 = die Lücke, die **U2** schliesst) |
| 10 | `:2028` `win_start()` → `win_view()` → `wind_calc_grect(1, WINDOW_STYLE, &G.g_desk, &t)` (`deskwin.c:95`) | AES 108 | `AES op=108 wind_calc wtype=1 kind=0x0fff in=(0,19,640,461) out=(1,61,616,396)` | **gleich**, aber **vor** #9 statt danach (D6) |
| 11 | `:2028` `win_start()` → `obj_init()` (`deskwin.c:117`) → `objc_add(G.g_screen, ROOT, DROOT+ii)` × (NUM_WNODES+1) (`deskobj.c:142-146`) | AES 40 × 8 | `AES op=40 objc_add [x8]` | **gleich**, aber **vor** #9 statt danach (D6) |
| 12 | `:2035` `desk_verify(DESKWH, FALSE);` | — | — | kein Trap, beidseitig |
| 13 | `:2039-2042` `menu_icheck(menutree, ICONITEM+g_iview/NAMEITEM+g_isort/FITITEM, …)` | AES 31 × 3 | in den 8 `menu_icheck` von #17 enthalten | **verschoben** (D7) |
| 14 | `:2046` `app_blddesk();` → `obj_ialloc` → `objc_add(G.g_screen, DROOT, objnum)` (`deskobj.c:232`) | AES 40 × n | `AES op=40 objc_add [x3]` (C:, Papierkorb, Drucker) | **gleich** |
| 15 | `:2049` `wind_set(DESKWH, WF_NEWDESK, G.g_screen, 1, 0);` | AES 105 | `AES op=105 wind_set -> 1` | **gleich** |
| 16 | `:2054-2056` `wind_update(BEG); cnx_get(); wind_update(END);` — `cnx_get` ruft `evnt_dclick(G.g_cdclkpref, TRUE)` (`deskmain.c:1460`) | AES 107, 26, 107 | `wind_update`, `evnt_dclick -> 2`, `wind_update` | **gleich** |
| — | *(zwischen #15 und #16 hat EmuTOS nichts)* | — | `AES op=30 menu_bar -> 1` | **zusätzlich / zu früh** (D8) |
| 17 | `:2069` `men_update();` | AES 32 × N_ITEMS, AES 31 × 8 | `menu_ienable [x31]`, `menu_icheck [x8]` | **gleich** (`menu.rs:1081-1108`) |
| 18 | `:2072-2075` `wind_update(BEG); menu_bar(menutree, 1); desk_busy_off(); wind_update(END);` | AES 107, 30, 78, 107 | `graf_mouse`, `form_dial`, `menu_bar`, `graf_mouse`, `graf_mouse` — **ohne** `wind_update`-Klammer | **teils gleich, teils zusätzlich** (D9, D10) |
| 19 | `:2077 ff.` `evnt_multi(flags, …)` | AES 25 | (der Trace wird genau hier geschrieben) | **gleich** |
| — | *(EmuTOS hat das nicht)* | GEMDOS 0x19 + 0x0e | `Dgetdrv`, `Dsetdrv` | **zusätzlich** (D11) |
| — | *(EmuTOS liest `_IDT` in `do_wredraw`, `desksupp.c:219`)* | GEMDOS 0x154 | `Ssystem(S_GETCOOKIE, _IDT) -> 4399` direkt nach #11 | **verschoben, im Baum begründet** (D12) |

## Die Aufrufkette, die der Auftrag nennt

`appl_init → graf_handle → wind_get(DESKWH, WF_WXYWH) → … → menu_bar(tree, 1)`

ist damit **messbar** — und sie stimmt an Anfang und Ende, mit **einer Lücke
genau an der zweiten Stelle**: `graf_handle` fehlt (D2/G4). Das ist kein
Nebenbefund, sondern exakt die Umgehung, die die Driftprobe als Gruppe
`tos_api::vdi::font` (8 Codezeilen, ≈ 200 Nutzungen) zählt und die **U7**
auflöst.

## Was diese Messung nicht zeigt

* **Nur der Boot.** Interaktion (Klick, Menü, Fensteröffnen) ist nicht
  aufgezeichnet — der Lauf ist headless, `evnt_multi` kehrt nie zurück.
* **Nur ein Bildschirmformat.** 640×480, `raspi4b`. `gl_wchar/gl_hchar` sind
  auf beiden Maschinen 8/16 (`docs/analysis/aes-r1-findings.md` §0.2), die
  fehlende `graf_handle`-Abfrage ist also heute folgenlos — aber sie fehlt.
* **Kein `NEWDESK.INF` mit Inhalt.** Das Testimage trägt einen Platzhaltertext,
  also kein `#W`, also kein Fenster aus `cnx_get`. Ein Lauf mit gespeicherten
  Fenstern hätte in #16 zusätzlich `wind_create`/`wind_open` gezeigt.
