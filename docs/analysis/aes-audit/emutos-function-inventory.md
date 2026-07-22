# EmuTOS-AES Funktions-Inventar — Vollständigkeits-Check (EmuTOS-seitig)

> Deterministisch via `ctags -x` aus references/emutos/aes/*.c extrahiert.
> Ziel: JEDE EmuTOS-AES-Funktion (public UND internal static) auflisten und
> fragen "haben wir ein Gegenstück?". Absenz ist der Default-Befund.
> Status je Zeile vom Researcher zu füllen: HAS (Gegenstück, ggf. umbenannt),
> NO-COUNTERPART (Funktionalität fehlt komplett), N/A (asm/build-glue/m68k).

## gemaplib.c  (6 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `ap_exit` | 251 | `void ap_exit(void)` | appl.rs:276 `appl_exit` | HAS |
| `ap_find` | 95 | `WORD ap_find(char *pname)` | appl.rs:285 `appl_find` | HAS |
| `ap_init` | 54 | `WORD ap_init(void)` | appl.rs:244 `appl_init` (via `pd::open`, sets `AP_OPEN`) | HAS |
| `ap_rdwr` | 69 | `WORD ap_rdwr(WORD code, AESPD *p, WORD length, WORD *pbuff)` | appl.rs:317/351 `appl_write`/`appl_read` (MU_SDMSG/MU_RDMSG inlined directly) | HAS |
| `ap_tplay` | 119 | `void ap_tplay(const EVNTREC *pbuff,WORD length,WORD scale)` | appl.rs:395 `appl_tplay` — Opcode-Stub: kein Event-Tape-Playback, ehrliches No-op | HAS |
| `ap_trecd` | 189 | `WORD ap_trecd(EVNTREC *pbuff,WORD length)` | appl.rs:387 `appl_trecord` — Opcode-Stub: keine Aufzeichnung, ehrliches No-op | HAS |

## gemasync.c  (8 Symbole)

> Ganzes File: die EVB-Warteschlangen-Primitiven des kooperativen AESPD-Schedulers
> (parken/aufwecken mehrerer gleichzeitiger AES-Prozesse). NewTOS ist synchron
> pro Trap (ein AES-Aufruf = ein Poll-Loop im aufrufenden Kontext, `event.rs`);
> es gibt keine Datenstruktur, die einen anderen wartenden Prozess aufweckt,
> abbricht oder in eine Zombie-Liste verschiebt.

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `acancel` | 217 | `EVSPEC acancel(EVSPEC m)` | — | NO-COUNTERPART |
| `apret` | 182 | `UWORD apret(EVSPEC mask)` | — | NO-COUNTERPART |
| `azombie` | 64 | `void azombie(EVB *e, UWORD ret)` | — | NO-COUNTERPART |
| `evinsert` | 81 | `void evinsert(EVB *e, EVB **root)` | — | NO-COUNTERPART |
| `iasync` | 130 | `EVSPEC iasync(WORD afunc, LONG aparm)` | — | NO-COUNTERPART |
| `mwait` | 117 | `EVSPEC mwait(EVSPEC mask)` | — | NO-COUNTERPART |
| `signal` | 35 | `static void signal(EVB *e)` | — | NO-COUNTERPART |
| `takeoff` | 96 | `static void takeoff(EVB *p)` | — | NO-COUNTERPART |

## gemctrl.c  (9 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `ct_chgown` | 392 | `void ct_chgown(AESPD *mpd, GRECT *pr)` | — Maus-/Ctrl-Rect-Ownership-Arbitrierung zwischen konkurrierenden AES-Prozessen; kein `gl_mowner`/`gl_ctrl`-Äquivalent | NO-COUNTERPART |
| `ct_mouse` | 400 | `void ct_mouse(WORD grabit)` | — dasselbe Ownership-Konzept (grab/release der Maus für den Control-Manager) | NO-COUNTERPART |
| `ct_msgup` | 81 | `static void ct_msgup(WORD message, AESPD *owner, WORD wh, WORD m1, WORD m2, WORD m3, WORD m4)` | window.rs:1807 `intercept` (msg[]+`wait_release()`-Muster inline statt gemeinsamem Helfer) | HAS |
| `ctlmgr` | 427 | `void ctlmgr(void)` | event.rs:58 `menu_intercept` + window.rs:1807 `intercept`, beide aus `evnt_multi` — kein separater Prozess, direkt in den Poll-Loop gefaltet | HAS |
| `handle_arrow_msg` | 111 | `static void handle_arrow_msg(WORD w_handle, WORD gadget)` | window.rs:1865-1874 (`intercept`, Gadget::Up/Dn/Lf/Rt) — einmaliges `wait_release()` statt kontinuierlichem Nachfeuern bei gehaltenem Button | HAS |
| `hctl_msg` | 380 | `static void hctl_msg(WORD *msgbuf)` | — WM_REDRAW-Abfangen+Weiterleiten an `w_redraw_desktop` (CONF_WITH_3D_OBJECTS); kein Message-Inspect-Schritt, Redraw läuft direkt | NO-COUNTERPART |
| `hctl_rect` | 318 | `static void hctl_rect(void)` | event.rs:58 `menu_intercept` (faltet `mn_do`+`do_chg`+`ct_msgup(MN_SELECTED)`); AC_OPEN/Accessory-Zweig (title==THEDESK) fehlt (keine Accessories) | HAS |
| `hctl_window` | 158 | `static void hctl_window(WORD w_handle, WORD mx, WORD my)` | window.rs:1807 `intercept` + `gadget_at`/`watchbox`/`drag_move`/`drag_size` | HAS |
| `perform_untop` | 146 | `static void perform_untop(WORD wh)` | — `WM_UNTOPPED`-Benachrichtigung des vorherigen Top-Fensters fehlt komplett (grep: kein `WM_UNTOPPED` im ganzen aes/); nur die "neues Fenster nach oben"-Hälfte existiert (`top_window`) | NO-COUNTERPART |

## gemdisp.c  (7 Symbole)

> Ganzes File: der kooperative m68k-Prozess-Scheduler (Context-Switch via
> `switchto()`, Ready-/Wait-/Fork-Listen). NewTOS hat keinen AES-Scheduler —
> ein Trap-Aufruf läuft synchron im Aufrufer-Kontext.

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `chkkbd` | 158 | `void chkkbd(void)` | — | NO-COUNTERPART |
| `disp` | 221 | `void disp(void)` | — | NO-COUNTERPART |
| `disp_act` | 74 | `static void disp_act(AESPD *p)` | — | NO-COUNTERPART |
| `forker` | 101 | `void forker(void)` | — | NO-COUNTERPART |
| `forkq` | 52 | `WORD forkq(FCODE fcode, LONG fdata)` | — | NO-COUNTERPART |
| `mwait_act` | 82 | `static void mwait_act(AESPD *p)` | — | NO-COUNTERPART |
| `schedule` | 181 | `static void schedule(void)` | — | NO-COUNTERPART |

## gemevlib.c  (7 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `ev_block` | 64 | `WORD ev_block(WORD code, LONG lvalue)` | event.rs — pro `evnt_*` inline als eigener Poll-Loop (kein gemeinsamer Blocking-Primitive nötig) | HAS |
| `ev_button` | 84 | `UWORD ev_button(WORD bflgclks, UWORD bmask, UWORD bstate, WORD rets[])` | event.rs:123 `evnt_button` | HAS |
| `ev_dclick` | 300 | `WORD ev_dclick(WORD rate, WORD setit)` | event.rs:234 `evnt_dclick` | HAS |
| `ev_mesag` | 100 | `void ev_mesag(WORD *mebuff)` | event.rs:399 `evnt_mesag` | HAS |
| `ev_mouse` | 123 | `void ev_mouse(MOBLK *pmo, WORD rets[])` | event.rs:211 `evnt_mouse` | HAS |
| `ev_rets` | 42 | `static void ev_rets(WORD rets[])` | event.rs:95 `fill_rets` | HAS |
| `ev_timer` | 134 | `void ev_timer(LONG count)` | event.rs:438 `evnt_timer` (+ `wait_ms`) | HAS |

## gemflag.c  (4 Symbole)

> Ganzes File: Semaphor-/Tick-Broadcast-Primitiven für den kooperativen
> Scheduler (Mutex zwischen AESPDs, Tick-Wecken von Timer-Wartern). NewTOS
> misst Zeit direkt über `host().monotonic_millis()`; kein AESPD-Broadcast nötig.

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `amutex` | 89 | `void amutex(EVB *e, LONG ls)` | — | NO-COUNTERPART |
| `tak_flag` | 71 | `WORD tak_flag(SPB *sy)` | — | NO-COUNTERPART |
| `tchange` | 32 | `void tchange(LONG c) /* c=number of ticks that have gone by */` | — | NO-COUNTERPART |
| `unsync` | 105 | `void unsync(SPB *sy)` | — | NO-COUNTERPART |

## gemfmalt.c  (7 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `endstring` | 74 | `#define endstring(a) ( ((a)==']') \|\| ((a)=='\0') )` | form.rs:427-446 `group()` (inline im Scan) | HAS |
| `endsubstring` | 75 | `#define endsubstring(a) ( ((a)=='\|') \|\| ((a)==']') \|\| ((a)=='\0') )` | form.rs:456-504 `split_copy()` (inline: `c=='\|' \|\| c==']'`) | HAS |
| `fm_alert` | 319 | `WORD fm_alert(WORD defbut, char *palstr)` | form.rs:114 `form_alert` / `do_alert` | HAS |
| `fm_build` | 188 | `static void fm_build(OBJECT *tree, WORD iconnum, WORD nummsg, WORD mlenmsg,` | form.rs:203-239 (inline in `do_alert`, Tree-Aufbau) | HAS |
| `fm_parse` | 157 | `static void fm_parse(OBJECT *tree, char *palstr, WORD *picnum, WORD *pnummsg,` | form.rs:427 `group()` | HAS |
| `fm_strbrk` | 78 | `static char *fm_strbrk(OBJECT *start,WORD maxnum,WORD maxlen,char *alert,` | form.rs:456 `split_copy()` | HAS |
| `isduplicate` | 76 | `#define isduplicate(a,b) ( (a!='\0') && ((a)==(b)) )` | form.rs:456-504 `split_copy()` (inline: doppeltes Trennzeichen = Literal) | HAS |

## gemfmlib.c  (10 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `eralert` | 416 | `WORD eralert(WORD n, WORD d)` | — kritischer-GEMDOS/BIOS-Fehler-Alert mit Laufwerksbuchstaben-Substitution + `ml_alrt`/`ml_pwlv`-Tabellen; `form_error` (form.rs:682) deckt nur reine Fehlernummer-Alerts über `i18n::err_string` ab, ohne %d-Laufwerksbuchstaben und ohne den kritischen-Fehler-Einstiegspunkt | NO-COUNTERPART |
| `find_obj` | 114 | `static WORD find_obj(OBJECT *tree, WORD start_obj, WORD which)` | form.rs:852 `find_obj` (nutzt `find_flag`/`next_editable`/`prev_editable`) | HAS |
| `fm_button` | 201 | `WORD fm_button(OBJECT *tree, WORD new_obj, WORD clks, WORD *pnew_obj)` | form.rs:882 `form_button` | HAS |
| `fm_dial` | 360 | `WORD fm_dial(WORD fmd_type, GRECT *pi, GRECT *pt)` | form.rs:772 `form_dial` | HAS |
| `fm_do` | 278 | `WORD fm_do(OBJECT *tree, WORD start_fld)` | form.rs:513/524 `form_do`/`run_form` | HAS |
| `fm_error` | 436 | `WORD fm_error(WORD n) /* n = dos error number */` | form.rs:682 `form_error` | HAS |
| `fm_inifld` | 153 | `static WORD fm_inifld(OBJECT *tree, WORD start_fld)` | form.rs:532 (inline in `run_form`: `find_flag(tree, EDITABLE)`) | HAS |
| `fm_keybd` | 163 | `WORD fm_keybd(OBJECT *tree, WORD obj, WORD *pchar, WORD *pnew_obj)` | form.rs:924 `form_keybd` | HAS |
| `fm_own` | 82 | `void fm_own(WORD beg_ownit)` | — Bildschirm-/Maus-Ownership-Grab von der Control-Manager-Arbitrierung (`ct_chgown`, bereits NO-COUNTERPART); da NewTOS kein zweites AES-Programm gleichzeitig laufen lässt, gibt es nichts, wovon Ownership übernommen werden müsste | N/A |
| `fm_show` | 389 | `WORD fm_show(WORD string, WORD *pwd, WORD level)` | — generischer Resource-String-Nummer+%d-Format-Alert-Builder (`rs_str`+`sprintf`); `form_error` baut Alerts stattdessen direkt aus `i18n::err_string`, kein `fm_show`-Äquivalent für beliebige RSC-String-Alerts | NO-COUNTERPART |

