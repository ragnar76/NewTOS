# AES Vollständigkeits-Check — Präsenz-Matrix (EmuTOS → NewTOS)

> Scope: **reine Präsenz/Coverage** — jeder AES-Opcode aus der EmuTOS-Quelle
> gegen unseren Dispatch (`tos-api/src/aes/mod.rs`) + die Manager-Implementierung
> geprüft. **Kein Verhaltens-Audit** — das leisten die P1–P6-Kataloge in diesem
> Ordner (`p1-appl-pd-dispatch.md` usw., siehe `README.md`); diese Matrix ist der
> Opcode-für-Opcode-Rollup darüber, ob überhaupt eine Entsprechung existiert.
>
> Stand: 2026-07-22, Branch `desk-rebuild`.

## Ground Truth — Korrektur zur Aufgabenstellung

`references/emutos/aes/funcdef.h` ist **nicht** die AES-Opcode-Tabelle — das ist
die VDI/GSX-Opcode-Liste (`OPEN_WORKSTATION`, `POLYLINE`, `FILL_RECTANGLE`, …,
Werte 1–134). Die tatsächliche AES-Opcode-Tabelle steht in
**`references/emutos/include/aesdefs.h:17-115`** (`APPL_INIT 10` … `SHEL_WDEF
127`), und der Dispatcher ist **`references/emutos/aes/gemsuper.c`**, Funktion
`crysbind()` (`gemsuper.c:77-420`), aufgerufen aus `super()` (`gemsuper.c:454-466`)
für AES-Trap-Selektor 200. Diese beiden Dateien sind die Ground Truth für diese
Matrix; `funcdef.h` wurde geprüft und als nicht einschlägig verworfen (keine
AES-Funktion definiert dort).

`aesdefs.h` enumeriert **79 Opcodes** über 11 Manager (Application 10–19 [Lücken
16, 18 unbelegt], Event 20–26, Menu 30–39, Object 40–48, Form 50–56, Graphics
70–79, Scrap 80–82, File Selector 90–91, Window 100–109, Resource 110–114, Shell
120–127).

## CONF_WITH_* Gating

Vier Opcode-Gruppen sind in `crysbind()` hinter `#if CONF_WITH_*` gegated:

| Guard | Opcodes | Default (Standard-Build) | Override (Minimal-ROM) |
|---|---|---|---|
| `CONF_WITH_PCGEM` | APPL_YIELD(17), SCRP_CLEAR(82), SHEL_RDEF(126), SHEL_WDEF(127) | **1** (`config.h:1319-1320`) | 0 unter `TARGET_192` (`config.h:352-353`) |
| `CONF_WITH_MENU_EXTENSION` | MENU_POPUP(36), MENU_ATTACH(37), MENU_ISTART(38), MENU_SETTINGS(39) | **1** (`config.h:1304-1305`) | 0 unter `TARGET_192` (`config.h:316-317`) |
| `CONF_WITH_3D_OBJECTS` | OBJC_SYSVAR(48) | **1** (`config.h:1258-1259`) | 0 unter `TARGET_192` (`config.h:310-311`) |

Der `TARGET_192`-Zweig (`config.h:210-394`) ist ein 192-KB-ROM-Minimalbuild für
speicherbeschränkte Maschinen — nicht der Standard-Desktop-Build, den NewTOS
nachbildet. Im Standard-Build (Default-Werte oben) sind **alle 79 Opcodes aktiv**;
NewTOS' Dispatch (`mod.rs`) deckt entsprechend alle 79 ab, nicht nur eine
Untermenge — siehe Teil 2.

---

## Teil 1 — Master-Matrix

Alle EmuTOS-`file:line` sind Funktions-*Definitionen* (nicht nur Deklarationen in
den `.h`), außer wo vermerkt. NewTOS-`file:line` ist die `pub fn`-Definition in
`tos-api/src/aes/`.

### Application Manager (10–19)

