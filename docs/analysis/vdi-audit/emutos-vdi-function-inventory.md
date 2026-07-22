# EmuTOS-VDI Funktions-Inventar — Vollständigkeits-Check (EmuTOS-seitig)

> Deterministisch via `ctags -x` aus references/emutos/vdi/*.c extrahiert.
> Ziel: JEDE EmuTOS-VDI-Funktion (public + internal static) auflisten und
> "haben wir ein Gegenstück?" beantworten. Status vom Researcher zu füllen:
> HAS / NO-COUNTERPART / N/A (m68k-asm-blit/GSX-Transport).
> Hinweis: die .S-Assembler-Dateien (vdi_blit/vdi_tblit/vdi_asm/endvdi) sind
> hier NICHT erfasst (ctags -x nur .c) — separat als überwiegend N/A behandeln.

## vdi_bezier.c  (9 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `draw_segs` | 172 | `draw_segs(Vwk * vwk, WORD nr_vertices, Point * point, WORD mode)` | — gesamte Datei `#if HAVE_BEZIER` (`vdi_defs.h:18`: `#define HAVE_BEZIER 0` — „entirely untested", von EmuTOS selbst deaktiviert) | N/A |
| `gen_segs` | 77 | `gen_segs(WORD *const array, WORD *px, const int bez_qual,` | — s.o., HAVE_BEZIER=0 | N/A |
| `IS_BEZ` | 47 | `#define IS_BEZ(f) ((f&1)!=0)` | — s.o. | N/A |
| `IS_JUMP` | 48 | `#define IS_JUMP(f) ((f&2)!=0)` | — s.o. | N/A |
| `labs` | 56 | `labs(LONG x)` | — s.o. | N/A |
| `v_bez` | 229 | `v_bez(Vwk * vwk, Point * ptsget, int nr_ptsin)` | — s.o. | N/A |
| `v_bez_control` | 584 | `v_bez_control(Vwk * vwk)` | — s.o. | N/A |
| `v_bez_fill` | 331 | `v_bez_fill(Vwk * vwk, Point * ptsget, int nr_ptsin)` | — s.o. | N/A |
| `v_bez_qual` | 605 | `v_bez_qual(Vwk * vwk)` | — s.o. | N/A |

## vdi_col.c  (22 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `adjust_mono_values` | 491 | `static WORD adjust_mono_values(WORD colnum,WORD *rgb)` | — Atari ST/STe Mono-Screen-Hardwarequirk (Palette-Reg-0-Invert-Bit); Backend ist Truecolor, kein solches Register | N/A |
| `adjust_tt_colnum` | 279 | `static WORD adjust_tt_colnum(WORD colnum)` | — CONF_WITH_TT_SHIFTER (Atari-TT-Hardware Bank-Switching), nicht anwendbar | N/A |
| `fivebits2vdi` | 804 | `static WORD fivebits2vdi(UWORD col)` | — CONF_WITH_VDI_16BIT (5-Bit-DAC-Konvertierung), Backend nimmt RGB direkt | N/A |
| `get_tt_hyper_mono` | 372 | `static void get_tt_hyper_mono(WORD colnum,WORD *retval)` | — CONF_WITH_TT_SHIFTER Hyper-Mono-Hardwarequirk | N/A |
| `init_colors` | 708 | `void init_colors(void)` | `kernel/src/vdi_backend.rs:26,85` `DEFAULT_PALETTE` (außerhalb tos-api/vdi, aber die dokumentierte Gegenstelle — `req_col.rs:15-19` verweist explizit darauf) | HAS |
| `query_tt_color` | 403 | `static void query_tt_color(WORD colnum,WORD *retval)` | — CONF_WITH_TT_SHIFTER | N/A |
| `set_color` | 526 | `static void set_color(WORD colnum, WORD *rgb)` | `tos-api/src/vdi/attributes.rs:226` `vs_color()` (ruft `backend.set_color()`) | HAS |
| `set_color16` | 601 | `void set_color16(Vwk *vwk, WORD colnum, WORD *rgb)` | — CONF_WITH_VDI_16BIT Truecolor-Pseudopalette, nicht benötigt (Backend ist bereits Truecolor) | N/A |
| `set_tt_color` | 304 | `static void set_tt_color(WORD colnum, WORD *rgb)` | — CONF_WITH_TT_SHIFTER | N/A |
| `st2vdi` | 214 | `#define st2vdi(col) st2vdi_lookup_table[(col)&0x07]` | — ST-Shifter-Hardwarekonvertierung, Backend nimmt RGB direkt | N/A |
| `ste2vdi` | 238 | `#define ste2vdi(col) ste2vdi_lookup_table[(col)&0x0f]` | — CONF_WITH_STE_SHIFTER | N/A |
| `tt2vdi` | 264 | `#define tt2vdi(col) tt2vdi_lookup_table[(col)&0x0f]` | — CONF_WITH_TT_SHIFTER | N/A |
| `vdi_vq_color` | 845 | `void vdi_vq_color(Vwk *vwk)` | `tos-api/src/vdi/inquiry.rs:75` `vq_color()` | HAS |
| `vdi_vs_color` | 656 | `void vdi_vs_color(Vwk *vwk)` | `tos-api/src/vdi/attributes.rs:226` `vs_color()` | HAS |
| `vdi2fivebits` | 588 | `static UWORD vdi2fivebits(WORD col)` | — CONF_WITH_VDI_16BIT | N/A |
| `vdi2st` | 204 | `static int vdi2st(int col)` | — ST-Shifter-Hardwarekonvertierung | N/A |
| `vdi2ste` | 225 | `static int vdi2ste(int col)` | — CONF_WITH_STE_SHIFTER | N/A |
| `vdi2tt` | 254 | `static int vdi2tt(int col)` | — CONF_WITH_TT_SHIFTER | N/A |
| `vdi2videl` | 457 | `static LONG vdi2videl(WORD col)` | — CONF_WITH_VIDEL (Falcon-VIDEL-DAC) | N/A |
| `videl2vdi` | 464 | `static WORD videl2vdi(LONG col)` | — CONF_WITH_VIDEL | N/A |
| `vq_color16` | 813 | `static void vq_color16(Vwk *vwk)` | — CONF_WITH_VDI_16BIT | N/A |
| `vs_color16` | 620 | `static void vs_color16(Vwk *vwk)` | — CONF_WITH_VDI_16BIT | N/A |

## vdi_control.c  (13 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `build_vwk_chain` | 390 | `static void build_vwk_chain(void)` | — internes Bookkeeping für EmuTOS' verkettete Vwk-Allokation; NewTOS nutzt eine statische Slot-Tabelle (`vwk.rs`), braucht keine Kette | N/A |
| `get_vwk_by_handle` | 178 | `Vwk * get_vwk_by_handle(WORD handle)` | `tos-api/src/vdi/vwk.rs:220` `resolve()` | HAS |
| `init_wk` | 269 | `static void init_wk(Vwk * vwk)` | `tos-api/src/vdi/workstation.rs:70` `init_wk()` | HAS |
| `update_rez_dependent` | 193 | `void update_rez_dependent(void)` | `tos-api/src/vdi/workstation.rs:70,156` — `init_wk()`/`vq_extnd()` rechnen DEV_TAB/INQ_TAB-Felder pro Aufruf frisch aus `backend.width()/height()/num_colors()` statt gecacht bei Rez-Wechsel | HAS |
| `validate_color_index` | 224 | `WORD validate_color_index(WORD colnum)` | `tos-api/src/vdi/vwk.rs:146` `validate_color_index()` | HAS |
| `vdi_v_clrwk` | 599 | `void vdi_v_clrwk(Vwk * vwk)` | `tos-api/src/vdi/workstation.rs:213` `v_clrwk()` | HAS |
| `vdi_v_clsvwk` | 462 | `void vdi_v_clsvwk(Vwk * vwk)` | `tos-api/src/vdi/workstation.rs:206` `v_clsvwk()` | HAS |
| `vdi_v_clswk` | 573 | `void vdi_v_clswk(Vwk * vwk)` | `tos-api/src/vdi/mod.rs:227-231` (Opcode 2 → `v_clsvwk()`, mit `workstation::v_clsvwk` zusammengelegt; das physische-Close-spezifische Verhalten — alle virt. WS schließen, `timer_exit`/`vdimouse_exit`/`esc_exit` — fehlt, s. dort) | HAS |
| `vdi_v_opnvwk` | 407 | `void vdi_v_opnvwk(Vwk * vwk)` | `tos-api/src/vdi/workstation.rs:140` `v_opnvwk()` | HAS |
| `vdi_v_opnwk` | 496 | `void vdi_v_opnwk(Vwk * vwk)` | `tos-api/src/vdi/workstation.rs:197` `v_opnwk()` | HAS |
| `vdi_vq_extnd` | 626 | `void vdi_vq_extnd(Vwk * vwk)` | `tos-api/src/vdi/workstation.rs:156` `vq_extnd()` | HAS |
| `vdi_vs_clip` | 235 | `void vdi_vs_clip(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:49` `vs_clip()` | HAS |
| `vdi_vswr_mode` | 256 | `void vdi_vswr_mode(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:35` `vswr_mode()` | HAS |

## vdi_esc.c  (25 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `bios_conws` | 39 | `static void bios_conws(char *string)` | — internes VT52-Escape-Relay über die BIOS-Konsole; NewTOS' Alpha-Layer schreibt direkt auf den `RasterBackend` (`escape.rs`) | N/A |
| `crawio` | 29 | `static LONG crawio(WORD ch)` | — internes Relay über den BIOS-Crawio-Trap für escfn12; ersetzt durch direktes `put_cell` | N/A |
| `esc_exit` | 365 | `void esc_exit(Vwk * vwk)` | — kein Aufruf beim Workstation-Close, der in den Alpha/Konsolen-Modus zurückschaltet (Clear+Home) | NO-COUNTERPART |
| `esc_init` | 356 | `void esc_init(Vwk * vwk)` | — kein Aufruf bei `v_opnwk`, der den Alpha-Cursor beim Eintritt in den Grafikmodus versteckt | NO-COUNTERPART |
| `escfn0` | 49 | `static void escfn0(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:141` `_ => {}` (impliziter No-op-Zweig deckt Subfunktion 0 mit ab) | HAS |
| `escfn1` | 62 | `static void escfn1(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:63-68` (Sub-op 1, `vq_chcells`) | HAS |
| `escfn10` | 156 | `static void escfn10(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:93-97` (Sub-op 10, `v_eeol`) | HAS |
| `escfn11` | 172 | `static void escfn11(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:98-102` (Sub-op 11, `vs_curaddress`) | HAS |
| `escfn12` | 196 | `static void escfn12(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:103-118` (Sub-op 12, `v_curtext`) | HAS |
| `escfn13` | 213 | `static void escfn13(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:119` (Sub-op 13, `v_rvon`) | HAS |
| `escfn14` | 222 | `static void escfn14(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:120` (Sub-op 14, `v_rvoff`) | HAS |
| `escfn15` | 231 | `static void escfn15(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:121-126` (Sub-op 15, `vq_curaddress`) | HAS |
| `escfn16` | 248 | `static void escfn16(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:127-133` (Sub-op 16, `vq_tabstatus`) | HAS |
| `escfn17` | 260 | `static void escfn17(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:134` (Sub-op 17, `v_hardcopy` — dokumentierter No-op statt `Scrdmp()`) | HAS |
| `escfn18` | 274 | `static void escfn18(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:135-137` (Sub-op 18 → `mouse::force_show`) | HAS |
| `escfn19` | 284 | `static void escfn19(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:138-140` (Sub-op 19 → `mouse::v_hide_c`) | HAS |
| `escfn2` | 81 | `static void escfn2(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:69` (Sub-op 2, `v_exit_cur`) | HAS |
| `escfn3` | 93 | `static void escfn3(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:70-76` (Sub-op 3, `v_enter_cur`) | HAS |
| `escfn4` | 102 | `static void escfn4(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:77` (Sub-op 4, Cursor hoch) | HAS |
| `escfn5` | 111 | `static void escfn5(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:78` (Sub-op 5, Cursor runter) | HAS |
| `escfn6` | 120 | `static void escfn6(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:79` (Sub-op 6, Cursor rechts) | HAS |
| `escfn7` | 129 | `static void escfn7(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:80` (Sub-op 7, Cursor links) | HAS |
| `escfn8` | 138 | `static void escfn8(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:81-84` (Sub-op 8, Cursor Home) | HAS |
| `escfn9` | 147 | `static void escfn9(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:85-92` (Sub-op 9, `v_eeos`) | HAS |
| `vdi_v_escape` | 334 | `void vdi_v_escape(Vwk * vwk)` | `tos-api/src/vdi/escape.rs:60` `v_escape()` | HAS |

## vdi_fill.c  (30 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `ABS` | 31 | `#define ABS(v) ((v) & 0x7FFF) /* strips DOWN_FLAG if present */` | — internes Bit-Encoding für EmuTOS' spezifische Seed-Queue (DOWN_FLAG); `contour.rs` nutzt eine andere Datenstruktur (Stack + Visited-Bitmap) | N/A |
| `bub_sort` | 394 | `bub_sort (WORD * buf, WORD count)` | `tos-api/src/vdi/primitives.rs:159-168` (Insertion-Sort der Scanline-Kreuzungen inline in `fill_polygon()`) | HAS |
| `clc_flit` | 434 | `void clc_flit(const VwkAttrib *attr, const VwkClip *clipper, const Point *point, WORD vectors, WORD start, WORD end)` | `tos-api/src/vdi/primitives.rs:123` `fill_polygon()` (Scanline-Fill, andere Algorithmusvariante: even-odd statt Queue+draw_rect_common) | HAS |
| `clipbox` | 252 | `static BOOL clipbox(const VwkClip *clip, Rect *rect)` | `tos-api/src/vdi/backend.rs:284` `ClipRect::clamp_to()` (generisches Clip-Rect-∩, von jeder Zeichenprimitive genutzt) | HAS |
| `contourfill` | 948 | `void contourfill(const VwkAttrib * attr, const VwkClip *clip)` | `tos-api/src/vdi/contour.rs:135` `v_contourfill()` (nur über den VDI-Opcode erreichbar, nicht über Line-A) | HAS |
| `crunch_queue` | 875 | `static void crunch_queue(void)` | — internes Queue-Kompaktieren für EmuTOS' feste Seed-Queue; `contour.rs`s Stack+Visited-Bitmap braucht keine Kompaktierung | N/A |
| `end_pts` | 836 | `static WORD end_pts(const VwkClip *clip, WORD x, WORD y, WORD *xleftout, WORD *xrightout)` | `tos-api/src/vdi/contour.rs:207-214` (Span-Grenzen-Suche inline in der while-Schleife) | HAS |
| `end_pts16` | 746 | `static WORD end_pts16(const VwkClip *clip, WORD x, WORD y, WORD *xleftout, WORD *xrightout)` | — CONF_WITH_VDI_16BIT Truecolor-Variante | N/A |
| `get_color` | 637 | `get_color (UWORD mask, UWORD * addr)` | — internes Bitplane-Pixel-Decode; ersetzt durch `RasterBackend::read_pixel()` | N/A |
| `get_pix` | 1129 | `get_pix(void)` | — Line-A-Einstiegspunkt, architektonisch ausgeschlossen (ADR: kein Line-A-Portierung) | N/A |
| `get_seed` | 888 | `static WORD get_seed(const VwkAttrib *attr, const VwkClip *clip,` | `tos-api/src/vdi/contour.rs:199-244` (Seed-Erweiterung inline in der Hauptschleife von `v_contourfill`) | HAS |
| `no_abort` | 1076 | `static WORD no_abort(void)` | `tos-api/src/vdi/contour.rs` — `v_contourfill` hat gar keinen Abort-Callback, verhält sich also immer wie mit `no_abort` installiert | HAS |
| `pixelread` | 671 | `pixelread(const WORD x, const WORD y)` | `tos-api/src/vdi/backend.rs:104` `RasterBackend::read_pixel()` | HAS |
| `polygon` | 560 | `polygon(Vwk * vwk, Point * ptsin, int count)` | `tos-api/src/vdi/primitives.rs:184` `v_fillarea()` (Fill + Perimeter zusammengefasst) | HAS |
| `put_pix` | 1158 | `put_pix(void)` | — Line-A-Einstiegspunkt, architektonisch ausgeschlossen | N/A |
| `search_to_left` | 798 | `search_to_left (const VwkClip * clip, WORD x, UWORD mask, const UWORD search_col, UWORD * addr)` | — internes Bitplane-Scan; ersetzt durch direkten `backend.read_pixel()`-Scan in `contour.rs` | N/A |
| `search_to_left16` | 722 | `static UWORD search_to_left16(const VwkClip *clip, WORD x, const UWORD search_col, UWORD *addr)` | — CONF_WITH_VDI_16BIT | N/A |
| `search_to_right` | 773 | `search_to_right (const VwkClip * clip, WORD x, UWORD mask, const UWORD search_col, UWORD * addr)` | — internes Bitplane-Scan | N/A |
| `search_to_right16` | 698 | `static UWORD search_to_right16(const VwkClip *clip, WORD x, const UWORD search_col, UWORD *addr)` | — CONF_WITH_VDI_16BIT | N/A |
| `st_fl_ptr` | 339 | `st_fl_ptr(Vwk * vwk)` | `tos-api/src/vdi/fill_patterns.rs:82` `select()` | HAS |
| `vdi_v_contourfill` | 1084 | `void vdi_v_contourfill(Vwk * vwk)` | `tos-api/src/vdi/contour.rs:135` `v_contourfill()` | HAS |
| `vdi_v_fillarea` | 617 | `void vdi_v_fillarea(Vwk * vwk)` | `tos-api/src/vdi/primitives.rs:184` `v_fillarea()` | HAS |
| `vdi_v_get_pixel` | 1095 | `void vdi_v_get_pixel(Vwk * vwk)` | `tos-api/src/vdi/inquiry.rs:112` `v_get_pixel()` | HAS |
| `vdi_vqf_attributes` | 321 | `void vdi_vqf_attributes(Vwk * vwk)` | `tos-api/src/vdi/inquiry.rs:43` `vqf_attributes()` | HAS |
| `vdi_vr_recfl` | 293 | `void vdi_vr_recfl(Vwk * vwk)` | `tos-api/src/vdi/primitives.rs:91` `vr_recfl()` | HAS |
| `vdi_vsf_color` | 206 | `void vdi_vsf_color(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:249` `vsf_color()` | HAS |
| `vdi_vsf_interior` | 173 | `void vdi_vsf_interior(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:279` `vsf_interior()` | HAS |
| `vdi_vsf_perimeter` | 219 | `void vdi_vsf_perimeter(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:214` `vsf_perimeter()` | HAS |
| `vdi_vsf_style` | 186 | `void vdi_vsf_style(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:293` `vsf_style()` | HAS |
| `vdi_vsf_udpat` | 149 | `void vdi_vsf_udpat(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:315` `vsf_udpat()` | HAS |

## vdi_gdp.c  (8 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `clc_arc` | 178 | `static void clc_arc(Vwk * vwk, int steps)` | `tos-api/src/vdi/gdp.rs:80` `curve()` | HAS |
| `clc_nsteps` | 347 | `static int clc_nsteps(void)` | `tos-api/src/vdi/gdp.rs:112` (inline Step-Berechnung in `curve()`) | HAS |
| `clc_pts` | 126 | `static void clc_pts(Point *point, WORD angle)` | `tos-api/src/vdi/gdp.rs:115-123` (inline Punktberechnung via `trig::sincos`) | HAS |
| `gdp_curve` | 371 | `static void gdp_curve(Vwk *vwk)` | `tos-api/src/vdi/gdp.rs:80` `curve()` | HAS |
| `gdp_rbox` | 229 | `static void gdp_rbox(Vwk *vwk)` | `tos-api/src/vdi/gdp.rs:152` `rbox()` | HAS |
| `Icos` | 113 | `static UWORD Icos(UWORD angle)` | `tos-api/src/vdi/trig.rs:28` `cos()` (andere Methode: Bhaskara-I-Approximation statt Lookup-Tabelle+Interpolation) | HAS |
| `Isin` | 92 | `static UWORD Isin(WORD angle)` | `tos-api/src/vdi/trig.rs:12` `sin()` | HAS |
| `vdi_v_gdp` | 426 | `void vdi_v_gdp(Vwk * vwk)` | `tos-api/src/vdi/gdp.rs:26` `v_gdp()` | HAS |

## vdi_input.c  (8 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `gchc_key` | 199 | `static WORD gchc_key(void)` | `tos-api/src/vdi/input.rs:124-127` (inline in `v_choice()` — Dummy-Verhalten übernommen) | HAS |
| `gchr_key` | 215 | `static WORD gchr_key(void)` | `tos-api/src/vdi/input.rs:77,83` `read_key_word()`/`try_read_key_word()` | HAS |
| `gshift_s` | 185 | `static WORD gshift_s(void)` | `tos-api/src/vdi/inquiry.rs:121` `vq_key_s()` (ruft `host.keyboard_shift_state()` statt `Kbshift(-1)`) | HAS |
| `vdi_v_choice` | 31 | `void vdi_v_choice(Vwk * vwk)` | `tos-api/src/vdi/input.rs:124` `v_choice()` | HAS |
| `vdi_v_string` | 79 | `void vdi_v_string(Vwk * vwk)` | `tos-api/src/vdi/input.rs:136` `v_string()` | HAS |
| `vdi_vq_key_s` | 110 | `void vdi_vq_key_s(Vwk * vwk)` | `tos-api/src/vdi/inquiry.rs:121` `vq_key_s()` | HAS |
| `vdi_vqin_mode` | 154 | `void vdi_vqin_mode(Vwk * vwk)` | `tos-api/src/vdi/input.rs:43` `vqin_mode()` | HAS |
| `vdi_vsin_mode` | 118 | `void vdi_vsin_mode(Vwk * vwk)` | `tos-api/src/vdi/input.rs:25` `vsin_mode()` | HAS |

## vdi_line.c  (44 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `abline` | 2266 | `void abline(const Line *line, const WORD wrt_mode, UWORD color)` | `tos-api/src/vdi/backend.rs:122` `RasterBackend::draw_line()` (Trait) + `primitives.rs:31` `segment()` (Aufrufer) | HAS |
| `ABS` | 51 | `#define ABS(x) ((x) >= 0 ? (x) : -(x))` | — internes Hilfsmakro für `perp_off`s Wideline-Geometrie; nicht repliziert (NewTOS nutzt vereinfachte Parallel-Kopien statt echter Perpendicular-Offsets) | N/A |
| `arrow` | 1597 | `void arrow(Vwk * vwk, Point * point, int count)` | — `line_beg`/`line_end` = ARROWED wird validiert/gespeichert (`attributes.rs:101` `vsl_ends`), aber `v_pline` (`primitives.rs:225`) prüft es nie und zeichnet nie einen Pfeil | NO-COUNTERPART |
| `BLITPARM` | 35 | `} BLITPARM;` | — internes Parameterstruct für den m68k-Software/Hardware-Blit; ersetzt durch `RasterBackend`-Trait-Methoden | N/A |
| `cir_dda` | 1240 | `static void cir_dda(WORD line_width)` | — DDA-Tabelle für Wideline-Rundkappen/-Gelenke; NewTOS' vereinfachte Breitlinie (`primitives.rs:31` `segment()`) hat keine Rundkappen | NO-COUNTERPART |
| `clip_code` | 1034 | `static WORD clip_code(Vwk * vwk, WORD x, WORD y)` | `tos-api/src/vdi/backend.rs:122-132` `draw_line()`s `clip`-Parameter (Clipping ans Backend delegiert, Cohen-Sutherland-Outcode selbst nicht repliziert) | HAS |
| `clip_line` | 1057 | `BOOL clip_line(Vwk * vwk, Line * line)` | `tos-api/src/vdi/backend.rs:122-132` (Clipping-Vertrag des Backends) | HAS |
| `do_circ` | 1322 | `static void do_circ(Vwk * vwk, WORD cx, WORD cy)` | — Rundkappen/-Gelenke für Wideline, in NewTOS' Ersatz fehlend | NO-COUNTERPART |
| `draw_arrow` | 1503 | `static void draw_arrow(Vwk * vwk, Point * point, int count, int inc)` | — Pfeilspitzen-Polygonkonstruktion, fehlt (s. `arrow`) | NO-COUNTERPART |
| `draw_line` | 1801 | `static void draw_line(const Line *line, WORD wrt_mode, UWORD color)` | `tos-api/src/vdi/backend.rs:122` `draw_line()` (Trait; Bitplane-Bresenham-Engine durch Backend-Implementierung ersetzt) | HAS |
| `draw_line16` | 1623 | `static void draw_line16(const Line *line, WORD wrt_mode, UWORD color)` | — CONF_WITH_VDI_16BIT-Variante, in einem einzigen Truecolor-Backend-Pfad aufgegangen | N/A |
| `draw_rect` | 834 | `void draw_rect(const Vwk * vwk, Rect * rect, const UWORD fillcolor)` | `tos-api/src/vdi/primitives.rs:91` `vr_recfl()` + `backend.fill_rect()/fill_pattern()` | HAS |
| `draw_rect_common` | 792 | `void draw_rect_common(const VwkAttrib *attr, const Rect *rect)` | `tos-api/src/vdi/backend.rs` `fill_rect()`/`fill_pattern()` (Trait-Methoden) | HAS |
| `draw_rect_setup` | 176 | `static __inline__ void draw_rect_setup(BLITPARM *b, const VwkAttrib *attr, const Rect *rect)` | — internes Wortmasken-Setup für den planaren Blit; Backend ist chunky, braucht keine Wortmasken | N/A |
| `horzline` | 846 | `static __inline__ void horzline(const Vwk * vwk, Line * line)` | `tos-api/src/vdi/primitives.rs:31` `segment()`/`backend.draw_line()` (generischer Linienpfad deckt horizontale Linien ohne separaten Fastpath ab) | HAS |
| `hwblit_rect_common` | 340 | `static void hwblit_rect_common(const VwkAttrib *attr, const Rect *rect)` | — CONF_WITH_BLITTER (Atari-Blitter-Chip), Hardware-spezifisch | N/A |
| `hwblit_rect_nonstd` | 284 | `static void hwblit_rect_nonstd(const VwkAttrib *attr, const Rect *rect, UWORD *addr)` | — CONF_WITH_BLITTER | N/A |
| `hwblit_vertical_line` | 194 | `static void hwblit_vertical_line(const Line *line, WORD wrt_mode, UWORD color)` | — CONF_WITH_BLITTER + CONF_WITH_VDI_VERTLINE | N/A |
| `linea_color` | 857 | `static UWORD linea_color(void)` | — Line-A-Einstiegspunkt (COLBITn-Variablen), architektonisch ausgeschlossen | N/A |
| `linea_fill` | 975 | `void linea_fill(void)` | — Line-A-Einstiegspunkt, architektonisch ausgeschlossen | N/A |
| `linea_hline` | 932 | `void linea_hline(void)` | — Line-A-Einstiegspunkt | N/A |
| `linea_line` | 2367 | `void linea_line(void)` | — Line-A-Einstiegspunkt | N/A |
| `linea_polygon` | 951 | `void linea_polygon(void)` | — Line-A-Einstiegspunkt | N/A |
| `linea_rect` | 903 | `void linea_rect(void)` | — Line-A-Einstiegspunkt | N/A |
| `lineA2Attrib` | 884 | `static void lineA2Attrib(VwkAttrib *attr)` | — internes Line-A→VwkAttrib-Mapping, gehört zum ausgeschlossenen Line-A-Subsystem | N/A |
| `perp_off` | 1169 | `static void perp_off(WORD * px, WORD * py)` | — echte Wideline-Perpendicular-Offset-Geometrie; NewTOS nutzt stattdessen parallele Versatz-Kopien (`primitives.rs::segment()`) | NO-COUNTERPART |
| `polyline` | 1101 | `void polyline(Vwk * vwk, Point * point, int count, WORD color)` | `tos-api/src/vdi/primitives.rs:63` `polyline()` | HAS |
| `quad_xform` | 1133 | `static void quad_xform(WORD quad, WORD x, WORD y, WORD *tx, WORD *ty)` | — Hilfsfunktion für `perp_off`, s.o. fehlend | NO-COUNTERPART |
| `r_fa_attr` | 1376 | `static void r_fa_attr(Vwk * vwk)` | — Restore der getauschten Fill-Attribute nach Wideline/Arrow-Polygon-Fill; kein Gegenstück (NewTOS' Breitlinie geht nicht über den Polygon-Fill-Pfad) | NO-COUNTERPART |
| `s_fa_attr` | 1355 | `static void s_fa_attr(Vwk * vwk)` | — Save der Fill-Attribute vor Wideline/Arrow, s.o. fehlend | NO-COUNTERPART |
| `set_LN_MASK` | 2384 | `void set_LN_MASK(Vwk *vwk)` | `tos-api/src/vdi/primitives.rs:18` `line_mask()` | HAS |
| `swblit_rect_common` | 575 | `static void OPTIMIZE_SMALL swblit_rect_common(const VwkAttrib *attr, const Rect *rect)` | — interne planare Software-Blit-Implementierung; ersetzt durch `RasterBackend::fill_rect/fill_pattern` (Backend-seitig, ungeprüft) | N/A |
| `swblit_rect_common16` | 450 | `static void OPTIMIZE_SMALL swblit_rect_common16(const VwkAttrib *attr, const Rect *rect)` | — CONF_WITH_VDI_16BIT-Variante | N/A |
| `swblit_vertical_line16` | 2084 | `static void swblit_vertical_line16(const Line *line, WORD wrt_mode, UWORD color)` | — CONF_WITH_VDI_16BIT + CONF_WITH_VDI_VERTLINE | N/A |
| `vdi_v_pline` | 1000 | `void vdi_v_pline(Vwk * vwk)` | `tos-api/src/vdi/primitives.rs:225` `v_pline()` (Kernopcode vorhanden; Arrow-Enden und HAVE_BEZIER-Zweig fehlen, s. dort) | HAS |
| `vdi_vql_attributes` | 156 | `void vdi_vql_attributes(Vwk * vwk)` | `tos-api/src/vdi/inquiry.rs:20` `vql_attributes()` | HAS |
| `vdi_vsl_color` | 143 | `void vdi_vsl_color(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:241` `vsl_color()` | HAS |
| `vdi_vsl_ends` | 128 | `void vdi_vsl_ends(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:101` `vsl_ends()` (Setter vorhanden; Arrow-Rendering fehlt, s. `arrow`) | HAS |
| `vdi_vsl_type` | 90 | `void vdi_vsl_type(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:78` `vsl_type()` | HAS |
| `vdi_vsl_udsty` | 81 | `void vdi_vsl_udsty(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:112` `vsl_udsty()` | HAS |
| `vdi_vsl_width` | 104 | `void vdi_vsl_width(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:88` `vsl_width()` | HAS |
| `vertical_line` | 2153 | `static void vertical_line(const Line *line, WORD wrt_mode, UWORD color)` | — interner Bitplane-Fastpath für vertikale Linien (CONF_WITH_VDI_VERTLINE); generischer `backend.draw_line()` deckt vertikale Linien uniform ab | N/A |
| `Vwk2Attrib` | 817 | `void Vwk2Attrib(const Vwk *vwk, VwkAttrib *attr, const UWORD color)` | `tos-api/src/vdi/gdp.rs:53` `fill_of()` (+ inline `PatternFill`-Literale in `primitives.rs`/`contour.rs`) | HAS |
| `wideline` | 1389 | `void wideline(Vwk * vwk, Point * point, int count)` | `tos-api/src/vdi/primitives.rs:31` `segment()` (width>1-Zweig — vereinfachter Ersatz: parallele Versatzkopien, KEINE Rundkappen/-gelenke, keine Arrow-Integration) | HAS |

## vdi_main.c  (3 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `screen` | 121 | `void screen(void)` | `tos-api/src/vdi/mod.rs:205` `dispatch()` | HAS |
| `vdi_jmptab` | 25 | `struct vdi_jmptab {` | `tos-api/src/vdi/mod.rs:221-317` (`match opcode { ... }` statt Jump-Tabelle) | HAS |
| `Vwk` | 22 | `typedef void (*VDI_OP_T)(Vwk *); /* Pointer type to VDI operation */` | — internes C-Funktionszeiger-Typedef für die Jump-Tabelle; Rusts Match-Dispatch braucht kein Äquivalent | N/A |

## vdi_marker.c  (5 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `vdi_v_pmarker` | 86 | `void vdi_v_pmarker(Vwk * vwk)` | `tos-api/src/vdi/primitives.rs:255` `v_pmarker()` (Marker-Vektortabellen `M_DOT`/`M_PLUS`/... decken sich exakt mit `m_dot`/`m_plus`/...) | HAS |
| `vdi_vqm_attributes` | 169 | `void vdi_vqm_attributes(Vwk * vwk)` | `tos-api/src/vdi/inquiry.rs:32` `vqm_attributes()` | HAS |
| `vdi_vsm_color` | 72 | `void vdi_vsm_color(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:139` `vsm_color()` | HAS |
| `vdi_vsm_height` | 30 | `void vdi_vsm_height(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:128` `vsm_height()` | HAS |
| `vdi_vsm_type` | 57 | `void vdi_vsm_type(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:118` `vsm_type()` | HAS |

## vdi_misc.c  (7 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `arb_corner` | 30 | `void arb_corner(Rect * rect)` | `tos-api/src/vdi/primitives.rs:96-106` (inline Ecken-Sortierung, mehrfach dupliziert: `gdp.rs:40-50`, `raster.rs:199-211`) | HAS |
| `arb_line` | 54 | `void arb_line(Line * line)` | — spezielle Ecken-Sortierung nur für `horzline`s Bitplane-Fastpath; der generische Backend-Linienpfad (`segment()`) braucht keine vorsortierten Endpunkte | N/A |
| `get_start_addr` | 166 | `UWORD * get_start_addr(const WORD x, const WORD y)` | — Bitplane-Framebuffer-Adressberechnung; durch `RasterBackend`-Trait abstrahiert (Methoden nehmen x/y direkt) | N/A |
| `tick_int` | 78 | `static void tick_int(int u)` | — periodischer Timer-C-ISR, der den installierten `vex_timv`-Vektor tatsächlich aufruft; nichts in NewTOS ruft `TIM_ADDR` je auf (`mouse.rs:41,434`) | NO-COUNTERPART |
| `timer_exit` | 144 | `void timer_exit(void)` | — Timer-Vektor-Teardown bei Workstation-Close, fehlt | NO-COUNTERPART |
| `timer_init` | 121 | `void timer_init(void)` | — Timer-Vektor-Installation bei Workstation-Open, fehlt | NO-COUNTERPART |
| `vdi_vex_timv` | 100 | `void vdi_vex_timv(Vwk * vwk)` | `tos-api/src/vdi/mouse.rs:433` `vex_timv()` (Austausch selbst funktioniert; der periodische Aufruf-Mechanismus fehlt, s. `tick_int`) | HAS |

## vdi_mouse.c  (28 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `cur_display` | 877 | `void cur_display (Mcdb *sprite, MCS *mcs, WORD x, WORD y)` | `tos-api/src/vdi/mouse.rs:225` `draw()` | HAS |
| `cur_display_clip` | 779 | `static void cur_display_clip(WORD op,Mcdb *sprite,MCS *mcs,UWORD *mask_start,UWORD shft)` | — interner Bitplane-Wortgrenzen-Clip für den Cursor-Blit; `draw()`/`erase()` clippen pixelweise, kein Sonderfall nötig | N/A |
| `cur_display16` | 696 | `static void cur_display16(Mcdb *sprite, MCS *mcs, WORD x, WORD y)` | — CONF_WITH_VDI_16BIT-Variante | N/A |
| `cur_replace` | 1066 | `void cur_replace (MCS *mcs)` | `tos-api/src/vdi/mouse.rs:253` `erase()` | HAS |
| `cur_replace16` | 1032 | `static void cur_replace16(MCS *mcs)` | — CONF_WITH_VDI_16BIT-Variante | N/A |
| `default_mform` | 110 | `#define default_mform() &arrow_mform` | `tos-api/src/vdi/mouse.rs:65-69,164-176` `PRESET_FORMS[0]` (ARROW) + `ensure_init()` | HAS |
| `dis_cur` | 127 | `static void dis_cur(void)` | `tos-api/src/vdi/mouse.rs:300` `dis_cur()` | HAS |
| `gloc_key` | 195 | `static WORD gloc_key(void)` | — Maustasten-/Bewegungs-Polling für den Locator; `input.rs:98` `v_locator()` dokumentiert dies selbst als fehlend (nur Tastatur, kein Maustasten-Terminator/Motion-Flag) | NO-COUNTERPART |
| `hide_cur` | 157 | `static void hide_cur(void)` | `tos-api/src/vdi/mouse.rs:337-343` (inline in `v_hide_c()`) | HAS |
| `linea_hide_mouse` | 1133 | `void linea_hide_mouse(void)` | `tos-api/src/vdi/mouse.rs:337` `v_hide_c()` (EmuTOS' `vdi_v_hide_c` ruft 1:1 nur diese Funktion — in NewTOS dieselbe Funktion) | HAS |
| `linea_show_mouse` | 1125 | `void linea_show_mouse(void)` | `tos-api/src/vdi/mouse.rs:315` `v_show_c()` | HAS |
| `linea_transform_mouse` | 1138 | `void linea_transform_mouse(void)` | `tos-api/src/vdi/mouse.rs:272` `vsc_form()` | HAS |
| `Mcdb` | 34 | `typedef struct Mcdb_ Mcdb;` | — internes Struct-Typedef, durch `mouse.rs`s eigene Zustandsdarstellung (`MASK`/`DATA`-Arrays) ersetzt | N/A |
| `Mcdb_` | 35 | `struct Mcdb_ {` | — s.o. | N/A |
| `set_mouse_form` | 450 | `static void set_mouse_form(const MFORM *src, Mcdb *dst)` | `tos-api/src/vdi/mouse.rs:193` `set_form()` | HAS |
| `vb_draw` | 668 | `static void vb_draw(void)` | `tos-api/src/vdi/mouse.rs:106,127` `set_state()`/`feed_relative()` (synchrones statt VBL-verzögertes Redraw — architektonisch anders, funktional gleichwertig) | HAS |
| `vdi_mousex_handler` | 525 | `static void vdi_mousex_handler (WORD scancode)` | — CONF_WITH_EXTENDED_MOUSE-Scancode-Decoder für Maustasten 3-5/Wheel; nichts in NewTOS dekodiert rohe IKBD-Scancodes oder ruft `USER_BUT`/`USER_WHEEL` auf | NO-COUNTERPART |
| `vdi_v_hide_c` | 331 | `void vdi_v_hide_c(Vwk * vwk)` | `tos-api/src/vdi/mouse.rs:337` `v_hide_c()` | HAS |
| `vdi_v_locator` | 280 | `void vdi_v_locator(Vwk * vwk)` | `tos-api/src/vdi/input.rs:98` `v_locator()` (vereinfacht: nur Tastatur, kein Maustasten-/Bewegungs-Terminator, s. `gloc_key`) | HAS |
| `vdi_v_show_c` | 321 | `void vdi_v_show_c(Vwk * vwk)` | `tos-api/src/vdi/mouse.rs:315` `v_show_c()` | HAS |
| `vdi_vex_butv` | 370 | `void vdi_vex_butv(Vwk * vwk)` | `tos-api/src/vdi/mouse.rs:415` `vex_butv()` (Austausch funktioniert; `USER_BUT` wird nirgends aufgerufen) | HAS |
| `vdi_vex_curv` | 414 | `void vdi_vex_curv(Vwk * vwk)` | `tos-api/src/vdi/mouse.rs:423` `vex_curv()` (`USER_CUR` wird nirgends aufgerufen) | HAS |
| `vdi_vex_motv` | 391 | `void vdi_vex_motv(Vwk * vwk)` | `tos-api/src/vdi/mouse.rs:419` `vex_motv()` (`USER_MOT` wird nirgends aufgerufen) | HAS |
| `vdi_vex_wheelv` | 438 | `void vdi_vex_wheelv(Vwk * vwk)` | `tos-api/src/vdi/mouse.rs:427` `vex_wheelv()` (`USER_WHEEL` wird nirgends aufgerufen) | HAS |
| `vdi_vq_mouse` | 341 | `void vdi_vq_mouse(Vwk * vwk)` | `tos-api/src/vdi/mouse.rs:398` `vq_mouse()` | HAS |
| `vdi_vsc_form` | 513 | `void vdi_vsc_form(Vwk * vwk)` | `tos-api/src/vdi/mouse.rs:272` `vsc_form()` | HAS |
| `vdimouse_exit` | 622 | `void vdimouse_exit(void)` | — Maus-Subsystem-Teardown bei Workstation-Close, fehlt (`v_clsvwk` tut nichts dergleichen) | NO-COUNTERPART |
| `vdimouse_init` | 565 | `void vdimouse_init(void)` | `tos-api/src/vdi/workstation.rs:136` `mouse::init()` (Kernaufgabe — Cursor parken, Default-Form laden, verstecken — vorhanden; `loc_mode`/`val_mode`/`chc_mode`/`str_mode`-Reset und XBIOS-`Initmous`-Analogon fehlen) | HAS |

## vdi_raster.c  (22 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `bit_blt` | 588 | `static void bit_blt(struct blit_frame *blit_info)` | — interner m68k-Software-Blitter-Fallback (ASM_BLIT_IS_AVAILABLE=0/ColdFire); ersetzt durch direkte Pixel-Kopierschleifen in `raster.rs` | N/A |
| `blit_frame` | 128 | `struct blit_frame {` | — internes Parameter-Marshalling-Struct; ersetzt durch `raster.rs`s `Mfdb`/`Rect` | N/A |
| `BLITVARS` | 46 | `} BLITVARS;` | — internes Blitter-Hardware-Register-Schatten-Struct; kein Blitter-Chip im Backend | N/A |
| `cpy_raster` | 1331 | `cpy_raster(struct raster_t *raster, struct blit_frame *info)` | `tos-api/src/vdi/raster.rs:266,215` `vro_cpyfm()`/`vrt_cpyfm()` (gemeinsame Rolle direkt in den beiden Opcode-Funktionen, kein Zwischen-`blit_frame` nötig) | HAS |
| `do_blit` | 425 | `do_blit(BLITVARS * blt)` | — `#if !ASM_BLIT_IS_AVAILABLE` Software-Blitter-Emulator-Fallback | N/A |
| `do_clip` | 809 | `do_clip (VwkClip *clipper, struct blit_frame *info)` | `tos-api/src/vdi/raster.rs:318-347` `mem_to_screen()` (Dest-Box ∩ Clip ∩ Screen für den einen Fall, wo geclippt wird: Ziel = Screen) | HAS |
| `dont_clip` | 883 | `dont_clip (struct blit_frame * info)` | — internes Gegenstück zu `do_clip` für den Memory-Ziel-Fall; `raster.rs`s `screen_to_mem`/`mem_to_mem` clippen grundsätzlich nicht (kein Schritt nötig) | N/A |
| `GetMemW` | 33 | `#define GetMemW(addr) ((ULONG)*(UWORD*)(addr))` | — internes Wort-Zugriffsmakro fürs `blit_frame`; ersetzt durch `Mfdb::get_mem`/`put_mem` (u32-basiert) | N/A |
| `hwblit_raster` | 366 | `hwblit_raster(BLITVARS *blt)` | — CONF_WITH_BLITTER, Hardware-spezifisch | N/A |
| `linea_blit` | 1521 | `void linea_blit(struct blit_frame *info)` | — Line-A-Einstiegspunkt, architektonisch ausgeschlossen | N/A |
| `linea_raster` | 1508 | `void linea_raster(void)` | — Line-A-Einstiegspunkt | N/A |
| `MFDB` | 176 | `} MFDB;` | `tos-api/src/vdi/raster.rs:51` `Mfdb`-Struct | HAS |
| `raster_t` | 780 | `struct raster_t {` | — internes Parameterstruct, das `vdi_vro_cpyfm`/`vdi_vrt_cpyfm`/`linea_raster` an `cpy_raster` weitergeben; NewTOS' `vro_cpyfm`/`vrt_cpyfm` nehmen `Vdipb`/`Vwk`/`backend` direkt | N/A |
| `SetMemW` | 34 | `#define SetMemW(addr, val) *(UWORD*)(addr) = val` | — s. `GetMemW` | N/A |
| `setup_info` | 909 | `setup_info (struct raster_t *raster, struct blit_frame * info)` | `tos-api/src/vdi/raster.rs:61` `Mfdb::from_ptr()` + `:199` `rect_from_ptsin()` | HAS |
| `setup_pattern` | 791 | `setup_pattern (struct raster_t *raster, struct blit_frame *info)` | — PAT_FLAG-Pattern-Blit-Modus von `vro_cpyfm`; NewTOS streift PAT_FLAG explizit ab und blittet nie ein Pattern (selbstdokumentiert „D-082", `raster.rs:276-281`) | NO-COUNTERPART |
| `vdi_vr_trnfm` | 269 | `void vdi_vr_trnfm(Vwk * vwk)` | `tos-api/src/vdi/raster.rs:413` `vr_trnfm()` | HAS |
| `vdi_vro_cpyfm` | 1471 | `vdi_vro_cpyfm(Vwk * vwk)` | `tos-api/src/vdi/raster.rs:266` `vro_cpyfm()` | HAS |
| `vdi_vrt_cpyfm` | 1493 | `vdi_vrt_cpyfm(Vwk * vwk)` | `tos-api/src/vdi/raster.rs:215` `vrt_cpyfm()` | HAS |
| `vr_trnfm16` | 190 | `static void vr_trnfm16(MFDB *src_mfdb, MFDB *dst_mfdb)` | — CONF_WITH_VDI_16BIT-Variante | N/A |
| `vro_cpyfm16` | 975 | `static void vro_cpyfm16(struct blit_frame *info)` | — CONF_WITH_VDI_16BIT-Variante | N/A |
| `vrt_cpyfm16` | 1226 | `static void vrt_cpyfm16(struct blit_frame *info)` | — CONF_WITH_VDI_16BIT-Variante | N/A |

## vdi_text.c  (28 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `act_siz` | 743 | `static UWORD act_siz(Vwk * vwk, UWORD top)` | — DDA-basierte Font-Skalierungs-Größenberechnung; NewTOS hat keine Font-Skalierung (ein fixes 8×16-Bitmap-Font) | NO-COUNTERPART |
| `calc_height` | 160 | `static WORD calc_height(Vwk *vwk)` | `tos-api/src/vdi/inquiry.rs:64` (inline `font::TOP + font::BOTTOM + 1`, feste Konstante statt dynamischer Skalierung/Outline-Padding) | HAS |
| `calc_width` | 176 | `static WORD calc_width(Vwk *vwk, WORD cnt, WORD *str)` | `tos-api/src/vdi/text.rs:99`, `inquiry.rs:132` (`count * font::CELL_W`, feste Monospace-Formel ohne Thicken/Skew/Outline-Zusatzbreite — bei 8px-Zellen bereits inklusive) | HAS |
| `clc_dda` | 1374 | `static UWORD clc_dda(Vwk * vwk, UWORD act, UWORD req)` | — DDA-Inkrement-Berechnung für eine angeforderte Fontgröße, fehlt (keine Skalierung) | NO-COUNTERPART |
| `gdp_justified` | 1177 | `void gdp_justified(Vwk * vwk)` | `tos-api/src/vdi/text.rs:122` `v_justified()` | HAS |
| `JUSTINFO` | 149 | `} JUSTINFO;` | `tos-api/src/vdi/text.rs:122-157` (inline Lokalvariablen `word_extra`/`char_extra`/`slack` — deckt nur den unrotierten Fall ab; rotierte Justified-Texte fallen laut eigenem Kommentar auf links-nach-rechts zurück) | HAS |
| `make_header` | 770 | `static void make_header(Vwk * vwk)` | — Konstruktion eines synthetischen skalierten Font-Headers, fehlt (keine Skalierung) | NO-COUNTERPART |
| `ok_for_direct_blit` | 227 | `static BOOL ok_for_direct_blit(Vwk *vwk, WORD width, JUSTINFO *justified)` | — Performance-Gate zur Wahl zwischen der allgemeinen DDA-Text-Engine und einem Direct-Blit-Fastpath; NewTOS hat nur EINEN Pfad (immer Direct-Glyph-Blit), die Entscheidung entfällt | N/A |
| `output_text` | 288 | `static void output_text(Vwk *vwk, WORD count, WORD *str, WORD width, JUSTINFO *justified)` | `tos-api/src/vdi/text.rs:37,92,122` `draw_cell()`+`v_gtext()`+`v_justified()` (gleiche Gesamtrolle, drastisch vereinfacht: keine Skalierung/Rotation-DDA/GDOS) | HAS |
| `setup_width_height` | 668 | `static void setup_width_height(const Fonthead *font)` | `tos-api/src/vdi/attributes.rs:178` `report_font_size()` | HAS |
| `text_init` | 592 | `void text_init(void)` | — VDI-internes Font-Ring/Tabellen-Setup beim Öffnen; NewTOS' einziges 8×16-Font ist eine Compile-Zeit-Konstante (`font.rs`), braucht keinen Init-Schritt — Architektur löst dies anders | N/A |
| `text_init2` | 573 | `void text_init2(Vwk * vwk)` | — Pro-Workstation-Font-Zustand-Reset; `Vwk` (`vwk.rs`) hat keine `cur_font`/`loaded_fonts`-Felder, da der Font ein globales Compile-Zeit-Konstrukt statt Pro-Workstation-Zustand ist | N/A |
| `trnsfont` | 562 | `static void trnsfont(void)` | — CONF_WITH_GDOS Byte-Swap-Konverter für geladene GDOS-Fonts, nicht anwendbar (kein GDOS-Fontladen) | N/A |
| `vdi_v_gtext` | 546 | `void vdi_v_gtext(Vwk * vwk)` | `tos-api/src/vdi/text.rs:92` `v_gtext()` | HAS |
| `vdi_vqt_attributes` | 999 | `void vdi_vqt_attributes(Vwk * vwk)` | `tos-api/src/vdi/inquiry.rs:54` `vqt_attributes()` | HAS |
| `vdi_vqt_extent` | 1024 | `void vdi_vqt_extent(Vwk * vwk)` | `tos-api/src/vdi/inquiry.rs:130` `vqt_extent()` | HAS |
| `vdi_vqt_fontinfo` | 1137 | `void vdi_vqt_fontinfo(Vwk * vwk)` | `tos-api/src/vdi/inquiry.rs:202` `vqt_fontinfo()` | HAS |
| `vdi_vqt_name` | 1100 | `void vdi_vqt_name(Vwk * vwk)` | `tos-api/src/vdi/inquiry.rs:191` `vqt_name()` (jeder Index liefert das einzige Font — korrekt für den reduzierten Umfang) | HAS |
| `vdi_vqt_width` | 1062 | `void vdi_vqt_width(Vwk * vwk)` | `tos-api/src/vdi/inquiry.rs:171` `vqt_width()` | HAS |
| `vdi_vst_alignment` | 887 | `void vdi_vst_alignment(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:267` `vst_alignment()` | HAS |
| `vdi_vst_color` | 988 | `void vdi_vst_color(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:257` `vst_color()` | HAS |
| `vdi_vst_effects` | 881 | `void vdi_vst_effects(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:148` `vst_effects()` (akzeptiert/speichert die Maske inkl. F_OUTLINE-Bit; das eigentliche Rendern des OUTLINE-Effekts fehlt, s. `vdi_textblit.c::outline`) | HAS |
| `vdi_vst_font` | 939 | `void vdi_vst_font(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:171` `vst_font()` (nur das eine Systemfont existiert — kein Gap, da nichts anderes wählbar ist) | HAS |
| `vdi_vst_height` | 681 | `void vdi_vst_height(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:189` `vst_height()` (meldet immer die festen 8×16-Metriken unabhängig von der Anfrage — dokumentierte Vereinfachung) | HAS |
| `vdi_vst_load_fonts` | 1292 | `void vdi_vst_load_fonts(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:204` `vst_load_fonts()` (entspricht exakt EmuTOS' `#else`-Zweig ohne CONF_WITH_GDOS: liefert 0) | HAS |
| `vdi_vst_point` | 820 | `void vdi_vst_point(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:195` `vst_point()` (meldet immer 10pt, dieselbe Vereinfachung wie `vst_height`) | HAS |
| `vdi_vst_rotation` | 924 | `void vdi_vst_rotation(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:159` `vst_rotation()` (reproduziert exakt die 3150-3599→3600-Rundungsregel) | HAS |
| `vdi_vst_unload_fonts` | 1353 | `void vdi_vst_unload_fonts(Vwk * vwk)` | `tos-api/src/vdi/attributes.rs:211` `vst_unload_fonts()` | HAS |

## vdi_textblit.c  (17 Symbole)

| EmuTOS fn | line | signature | NewTOS-Gegenstück | Status |
|---|---|---|---|---|
| `char_resize` | 1257 | `static UWORD char_resize(UWORD init, UWORD size)` | — Pro-Zeichen-DDA-Skalierung, fehlt (keine Font-Skalierung) | NO-COUNTERPART |
| `check_clip` | 128 | `static WORD check_clip(LOCALVARS *vars, WORD delx, WORD dely)` | — interner Clip-Check der Low-Level-Glyph-Blit-Engine; `draw_glyph()` delegiert Clipping ans Backend (`ClipRect`), kein separater Vorab-Check | N/A |
| `direct_screen_blit` | 849 | `void direct_screen_blit(WORD count, WORD *str)` | `tos-api/src/vdi/text.rs:37` `draw_cell()`/`backend.draw_glyph()` (NewTOS' normaler Glyphen-Blit IST immer ein „direct screen blit", da es keine allgemeine Scratch-Buffer-Engine als Fallback gibt) | HAS |
| `direct_screen_blit16` | 751 | `static void direct_screen_blit16(WORD count, WORD *str)` | — CONF_WITH_VDI_16BIT-Variante | N/A |
| `do_clip` | 176 | `static WORD do_clip(LOCALVARS *vars)` | — internes Clipping der Scratch-Buffer-Text-Blit-Pipeline (anderer Name-Twin zu `vdi_raster.c::do_clip`); durch Backend-seitiges `ClipRect`-Handling ersetzt | N/A |
| `LOCALVARS` | 99 | `} LOCALVARS;` | — internes Parameterstruct für die Assembler-Text-Blit-Engine (Skalierung/Rotation/Skew/Outline-Zustand); nicht benötigt, da diese Engine fehlt | N/A |
| `merge_byte` | 239 | `static ULONG merge_byte(UWORD *p, UWORD n)` | — internes Bit-Merge für den Outline-Effekt-Algorithmus, fehlt (kein Outline-Rendering) | N/A |
| `outline` | 266 | `void outline(LOCALVARS *vars)` | — rendert den F_OUTLINE-Texteffekt (1px-Rahmen um jedes Glyph); fehlt in `text.rs` (nur THICKEN/LIGHT/SKEW/UNDERLINE implementiert, kein `T_OUTLINE`) | NO-COUNTERPART |
| `pre_blit` | 352 | `static void pre_blit(LOCALVARS *vars)` | — Setup-Schritt der Scratch-Buffer-Pipeline (Thicken/Skew-Vorverarbeitung); NewTOS wendet Effekte direkt pro Zeile in `effect_row()`/`draw_cell()` an, keine Pipeline nötig | N/A |
| `rotate` | 442 | `void rotate(LOCALVARS *vars)` | — rotiert ein Glyph 90/180/270°; `text.rs` zeichnet laut eigenem Kommentar „nur chup == 0 horizontal" — Glyphen-Rotation fehlt vollständig (chup beeinflusst nur `vqt_extent`s Bounding-Box) | NO-COUNTERPART |
| `scale` | 672 | `void scale(LOCALVARS *vars)` | — DDA-basierte Glyph-Bitmap-Skalierungs-Dispatch, fehlt | NO-COUNTERPART |
| `scaledown` | 628 | `static void scaledown(LOCALVARS *vars, UWORD *src, UWORD *dst)` | — Glyph-Verkleinerung, fehlt | NO-COUNTERPART |
| `scaleup` | 577 | `static void scaleup(LOCALVARS *vars, UWORD *src, UWORD *dst)` | — Glyph-Vergrößerung, fehlt | NO-COUNTERPART |
| `screen_blit` | 1191 | `static void screen_blit(LOCALVARS *vars)` | — Dispatch des allgemeinen (Nicht-Direct-) Scratch-Buffer-Textpfads (pre_blit→scale/rotate/outline→Blit); NewTOS hat keinen solchen allgemeinen Pfad | N/A |
| `screen_blit16` | 967 | `static void screen_blit16(LOCALVARS *vars)` | — CONF_WITH_VDI_16BIT-Variante | N/A |
| `shift_and_update` | 560 | `static __inline__ UWORD *shift_and_update(UWORD *dst, UWORD *dstbit, UWORD *out)` | — interner Bit-Shift-Helfer für `scaleup`/`scaledown`, fehlt (keine Skalierung) | N/A |
| `text_blt` | 1281 | `void text_blt(void)` | `tos-api/src/vdi/text.rs:37` `draw_cell()` (Top-Level „ein Zeichen blitten"-Rolle; drastisch vereinfacht — kein Scale/Rotate/Outline) | HAS |

---
**Gesamt: 269 Symbole über 15 .c-Dateien** (+ .S-asm separat).

## Ergebnis

### Zähler

**Gesamt (269 Symbole):** HAS = **150**, N/A = **93**, NO-COUNTERPART = **26**.

| Datei | Symbole | HAS | N/A | NO-COUNTERPART |
|---|---:|---:|---:|---:|
| vdi_bezier.c | 9 | 0 | 9 | 0 |
| vdi_col.c | 22 | 4 | 18 | 0 |
| vdi_control.c | 13 | 12 | 1 | 0 |
| vdi_esc.c | 25 | 21 | 2 | 2 |
| vdi_fill.c | 30 | 20 | 10 | 0 |
| vdi_gdp.c | 8 | 8 | 0 | 0 |
| vdi_input.c | 8 | 8 | 0 | 0 |
| vdi_line.c | 44 | 18 | 18 | 8 |
| vdi_main.c | 3 | 2 | 1 | 0 |
| vdi_marker.c | 5 | 5 | 0 | 0 |
| vdi_misc.c | 7 | 2 | 2 | 3 |
| vdi_mouse.c | 28 | 20 | 5 | 3 |
| vdi_raster.c | 22 | 7 | 14 | 1 |
| vdi_text.c | 28 | 21 | 4 | 3 |
| vdi_textblit.c | 17 | 2 | 9 | 6 |
| **Summe** | **269** | **150** | **93** | **26** |

Zur Einordnung: von den 93 N/A entfallen 9 auf die von EmuTOS selbst deaktivierte
Bezier-Unterstützung (`HAVE_BEZIER=0`) und ca. 45 auf Atari-Hardware-spezifische
Pfade (ST/STe/TT/Falcon-Farbkonvertierung, Blitter-Chip, 16-Bit-Truecolor-VDI-
Modus, Line-A-Subsystem) — beides architektonisch erwartete Lücken, kein
Befund gegen NewTOS.

### Explizite NO-COUNTERPART-Liste (nach Subsystem gruppiert)

**VT52-Alpha/Konsolen-Übergang (vdi_esc.c) — 2**
- `esc_init` (vdi_esc.c:356) — kein Verstecken des Alpha-Cursors beim Eintritt in den Grafikmodus (`v_opnwk`).
- `esc_exit` (vdi_esc.c:365) — kein Clear+Home-Rücksprung in den Alpha-Modus beim Workstation-Close.

**Breitlinien-Geometrie: Rundkappen, Gelenke, Pfeilspitzen (vdi_line.c) — 8**
- `cir_dda` (vdi_line.c:1240) — Kreis-DDA-Tabelle für Wideline-Kappen, fehlt.
- `do_circ` (vdi_line.c:1322) — zeichnet die Rundkappen/-gelenke selbst, fehlt.
- `perp_off` (vdi_line.c:1169) — echte Perpendicular-Offset-Geometrie, ersetzt durch simplen Parallel-Versatz.
- `quad_xform` (vdi_line.c:1133) — Hilfsfunktion für `perp_off`, fehlt.
- `s_fa_attr` (vdi_line.c:1355) / `r_fa_attr` (vdi_line.c:1376) — Save/Restore der Fill-Attribute rund um den Wideline/Arrow-Polygon-Fill, fehlt.
- `arrow` (vdi_line.c:1597) / `draw_arrow` (vdi_line.c:1503) — Pfeilspitzen an Linienenden (`ARROWED`-Stil wird validiert/gespeichert, aber nie gezeichnet).

**Font-Skalierung, -Rotation, Outline-Effekt (vdi_text.c + vdi_textblit.c) — 9**
- `act_siz` (vdi_text.c:743), `clc_dda` (vdi_text.c:1374), `make_header` (vdi_text.c:770) — DDA-Skalierungskern für `vst_height`/`vst_point` mit Nicht-Default-Größe.
- `char_resize` (vdi_textblit.c:1257), `scale`/`scaleup`/`scaledown` (vdi_textblit.c:672/577/628) — Glyph-Bitmap-Skalierung.
- `rotate` (vdi_textblit.c:442) — 90/180/270°-Glyphen-Rotation (nur die Bounding-Box in `vqt_extent` dreht sich, die Pixel nie).
- `outline` (vdi_textblit.c:266) — F_OUTLINE-Texteffekt (Bit wird akzeptiert/gespeichert, nie gerendert).

**VDI-Timer-C-Tick-Zustellung (vdi_misc.c) — 3**
- `timer_init` (vdi_misc.c:121) / `timer_exit` (vdi_misc.c:144) — Installation/Teardown des Timer-C-Vektors.
- `tick_int` (vdi_misc.c:78) — der periodische ISR, der den via `vex_timv` installierten Vektor tatsächlich aufruft; `vex_timv` selbst (Austausch + Tickcal-Report) funktioniert, aber `TIM_ADDR` wird nie aufgerufen.

**Maus-Exchange-Vektoren feuern nie + erweiterte Maus (vdi_mouse.c) — 3**
- `gloc_key` (vdi_mouse.c:195) — Maustasten-/Bewegungs-Polling für den Locator (`v_locator` ist rein tastaturbasiert).
- `vdi_mousex_handler` (vdi_mouse.c:525) — Scancode-Decoder für Maustasten 3-5/Wheel; nichts speist `USER_BUT`/`USER_WHEEL`.
- `vdimouse_exit` (vdi_mouse.c:622) — Maus-Subsystem-Teardown bei Workstation-Close.
  (Verwandt, aber als HAS gewertet, weil der Austausch selbst funktioniert: `vex_butv`/`vex_motv`/`vex_curv`/`vex_wheelv` installieren/tauschen `USER_BUT`/`USER_MOT`/`USER_CUR`/`USER_WHEEL` korrekt — nur ruft sie **niemand** je auf, weder bei Mausbewegung noch -klick noch Cursor-Redraw noch Wheel. Ein GEM-Programm, das diese Vektoren hookt, wird nie benachrichtigt.)

**Pattern-Blit-Modus (vdi_raster.c) — 1**
- `setup_pattern` (vdi_raster.c:791) — `vro_cpyfm`s PAT_FLAG-Pattern-Kombinationsmodus; NewTOS streift PAT_FLAG explizit ab (selbstdokumentiert als „D-082").

### Auffällige Muster

1. **Bezier-Kurven (9 Symbole, vdi_bezier.c) sind kein NewTOS-Defizit** — die
   gesamte Datei steht bei EmuTOS selbst hinter `#if HAVE_BEZIER` mit
   `HAVE_BEZIER=0` und dem Kommentar „entirely untested" (`vdi_defs.h:18`).
   Kein reales Atari-TOS-Programm kann sich auf `v_bez` verlassen.
2. **Wideline-Rundkappen/-gelenke und Pfeilspitzen fehlen komplett** (8
   Funktionen, s.o.) — `vsl_ends` akzeptiert und speichert `ARROWED`/`ROUND`,
   aber nichts rendert es. Ein Programm, das Pfeil-Enden anfordert, bekommt
   stillschweigend quadratische Enden.
3. **Font-Subsystem ist auf EIN fixes 8×16-Bitmap-Font reduziert** — kein
   Font-Ring (6x6/8x8 fehlen ebenfalls faktisch, da text_init/text_init2
   architektonisch anders gelöst sind), keine Skalierung (`vst_height`/
   `vst_point` ignorieren die angeforderte Größe), keine Glyphen-Rotation
   (nur die Bounding-Box in `vqt_extent` dreht sich), kein GDOS-Fontladen,
   kein Outline-Effekt.
4. **Alle vier Maus-Exchange-Vektoren (`vex_butv`/`vex_motv`/`vex_curv`/
   `vex_wheelv`) installieren korrekt, feuern aber nie** — das komplette
   IKBD-Paket→Vektor-Aufruf-Bindeglied (`mouse_int`/`wheel_int`/`mov_cur`/
   `call_user_but`/`call_user_wheel` in `vdi_asm.S`, s.u.) hat kein
   NewTOS-Gegenstück. `feed_relative()` bewegt den Cursor und malt ihn neu,
   ruft aber nie einen der vier Vektoren auf.
5. **VDI-Timer-C-Tick wird nie zugestellt** — dieselbe Grundursache wie #4:
   der Vektor-Austausch (`vex_timv`) funktioniert, aber es gibt keinen
   periodischen Aufrufer.
6. **Workstation-Close-Teardown ist ein bloßes Handle-Free** — `esc_exit`,
   `timer_exit`, `vdimouse_exit` (alle bei EmuTOS' `vdi_v_clswk` aufgerufen)
   haben kein Gegenstück; NewTOS' `v_clsvwk` gibt nur den Slot frei.
7. **Line-A-Subsystem ist absichtlich nicht portiert** (ADR, s.
   `docs/MEMORY.md` „altitude-syscall-not-hardware-emulation") — ca. 15
   `linea_*`-Symbole über vdi_line.c/vdi_fill.c/vdi_raster.c sind dadurch
   N/A, nicht NO-COUNTERPART, weil kein TOS-Programm über den offiziellen
   VDI-Trap auf sie angewiesen ist (nur direkte Line-A-Vektor-Aufrufer wären
   betroffen, was außerhalb des VDI-Traps liegt).
8. **Atari-Hardware-Farbkonvertierung (vdi_col.c) ist fast vollständig N/A**
   — ST/STe/TT-Shifter- und Falcon-VIDEL-DAC-Konvertierungstabellen sind
   für ein Truecolor-RGB-Backend gegenstandslos; nur die beiden echten
   VDI-Opcodes (`vs_color`/`vq_color`) und die Init-Funktion haben ein
   Gegenstück.

### m68k-Assembler-Dateien (.S) — Kurzbewertung

Fünf Dateien, nicht in der ctags-Liste (keine .c): `endvdi.S` (16 Zeilen,
nur ein BSS-Endsymbol, trivial N/A), `vdi_asm.S` (554 Zeilen), `vdi_blit.S`
(1406 Zeilen), `vdi_tblit.S` (1394 Zeilen) und `vdi_tblit_cf.S` (1922 Zeilen,
ColdFire-Variante von `vdi_tblit.S`).

- **`vdi_blit.S`** (`fast_bit_blt`) — der m68k-Assembler-Bitplane-Blitkern
  hinter `vdi_vro_cpyfm`/`vdi_vrt_cpyfm`. Überwiegend **N/A**: durch
  `RasterBackend` (chunky 32-bpp statt planar) ersetzt. Die 16 BitBlt-
  Logikoperationen (`BM_ALL_WHITE` … `BM_ALL_BLACK`) sind in
  `raster.rs:177-196` (`raster_op()`) vollständig repliziert — diese
  **Blit-Logik-Fähigkeit fehlt dem Backend NICHT**. Was dem Backend
  tatsächlich fehlt: der PAT_FLAG-Pattern-Kombinationsmodus (s.
  `setup_pattern`, NO-COUNTERPART oben) — `vro_cpyfm` kann in EmuTOS S/D
  während des Kopierens zusätzlich mit dem aktiven Füllmuster maskieren,
  NewTOS' `raster_op()` kennt nur die reine S/D-Logik ohne Pattern-Term.
- **`vdi_asm.S`** (`mouse_int`, `wheel_int`, `mov_cur`, `call_user_but`,
  `call_user_wheel`, `_vditrap`/`_GSX_ENTRY`) — der IKBD-Paket-Dekoder und
  das Vektor-Aufruf-Bindeglied für die Maus-Exchange-Vektoren, plus der
  rohe Trap-Einstieg (Parametervalidierung, Screen-Clip-Setup). **Größtenteils
  NO-COUNTERPART** (nicht bloß N/A): dies ist die konkrete Implementierung
  hinter Muster #4/#5 oben — `mouse_int`/`wheel_int`/`mov_cur` sind genau
  die Routinen, deren Fehlen bedeutet, dass `USER_BUT`/`USER_MOT`/
  `USER_CUR`/`USER_WHEEL`/`TIM_ADDR` nie aufgerufen werden. Der Trap-Einstieg
  selbst (`_vditrap`) ist durch `mod.rs::dispatch()` funktional ersetzt (HAS).
- **`vdi_tblit.S` / `vdi_tblit_cf.S`** (`normal_blit`, `outline`, `rotate`,
  `scale` — die tatsächlichen Assembler-Körper hinter den in
  `vdi_textblit.c` deklarierten Funktionen) — der Low-Level-Text-Blit-Kern
  mit Wort-Verschieben/-Mergen für Skalierung, Rotation, Skew, Outline.
  **NO-COUNTERPART**, deckungsgleich mit den bereits oben gelisteten
  `rotate`/`scale`/`outline`-Einträgen aus `vdi_textblit.c`. Konkrete
  fehlende Blit-Fähigkeit des `RasterBackend`: `draw_glyph()`
  (`backend.rs:148`) hat **keinen Parameter für Skalierungsfaktor,
  Rotationswinkel oder Outline-Randbreite** — die Trait-Signatur ist
  fest auf eine 8×16-Zelle in Originalgröße/-orientierung zugeschnitten.
  Eine zukünftige Erweiterung (falls gewünscht) müsste `draw_glyph()`
  um genau diese drei Achsen erweitern, nicht nur die Aufrufer in
  `text.rs`.

---
**Reine Präsenz-Prüfung — kein Verhaltens-Audit.** Alle 269 Zeilen tragen
Status + Fundstelle/Begründung; nichts wurde ausgelassen.