## gemfslib.c  (17 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `centre_title` | 179 | `static void centre_title(OBJECT *tree,WORD objnum)` | — `fsel_exinput`s Titel-Parameter wird ignoriert ("Batch 2" laut fsel.rs:241-242 Doku), kein Titel wird gezeichnet | NO-COUNTERPART |
| `fs_1scroll` | 345 | `static WORD fs_1scroll(WORD curr, WORD count, WORD touchob)` | fsel.rs:393 `scroll()` | HAS |
| `fs_active` | 268 | `static WORD fs_active(char *ppath, char *pspec, WORD *pcount)` | fsel.rs:321 `relist()` (ruft `bdos::fs::find::list_dir`) | HAS |
| `fs_add` | 249 | `static LONG fs_add(WORD thefile, LONG fs_index)` | — sortiertes Einfügen (alphabetisch, Case-insensitiv) in die Dateiliste; `list_dir` (bdos/fs/find.rs:210) liefert unsortierte rohe FAT-Reihenfolge, keine Sortierung | NO-COUNTERPART |
| `fs_back` | 199 | `static char *fs_back(char *pstr, char *pend)` | fsel.rs:415-477 (inline in `nav_into_folder`: `mask_start`-Berechnung) | HAS |
| `fs_comp` | 243 | `static WORD fs_comp(char *name1, char *name2)` | — Case-insensitiver 8.3-Namensvergleich, nur von `fs_add`s (fehlender) Sortierung genutzt | NO-COUNTERPART |
| `fs_format` | 363 | `static void fs_format(OBJECT *tree, WORD currtop, WORD count)` | fsel.rs:347 `refill()` | HAS |
| `fs_input` | 609 | `WORD fs_input(char *pipath, char *pisel, WORD *pbutton, char *pilabel)` | fsel.rs:237/247 `fsel_input`/`run()` | HAS |
| `fs_newdir` | 487 | `static WORD fs_newdir(char *fpath, char *pspec, OBJECT *tree, WORD *pcount)` | fsel.rs:574-582 (inline `apply_path`-Closure in `modal()`) | HAS |
| `fs_nscroll` | 424 | `static WORD fs_nscroll(OBJECT *tree, WORD *psel, WORD curr, WORD count,` | — Scrollbar-Thumb-Drag / Track-Klick-Paging; `modal()`s Hit-Test-Liste (fsel.rs:584-684) prüft `FUPAROW`/`FDNAROW` aber nicht `FSVSLID`/`FSVELEV` | NO-COUNTERPART |
| `fs_pspec` | 229 | `static char *fs_pspec(char *pstr, char *pend)` | fsel.rs:415-477 (inline in `nav_into_folder`) | HAS |
| `fs_sel` | 413 | `static void fs_sel(WORD sel, WORD state)` | fsel.rs:651-654 (inline in `modal()`: `set_state_bit_t`-Aufrufe) | HAS |
| `fs_start` | 59 | `void fs_start(void)` | fsel.rs:112 `set_3d_flags` (explizit als "EmuTOS `fs_start`" zitiert) | HAS |
| `get_drive` | 590 | `static WORD get_drive(char *path)` | fsel.rs:284 (inline in `run()`: `current_drive()`-Default) | HAS |
| `path_changed` | 572 | `static WORD path_changed(char *path)` | fsel.rs:483 `path_differs` | HAS |
| `select_drive` | 540 | `static void select_drive(OBJECT *treeaddr, WORD drive, WORD redraw)` | fsel.rs:666-680 (inline in `modal()`: Drive-Klick-Handler) | HAS |
| `set_mask` | 524 | `static void set_mask(char *mask, char *path)` | fsel.rs:415-477 (inline in `nav_into_folder`, Mask-Extraktion) | HAS |

## gemgraf.c  (30 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `bb_fill` | 434 | `void bb_fill(WORD mode, WORD fis, WORD patt, WORD hx, WORD hy, WORD hw, WORD hh)` | object.rs:1544 `gr_rect` (IP_PATTERNS-Array + `fill_rect`) | HAS |
| `bb_screen` | 357 | `void bb_screen(WORD scsx, WORD scsy, WORD scdx, WORD scdy,` | window.rs:1684 (inline `backend.copy_rect`, z.B. `drag_move`) | HAS |
| `ch_height` | 468 | `static UWORD ch_height(WORD fn)` | `crate::vdi::font::CELL_H` — feste Konstante (ein System-Font statt IBM/SMALL-Unterscheidung) | HAS |
| `ch_width` | 456 | `static UWORD ch_width(WORD fn)` | `crate::vdi::font::CELL_W` | HAS |
| `dither_mask` | 680 | `static WORD *dither_mask(WORD *pdata, WORD w, WORD h)` | — Colour-Icon-Dithering; kein CICON-Support im ganzen aes/ (grep bestätigt) | NO-COUNTERPART |
| `gr_box` | 813 | `void gr_box(WORD x, WORD y, WORD w, WORD h, WORD th)` | object.rs:2038-2080 `draw_border`/`draw_signed_border` (explizit "EmuTOS `gr_box`" zitiert) | HAS |
| `gr_colourblit` | 642 | `static void gr_colourblit(WORD *pdata, GRECT *pi, WORD num_planes)` | — Colour-Icon-Blit; kein CICON-Support | NO-COUNTERPART |
| `gr_crack` | 612 | `void gr_crack(UWORD color, WORD *pbc, WORD *ptc, WORD *pip, WORD *pic, WORD *pmd)` | object.rs:1514 `gr_crack` | HAS |
| `gr_gblt` | 631 | `static void gr_gblt(WORD *pimage, GRECT *pi, WORD col1, WORD col2)` | object.rs:2227 `blit_plane` | HAS |
| `gr_gicon` | 710 | `void gr_gicon(WORD state, ICONBLK *ib, CICON *cicon)` | object.rs:2158 `draw_icon` (nur Mono-ICONBLK, kein `cicon`-Zweig) | HAS |
| `gr_gtext` | 595 | `void gr_gtext(WORD just, WORD font, char *ptext, GRECT *pt)` | object.rs:2084 `draw_text` | HAS |
| `gr_inside` | 535 | `void gr_inside(GRECT *pt, WORD th)` | object.rs:2064 (inline in `draw_signed_border`, explizit zitiert) | HAS |
| `gr_just` | 567 | `WORD gr_just(WORD just, WORD font, char *ptext, WORD w, WORD h, GRECT *pt)` | object.rs:2097-2101 (inline in `draw_text`: `tx`-Justierung) | HAS |
| `gr_rect` | 547 | `void gr_rect(UWORD icolor, UWORD ipattern, GRECT *pt)` | object.rs:1544 `gr_rect` | HAS |
| `gsx_attr` | 201 | `void gsx_attr(UWORD text, UWORD mode, UWORD color)` | — VDI-Attribut-Cache-Transport; Farbe/Mode werden direkt als Backend-Aufrufparameter übergeben | N/A |
| `gsx_blt` | 328 | `void gsx_blt(void *saddr, WORD sx, WORD sy,` | object.rs/window.rs (inline `backend.blit_mono`/`copy_rect`, gemeinsame Engine hinter `gr_gblt`/`bb_screen`) | HAS |
| `gsx_box` | 264 | `static void gsx_box(GRECT *pt)` | window.rs:764 `rect_outline` | HAS |
| `gsx_bxpts` | 246 | `static void gsx_bxpts(GRECT *pt)` | window.rs (inline Eckpunkt-Mathematik in `draw_border`/`rect_outline`) | HAS |
| `gsx_chkclip` | 121 | `WORD gsx_chkclip(GRECT *pt)` | window.rs:509 `intersect_grect` / object.rs `draw_obj_clip` (ClipRect-Intersection statt globalem Zustand) | HAS |
| `gsx_cline` | 188 | `void gsx_cline(UWORD x1, UWORD y1, UWORD x2, UWORD y2)` | object.rs (inline `backend.draw_line`, z.B. `draw_3d_outline`) | HAS |
| `gsx_gclip` | 111 | `void gsx_gclip(GRECT *pt)` | — kein impliziter VDI-Clip-Zustand zum Abfragen; ClipRect wird überall explizit als Parameter durchgereicht | N/A |
| `gsx_pline` | 170 | `void gsx_pline(WORD offx, WORD offy, WORD cnt, const Point *pts)` | object.rs (inline mehrsegmentige `backend.draw_line`-Schleifen, z.B. `draw_3d_outline`) | HAS |
| `gsx_sclip` | 91 | `void gsx_sclip(const GRECT *pt)` | object.rs:1389 (inline: ClipRect-Parameter-Threading; Kommentar zitiert `gsx_sclip` direkt) | HAS |
| `gsx_start` | 383 | `void gsx_start(void)` | — VDI-Workstation-Open-Initialisierung; ersetzt durch Boot-Zeit-Backend-Init mit festen Konstanten | N/A |
| `gsx_tblt` | 500 | `void gsx_tblt(WORD tb_f, WORD x, WORD y, WORD tb_nc)` | object.rs:2084 (inline in `draw_text`/`backend.draw_glyph`) | HAS |
| `gsx_tcalc` | 480 | `static void gsx_tcalc(WORD font, char *ptext, WORD *ptextw, WORD *ptexth, WORD *pnumchs)` | object.rs:2096 (inline in `draw_text`: `tw = len * CELL_W`) | HAS |
| `gsx_trans` | 369 | `void gsx_trans(void *addr, UWORD wb, UWORD h)` | — Mono-Standard→Device-Form-Transform; obsolet, `blit_mono` konsumiert Standard-Form-Bits direkt | N/A |
| `gsx_xbox` | 274 | `void gsx_xbox(GRECT *pt)` | graf.rs:44 `outline()` (volle Box statt nur Ecken, XOR-Mode) | HAS |
| `gsx_xcbox` | 285 | `void gsx_xcbox(GRECT *pt)` | graf.rs:44 `outline()` (volle Box statt Ecken-Teilstücke) | HAS |
| `gsx_xline` | 140 | `static void gsx_xline(WORD ptscount, WORD *ppoints)` | graf.rs:44 `outline()` — explizit "Matches EmuTOS `gr_rubwind` / `gsx_xline` (MD_XOR)" zitiert | HAS |

## gemgrlib.c  (19 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `gr_2box` | 271 | `void gr_2box(WORD flag1, WORD cnt, GRECT *pt, WORD xstep, WORD ystep, WORD flag2)` | graf.rs:213 `animate()` | HAS |
| `gr_clamp` | 83 | `static void gr_clamp(WORD xorigin, WORD yorigin, WORD wmin, WORD hmin,` | graf.rs:92 `gr_clamp` (3-Parameter statt 4, pro Achse aufgerufen) | HAS |
| `gr_dragbox` | 243 | `void gr_dragbox(WORD w, WORD h, WORD sx, WORD sy, GRECT *pc,` | graf.rs:171 `graf_dragbox` | HAS |
| `gr_draw` | 155 | `static void gr_draw(WORD have2box, GRECT *po, GRECT *poff)` | graf.rs (inline `outline()`-Aufrufe); Zwei-Box-Variante (`have2box`) nicht reproduziert | HAS |
| `gr_growbox` | 309 | `void gr_growbox(GRECT *po, GRECT *pt)` | graf.rs:261 `graf_growbox` | HAS |
| `gr_mkstate` | 458 | `void gr_mkstate(WORD *pmx, WORD *pmy, WORD *pmstat, WORD *pkstat)` | graf.rs:476 `graf_mkstate` | HAS |
| `gr_mouse` | 419 | `void gr_mouse(WORD mode, MFORM *maddr)` | graf.rs:420 `graf_mouse` | HAS |
| `gr_movebox` | 287 | `void gr_movebox(WORD w, WORD h, WORD srcx, WORD srcy, WORD dstx, WORD dsty)` | graf.rs:248 `graf_movebox` | HAS |
| `gr_rubbox` | 228 | `void gr_rubbox(WORD xorigin, WORD yorigin, WORD wmin, WORD hmin,` | graf.rs:121 `graf_rubberbox` (vereinigt beide Varianten via signed Clamp) | HAS |
| `gr_rubwind` | 199 | `void gr_rubwind(WORD xorigin, WORD yorigin, WORD wmin, WORD hmin,` | graf.rs:121 `graf_rubberbox` | HAS |
| `gr_scale` | 91 | `static void gr_scale(WORD xdist, WORD ydist, WORD *pcnt, WORD *pxstep, WORD *pystep)` | graf.rs:213 (inline in `animate()`: feste `STEPS=8` statt distanzbasierter Berechnung) | HAS |
| `gr_setup` | 76 | `static void gr_setup(WORD color)` | graf.rs (inline: `WriteMode::Xor` wird pro Aufruf übergeben statt Attribut-Setup) | HAS |
| `gr_shrinkbox` | 326 | `void gr_shrinkbox(GRECT *po, GRECT *pt)` | graf.rs:270 `graf_shrinkbox` | HAS |
| `gr_slidebox` | 358 | `WORD gr_slidebox(OBJECT *tree, WORD parent, WORD obj, WORD isvert)` | graf.rs:328 `graf_slidebox` | HAS |
| `gr_stepcalc` | 114 | `void gr_stepcalc(WORD orgw, WORD orgh, GRECT *pt, WORD *pcx, WORD *pcy,` | graf.rs:219 (inline `lerp()` in `animate()`) | HAS |
| `gr_stilldn` | 47 | `static BOOL gr_stilldn(BOOL out, WORD x, WORD y, WORD w, WORD h)` | graf.rs (inline `mouse_down()`+`pump_input()`-Schleifen) | HAS |
| `gr_wait` | 169 | `static BOOL gr_wait(GRECT *po, GRECT *poff)` | graf.rs (inline Draw/Wait/Erase-Muster in `graf_dragbox`/`graf_rubberbox`) | HAS |
| `gr_watchbox` | 337 | `WORD gr_watchbox(OBJECT *tree, WORD obj, WORD instate, WORD outstate)` | graf.rs:280/296 `graf_watchbox`/`watchbox` | HAS |
| `gr_xor` | 127 | `static void gr_xor(WORD clipped, WORD cnt, WORD cx, WORD cy, WORD cw, WORD ch,` | graf.rs:213 `animate()` | HAS |