| Opcode | EmuTOS fn | EmuTOS decl (file:line) | NewTOS fn (file:line) | Status |
|---|---|---|---|---|
| APPL_INIT (10) | `ap_init` | `aes/gemaplib.c:54` | `aes/appl.rs:244 appl_init` | PRESENT |
| APPL_READ (11) | `ap_rdwr(MU_MESAG,…)` | `aes/gemaplib.c:69` | `aes/appl.rs:351 appl_read` | PRESENT |
| APPL_WRITE (12) | `ap_rdwr(MU_SDMSG,…)` | `aes/gemaplib.c:69` | `aes/appl.rs:317 appl_write` | PRESENT |
| APPL_FIND (13) | `ap_find` | `aes/gemaplib.c:95` | `aes/appl.rs:285 appl_find` | PRESENT |
| APPL_TPLAY (14) | `ap_tplay` | `aes/gemaplib.c:119` | `aes/appl.rs:395 appl_tplay` | **DISPATCH-ONLY (Verdacht)** |
| APPL_TRECORD (15) | `ap_trecd` | `aes/gemaplib.c:189` | `aes/appl.rs:387 appl_trecord` | **DISPATCH-ONLY (Verdacht)** |
| — (16) | *(kein Opcode definiert)* | — | — | n/a |
| APPL_YIELD (17) `CONF_WITH_PCGEM` | `dsptch()` | `aes/gemasm.S:129` (`_dsptch`, extern in `gemasm.h:27`) | `aes/appl.rs:307 appl_yield` | **DISPATCH-ONLY (Verdacht)** |
| — (18) | *(kein Opcode definiert)* | — | — | n/a |
| APPL_EXIT (19) | `ap_exit` | `aes/gemaplib.c:251` | `aes/appl.rs:276 appl_exit` | PRESENT |

### Event Manager (20–26)

| Opcode | EmuTOS fn | EmuTOS decl (file:line) | NewTOS fn (file:line) | Status |
|---|---|---|---|---|
| EVNT_KEYBD (20) | `ev_block(MU_KEYBD,0)` | `aes/gemevlib.c:64` | `aes/event.rs:376 evnt_keybd` | PRESENT |
| EVNT_BUTTON (21) | `ev_button` | `aes/gemevlib.c:84` | `aes/event.rs:123 evnt_button` | PRESENT |
| EVNT_MOUSE (22) | `ev_mouse` | `aes/gemevlib.c:123` | `aes/event.rs:211 evnt_mouse` | PRESENT |
| EVNT_MESAG (23) | `ev_mesag` | `aes/gemevlib.c:100` | `aes/event.rs:399 evnt_mesag` | PRESENT |
| EVNT_TIMER (24) | `ev_timer` | `aes/gemevlib.c:134` | `aes/event.rs:438 evnt_timer` | PRESENT |
| EVNT_MULTI (25) | `ev_multi` | `aes/gemevlib.c:152` (`CONF_WITH_MENU_EXTENSION` variant) / `:155` (else) | `aes/event.rs:260 evnt_multi` | PRESENT |
| EVNT_DCLICK (26) | `ev_dclick` | `aes/gemevlib.c:300` | `aes/event.rs:234 evnt_dclick` | PRESENT |

### Menu Manager (30–39)

| Opcode | EmuTOS fn | EmuTOS decl (file:line) | NewTOS fn (file:line) | Status |
|---|---|---|---|---|
| MENU_BAR (30) | `mn_bar` | `aes/gemmnlib.c:703` | `aes/menu.rs:101 menu_bar` | PRESENT |
| MENU_ICHECK (31) | `do_chg` | `aes/gemmnlib.c:145` | `aes/menu.rs:633 menu_icheck` | PRESENT |
| MENU_IENABLE (32) | `do_chg` | `aes/gemmnlib.c:145` | `aes/menu.rs:642 menu_ienable` | PRESENT |
| MENU_TNORMAL (33) | `do_chg` | `aes/gemmnlib.c:145` | `aes/menu.rs:654 menu_tnormal` | PRESENT |
| MENU_TEXT (34) | *(inline `strcpy`, kein Lib-Fn)* | `aes/gemsuper.c:181-184` | `aes/menu.rs:680 menu_text` | PRESENT |
| MENU_REGISTER (35) | `mn_register` | `aes/gemmnlib.c:789` | `aes/menu.rs:733 menu_register` | PRESENT |
| MENU_POPUP (36) `CONF_WITH_MENU_EXTENSION` | `mn_popup` | `aes/gemmnext.c:304` | `aes/menu.rs:873 menu_popup` | PRESENT |
| MENU_ATTACH (37) `CONF_WITH_MENU_EXTENSION` | `mn_attach` | `aes/gemmnext.c:535` | `aes/menu.rs:1288 menu_attach` | PRESENT |
| MENU_ISTART (38) `CONF_WITH_MENU_EXTENSION` | `mn_istart` | `aes/gemmnext.c:562` | `aes/menu.rs:1326 menu_istart` | PRESENT |
| MENU_SETTINGS (39) `CONF_WITH_MENU_EXTENSION` | `mn_settings` | `aes/gemmnext.c:596` | `aes/menu.rs:1019 menu_settings` | PRESENT |