## gemgsxif.c  (44 Symbole)

> Kontextnote: die AES-eigene m68k-Software-Trap-Bindung an die VDI
> (`gsx_ncode`/`gsx2()`). NewTOS zeichnet direkt über `&dyn RasterBackend`
> ohne interne VDI-Workstation-Indirektion. Reine Transport-Wrapper sind N/A;
> Funktionen mit erhaltenem AES-Verhalten (Bildschirmsicherung, Mauszeiger,
> Tastaturstatus …) sind HAS über ein anderes Modul (oft `tos-api/src/vdi/`).

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `aestrap_intercepted` | 259 | `static BOOL aestrap_intercepted(void)` | — m68k-Trap#2/NVDI-Kompatibilitäts-Check | N/A |
| `bb_restore` | 390 | `void bb_restore(GRECT *pr)` | form.rs:338 `BbSave::restore`; menu.rs `restore_under` | HAS |
| `bb_save` | 383 | `void bb_save(GRECT *ps)` | form.rs:314 `BbSave::capture`; menu.rs `save_under` | HAS |
| `bb_set` | 320 | `static void bb_set(BOOL save, GRECT *r)` | form.rs (inline: gemeinsame Save/Restore-Logik in `BbSave`) | HAS |
| `gsx_0code` | 144 | `void gsx_0code(WORD code)` | — roher VDI-Opcode-Transport ohne Argumente | N/A |
| `gsx_1code` | 151 | `void gsx_1code(WORD code, WORD value)` | — roher VDI-Opcode-Transport mit einem Argument | N/A |
| `gsx_char` | 459 | `WORD gsx_char(void)` | event.rs (inline `host().console_try_read_key()`) | HAS |
| `gsx_fix` | 532 | `void gsx_fix(FDB *pfd, void *theaddr, WORD wb, WORD h)` | — FDB-Raster-Form-Setup für VDI-Transport | N/A |
| `gsx_fix_screen` | 518 | `void gsx_fix_screen(FDB *pfd)` | — FDB-Screen-Form-Setup für VDI-Transport | N/A |
| `gsx_graphic` | 298 | `void gsx_graphic(BOOL tographic)` | — Alpha/Grafik-Modus-Umschaltung; nicht anwendbar (fester Framebuffer) | N/A |
| `gsx_init` | 267 | `void gsx_init(void)` | — VDI-Bindungs-Init-Glue | N/A |
| `gsx_kstate` | 432 | `WORD gsx_kstate(void)` | event.rs:105 `shift_state` / `host().keyboard_shift_state()` | HAS |
| `gsx_malloc` | 108 | `void gsx_malloc(void)` | form.rs (inline: `BbSave` nutzt `alloc::Vec`) | HAS |
| `gsx_mcalc` | 98 | `static ULONG gsx_mcalc(void)` | form.rs (inline: `BbSave` berechnet eigene Puffergröße) | HAS |
| `gsx_mfree` | 118 | `void gsx_mfree(void)` | form.rs (inline: Rust-RAII-Drop von `BbSave`) | HAS |
| `gsx_mfset` | 407 | `void gsx_mfset(const MFORM *pmfnew)` | graf.rs:456 `set_user_form` | HAS |
| `gsx_moff` | 440 | `void gsx_moff(void)` | `crate::vdi::cursor_hide` = `mouse.rs:354 hide` (HIDE_COUNT-Semaphor, entspricht `gl_moff`) | HAS |
| `gsx_mon` | 450 | `void gsx_mon(void)` | `crate::vdi::cursor_show` = `mouse.rs:363 show` | HAS |
| `gsx_mret` | 125 | `void gsx_mret(LONG *pmaddr, LONG *pmlen)` | form.rs (inline: `BbSave`-Felder direkt gelesen, kein Getter nötig) | HAS |
| `gsx_mxmy` | 424 | `void gsx_mxmy(WORD *pmx, WORD *pmy)` | `crate::vdi::mouse_position` = `mouse.rs:138 position` | HAS |
| `gsx_ncode` | 133 | `static void gsx_ncode(WORD code, WORD n, WORD m)` | — roher VDI-Opcode-Dispatch-Transport, gemeinsame Engine hinter 0code/1code | N/A |
| `gsx_nplanes` | 497 | `WORD gsx_nplanes(void)` | appl.rs:215 `screen_planes` | HAS |
| `gsx_resetmb` | 238 | `static void gsx_resetmb(void)` | — Maus/Button-Interrupt-Vektor-Deinstallation, m68k-Transport | N/A |
| `gsx_setmb` | 219 | `static void gsx_setmb(PFVOID boff, PFVOID moff, PFVOID *pdrwaddr)` | — Maus/Button-Interrupt-Vektor-Installation, m68k-Transport (ersetzt durch `host().pump_input()`-Polling) | N/A |
| `gsx_setmousexy` | 484 | `void gsx_setmousexy(WORD x, WORD y)` | `tos-api/src/vdi/mouse.rs:106 set_state` (nicht an `appl_tplay` angebunden, das ein dokumentierter No-op ist) | HAS |
| `gsx_textsize` | 505 | `void gsx_textsize(WORD *charw, WORD *charh, WORD *cellw, WORD *cellh)` | `crate::vdi::font::CELL_W`/`CELL_H` (feste Konstanten) | HAS |
| `gsx_tick` | 397 | `WORD gsx_tick(void *tcode, void *ptsave)` | — VDI-Timer-Vektor-Installation, m68k-Transport | N/A |
| `gsx_wsclear` | 198 | `void gsx_wsclear(void)` | — VDI-Workstation-Clear-Transport | N/A |
| `gsx_wsclose` | 191 | `void gsx_wsclose(void)` | — VDI-Workstation-Close-Transport | N/A |
| `gsx_wsopen` | 169 | `static void gsx_wsopen(void)` | — VDI-Workstation-Open-Transport | N/A |
| `memsize` | 42 | `#define memsize(wdwidth,h,nplanes) ((LONG)(wdwidth)*(h)*(nplanes)*2)` | form.rs (inline: `BbSave` berechnet eigene Puffergröße) | HAS |
| `ratexit` | 212 | `void ratexit(void)` | `tos-api/src/vdi/mouse.rs` HIDE_COUNT-Semaphor / `show_reset` (Aufrufstelle `ct_mouse` selbst NO-COUNTERPART) | HAS |
| `ratinit` | 205 | `void ratinit(void)` | `tos-api/src/vdi/mouse.rs` HIDE_COUNT-Semaphor / `show_reset` | HAS |
| `screen_rez` | 159 | `static WORD screen_rez(void)` | — Falcon/ST-Videomodus-Enumeration für VDI-Open, nicht anwendbar | N/A |
| `v_opnwk` | 552 | `static void v_opnwk(WORD *pwork_in, WORD *phandle, WS *pwork_out)` | — roher VDI-Open-Workstation-Trap-Aufruf | N/A |
| `v_pline` | 573 | `void v_pline(WORD count, WORD *pxyarray)` | object.rs (inline `backend.draw_line`-Schleifen) | HAS |
| `vex_wheelv` | 663 | `void vex_wheelv(PFVOID new, PFVOID *old)` | `tos-api/src/vdi/mouse.rs:427 vex_wheelv` | HAS |
| `vr_recfl` | 604 | `void vr_recfl(WORD *pxyarray)` | object.rs (inline `backend.fill_rect`, `gr_rect`) | HAS |
| `vrn_trnfm` | 640 | `void vrn_trnfm(FDB *psrcMFDB, FDB *pdesMFDB)` | — Mono-Standard→Device-Form-Transform; obsolet (wie `gsx_trans`) | N/A |
| `vro_cpyfm` | 613 | `void vro_cpyfm(WORD wr_mode, WORD *pxyarray, FDB *psrcMFDB, FDB *pdesMFDB )` | form.rs/window.rs (inline `backend.copy_rect`) | HAS |
| `vrt_cpyfm` | 625 | `void vrt_cpyfm(WORD wr_mode, WORD *pxyarray, FDB *psrcMFDB, FDB *pdesMFDB,` | object.rs (inline `backend.blit_mono` mit fg/bg, `blit_plane`) | HAS |
| `vs_clip` | 581 | `void vs_clip(WORD clip_flag, WORD *pxyarray )` | object.rs (inline: ClipRect-Parameter-Threading, siehe `gsx_sclip`) | HAS |
| `vsl_width` | 653 | `void vsl_width(WORD width)` | — Linienbreiten-Attribut; NewTOS zeichnet immer 1px-Linien, kein Setter nötig | N/A |
| `vst_height` | 590 | `void vst_height(WORD height, WORD *pchr_width, WORD *pchr_height,` | — VDI-Font-Höhen-Auswahl-Transport; ersetzt durch feste `CELL_W`/`CELL_H`-Konstanten | N/A |

## geminit.c  (22 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `accdesk_start` | 63 | `void accdesk_start(void) NORETURN; /* called only from gemstart.S */` | — Boot-/Asm-Entry-Glue aus `gemstart.S` | N/A |
| `all_run` | 554 | `void all_run(void)` | — weckt alle wartenden AESPDs (Scheduler); kein Scheduler | NO-COUNTERPART |
| `count_accs` | 264 | `static WORD count_accs(void)` | — zählt `.ACC`-Dateien; kein Accessory-Support | NO-COUNTERPART |
| `default_mform` | 174 | `MFORM *default_mform(void)` | `tos-api/src/vdi/mouse.rs:214 set_preset` (ARROW-Preset) | HAS |
| `ev_init` | 198 | `static void ev_init(EVB evblist[], WORD cnt)` | — EVB-Freipool-Init (Scheduler-intern, s. gemasync.c) | N/A |
| `free_accs` | 296 | `static void free_accs(WORD n)` | — Accessory-Speicherfreigabe; kein Accessory-Support | NO-COUNTERPART |
| `gem_main` | 753 | `void gem_main(void)` | — kein einzelner "AES-Main"-Orchestrator in tos-api/aes | NO-COUNTERPART |
| `init_p0_stkptr` | 161 | `LONG init_p0_stkptr(void)` | — m68k Prozess-0-Supervisor-Stack-Setup; anderes Kernel-Boot-/Speichermodell | N/A |
| `iprocess` | 215 | `static AESPD *iprocess(char *pname, PFVOID routine)` | — internes Prozess-Spawning (`pstart`); kein Prozess-Spawning | NO-COUNTERPART |
| `load_accs` | 309 | `static void load_accs(WORD n)` | — Accessories laden; kein Accessory-Support | NO-COUNTERPART |
| `load_mouse_cursors` | 471 | `static WORD load_mouse_cursors(char *buf)` | `tos-api/src/vdi/mouse.rs:214 set_preset`-Tabelle (CONF_WITH_LOADABLE_CURSORS-Variante selbst nicht reproduziert) | HAS |
| `load_one_acc` | 236 | `static void load_one_acc(ACC *acc)` | — ein Accessory laden+starten; kein Accessory-Support | NO-COUNTERPART |
| `process_inf1` | 343 | `static void process_inf1(void)` | — EMUDESK.INF-Parsing (Auflösung/Hintergrund); gehört zum Desktop, nicht zu tos-api/aes | NO-COUNTERPART |
| `process_inf2` | 407 | `static BOOL process_inf2(BOOL *isauto)` | — EMUDESK.INF-Parsing Teil 2; gehört zum Desktop | NO-COUNTERPART |
| `readfile` | 323 | `static LONG readfile(char *filename, LONG count, char *buf)` | fsel.rs:98 (inline `crate::bdos::fs::read_file`) | HAS |
| `run_accs_and_desktop` | 654 | `void run_accs_and_desktop(void)` | — Accessory+Desktop-Start-Orchestrierung; Desktop-Start liegt außerhalb tos-api/aes | NO-COUNTERPART |
| `set_aes_background` | 635 | `void set_aes_background(UBYTE patcol)` | window.rs:334 `desktop_bg_obspec` (+ WF_NEWDESK-Registrierung); direkt als "EmuTOS `set_aes_background`" zitiert | HAS |
| `set_mouse_to_arrow` | 183 | `void set_mouse_to_arrow(void)` | `tos-api/src/vdi/mouse.rs` (inline `set_preset` ARROW) | HAS |
| `set_mouse_to_hourglass` | 192 | `void set_mouse_to_hourglass(void)` | `tos-api/src/vdi/mouse.rs` (inline `set_preset` HOURGLASS, via `graf_mouse`) | HAS |
| `setup_mouse_cursors` | 513 | `static void setup_mouse_cursors(void)` | `tos-api/src/vdi/mouse.rs` Preset-Tabelle | HAS |
| `size_theglo` | 149 | `LONG size_theglo(void)` | — m68k globale-Datenbereich-Größenberechnung; anderes Speichermodell | N/A |
| `wait_for_accs` | 599 | `void wait_for_accs(WORD bitmask)` | — auf Accessory-AC_CLOSE-Quittung warten; kein Accessory-Support | NO-COUNTERPART |