### Object Manager (40–48)

| Opcode | EmuTOS fn | EmuTOS decl (file:line) | NewTOS fn (file:line) | Status |
|---|---|---|---|---|
| OBJC_ADD (40) | `ob_add` | `aes/gemoblib.c:1121` | `aes/object.rs:694 objc_add` | PRESENT |
| OBJC_DELETE (41) | `ob_delete` | `aes/gemoblib.c:1147` | `aes/object.rs:708 objc_delete` | PRESENT |
| OBJC_DRAW (42) | `ob_draw` (+ `gsx_sclip`) | `aes/gemoblib.c:959` | `aes/object.rs:570 objc_draw` | PRESENT |
| OBJC_FIND (43) | `ob_find` | `aes/gemoblib.c:1034` | `aes/object.rs:1284 objc_find` | PRESENT |
| OBJC_OFFSET (44) | `ob_offset` | `aes/gemoblib.c:285` | `aes/object.rs:679 objc_offset` | PRESENT |
| OBJC_ORDER (45) | `ob_order` | `aes/gemoblib.c:1201` | `aes/object.rs:722 objc_order` | PRESENT |
| OBJC_EDIT (46) | `ob_edit` | `aes/gemobed.c:335` | `aes/object.rs:994 objc_edit` | PRESENT |
| OBJC_CHANGE (47) | `ob_change` | `aes/gemoblib.c:1253` | `aes/object.rs:850 objc_change` | PRESENT |
| OBJC_SYSVAR (48) `CONF_WITH_3D_OBJECTS` | `ob_sysvar` | `aes/gemoblib.c:1350` | `aes/object.rs:892 objc_sysvar` | PRESENT |

### Form Manager (50–56)

| Opcode | EmuTOS fn | EmuTOS decl (file:line) | NewTOS fn (file:line) | Status |
|---|---|---|---|---|
| FORM_DO (50) | `fm_do` | `aes/gemfmlib.c:278` | `aes/form.rs:513 form_do` | PRESENT |
| FORM_DIAL (51) | `fm_dial` | `aes/gemfmlib.c:360` | `aes/form.rs:772 form_dial` | PRESENT |
| FORM_ALERT (52) | `fm_alert` | `aes/gemfmalt.c:319` | `aes/form.rs:114 form_alert` | PRESENT |
| FORM_ERROR (53) | `fm_error` | `aes/gemfmlib.c:436` | `aes/form.rs:682 form_error` | PRESENT |
| FORM_CENTER (54) | `ob_center` | `aes/gemobed.c:67` | `aes/form.rs:716 form_center` | PRESENT |
| FORM_KEYBD (55) | `fm_keybd` | `aes/gemfmlib.c:163` | `aes/form.rs:924 form_keybd` | PRESENT |
| FORM_BUTTON (56) | `fm_button` | `aes/gemfmlib.c:201` | `aes/form.rs:882 form_button` | PRESENT |

### Graphics Manager (70–79)