## geminput.c  (26 Symbole)

> Kontextnote: die Low-Level-Input-Treiberschicht, die den AESPD/EVB-Scheduler
> speist (Button/Maus/Tastatur-Interrupt-Handler bewaffnen EVBs, Ownership-
> Arbitrierung, CDA-Warteschlangen). `event.rs` liest stattdessen direkt vom
> `host()` und pollt — die Hochsprachen-Funktionen (`evnt_*`) haben ein
> Gegenstück, die interne Poste/Warteschlangen-/Ownership-Maschinerie nicht.

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `abutton` | 545 | `void abutton(EVB *e, LONG p)` | — EVB für Button-Event bewaffnen | NO-COUNTERPART |
| `adelay` | 496 | `void adelay(EVB *e, LONG c)` | — EVB für Timer-Delay bewaffnen | NO-COUNTERPART |
| `akbin` | 486 | `void akbin(EVB *e)` | — EVB für Tastatur-Input bewaffnen | NO-COUNTERPART |
| `amouse` | 568 | `void amouse(EVB *e, LONG pmo)` | — EVB für Maus-Rect-Event bewaffnen | NO-COUNTERPART |
| `b_click` | 114 | `void b_click(WORD state)` | event.rs:183 `count_clicks` | HAS |
| `b_delay` | 155 | `void b_delay(WORD amnt)` | event.rs:144/147 `DCRATES`/`dclick_ms` | HAS |
| `bchange` | 328 | `void bchange(LONG fdata)` | — Fork-Routine bei Button-Wechsel, weckt wartende AESPDs | NO-COUNTERPART |
| `chk_ctrl` | 87 | `static WORD chk_ctrl(WORD mx, WORD my)` | — Ctrl-Rect-Zugehörigkeitsprüfung (Ownership-Arbitrierung) | NO-COUNTERPART |
| `chkown` | 316 | `static AESPD *chkown(void)` | — aktuellen Maus-Owner-AESPD bestimmen | NO-COUNTERPART |
| `downorup` | 348 | `WORD downorup(WORD new, LONG buparm)` | event.rs (inline `matches()`-Closures) | HAS |
| `dq` | 256 | `UWORD dq(CQUEUE *qptr)` | event.rs (inline `host().console_try_read_key()`) | HAS |
| `evremove` | 279 | `void evremove(EVB *e, UWORD ret)` | — EVB entfernen/abschließen (Scheduler-intern) | NO-COUNTERPART |
| `fq` | 272 | `void fq(void)` | — Tastatur-Warteschlange leeren; kein Input-Flush-Primitive (grep: kein `flush`/`drain` in aes/) | NO-COUNTERPART |
| `get_ctrl` | 200 | `void get_ctrl(GRECT *pt)` | — Ctrl-Rect abfragen | NO-COUNTERPART |
| `get_mown` | 209 | `void get_mown(AESPD **pmown)` | — Maus-Owner abfragen | NO-COUNTERPART |
| `in_mrect` | 76 | `UWORD in_mrect(MOBLK *pmo)` | event.rs:42 `rect_event` | HAS |
| `inorout` | 427 | `static WORD inorout(EVB *e, WORD rx, WORD ry)` | — EVB-Rect-Enter/Leave-Test (Scheduler-intern) | NO-COUNTERPART |
| `kchange` | 288 | `void kchange(LONG fdata)` | — Fork-Routine bei Tastatur-Wechsel, weckt wartende AESPDs | NO-COUNTERPART |
| `mchange` | 360 | `void mchange(LONG fdata)` | — Fork-Routine bei Maus-Bewegung, weckt wartende AESPDs | NO-COUNTERPART |
| `nq` | 241 | `static void nq(UWORD ch, CQUEUE *qptr)` | — Taste in Ringpuffer einreihen (ersetzt durch Host-Key-Queue) | NO-COUNTERPART |
| `post_button` | 463 | `static void post_button(AESPD *pd, WORD new, WORD numclicks)` | — Button-Event an ein AESPD posten (Scheduler-intern) | NO-COUNTERPART |
| `post_keybd` | 299 | `void post_keybd(CDA *c, UWORD ch)` | — Taste an eine CDA posten (Scheduler-intern) | NO-COUNTERPART |
| `post_mouse` | 446 | `static void post_mouse(AESPD *pd, WORD x, WORD y)` | — Mausposition an ein AESPD posten (Scheduler-intern) | NO-COUNTERPART |
| `set_ctrl` | 189 | `void set_ctrl(GRECT *pt)` | — Ctrl-Rect setzen | NO-COUNTERPART |
| `set_mown` | 220 | `void set_mown(AESPD *mp)` | — Maus-Owner setzen | NO-COUNTERPART |
| `wheel_change` | 395 | `void wheel_change(WORD wheel_number, WORD wheel_amount)` | — Mausrad-Event-Fork-Routine; kein Mausrad-Support (grep: kein `wheel`/`Wheel` in aes/) | NO-COUNTERPART |

## gemmnext.c  (17 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `attach_submenu` | 465 | `static BOOL attach_submenu(OBJECT *tree, WORD item, MENU *mdata)` | menu.rs:1230 `attach_submenu` | HAS |
| `clamp_height` | 83 | `static WORD clamp_height(WORD height)` | menu.rs:992 `clamp_height` | HAS |
| `clamp_ypos` | 115 | `static void clamp_ypos(OBJECT *obj)` | menu.rs:359 (inline y-Clamping in `open_submenu`) | HAS |
| `detach_submenu` | 438 | `static void detach_submenu(OBJECT *tree, WORD item)` | menu.rs `detach_submenu` (Submenu-Tab-Verwaltung) | HAS |
| `display_submenu` | 625 | `OBJECT *display_submenu(OBJECT *tree, WORD objnum, WORD *smroot)` | menu.rs `display_submenu` | HAS |
| `find_submenu_from_obtype` | 361 | `static SMIB *find_submenu_from_obtype(WORD ob_type)` | menu.rs:162 (inline: `track()` hält den offenen Submenu-Zustand direkt statt per Reverse-Lookup) | HAS |
| `find_submenu_from_tree` | 380 | `static SMIB *find_submenu_from_tree(OBJECT *tree, WORD root)` | menu.rs:1133 `smib_find_sub` | HAS |
| `handle_popup` | 245 | `static WORD handle_popup(MENU *menu)` | menu.rs:917 `popup_track` | HAS |
| `mn_attach` | 535 | `WORD mn_attach(WORD flag, OBJECT *tree, WORD item, MENU *mdata)` | menu.rs:1288 `menu_attach` | HAS |
| `mn_istart` | 562 | `WORD mn_istart(WORD flag, OBJECT *tree, WORD menu, WORD start)` | menu.rs:1326 `menu_istart` | HAS |
| `mn_popup` | 304 | `WORD mn_popup(MENU *menu, WORD xpos, WORD ypos, MENU *mdata)` | menu.rs:873 `menu_popup` | HAS |
| `mn_settings` | 596 | `void mn_settings(WORD flag, MN_SET *set)` | menu.rs:1019 `menu_settings` | HAS |
| `mnext_init` | 103 | `void mnext_init(void)` | menu.rs:1121 (inline: `SUBMENU_TAB`-Static-Null-Init) | HAS |
| `popup_blit` | 136 | `static void *popup_blit(OBJECT *tree, void *buf)` | menu.rs `save_under`/`restore_under` (Popup-Hintergrund-Sicherung) | HAS |
| `query_submenu` | 401 | `static BOOL query_submenu(OBJECT *tree, WORD item, MENU *mdata)` | menu.rs:1201 `query_submenu` | HAS |
| `setup_moblk` | 220 | `static void setup_moblk(MOBLK *pmo, MENU *menu, WORD mx, WORD my)` | menu.rs:917 (inline: `popup_track` prüft Punkt-in-Rect direkt statt inkrementellem MOBLK) | HAS |
| `undisplay_submenu` | 683 | `void undisplay_submenu(OBJECT *tree, WORD objnum)` | menu.rs `undisplay_submenu` | HAS |

## gemmnlib.c  (15 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `build_menuid_lookup` | 766 | `static void build_menuid_lookup(void)` | — Accessory-Item→Owner-Reverse-Lookup; laut menu.rs:711-716 Doku explizit als Folgearbeit zurückgestellt (Registrierung ja, Verdrahtung ins Desk-Menü nein) | NO-COUNTERPART |
| `do_chg` | 145 | `BOOL do_chg(OBJECT *tree, WORD iitem, UWORD chgvalue,` | menu.rs:768 `do_chg` | HAS |
| `item_changed` | 178 | `static BOOL item_changed(WORD last_item, WORD cur_item)` | menu.rs:291 (inline in `rehilite`: `old != new`-Prüfung) | HAS |
| `menu_down` | 229 | `static WORD menu_down(OBJECT *tree, WORD ititle)` | menu.rs:162 `track()` (Hover-Tracking statt Klick-Tracking) | HAS |
| `menu_fixup` | 89 | `static void menu_fixup(void)` | menu.rs:125 (inline in `menu_bar`: Breiten-Fixup) | HAS |
| `menu_select` | 192 | `static BOOL menu_select(OBJECT *tree, WORD last_item, WORD cur_item, WORD setit)` | menu.rs:291 `rehilite` | HAS |
| `menu_sr` | 208 | `static void menu_sr(WORD saveit, OBJECT *tree, WORD imenu)` | menu.rs `save_under`/`restore_under` (explizit als "GEM's `menu_sr`" zitiert) | HAS |
| `menu_sub` | 71 | `static WORD menu_sub(OBJECT *tree, WORD ititle)` | menu.rs:421 `menu_sub` | HAS |
| `mn_bar` | 703 | `void mn_bar(OBJECT *tree, WORD showit)` | menu.rs:101 `menu_bar` | HAS |
| `mn_cleanup` | 740 | `void mn_cleanup(void)` | — Accessory-Slot-Freigabe; keine Freigabe-Funktion für `ACCS`-Registry gefunden | NO-COUNTERPART |
| `mn_do` | 249 | `WORD mn_do(WORD *ptitle, WORD *pitem, OBJECT **ptree)` | menu.rs `track()` + event.rs:58 `menu_intercept` | HAS |
| `mn_getownid` | 838 | `void mn_getownid(AESPD **owner,WORD *id,WORD item)` | — Accessory-Owner-Auflösung; explizit zurückgestellt (s. `build_menuid_lookup`) | NO-COUNTERPART |
| `mn_init` | 821 | `void mn_init(void)` | menu.rs:49 (inline: `MNTREE`-Static-Null-Init) | HAS |
| `mn_register` | 789 | `WORD mn_register(WORD pid, char *pstr)` | menu.rs:733 `menu_register` | HAS |
| `rect_change` | 131 | `static void rect_change(OBJECT *tree, MOBLK *prmob, WORD iob, BOOL x)` | menu.rs:162 (inline: `track()` berechnet Hit-Info jede Iteration frisch statt inkrementell) | HAS |

## gemobed.c  (12 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `check` | 252 | `static WORD check(char *in_char, char valchar)` | object.rs:1236 `check` | HAS |
| `curfld` | 194 | `static void curfld(OBJECT *tree, WORD obj, WORD new_pos, WORD dist)` | object.rs:1083-1096 (inline in `edit_char`: SC_LEFT/SC_RIGHT-Handling) | HAS |
| `find_pos` | 158 | `static WORD find_pos(char *str, WORD pos)` | object.rs:1173 `find_pos` | HAS |
| `ins_char` | 117 | `void ins_char(char *str, WORD pos, char chr, WORD tot_len)` | object.rs:1147 `ins_char` | HAS |
| `instr` | 228 | `static WORD instr(char chr, char *str)` | object.rs:1211 `instr` | HAS |
| `ob_center` | 67 | `void ob_center(OBJECT *tree, GRECT *pt)` | form.rs:716 `form_center` | HAS |
| `ob_delit` | 323 | `static WORD ob_delit(WORD idx)` | object.rs:1134 `delete_at` | HAS |
| `ob_edit` | 335 | `WORD ob_edit(OBJECT *tree, WORD obj, WORD in_char, WORD *idx, WORD kind)` | object.rs:994/1031 `objc_edit`/`edit_field_t` | HAS |
| `ob_getsp` | 55 | `static void ob_getsp(OBJECT *tree, WORD obj, TEDINFO *pted)` | object.rs:1040 (inline: `TedInfo::read_abi`) | HAS |
| `ob_stfn` | 316 | `static void ob_stfn(WORD idx, WORD *pstart, WORD *pfinish)` | — Template-Segment-Grenzen-Berechnung; Pfeiltasten bewegen nur ±1 Rohindex statt segmentbewusst zu springen | NO-COUNTERPART |
| `pxl_rect` | 176 | `static void pxl_rect(OBJECT *tree, WORD obj, WORD ch_pos, GRECT *pt)` | object.rs:1061-1063 (inline: Caret-Spalten-Berechnung in `edit_field_t`) | HAS |
| `scan_to_end` | 141 | `static WORD scan_to_end(char *pstr, WORD idx, char chr)` | — String-Scan-Hilfsfunktion für das fehlende `ob_stfn`-Segment-Handling | NO-COUNTERPART |

## gemobjop.c  (3 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `everyobj` | 76 | `void everyobj(OBJECT *tree, WORD this, WORD last, EVERYOBJ_CALLBACK routine,` | object.rs:1478 (inline: jeder Walker — `find_hit`/`draw_tree` — implementiert seine eigene Rekursion statt eines generischen Callback-Walkers) | HAS |
| `get_par` | 145 | `WORD get_par(OBJECT *tree, WORD obj)` | object.rs:1435 `parent()` (explizit "EmuTOS `get_par`" zitiert) | HAS |
| `ob_sst` | 25 | `char ob_sst(OBJECT *tree, WORD obj, LONG *pspec, WORD *pstate, WORD *ptype,` | object.rs:1687-1695 (inline: 3D-bewusste Attribut-Extraktion in `draw_tree`, explizit zitiert) | HAS |

## gemoblib.c  (23 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `add_3d_effect` | 220 | `static void add_3d_effect(GRECT *pt, WORD obstate, WORD th, WORD color)` | object.rs:1986 `add_3d_effect` | HAS |
| `call_usercode` | 411 | `static __inline__ WORD call_usercode(USERBLK *ub, PARMBLK *pb)` | — Aufruf einer app-definierten G_USERDEF-Zeichnen/Hit-Test-Callback; kein `USERBLK`/`PARMBLK`/`G_USERDEF`-Support (grep bestätigt) | NO-COUNTERPART |
| `draw_3d_outline` | 153 | `static void draw_3d_outline(GRECT *pt)` | object.rs:2010 `draw_3d_outline` | HAS |
| `get_prev` | 987 | `static WORD get_prev(OBJECT *tree, WORD parent, WORD obj)` | object.rs:830 `get_prev` | HAS |
| `init_3d` | 1327 | `void init_3d(void)` | — Boot-Zeit-Umschalter für 3D-Objekte aus dem INF; 3D-Look ist bei NewTOS fest verdrahtet (kein Laufzeit-Toggle nötig) | N/A |
| `is_dashes` | 455 | `static BOOL is_dashes(char *s, WORD len)` | — Erkennung von "------"-Menü-Separator-Einträgen; kein Dash-Separator-Rendering in menu.rs | NO-COUNTERPART |
| `just_draw` | 471 | `static void just_draw(OBJECT *tree, WORD obj, WORD sx, WORD sy)` | object.rs:1609 `just_draw` (+ fsel.rs, menu.rs) | HAS |
| `ob_actxywh` | 313 | `void ob_actxywh(OBJECT *tree, WORD obj, GRECT *pt)` | object.rs:1327/1333 `abs_rect`/`abs_rect_t` | HAS |
| `ob_add` | 1121 | `void ob_add(OBJECT *tree, WORD parent, WORD child)` | object.rs:735 `ob_add` | HAS |
| `ob_change` | 1253 | `void ob_change(OBJECT *tree, WORD obj, UWORD new_state, WORD redraw)` | object.rs:955/962 `change`/`change_t` | HAS |
| `ob_delete` | 1147 | `WORD ob_delete(OBJECT *tree, WORD obj)` | object.rs:751 `ob_delete` | HAS |
| `ob_draw` | 959 | `void ob_draw(OBJECT *tree, WORD obj, WORD depth)` | object.rs:1370/1377 `draw_obj`/`draw_obj_t` | HAS |
| `ob_find` | 1034 | `WORD ob_find(OBJECT *tree, WORD currobj, WORD depth, WORD mx, WORD my)` | object.rs:1284/2263 `objc_find`/`find_hit` | HAS |
| `ob_format` | 361 | `void ob_format(WORD just, char *raw_str, char *tmpl_str, char *fmt_str)` | object.rs:1572 `ob_format` | HAS |
| `ob_fs` | 1427 | `UWORD ob_fs(OBJECT *tree, WORD obj, WORD *pflag)` | object.rs (inline `ObjTree::flags`/`state`); form.rs:803 `ob_fs` | HAS |
| `ob_offset` | 285 | `void ob_offset(OBJECT *tree, WORD obj, WORD *pxoff, WORD *pyoff)` | object.rs:1418 `abs_offset` (explizit zitiert) | HAS |
| `ob_order` | 1201 | `BOOL ob_order(OBJECT *tree, WORD mov_obj, WORD new_pos)` | object.rs:784 `ob_order` | HAS |
| `ob_relxywh` | 302 | `void ob_relxywh(OBJECT *tree, WORD obj, GRECT *pt)` | object.rs (inline: `ObjTree::x`/`y`/`width`/`height`-Accessoren) | HAS |
| `ob_setxywh` | 349 | `void ob_setxywh(OBJECT *tree, WORD obj, GRECT *pt)` | form.rs:203-239 (inline: direkte Struct-Literal-Feldzuweisung in `do_alert`) | HAS |
| `ob_sysvar` | 1350 | `WORD ob_sysvar(WORD mode, WORD which, WORD in1, WORD in2, WORD *out1, WORD *out2)` | object.rs:892 `objc_sysvar` | HAS |
| `ob_user` | 433 | `static WORD ob_user(OBJECT *tree, WORD obj, GRECT *pt, LONG spec,` | — G_USERDEF-Dispatcher; kein G_USERDEF-Support | NO-COUNTERPART |
| `xor_color` | 93 | `static WORD xor_color(WORD color)` | object.rs:1526 `xor_color` | HAS |
| `xor_is_ok` | 107 | `static BOOL xor_is_ok(WORD obtype, WORD obflags, LONG obspec)` | object.rs `xor_is_ok` | HAS |

## gempd.c  (7 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `fpdnm` | 38 | `AESPD *fpdnm(char *pname, UWORD pid)` | pd.rs:255 `find_by_name` (+ `by_pid`) | HAS |
| `getpd` | 60 | `static AESPD *getpd(void)` | pd.rs:207-224 (inline in `open()`: freien Slot beanspruchen) | HAS |
| `insert_process` | 134 | `void insert_process(AESPD *pi, AESPD **root)` | — verkettete Run-Queue-Einfügung; kein Run-Queue (pd.rs nutzt flaches Array mit `in_use`) | NO-COUNTERPART |
| `p_nameit` | 77 | `void p_nameit(AESPD *p, char *pname)` | — `Aespd.name`-Feld existiert (pd.rs:161), wird aber **nie beschrieben** (nur `[b' '; 8]`-Init); `appl_find` kann daher nie erfolgreich per Namen finden | NO-COUNTERPART |
| `p_setappdir` | 90 | `void p_setappdir(AESPD *pd, char *pfilespec)` | — kein `cwd`/App-Verzeichnis-Feld in `Aespd` | NO-COUNTERPART |
| `pd_index` | 32 | `AESPD *pd_index(WORD i)` | — kein öffentlicher Index-basierter Accessor (`table()` ist privat) | NO-COUNTERPART |
| `pstart` | 110 | `AESPD *pstart(PFVOID pcode, char *pfilespec, LONG ldaddr)` | — neuen Prozesskontext erzeugen+starten; kein internes Prozess-Spawning | NO-COUNTERPART |

## gemqueue.c  (2 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `aqueue` | 85 | `void aqueue(WORD isqwrite, EVB *e, LONG lm)` | pd.rs `post_msg`/`MsgQueue::push` (EVB-Bewaffnungs-Hälfte fehlt, aber die Routing-Rolle ist erfüllt; volles Blockieren bei voller Queue durch ehrliches `false` ersetzt) | HAS |
| `doq` | 31 | `static void doq(WORD donq, AESPD *p, QPB *m)` | pd.rs:96/123 `MsgQueue::push`/`pop` (explizit als "EmuTOS `doq`" zitiert, inkl. WM_REDRAW-Koaleszenz-Regeln) | HAS |

## gemrslib.c  (28 Symbole)

> Kontextnote: NewTOS nutzt ein natives `.RSC`-Format (kleine-Endian, Byte-
> Offsets statt klassischem big-endian TOS-RSC) — die host-seitige `mkrsc.py`
> übernimmt die Endian-/Zeiger-Konvertierung beim Build; zur Laufzeit bleibt
> nur eine simple Offset→Zeiger-Relozierung übrig (`resource.rs:438
> relocate_blob_core`, explizit "EmuTOS `fix_objects`" zitiert). Colour-Icons
> (CICON) sind komplett nicht implementiert.

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `best_match` | 267 | `static CICON *best_match(CICONBLK *start)` | — Colour-Icon-Planzahl-Matching; kein CICON-Support | NO-COUNTERPART |
| `expand_cicondata` | 296 | `static void expand_cicondata(WORD *src, WORD *dst, WORD *mask, WORD w, WORD h, WORD src_planes, WORD dst_planes)` | — Colour-Icon-Daten-Expansion; kein CICON-Support | NO-COUNTERPART |
| `fix_chpos` | 53 | `static void fix_chpos(WORD *pfix, WORD offset)` | resource.rs:1018 `fix_chpos` | HAS |
| `fix_cicons` | 549 | `static void fix_cicons(void)` | — Colour-Icon-Fixup-Pass; kein CICON-Support | NO-COUNTERPART |
| `fix_long` | 576 | `static BOOL fix_long(LONG *plong)` | resource.rs:538 `reloc` (vereinfacht: kein Endian-Swap nötig, natives Format) | HAS |
| `fix_nptrs` | 640 | `static void fix_nptrs(WORD cnt, WORD type)` | resource.rs:438-491 (inline: `substruct_seen`+Typ-Dispatch in `relocate_blob_core`) | HAS |
| `fix_objects` | 606 | `static void fix_objects(void)` | resource.rs:438 `relocate_blob_core` (explizit "EmuTOS `fix_objects`, gemrslib.c:619" zitiert) | HAS |
| `fix_ptr` | 649 | `static BOOL fix_ptr(WORD type, WORD index)` | resource.rs:538 `reloc` | HAS |
| `fix_tedinfo_std` | 655 | `static void fix_tedinfo_std(void)` | resource.rs:462-471 (inline: G_TEXT/G_BOXTEXT/G_FTEXT/G_FBOXTEXT-Zweig in `relocate_blob_core`) | HAS |
| `fix_trindex` | 591 | `static void fix_trindex(void)` | resource.rs:526-533 `tree_root` (Trindex wird On-Demand gelesen, kein separater Fixup-Pass nötig) | HAS |
| `fixup_all_ciconblks` | 226 | `static void fixup_all_ciconblks(LONG num_blks, CICONBLK **ciconblkptr, CICON *cicondata)` | — Colour-Icon-Fixup; kein CICON-Support | NO-COUNTERPART |
| `fixup_colour_icons` | 185 | `static CICONBLK *fixup_colour_icons(LONG num_cicons, LONG mono_words, CICON *start)` | — Colour-Icon-Fixup; kein CICON-Support | NO-COUNTERPART |
| `free_cicon_buffers` | 515 | `static WORD free_cicon_buffers(RSHDR *hdr)` | — Colour-Icon-Puffer-Freigabe; kein CICON-Support | NO-COUNTERPART |
| `get_addr` | 112 | `static void *get_addr(UWORD rstype, UWORD rsindex)` | resource.rs:875/526 `rsrc_gaddr`/`tree_root` — nur Teilmenge der R_*-Selektoren (R_TREE/R_OBJECT/R_STRING/R_IMAGEDATA/R_FRSTR/R_FRIMG); R_TEDINFO/R_ICONBLK/R_BITBLK/R_OBSPEC/R_TEP*/R_IBP*/R_BIPDATA fehlen | HAS |
| `get_ciconblkptr` | 484 | `static CICONBLK **get_ciconblkptr(RSHDR *hdr)` | — Colour-Icon-Block-Zeiger; kein CICON-Support | NO-COUNTERPART |
| `get_sub` | 102 | `static void *get_sub(UWORD rsindex, UWORD offset, UWORD rsize)` | — indizierter Sub-Array-Lookup für R_TEDINFO/R_ICONBLK/R_BITBLK, die selbst nicht implementiert sind | NO-COUNTERPART |
| `rs_fixit` | 816 | `void rs_fixit(AESGLOBAL *pglobal)` | resource.rs:412/438 `relocate_blob`/`relocate_blob_core` | HAS |
| `rs_free` | 685 | `WORD rs_free(AESGLOBAL *pglobal)` | resource.rs:824 `rsrc_free` | HAS |
| `rs_gaddr` | 715 | `WORD rs_gaddr(AESGLOBAL *pglobal, UWORD rtype, UWORD rindex, void **rsaddr)` | resource.rs:875 `rsrc_gaddr` (Teilmenge, s. `get_addr`) | HAS |
| `rs_load` | 826 | `WORD rs_load(AESGLOBAL *pglobal, char *rsfname)` | resource.rs:657 `rsrc_load` | HAS |
| `rs_obfix` | 89 | `void rs_obfix(OBJECT *tree, WORD curob)` | resource.rs:986 `rsrc_obfix` | HAS |
| `rs_readit` | 751 | `static WORD rs_readit(AESGLOBAL *pglobal,UWORD fd)` | resource.rs:657 (inline: `rsrc_load`s `crate::bdos::fs::read_file`-Aufruf) | HAS |
| `rs_saddr` | 728 | `WORD rs_saddr(AESGLOBAL *pglobal, UWORD rtype, UWORD rindex, void *rsaddr)` | resource.rs:958 `rsrc_saddr` | HAS |
| `rs_sglobe` | 675 | `static void rs_sglobe(AESGLOBAL *pglobal)` | resource.rs:657-823 (inline: AESGLOBAL-Feld-Rückschreiben in `rsrc_load`) | HAS |
| `rs_str` | 854 | `char *rs_str(UWORD stnum)` | form.rs (inline: `crate::i18n::sys_string`/`err_string`, andere Mechanik als RSC-String-Index) | HAS |
| `transform_all_cicons` | 403 | `static void transform_all_cicons(LONG num_cicons, CICONBLK **ciconblkptr)` | — Colour-Icon-Transform; kein CICON-Support | NO-COUNTERPART |
| `transform_cicon` | 348 | `static void transform_cicon(WORD *src, WORD *dest, WORD w, WORD h, WORD planes)` | — Colour-Icon-Transform; kein CICON-Support | NO-COUNTERPART |
| `xlate_obj_array` | 865 | `void xlate_obj_array(OBJECT *obj_array, int nobj)` | — Big-Endian-Byteswap für OBJECT-Arrays; natives Format ist bereits Little-Endian, Konvertierung erfolgt einmalig host-seitig in `tools/mkrsc.py`, nicht zur Laufzeit | N/A |