| Opcode | EmuTOS fn | EmuTOS decl (file:line) | NewTOS fn (file:line) | Status |
|---|---|---|---|---|
| GRAF_RUBBOX (70) | `gr_rubbox` | `aes/gemgrlib.c:228` | `aes/graf.rs:121 graf_rubberbox` | PRESENT |
| GRAF_DRAGBOX (71) | `gr_dragbox` | `aes/gemgrlib.c:243` | `aes/graf.rs:171 graf_dragbox` | PRESENT |
| GRAF_MBOX (72) | `gr_movebox` | `aes/gemgrlib.c:287` | `aes/graf.rs:248 graf_movebox` | PRESENT |
| GRAF_GROWBOX (73) | `gr_growbox` | `aes/gemgrlib.c:309` | `aes/graf.rs:261 graf_growbox` | PRESENT |
| GRAF_SHRINKBOX (74) | `gr_shrinkbox` | `aes/gemgrlib.c:326` | `aes/graf.rs:270 graf_shrinkbox` | PRESENT |
| GRAF_WATCHBOX (75) | `gr_watchbox` | `aes/gemgrlib.c:337` | `aes/graf.rs:280 graf_watchbox` | PRESENT |
| GRAF_SLIDEBOX (76) | `gr_slidebox` | `aes/gemgrlib.c:358` | `aes/graf.rs:328 graf_slidebox` | PRESENT |
| GRAF_HANDLE (77) | *(inline, kein Lib-Fn: liest `gl_wchar/gl_hchar/gl_wbox/gl_hbox/gl_handle`)* | `aes/gemsuper.c:287-293` | `aes/graf.rs:405 graf_handle` | PRESENT |
| GRAF_MOUSE (78) | `gr_mouse` (+ `ct_mouse`-Wrapper) | `aes/gemgrlib.c:419` (+ `aes/gemctrl.c`) | `aes/graf.rs:420 graf_mouse` | PRESENT |
| GRAF_MKSTATE (79) | `gr_mkstate` | `aes/gemgrlib.c:458` | `aes/graf.rs:476 graf_mkstate` | PRESENT |

### Scrap Manager (80–82)

| Opcode | EmuTOS fn | EmuTOS decl (file:line) | NewTOS fn (file:line) | Status |
|---|---|---|---|---|
| SCRP_READ (80) | `sc_read` | `aes/gemsclib.c:40` | `aes/shell.rs:115 scrp_read` | PRESENT |
| SCRP_WRITE (81) | `sc_write` | `aes/gemsclib.c:57` | `aes/shell.rs:124 scrp_write` | PRESENT |
| SCRP_CLEAR (82) `CONF_WITH_PCGEM` | `sc_clear` | `aes/gemsclib.c:74` | `aes/shell.rs:134 scrp_clear` | **DISPATCH-ONLY (Verdacht, selbst dokumentiert)** |

### File Selector Manager (90–91)

| Opcode | EmuTOS fn | EmuTOS decl (file:line) | NewTOS fn (file:line) | Status |
|---|---|---|---|---|
| FSEL_INPUT (90) | `fs_input(…, NULL)` | `aes/gemfslib.c:609` | `aes/fsel.rs:237 fsel_input` | PRESENT |
| FSEL_EXINPUT (91) | `fs_input(…, pilabel)` | `aes/gemfslib.c:609` | `aes/fsel.rs:243 fsel_exinput` | PRESENT |

### Window Manager (100–109)

| Opcode | EmuTOS fn | EmuTOS decl (file:line) | NewTOS fn (file:line) | Status |
|---|---|---|---|---|
| WIND_CREATE (100) | `wm_create` | `aes/gemwmlib.c:1369` | `aes/window.rs:1103 wind_create` | PRESENT |
| WIND_OPEN (101) | `wm_open` | `aes/gemwmlib.c:1410` | `aes/window.rs:1122 wind_open` | PRESENT |
| WIND_CLOSE (102) | `wm_close` | `aes/gemwmlib.c:1438` | `aes/window.rs:1157 wind_close` | PRESENT |
| WIND_DELETE (103) | `wm_delete` | `aes/gemwmlib.c:1466` | `aes/window.rs:1189 wind_delete` | PRESENT |
| WIND_GET (104) | `wm_get` | `aes/gemwmlib.c:1505` | `aes/window.rs:1208 wind_get` | PRESENT |
| WIND_SET (105) | `wm_set` | `aes/gemwmlib.c:1623` | `aes/window.rs:1321 wind_set` | PRESENT |
| WIND_FIND (106) | `wm_find` | `aes/gemwmlib.c:1767` | `aes/window.rs:1455 wind_find` | PRESENT |
| WIND_UPDATE (107) | `wm_update` | `aes/gemwmlib.c:1778` | `aes/window.rs:1461 wind_update` | PRESENT |
| WIND_CALC (108) | `wm_calc` | `aes/gemwmlib.c:1807` | `aes/window.rs:1466 wind_calc` | PRESENT |
| WIND_NEW (109) | `wm_new` | `aes/gemwmlib.c:1875` | `aes/window.rs:1479 wind_new` | PRESENT |

### Resource Manager (110–114)