## gemsclib.c  (3 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `sc_clear` | 74 | `WORD sc_clear(void)` | shell.rs:134 `scrp_clear` | HAS |
| `sc_read` | 40 | `WORD sc_read(char *pscrap)` | shell.rs:115 `scrp_read` | HAS |
| `sc_write` | 57 | `WORD sc_write(const char *pscrap)` | shell.rs:124 `scrp_write` | HAS |

## gemshlib.c  (24 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `aes_run_rom_program` | 570 | `LONG aes_run_rom_program(PRG_ENTRY *entry)` | — ROM-residentes Programm ausführen; Konzept nicht anwendbar (NewTOS lädt den Desktop von SD, nicht aus ROM) | NO-COUNTERPART |
| `findfile` | 406 | `static WORD findfile(char *pspec)` | shell.rs:274-311 (inline in `shel_find`, nur 2 von 4 Suchschritten) | HAS |
| `set_default_desktop` | 583 | `static void set_default_desktop(SHELL *psh)` | — feste `DEF_DESKTOP`/`DESKTOP_APP`-Zuweisung im SHELL-Struct; `shel_write`s SHW_NOEXEC-Zweig löscht nur cmd/tail, ohne eine Desktop-Pfad-Konstante zu setzen | NO-COUNTERPART |
| `set_drvdir` | 526 | `static void set_drvdir(char *path)` | — Laufwerk/Verzeichnis aus gespeichertem Pfad wiederherstellen; kein Restore-Mechanismus | NO-COUNTERPART |
| `sh_chdef` | 537 | `static void sh_chdef(SHELL *psh)` | — "Wechsle zu Default"-Desktop-Parameter berechnen | NO-COUNTERPART |
| `sh_chgrf` | 513 | `static void sh_chgrf(SHELL *psh)` | — Grafik-Modus-Wechsel-Bedarf zwischen altem/neuem `isgem` bestimmen (Orchestrator für `sh_toalpha`/`sh_tographic`) | NO-COUNTERPART |
| `sh_curdrvdir` | 109 | `static void sh_curdrvdir(char *ppath)` | resource.rs `set_app_dir`/`APP_DIR` (anderer Mechanismus: vom Kernel-Launcher aus dem Startpfad gesetzt statt via `dos_gdrv`/`dos_gdir` berechnet; gleiche Rolle: App-Verzeichnis für spätere Auflösung merken) | HAS |
| `sh_draw` | 269 | `static void sh_draw(const char *lcmd, BOOL clear)` | window.rs:293 `fallback_root` (explizit "`sh_draw(cmd, TRUE)`" zitiert; nur der `clear=TRUE`-Weißbild-Fall, nicht das volle Tree-Zeichnen) | HAS |
| `sh_envrn` | 327 | `void sh_envrn(char **ppath, const char *psrch)` | shell.rs:326 `shel_envrn` | HAS |
| `sh_find` | 474 | `WORD sh_find(char *pspec)` | shell.rs:274 `shel_find` | HAS |
| `sh_get` | 203 | `void sh_get(void *pbuffer, WORD len)` | shell.rs:219 `shel_get` | HAS |
| `sh_ldapp` | 592 | `static WORD sh_ldapp(SHELL *psh)` | shell.rs:419 `pending_launch` — nur die Anfrage-Seite; das tatsächliche `Pexec` ist laut shell.rs:11-15 Doku bewusst zurückgestellt (Shell-Loop/Desktop noch nicht verdrahtet) | HAS |
| `sh_main` | 692 | `void sh_main(BOOL isauto, BOOL isgem)` | — Shell-Haupt-Loop-Orchestrator; kein einzelnes Gegenstück in tos-api/aes | NO-COUNTERPART |
| `sh_name` | 305 | `char *sh_name(char *ppath)` | — Basename-Extraktion aus Pfad, nur von der fehlenden `sh_draw`/`sh_show`-Anzeige genutzt | NO-COUNTERPART |
| `sh_path` | 356 | `static char *sh_path(char *src, char *dest, char *pname)` | — Pfad-Zusammenbau für `findfile`s fehlende Suchschritte 1+4 | NO-COUNTERPART |
| `sh_put` | 213 | `void sh_put(const void *pdata, WORD len)` | shell.rs:236 `shel_put` | HAS |
| `sh_rdef` | 491 | `void sh_rdef(char *lpcmd, char *lpdir)` | shell.rs:377 `shel_rdef` | HAS |
| `sh_read` | 102 | `void sh_read(char *pcmd, char *ptail)` | shell.rs:157 `shel_read` | HAS |
| `sh_show` | 292 | `static void sh_show(const char *lcmd)` | — "wird geladen …"-Anzeige während `sh_draw`; `fallback_root` zeichnet nur eine leere weiße Fläche, kein Text | NO-COUNTERPART |
| `sh_toalpha` | 246 | `static void sh_toalpha(void)` | — Rückkehr in Alpha/Text-Modus vor TTP-Start (Orchestrator); `shel_write` ruft keine solche Orchestrierung auf | NO-COUNTERPART |
| `sh_tographic` | 223 | `void sh_tographic(void)` | — Umschalten in Grafik-Modus vor GEM-App-Start (Orchestrator); nicht reproduziert | NO-COUNTERPART |
| `sh_wdef` | 504 | `void sh_wdef(const char *lpcmd, const char *lpdir)` | shell.rs:392 `shel_wdef` | HAS |
| `sh_write` | 157 | `WORD sh_write(WORD doex, WORD isgem, WORD isover, const char *pcmd, const char *ptail)` | shell.rs:184 `shel_write` | HAS |
| `SHELL` | 83 | `} SHELL;` | shell.rs:36 `State`-Struct (Datenstruktur, kein Funktions-Gegenstück nötig) | HAS |

## gemsuper.c  (6 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `aestrace` | 61 | `static void aestrace(const char* message)` | — KDEBUG-Trace-Logging-Glue | N/A |
| `crysbind` | 77 | `static UWORD crysbind(WORD opcode, AESGLOBAL *pglobal, WORD control[], WORD int_in[], WORD int_out[], LONG addr_in[])` | mod.rs:166 `dispatch` (Opcode-Switch, Kommentar zitiert `crysbind`s `default:`-Arm direkt) | HAS |
| `super` | 454 | `LONG super(WORD cx, AESPB *pcrys_blk)` | mod.rs:166 `dispatch` (Top-Level-Trap-Einstiegspunkt) | HAS |
| `trapaes_debug_enter` | 482 | `void trapaes_debug_enter(void)` | — Debug-Trace-Hook um den AES-Trap | N/A |
| `trapaes_debug_exit` | 501 | `void trapaes_debug_exit(void)` | — Debug-Trace-Hook um den AES-Trap | N/A |
| `xif` | 428 | `static void xif(AESPB *pcrys_blk)` | — Trap#2-Handler-Einstiegs-Glue; ersetzt durch den `svc`-basierten Trap-Dispatcher | N/A |

## gemwmlib.c  (39 Symbole)

> Kontextnote: EmuTOS repräsentiert die Fenster-Gadgets (Titel/Schließen/
> Vergrößern/Rollbalken) als eigenen synthetischen OBJECT-Baum (`gl_awind`),
> den es mit `ob_find`/`ob_add` durchläuft. `window.rs` erreicht dasselbe
> Ergebnis rein geometrisch (`gadget_at`/`draw_frame`) ohne einen solchen
> Baum — die reinen Baum-Aufbau-Helfer sind daher N/A, die Endfunktionen
> (Hit-Test, Zeichnen) HAS.

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `ap_sendmsg` | 851 | `void ap_sendmsg(WORD ap_msg[], WORD type, AESPD *towhom,` | window.rs (inline: `msg[]`-Aufbau + `emit(pbuff, &msg)`) | HAS |
| `do_walk` | 286 | `static void do_walk(WORD wh, OBJECT *tree, WORD obj, WORD depth, GRECT *pc)` | window.rs:1554 (inline: `gadget_at`s direkte Geometrie statt Baum-Walk) | HAS |
| `draw_change` | 1058 | `static void draw_change(WORD w_handle, GRECT *pt)` | window.rs `draw_change`/Redraw-Pfad | HAS |
| `get_pwin` | 1391 | `static WINDOW *get_pwin(WORD w_handle)` | window.rs:713 `win()` | HAS |
| `w_adjust` | 271 | `static void w_adjust( WORD parent, WORD obj, WORD x, WORD y, WORD w, WORD h)` | — Positionierung innerhalb des synthetischen `gl_awind`-Baums; obsolet (direkte Geometrie) | N/A |
| `w_bldactive` | 479 | `void w_bldactive(WORD w_handle)` | window.rs:1554/819 `gadget_at`/`draw_frame` (ersetzen den ganzen `gl_awind`-Baum-Aufbau) | HAS |
| `w_bldhbar` | 441 | `static void w_bldhbar(UWORD kind, BOOL istop, WINDOW *pw, WORD x, WORD y, WORD w, WORD h)` | window.rs:985 `draw_hslider` | HAS |
| `w_bldvbar` | 403 | `static void w_bldvbar(UWORD kind, BOOL istop, WINDOW *pw, WORD x, WORD y, WORD w, WORD h)` | window.rs:917 `draw_vslider` | HAS |
| `w_cpwalk` | 363 | `static void w_cpwalk(WORD wh, WORD obj, WORD depth, BOOL usetrue)` | window.rs (inline: direkte Geometrie statt Baum-Walk) | HAS |
| `w_drawdesk` | 340 | `void w_drawdesk(GRECT *pc)` | window.rs `w_drawdesk`/Desktop-Hintergrund-Zeichnen | HAS |
| `w_getsize` | 254 | `void w_getsize(WORD which, WORD w_handle, GRECT *pt)` | window.rs:254 `w_getsize` | HAS |
| `w_getxptr` | 231 | `static GRECT *w_getxptr(WORD which, WORD w_handle)` | window.rs (inline: direkter Feldzugriff auf `Window.curr`/`.prev`) | HAS |
| `w_move` | 938 | `static BOOL w_move(WORD w_handle, WORD *pstop, GRECT *prc)` | window.rs (inline: `wind_set`-WF_CURRXYWH-Pfad / `drag_move`) | HAS |
| `w_mvfix` | 915 | `static BOOL w_mvfix(GRECT *ps, GRECT *pd)` | window.rs:1672-1673 (inline: Clamp-Logik in `drag_move`) | HAS |
| `w_nilit` | 172 | `static void w_nilit(WORD num, OBJECT olist[])` | — Null-Init des synthetischen `gl_awind`-Baums; obsolet | N/A |
| `w_obadd` | 186 | `static void w_obadd(OBJECT olist[], WORD parent, WORD child)` | — Verkettung im synthetischen `gl_awind`-Baum; obsolet | N/A |
| `w_owns` | 1230 | `static void w_owns(WINDOW *pwin, ORECT *po, GRECT *pt, GRECT *poutwds)` | window.rs `w_owns`/Rechteckliste | HAS |
| `w_redraw` | 887 | `static void w_redraw(WORD w_handle, GRECT *pt)` | window.rs:887 `w_redraw` | HAS |
| `w_redraw_desktop` | 1902 | `void w_redraw_desktop(GRECT *pt)` | window.rs:486/453 `redraw_clip`/`compose` | HAS |
| `w_setactive` | 321 | `void w_setactive(void)` | — synchronisiert `gl_awind`/Ctrl-Rect jede `ctlmgr`-Loop-Iteration; kein `ctlmgr`-Loop | NO-COUNTERPART |
| `w_setcolor` | 382 | `static void w_setcolor(WINDOW *pw, WORD gadget, BOOL istop)` | window.rs:819 (inline: `istop`-abhängiges Gadget-Kolorit in `draw_frame`) | HAS |
| `w_setsize` | 265 | `static void w_setsize(WORD which, WORD w_handle, GRECT *pt)` | window.rs (inline: `wind_set`-WF_CURRXYWH-Pfad) | HAS |
| `w_setup` | 205 | `static void w_setup(AESPD *ppd, WORD w_handle, WORD kind)` | window.rs:1103 (inline: Slot-Init in `wind_create`) | HAS |
| `w_top` | 315 | `static WORD w_top(void)` | window.rs:211 `get_top` | HAS |
| `w_union` | 869 | `static WORD w_union(ORECT *po, GRECT *pt)` | window.rs:498 `union_grect` | HAS |
| `w_update` | 1004 | `void w_update(WORD bottom, GRECT *pt, WORD top, BOOL moved)` | window.rs:1004 `w_update` | HAS |
| `wm_calc` | 1807 | `void wm_calc(WORD wtype, UWORD kind, WORD x, WORD y, WORD w, WORD h,` | window.rs:1466 `wind_calc` | HAS |
| `wm_close` | 1438 | `BOOL wm_close(WORD w_handle)` | window.rs:1157 `wind_close` | HAS |
| `wm_create` | 1369 | `WORD wm_create(WORD kind, GRECT *pt)` | window.rs:1103 `wind_create` | HAS |
| `wm_delete` | 1466 | `BOOL wm_delete(WORD w_handle)` | window.rs:1189 `wind_delete` | HAS |
| `wm_find` | 1767 | `WORD wm_find(WORD x, WORD y)` | window.rs:1455 `wind_find` | HAS |
| `wm_get` | 1505 | `BOOL wm_get(WORD w_handle, WORD w_field, WORD *poutwds, WORD *pinwds)` | window.rs:1208 `wind_get` | HAS |
| `wm_init` | 1247 | `void wm_init(void)` | window.rs:169 (inline: `TABLE`-Static-Null-Init) | HAS |
| `wm_mktop` | 1605 | `static void wm_mktop(WORD w_handle)` | window.rs:227 `top_window` | HAS |
| `wm_new` | 1875 | `void wm_new(void)` | window.rs:1479 `wind_new` | HAS |
| `wm_open` | 1410 | `BOOL wm_open(WORD w_handle, GRECT *pt)` | window.rs:1122 `wind_open` | HAS |
| `wm_set` | 1623 | `BOOL wm_set(WORD w_handle, WORD w_field, WORD *pinwds)` | window.rs:1321 `wind_set` | HAS |
| `wm_start` | 1332 | `void wm_start(void)` | window.rs:169/187 (inline: `TABLE`/`SAVE`-Static-Null-Init) | HAS |
| `wm_update` | 1778 | `void wm_update(WORD beg_update)` | — BEG_UPDATE/END_UPDATE-Bildschirm-Lock-Semaphor für konkurrierende AES-Prozesse; keine Konkurrenz vorhanden | NO-COUNTERPART |

## gemwrect.c  (6 Symbole)

> Die ORECT-"Owner-Rectangle"-Buchhaltung für Fenster-Verdeckung. NewTOS
> reimplementiert dieselbe Funktionalität algorithmisch anders (Rechteck-
> Subtraktion in ein festes Array statt verketteter ORECT-Freiliste), s.
> `window.rs:517-628` (`RLIST`/`compute_rlist`/`subtract`/`put`).

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `brkrct` | 101 | `static ORECT *brkrct(ORECT *new, ORECT *r, ORECT *p)` | window.rs:545 `subtract` (Rechteck-Aufspaltungs-Logik) | HAS |
| `get_orect` | 53 | `ORECT *get_orect(void)` | window.rs:536 `put` (Array-basierte Allokation statt ORECT-Freiliste) | HAS |
| `mkpiece` | 64 | `static ORECT *mkpiece(WORD tlrb, ORECT *new, ORECT *old)` | window.rs:545 (inline: Pro-Kante-Clip-Logik in `subtract`) | HAS |
| `mkrect` | 136 | `static void mkrect(OBJECT *tree, WORD wh)` | window.rs:583 `compute_rlist` | HAS |
| `newrect` | 165 | `void newrect(OBJECT *tree, WORD wh)` | window.rs:583 `compute_rlist` | HAS |
| `or_start` | 40 | `void or_start(void)` | window.rs:530 (inline: `RLIST`-Static-Null-Init) | HAS |

## gsx2.c  (1 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `gsx2` | 20 | `void gsx2(void)` | — roher VDI-Trap-Aufruf (Software-Trap-Indirektion); ersetzt durch direkte `RasterBackend`-Aufrufe ohne Trap-Umweg | N/A |

---
**Gesamt: 402 Symbole über 28 .c-Dateien.**

## Ergebnis

### Zähler gesamt

| Status | Anzahl |
|---|---|
| HAS | 265 |
| NO-COUNTERPART | 99 |
| N/A | 38 |
| **Gesamt** | **402** |

### Zähler pro Datei

| Datei | Symbole | HAS | NO-COUNTERPART | N/A |
|---|---|---|---|---|
| gemaplib.c | 6 | 6 | 0 | 0 |
| gemasync.c | 8 | 0 | 8 | 0 |
| gemctrl.c | 9 | 5 | 4 | 0 |
| gemdisp.c | 7 | 0 | 7 | 0 |
| gemevlib.c | 7 | 7 | 0 | 0 |
| gemflag.c | 4 | 0 | 4 | 0 |
| gemfmalt.c | 7 | 7 | 0 | 0 |
| gemfmlib.c | 10 | 7 | 2 | 1 |
| gemfslib.c | 17 | 13 | 4 | 0 |
| gemgraf.c | 30 | 24 | 2 | 4 |
| gemgrlib.c | 19 | 19 | 0 | 0 |
| gemgsxif.c | 44 | 25 | 0 | 19 |
| geminit.c | 22 | 7 | 11 | 4 |
| geminput.c | 26 | 5 | 21 | 0 |
| gemmnext.c | 17 | 17 | 0 | 0 |
| gemmnlib.c | 15 | 12 | 3 | 0 |
| gemobed.c | 12 | 10 | 2 | 0 |
| gemobjop.c | 3 | 3 | 0 | 0 |
| gemoblib.c | 23 | 19 | 3 | 1 |
| gempd.c | 7 | 2 | 5 | 0 |
| gemqueue.c | 2 | 2 | 0 | 0 |
| gemrslib.c | 28 | 17 | 10 | 1 |
| gemsclib.c | 3 | 3 | 0 | 0 |
| gemshlib.c | 24 | 13 | 11 | 0 |
| gemsuper.c | 6 | 2 | 0 | 4 |
| gemwmlib.c | 39 | 34 | 2 | 3 |
| gemwrect.c | 6 | 6 | 0 | 0 |
| gsx2.c | 1 | 0 | 0 | 1 |
| **Gesamt** | **402** | **265** | **99** | **38** |

### Die NO-COUNTERPART-Liste (99 Funktionen), nach Subsystem gruppiert

Jede Zeile: `EmuTOS file:line` — was die Funktion tut.

**A. Kooperativer AESPD/EVB-Scheduler (Multi-Prozess-Kern) — 8+7+4 = 19 Funktionen**

Der gesamte Mechanismus, mit dem EmuTOS mehrere AES-Prozesse (Apps + Accessories
+ interne Prozesse wie `ctlmgr`) gleichzeitig laufen lässt, parkt und weckt, hat
kein Gegenstück. NewTOS ist pro Trap synchron (ein `evnt_*`-Aufruf = ein
Poll-Loop im Aufrufer-Kontext, `event.rs`).

- gemasync.c:217 `acancel` — bricht ausstehende Event-Waits eines Prozesses ab
- gemasync.c:182 `apret` — liest das Ergebnis eines abgeschlossenen EVB und gibt es frei
- gemasync.c:64 `azombie` — markiert ein EVB als "fertig, Ergebnis wartet"
- gemasync.c:81 `evinsert` — fügt ein EVB in eine Warteliste ein
- gemasync.c:130 `iasync` — bewaffnet ein EVB für einen Event-Typ (Dispatch auf abutton/amouse/…)
- gemasync.c:117 `mwait` — parkt den aktuellen Prozess bis eine Event-Maske erfüllt ist
- gemasync.c:35 `signal` — weckt einen auf ein Event wartenden Prozess
- gemasync.c:96 `takeoff` — entfernt ein EVB aus der Event-Liste
- gemdisp.c:158 `chkkbd` — pollt die Tastatur, reiht ein Tastatur-Event in den Fork-Ring
- gemdisp.c:221 `disp` — Kern-Dispatcher: nimmt den nächsten Prozess von der Ready-Liste, kontext-wechselt
- gemdisp.c:74 `disp_act` — setzt einen Prozess auf die Ready-Liste
- gemdisp.c:101 `forker` — leert den Fork-Ring, ruft jede FPD-Routine auf
- gemdisp.c:52 `forkq` — reiht eine Funktion+Parameter in den Fork-Ring ein
- gemdisp.c:82 `mwait_act` — legt einen Prozess schlafen, wenn seine Event-Maske nicht erfüllt ist
- gemdisp.c:181 `schedule` — Haupt-Scheduling-Schleife: pollt Tastatur, bewegt Prozesse auf die Ready-Liste
- gemflag.c:89 `amutex` — bewaffnet ein EVB für einen Mutex
- gemflag.c:71 `tak_flag` — versucht ein Semaphor zu nehmen
- gemflag.c:32 `tchange` — Tick-Broadcast, weckt Timer-wartende Prozesse
- gemflag.c:105 `unsync` — gibt ein Semaphor frei

**B. Low-Level-Input-Treiber-/Ownership-Schicht — 21 Funktionen (geminput.c)**

Bewaffnet EVBs für Button/Maus/Tastatur, routet Events an konkrete AESPDs, und
verwaltet, welcher Prozess gerade die Maus/Kontroll-Rechtecke besitzt. `event.rs`
liest stattdessen direkt vom `host()` und pollt inline — die
Hochsprachen-Funktionen (`evnt_button` etc.) haben ein Gegenstück, diese interne
Post-/Ownership-Maschinerie nicht.

- geminput.c:545 `abutton` — EVB für Button-Event bewaffnen
- geminput.c:496 `adelay` — EVB für Timer-Delay bewaffnen
- geminput.c:486 `akbin` — EVB für Tastatur-Input bewaffnen
- geminput.c:568 `amouse` — EVB für Maus-Rechteck-Event bewaffnen
- geminput.c:328 `bchange` — Fork-Routine bei Button-Wechsel, weckt Warter
- geminput.c:87 `chk_ctrl` — prüft Zugehörigkeit zum Control-Rechteck
- geminput.c:316 `chkown` — bestimmt den aktuellen Maus-Owner-Prozess
- geminput.c:279 `evremove` — entfernt/schließt ein EVB ab
- geminput.c:272 `fq` — leert die Tastatur-Warteschlange
- geminput.c:200 `get_ctrl` — liest das Control-Rechteck
- geminput.c:209 `get_mown` — liest den Maus-Owner
- geminput.c:427 `inorout` — EVB-Rechteck-Enter/Leave-Test
- geminput.c:288 `kchange` — Fork-Routine bei Tastatur-Wechsel, weckt Warter
- geminput.c:360 `mchange` — Fork-Routine bei Mausbewegung, weckt Warter
- geminput.c:241 `nq` — Taste in Ringpuffer einreihen
- geminput.c:463 `post_button` — Button-Event an ein konkretes AESPD posten
- geminput.c:299 `post_keybd` — Taste an eine CDA posten
- geminput.c:446 `post_mouse` — Mausposition an ein konkretes AESPD posten
- geminput.c:189 `set_ctrl` — Control-Rechteck setzen
- geminput.c:220 `set_mown` — Maus-Owner setzen
- geminput.c:395 `wheel_change` — Mausrad-Event-Fork-Routine (kein Mausrad-Support überhaupt)