| Opcode | EmuTOS fn | EmuTOS decl (file:line) | NewTOS fn (file:line) | Status |
|---|---|---|---|---|
| RSRC_LOAD (110) | `rs_load` | `aes/gemrslib.c:826` | `aes/resource.rs:657 rsrc_load` | PRESENT |
| RSRC_FREE (111) | `rs_free` | `aes/gemrslib.c:685` | `aes/resource.rs:824 rsrc_free` | PRESENT |
| RSRC_GADDR (112) | `rs_gaddr` | `aes/gemrslib.c:715` | `aes/resource.rs:875 rsrc_gaddr` | PRESENT |
| RSRC_SADDR (113) | `rs_saddr` | `aes/gemrslib.c:728` | `aes/resource.rs:958 rsrc_saddr` | PRESENT |
| RSRC_OBFIX (114) | `rs_obfix` | `aes/gemrslib.c:89` | `aes/resource.rs:986 rsrc_obfix` | PRESENT |

### Shell Manager (120–127)

| Opcode | EmuTOS fn | EmuTOS decl (file:line) | NewTOS fn (file:line) | Status |
|---|---|---|---|---|
| SHEL_READ (120) | `sh_read` | `aes/gemshlib.c:102` | `aes/shell.rs:157 shel_read` | PRESENT (Note: Producer-Seite (`D.s_cmd`-Befüllung durch die Shell-Loop) laut eigenem Kommentar `shell.rs:143-149` noch nicht verdrahtet — betrifft Verhalten, nicht Präsenz) |
| SHEL_WRITE (121) | `sh_write` | `aes/gemshlib.c:157` | `aes/shell.rs:184 shel_write` | PRESENT |
| SHEL_GET (122) | `sh_get` | `aes/gemshlib.c:203` | `aes/shell.rs:219 shel_get` | PRESENT |
| SHEL_PUT (123) | `sh_put` | `aes/gemshlib.c:213` | `aes/shell.rs:236 shel_put` | PRESENT |
| SHEL_FIND (124) | `sh_find` | `aes/gemshlib.c:474` | `aes/shell.rs:274 shel_find` | PRESENT (Note: nur Suchstufen (2)/(3) von EmuTOS' 4-stufigem `findfile`, laut eigenem Kommentar `shell.rs:255-273` — Verhalten, nicht Präsenz) |
| SHEL_ENVRN (125) | `sh_envrn` | `aes/gemshlib.c:327` | `aes/shell.rs:326 shel_envrn` | PRESENT |
| SHEL_RDEF (126) `CONF_WITH_PCGEM` | `sh_rdef` | `aes/gemshlib.c:491` | `aes/shell.rs:377 shel_rdef` | PRESENT |
| SHEL_WDEF (127) `CONF_WITH_PCGEM` | `sh_wdef` | `aes/gemshlib.c:504` | `aes/shell.rs:392 shel_wdef` | PRESENT |

---

## Teil 2 — Zusammenfassung

### Zähler

| | Anzahl |
|---|---|
| Opcodes total (aesdefs.h, Standard-Build) | **79** |
| PRESENT (im Dispatch verdrahtet, substantielle Implementierung) | **75** |
| DISPATCH-ONLY (Verdacht: verdrahtet, aber fester/hartkodierter Return) | **4** |
| MISSING (Opcode existiert in EmuTOS, keine Entsprechung bei uns) | **0** |

Der Dispatch in `tos-api/src/aes/mod.rs:171-261` matcht **exakt** alle 79
Opcode-Werte aus `aesdefs.h` — inklusive aller vier `CONF_WITH_*`-gegateten
Gruppen (siehe oben), d.h. NewTOS bildet den vollen Standard-Feature-Umfang nach,
nicht nur den `TARGET_192`-Minimalbuild. Kein Opcode fällt in den `default: →
RET_CODE = -1`-Zweig (`mod.rs:251-260`, das NewTOS-Äquivalent zu `crysbind`s
`unsupported`-Pfad, `gemsuper.c:406-417`).

Keine der elf Manager-Dateien (`appl.rs`, `event.rs`, `menu.rs`, `object.rs`,
`form.rs`, `graf.rs`, `fsel.rs`, `window.rs`, `resource.rs`, `shell.rs`, `pd.rs`)
enthält `todo!()`, `unimplemented!()` oder ein Wort `stub` außerhalb von
Kommentaren, die ausdrücklich *"not a stub"* erklären (`shell.rs:14`, `:148`;
`object.rs:13`; `form.rs:9`) — gegengeprüft per Volltextsuche.

### MISSING-Funktionen

**Keine.** Jeder der 79 EmuTOS-AES-Opcodes hat eine verdrahtete Entsprechung im
NewTOS-Dispatch.

### DISPATCH-ONLY-Verdachtsfälle (für die Verhaltens-Auditoren)

Diese vier geben unabhängig vom Input einen fest verdrahteten Wert zurück. Alle
vier sind im Code selbst als bewusste Lücke kommentiert (kein blinder Stub), aber
sie erfüllen das Kriterium "hartkodierter Return" und gehören in die
P1/P6-Tiefenprüfung:

1. **`appl_tplay`** (Opcode 14, `appl.rs:395`) — gibt immer `0` zurück ("no path
   to inject events back into the AES input"); EmuTOS `ap_tplay` ist `void`
   (kein Return-Wert außer dem impliziten `RET_CODE`-Default `TRUE`,
   `gemsuper.c:109-111`), spielt aber tatsächlich den Event-Tape ab
   (`gemaplib.c:119-188`) — NewTOS tut das nicht.
2. **`appl_trecord`** (Opcode 15, `appl.rs:387`) — gibt immer `0` zurück
   ("NewTOS has no event-tape recorder"); EmuTOS `ap_trecd` zeichnet real auf und
   gibt die Anzahl aufgezeichneter Events zurück (`gemaplib.c:189-249`).
3. **`appl_yield`** (Opcode 17, `appl.rs:307`) — gibt immer `0` zurück. EmuTOS'
   `APPL_YIELD`-Zweig ruft nur `dsptch()` auf und lässt `ret` auf seinem
   Initialwert `TRUE` (`gemsuper.c:85,116-119`) — der EmuTOS-Rückgabewert wäre
   also `1`, nicht `0`. Das ist ein Verhaltens-/Return-Code-Verdacht, keine
   Präsenzlücke (die Funktion ist inhaltlich richtig: mit einer einzigen
   AES-Anwendung gibt es nichts, wohin `dsptch()` umschalten könnte).
4. **`scrp_clear`** (Opcode 82, `shell.rs:134`) — gibt immer `0` zurück, mit
   explizitem Kommentar "Not implemented yet (needs directory-enumerated
   deletion); honestly reports 0 rather than faking success". EmuTOS `sc_clear`
   löscht real `SCRAP.*`-Dateien im Scrap-Verzeichnis (`gemsclib.c:74-92`).

### CONF_WITH_*-Gating — Zusammenfassung

Keine der gegateten Gruppen ist bei uns "legitim MISSING": NewTOS dispatcht alle
vier Gruppen (APPL_YIELD; MENU_POPUP/ATTACH/ISTART/SETTINGS; OBJC_SYSVAR;
SCRP_CLEAR + SHEL_RDEF/WDEF), passend zum EmuTOS-Standard-Build-Default (`1` für
alle drei Switches, siehe Tabelle oben). Nur `sc_clear`s Payload ist (Verdacht,
s.o.) nicht vollständig — der Opcode selbst ist nicht ausgelassen.

### Cross-Referenz zu bestehenden Audits

Diese Matrix ist ein Coverage-Rollup; die Tiefenprüfung pro Funktion (exakte
Semantik, Edge-Cases, Return-Codes) läuft in den bereits vorhandenen
`p1-appl-pd-dispatch.md` und den noch ausstehenden `p3`–`p6`-Katalogen laut
`README.md`. Gegengeprüft: drei der vier DISPATCH-ONLY-Verdachtsfälle sind in
`p1-appl-pd-dispatch.md` **bereits als eigene Findings erfasst** —
`appl_yield` → `AUD-P1-002` (`p1-appl-pd-dispatch.md:54`), `appl_tplay` →
`AUD-P1-003` (`:66`), `appl_trecord` → `AUD-P1-014` (`:202`), alle mit Severity
und 1:1-Fix. **`scrp_clear` (Opcode 82) ist in keinem vorhandenen Katalog
erfasst** — gehört zu Paket P6 (`p6-rsrc-fsel-scrp-shel.md`), das laut
`README.md` noch nicht geschrieben ist; dieser Fund sollte dort als eigenes
`AUD-P6-NNN` aufgenommen werden.