**C. Maus-/Kontroll-Rechteck-Ownership-Arbitrierung (gemctrl.c) — 4 Funktionen**

- gemctrl.c:392 `ct_chgown` — überträgt Ownership eines Rechtecks+Prozesses
- gemctrl.c:400 `ct_mouse` — grabt/gibt die Maus für den Control-Manager frei
- gemctrl.c:380 `hctl_msg` — fängt WM_REDRAW ab, leitet an `w_redraw_desktop` weiter
- gemctrl.c:146 `perform_untop` — sendet WM_UNTOPPED ans vorherige Top-Fenster (grep: kein `WM_UNTOPPED` im ganzen aes/)

**D. Desk-Accessories (.ACC) — komplett fehlend — 13 Funktionen (geminit.c + gemmnlib.c)**

Kein einziger Baustein für das Laden, Starten oder Verwalten von Desk-Accessories
existiert. `menu_register` (Item-Registrierung) ist zwar HAS, aber die
Verdrahtung ins Desk-Menü/Klick-Routing ist explizit als Folgearbeit vermerkt.

- geminit.c:264 `count_accs` — zählt `.ACC`-Dateien
- geminit.c:296 `free_accs` — gibt Accessory-Speicher frei
- geminit.c:309 `load_accs` — lädt alle gefundenen Accessories
- geminit.c:236 `load_one_acc` — lädt+startet ein einzelnes Accessory
- geminit.c:599 `wait_for_accs` — wartet auf AC_CLOSE-Quittung aller Accessories
- geminit.c:215 `iprocess` — erzeugt einen internen Prozess für eine Routine (u.a. für Accessories)
- geminit.c:654 `run_accs_and_desktop` — startet Accessories + Desktop
- gempd.c:110 `pstart` — erzeugt einen neuen Prozesskontext (Basis für `load_one_acc`/`iprocess`)
- gemmnlib.c:766 `build_menuid_lookup` — Item→Accessory-Owner-Reverse-Lookup
- gemmnlib.c:838 `mn_getownid` — löst Accessory-Owner+lokale-Id auf
- gemmnlib.c:740 `mn_cleanup` — gibt Accessory-Menüslots frei
- geminit.c:753 `gem_main` — Top-Level-AES-Orchestrator (u.a. Accessory-Start)
- gemshlib.c:570 `aes_run_rom_program` — ROM-residentes Programm ausführen

**E. EMUDESK.INF-Parsing / Shell-Hauptschleife (geminit.c + gemshlib.c) — 8 Funktionen**

Gehören zum Desktop-Shell-Prozess, der (falls überhaupt) außerhalb von
`tos-api/src/aes` liegt.

- geminit.c:343 `process_inf1` — parst EMUDESK.INF Teil 1 (Auflösung/Hintergrund)
- geminit.c:407 `process_inf2` — parst EMUDESK.INF Teil 2
- gemshlib.c:692 `sh_main` — Shell-Hauptschleife (lädt/startet Programme in einer Schleife)
- gemshlib.c:583 `set_default_desktop` — setzt feste Desktop-Programm-Konstante
- gemshlib.c:526 `set_drvdir` — stellt Laufwerk/Verzeichnis aus gespeichertem Pfad wieder her
- gemshlib.c:537 `sh_chdef` — "wechsle zu Default"-Parameter berechnen
- gemshlib.c:513 `sh_chgrf` — Grafik-Modus-Wechsel-Bedarf bestimmen
- gemshlib.c:246 `sh_toalpha` / gemshlib.c:223 `sh_tographic` — Text/Grafik-Modus-Umschalt-Orchestrierung vor TTP/GEM-Start

**F. Colour-Icons (CICON) — komplett fehlend — 10 Funktionen (gemrslib.c) + 2 (gemgraf.c)**

Kein `CICON`/`CICONBLK` taucht irgendwo in `tos-api/src/aes` auf (grep-bestätigt).
Icons sind nur als Mono-`ICONBLK` implementiert.

- gemgraf.c:680 `dither_mask` — dithert eine Colour-Icon-Maske
- gemgraf.c:642 `gr_colourblit` — blittet ein transformiertes Colour-Icon
- gemrslib.c:267 `best_match` — wählt das planzahl-passendste CICON
- gemrslib.c:296 `expand_cicondata` — expandiert Colour-Icon-Rohdaten
- gemrslib.c:549 `fix_cicons` — Colour-Icon-Fixup-Pass
- gemrslib.c:226 `fixup_all_ciconblks` — Fixup aller CICONBLKs
- gemrslib.c:185 `fixup_colour_icons` — Fixup-Einstieg für Colour-Icons
- gemrslib.c:515 `free_cicon_buffers` — gibt Colour-Icon-Puffer frei
- gemrslib.c:484 `get_ciconblkptr` — liefert den CICONBLK-Zeiger
- gemrslib.c:102 `get_sub` — indizierter Sub-Array-Lookup (auch für R_TEDINFO/R_ICONBLK/R_BITBLK, die selbst fehlen)
- gemrslib.c:403 `transform_all_cicons` — Transform aller Colour-Icons
- gemrslib.c:348 `transform_cicon` — Transform eines einzelnen Colour-Icons

**G. Sonstige Einzel-Lücken — 24 Funktionen**

- gemfmlib.c:416 `eralert` — kritischer GEMDOS/BIOS-Fehler-Alert mit Laufwerksbuchstaben-Substitution
- gemfmlib.c:389 `fm_show` — generischer Resource-String-Nummer+%d-Format-Alert-Builder
- gemfslib.c:179 `centre_title` — File-Selector-Titel zentrieren (fsel_exinput ignoriert den Titel-Parameter)
- gemfslib.c:249 `fs_add` — sortiertes Einfügen in die Dateiliste (Listing ist unsortiert)
- gemfslib.c:243 `fs_comp` — Case-insensitiver Namensvergleich (nur für die fehlende Sortierung)
- gemfslib.c:424 `fs_nscroll` — Scrollbar-Thumb-Drag / Track-Klick-Paging im File-Selector
- gemobed.c:316 `ob_stfn` — Template-Segment-Grenzen (Pfeiltasten-Navigation ist nicht segmentbewusst)
- gemobed.c:141 `scan_to_end` — String-Scan-Hilfsfunktion für das fehlende Segment-Handling
- gemoblib.c:411 `call_usercode` — ruft eine app-definierte G_USERDEF-Zeichnen-Callback auf
- gemoblib.c:433 `ob_user` — G_USERDEF-Dispatcher (kein G_USERDEF-Support überhaupt)
- gemoblib.c:455 `is_dashes` — erkennt "------"-Menü-Separator-Einträge
- gempd.c:134 `insert_process` — verkettete Run-Queue-Einfügung
- gempd.c:77 `p_nameit` — setzt den Prozessnamen (Feld existiert, wird nie beschrieben → `appl_find` findet nie etwas per Namen)
- gempd.c:90 `p_setappdir` — setzt das App-Arbeitsverzeichnis eines Prozesses
- gempd.c:32 `pd_index` — Index-basierter AESPD-Accessor
- gemshlib.c:305 `sh_name` — Basename-Extraktion aus Pfad
- gemshlib.c:356 `sh_path` — Pfad-Zusammenbau für die fehlenden `findfile`-Suchschritte
- gemshlib.c:292 `sh_show` — "wird geladen …"-Anzeige während `sh_draw`
- gemwmlib.c:321 `w_setactive` — synchronisiert `gl_awind`/Ctrl-Rect jede Control-Manager-Loop-Iteration
- gemwmlib.c:1778 `wm_update` — BEG_UPDATE/END_UPDATE-Bildschirm-Lock-Semaphor
- gemqueue.c: *(keine — beide HAS)*

### Auffällige Muster

1. **Der komplette kooperative Multi-Prozess-Kern fehlt** (Gruppe A+B+C, 44
   Funktionen über gemasync.c, gemdisp.c, gemflag.c, geminput.c, gemctrl.c).
   Das ist kein Zufalls-Lückenfeld, sondern eine bewusste Architekturentscheidung
   (`event.rs`-Kopfkommentar: "Bring-up model (single application, no AES
   scheduler)"). Die **Außenwirkung** (was ein `evnt_*`-Aufruf zurückgibt) ist
   über Poll-Loops nachgebildet und HAS; die **interne Nebenläufigkeits-Maschinerie**
   (mehrere AES-Prozesse gleichzeitig parken/wecken/canceln) ist es nicht.
   Das trifft heute niemanden, weil NewTOS ohnehin nur eine AES-App gleichzeitig
   laufen lässt — wird aber zur harten Wand, sobald ein zweiter gleichzeitiger
   AES-Client (Accessory, Multitasking) verlangt wird.

2. **Desk-Accessories (.ACC) sind komplett unimplementiert** (Gruppe D, 13
   Funktionen). `menu_register` täuscht auf den ersten Blick Support vor (die
   Registrierung selbst funktioniert), aber ohne Laden/Starten/Owner-Routing
   ist das nur ein Namensplatzhalter — laut shell.rs/menu.rs-Dokumentation
   selbst als Folgearbeit vermerkt.

3. **Colour-Icons (CICON) fehlen komplett** (Gruppe F, 12 Funktionen). Nur
   Mono-`ICONBLK` wird gezeichnet; jede Ressource mit Colour-Icons würde beim
   Farbicon-Pfad nichts anzeigen (auf Mono zurückfallen, falls die Ressource
   das anbietet) oder — falls nur Colour-Icons vorhanden sind — gar nichts.

4. **`p_nameit` fehlt, `appl_find` ist dadurch faktisch nie erfolgreich.**
   Das `Aespd.name`-Feld existiert (`pd.rs:161`), wird aber nirgendwo
   beschrieben — `appl_find` (HAS, opcode-technisch korrekt implementiert)
   sucht folglich immer gegen ausschließlich Leerzeichen-gefüllte Namen und
   kann kein Programm je per Namen finden. Eine **HAS-Klassifizierung mit
   einer stillen NO-COUNTERPART-Abhängigkeit** — wichtig für den Entwickler,
   weil der Opcode "funktioniert", aber nie das erwartete Ergebnis liefert.

5. **`rsrc_gaddr`/`rsrc_saddr` decken nur 6 von 17 `R_*`-Selektoren ab**
   (R_TREE/R_OBJECT/R_STRING/R_IMAGEDATA/R_FRSTR/R_FRIMG; R_TEDINFO/
   R_ICONBLK/R_BITBLK/R_OBSPEC/R_TEPTEXT/R_TEPTMPLT/R_TEPVALID/R_IBPMASK/
   R_IBPDATA/R_IBPTEXT/R_BIPDATA fehlen). Das ist eine HAS-Funktion mit
   dokumentierter Teilabdeckung, kein reiner Präsenz-Fund — als Hinweis für
   ein mögliches Folge-Verhaltens-Audit vermerkt.

6. **Die GSX/VDI-Transport-Schicht (gemgsxif.c, gsx2.c, Teile von gemgraf.c)**
   ist überwiegend N/A (19 von 44 in gemgsxif.c, 4 von 30 in gemgraf.c, plus
   gsx2.c komplett) — das ist die erwartete, dokumentierte Konsequenz der
   direkten `RasterBackend`-Architektur ohne interne VDI-Workstation-
   Indirektion, kein Befund.

7. **Das Fenster-Gadget-System ist geometrisch statt baum-basiert** — EmuTOS
   baut einen synthetischen OBJECT-Baum (`gl_awind`) für Titel/Rollbalken/
   Schließer und läuft ihn mit `ob_find` ab; `window.rs` testet stattdessen
   direkt gegen Rechteck-Geometrie (`gadget_at`). Die reinen Baum-Aufbau-
   Helfer (`w_adjust`/`w_nilit`/`w_obadd`) sind daher N/A — die Endergebnisse
   (Hit-Test, Zeichnen, Drag/Resize) sind vollständig HAS. Kein Befund, aber
   der Grund für einen ungewöhnlich hohen N/A-Anteil in gemwmlib.c.

### Out-of-scope observations

- form.rs's `form_do`/`run_form` deckt laut eigener Doku nur Buttons+Enter+
  Klick ab; volle `objc_edit`-Tastaturnavigation (Tab durch mehrere Felder,
  Pfeiltasten zwischen Feldern) ist als spätere Batch vermerkt — ein
  Verhaltens-Detail, keine Präsenz-Lücke, daher hier nur notiert statt separat
  auditiert.
- `shel_find`/`findfile` deckt nur 2 von 4 EmuTOS-Suchschritten ab
  (Anwendungsverzeichnis via `p_appdir` und `PATH=`-Umgebungsvariable fehlen);
  als HAS mit dokumentierter Teilabdeckung eingestuft, nicht als eigener Fund.
