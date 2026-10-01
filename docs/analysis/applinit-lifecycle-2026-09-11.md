# `appl_init` im Lebenszyklus — EmuTOS-Kette, NewTOS-Kette, Abweichungen

**Datum:** 2026-09-11 · **Art:** reine Quellenanalyse, keine Codeänderung, kein Fix,
kein Commit, keine Instrumentierung.
**Gepinnte EmuTOS-Quelle:** `references/emutos` @
`1a7484aba1bfba23e2c0899e243b6d8177fe41df` (2026-04-14, „Update GitHub Actions to
use Node 24"; `git -C references/emutos log -1` selbst nachgeprüft).
**NewTOS-Stand:** Arbeitsbaum auf `desk-rebuild`, letzter Commit `70caee2`,
darüber uncommittete Arbeit mehrerer Stränge.

---

## 1. Die Frage

Was läuft bei EmuTOS rund um `appl_init` — davor, darin und danach —, welchen
Zustand findet ein frisch gestartetes GEM-Programm vor, und wo weicht NewTOS
davon ab? Anlass ist die Panik in `blit_plane`
(`tos-api/src/aes/object.rs:3006`) beim Start von TeraDesk und QED.

---

## 2. Die EmuTOS-Kette

### 2.1 Wer ist überhaupt „der Prozess"?

Der AES legt seine Prozessdeskriptoren **einmal beim Boot** an und nie wieder:

| Ort | Inhalt |
|---|---|
| `aes/geminit.c:819` | `totpds = num_accs + 2` — AESPD 0 = Anwendung, 1 = SCRENMGR, 2… = Accessories |
| `aes/geminit.c:851-874` | Boot-Init **jedes** AESPD: `p_qaddr = p_queue`, `p_qindex = 0`, `p_name = ' '×8`, `p_appdir[0] = 0`, `p_pid = i`, `p_stat = 0`, `p_msg.action = -1`. `p_flags` bleibt 0 (BSS bzw. `bzero`, `:816`) |
| `aes/geminit.c:875-877` | `curpid = 0; rlr->p_pid = curpid++; rlr->p_link = NULL` — `rlr` ist AESPD 0 |
| `aes/gempd.c:60-70` | `getpd`: setzt **nur** `p_pid = curpid++`, löscht nichts |
| `aes/gempd.c:110-131` | `pstart` = `getpd` + `p_nameit` + `p_setappdir` + `psetup` + Einhängen |
| `aes/geminit.c:215-227, 236-257` | Die **einzigen** `pstart`-Aufrufer: SCRENMGR (`:888`) und `load_one_acc`. Für eine gestartete Anwendung wird **nie** ein AESPD erzeugt. |

Folge: Desktop und jedes vom Desktop gestartete Programm teilen sich **denselben
Deskriptor AESPD 0**. Alles, was an der Startgrenze passieren muss, muss also
ausdrücklich passieren — der Deskriptor wird nicht ausgetauscht.

### 2.2 Davor — die Shellschleife

`aes/gemshlib.c:692-731` (`sh_main`), Rumpf pro Durchlauf:

```
:712  sh_chdef(psh)           Kommandozeile für den nächsten Start zusammensetzen
:713  sh_chgrf(psh)           Alpha-/Grafikmodus umschalten (sh_tographic, :223-239)
:717  wm_init()               ← der große Rücksetzer, siehe unten
:718  ratinit()               Maus einschalten
:719  sh_draw(D.s_cmd, TRUE)  Bildschirm weiß (nur ROOT der AES-eigenen DESKTOP-
                              Baums, ob_spec temporär CLEAR_SCREEN, :269-289)
:722  fm_error() für rc des vorigen Durchlaufs
:725  rc = sh_ldapp(psh)
```

`wm_init` (`aes/gemwmlib.c:1247-1326`) ist die eigentliche Zustandshygiene vor
**jedem** Programm:

* `or_start()` (`:1258`) — Rechteckvorrat neu,
* `bzero(W_TREE, …)`, `w_nilit`, `w_flags = 0`, `w_rlist = NULL` für alle Fenster (`:1261-1268`),
* `W_TREE[ROOT].ob_spec = rs_trees[DESKTOP]->ob_spec` (`:1270-1271`),
* Fensterelemente aus `gl_watype`/`gl_waspec` (`:1274-1284`) — darin **kein**
  `G_ICON`/`G_IMAGE` (`:61-105`), die Fensterdekoration kann also nie in einen
  Icon-Blit laufen,
* Desktopfenster = ganzer Schirm (`:1287-1294`),
* `gl_wtop = NIL`, **`gl_newdesk = NULL`** (`:1297-1300`).

`sh_ldapp` (`aes/gemshlib.c:592-689`), Arm „normales/Autorun-Programm":

```
:629  ret = (sh_nextapp == AUTORUN_APP) ? 1 : sh_find(D.s_cmd)
:633  sh_show(fname)          Namen in die Menüzeile
:634  p_nameit(rlr, fname)    p_name neu belegen  (gempd.c:77-86)
:635  p_setappdir(rlr, …)     p_appdir neu belegen (gempd.c:90-107)
:636  rlr->p_flags = 0;       ← alle Flags weg, nicht nur AP_OPEN
:645  ret = dos_exec(PE_LOADGO, D.s_cmd, ad_stail, ad_envrn)
```

Beim ROM-Desktop-Arm (`:600-612`) fehlt `p_flags = 0` — dort ist es entbehrlich,
weil der Vorgänger-Aufräumer (`:650-657`) `AP_OPEN` schon gelöscht hat.

**Zustand beim Einsprung in das Kindprogramm:** ein AESPD mit `p_flags == 0`,
`p_qindex == 0` (geleert, s. 2.4), frisch gesetztem `p_name`/`p_appdir`, keinem
offenen Fenster, `gl_newdesk == NULL`, weißem Schirm, aktiver Maus. Der
`AESGLOBAL`-Block des Programms liegt in **dessen eigenem** Speicher und ist
das, was sein C-Startup hinterlassen hat (BSS ⇒ 0).

### 2.3 Darin — `ap_init`

`aes/gemaplib.c:54-63`, vollständig:

```c
WORD ap_init(void)
{
    WORD pid = rlr->p_pid;
    rlr->p_flags |= AP_OPEN;    /* appl_init() done */
    return pid;
}
```

Der Dispatcher-Arm davor, `aes/gemsuper.c:90-100`:

```c
pglobal->ap_version = AES_VERSION;
pglobal->ap_count   = 1;
pglobal->ap_id      = rlr->p_pid;
tree = rs_trees[DESKTOP];
pglobal->ap_private = tree[ROOT].ob_spec;
pglobal->ap_planes  = gl_nplanes;
pglobal->ap_3resv   = (LONG)&D;
ret = ap_init();
```

**Was `appl_init` ausdrücklich NICHT tut:**

* es leert die Nachrichtenwarteschlange **nicht** (`p_queue`/`p_qindex` bleiben),
* es löscht keine anderen Flags (`AP_MESAG` 0x02, `AP_ACCLOSE` 0x04 bleiben —
  `aes/struct.h:115-117`),
* es setzt weder `p_name` noch `p_appdir`,
* es kann **nicht scheitern** (`rlr` existiert immer),
* und — für diese Untersuchung entscheidend — es fasst
  **`ap_ptree`, `ap_rscmem`, `ap_rsclen` und `ap_4resv` nicht an**
  (`aes/gemrslib.h:15-27`: Feld 5/6, 7/8, 9, 13/14). Ein Programm findet dort
  also genau das vor, was es selbst hineingeschrieben hat.

### 2.4 Danach — bis das Programm zeichnen kann

Der typische Anfang ist `rsrc_load` → `rs_load` (`aes/gemrslib.c:826-850`):

```
:836  sh_find(tmprsfname)                        Datei suchen (p_appdir zuerst)
:839  dos_open(…, 0)
:844  rs_readit(pglobal, fd)
:846  rs_fixit(pglobal)
:847  dos_close(fd)
```

`rs_readit` (`:751-808`) — die Fixup-Kette, in dieser Reihenfolge:

| Zeile | Schritt |
|---|---|
| `:758-762` | RSHDR lesen, `rslsize = rsh_rssize` |
| `:766-772` | bei `NEW_FORMAT_RSC` die echte Größe vom Dateiende nachlesen |
| `:775` | **`rs_hdr = dos_alloc_anyram(rslsize)`** — die Resource liegt im **Speicher der Anwendung** |
| `:780-783` | ganze Datei einlesen |
| `:786-788` | **`rs_global = pglobal; rs_global->ap_rscmem = rs_hdr; rs_global->ap_rsclen = rslsize;`** |
| `:794` | `fix_trindex()` → `:596-597` **`rs_global->ap_ptree = ptreebase`**, dann jeder Baumzeiger `+= rs_hdr` |
| `:796` | `fix_cicons()` (CONF_WITH_COLOUR_ICONS) |
| `:798` | `fix_tedinfo_std()` → `te_ptext`/`te_ptmplt`/`te_pvalid` |
| `:800-802` | `fix_nptrs(nib, R_IBPMASK / R_IBPDATA / R_IBPTEXT)` — **hier entstehen `ib_pmask`/`ib_pdata`** |
| `:803` | `fix_nptrs(nbb, R_BIPDATA)` — `bi_pdata` |
| `:804-805` | `fix_nptrs(nstring, R_FRSTR)`, `fix_nptrs(nimages, R_FRIMG)` |

Jeder einzelne Fixup ist `fix_long` (`:576-588`): `*plong = (LONG)rs_hdr +
lngval`, **außer** bei `-1L` (bleibt stehen). `rs_fixit` (`:816-820`) ruft dann
`fix_objects` (`:606-637`): `rs_obfix` (Zeichen→Pixel) und `ob_spec`-Fixup, wobei
`G_BOX`/`G_IBOX`/`G_BOXCHAR` ausgenommen sind und `G_CICON` einen Tabellenindex
statt eines Offsets trägt.

Alle späteren Zugriffe gehen über `rs_sglobe(pglobal)` (`:675-679`), das die
beiden AES-Statics `rs_hdr`/`rs_global` (`:44-45`) **bei jedem Aufruf neu** aus
dem übergebenen `AESGLOBAL` setzt. `rs_gaddr` (`:715-721`), `rs_saddr`
(`:728-742`), `rs_free` (`:685-708`) — alle drei tun das als erstes. Der
Resourcezustand ist damit in EmuTOS **vollständig prozessgebunden**: er lebt im
`AESGLOBAL`-Array der Anwendung; die AES-Statics sind nur ein pro Aufruf
nachgeladener Cache.

### 2.5 Die Rücksetzungen an der Grenze Desktop → Kindprogramm

Vollständige Liste, in Ablaufreihenfolge:

| # | Ort | Wirkung |
|---|---|---|
| 1 | `desk/deskmain.c:2154-2155` | `cnx_put()`, `app_save(FALSE)` — Desktopzustand sichern |
| 2 | `desk/deskmain.c:2160` | `menu_bar(NULL, 0)` — Menüzeile weg |
| 3 | `desk/deskmain.c:2163` | `wind_set(DESKWH, WF_NEWDESK, NULL, 0, 0)` — Hintergrundbaum abmelden |
| 4 | `desk/deskmain.c:2166` | `appl_exit()` → `ap_exit` (`aes/gemaplib.c:251-261`): `wm_update(BEG_UPDATE)`, `mn_cleanup()` (`gemmnlib.c:740-758`: `AC_CLOSE` an alle DAs, Submenü-Puffer frei), `wait_for_accs(AP_ACCLOSE)` (`geminit.c:599-627`), **Warteschlange leeren** (`:256-257`), `wm_update(END_UPDATE)`, `all_run()` (`geminit.c:554-566`), `p_flags &= ~AP_OPEN` |
| 5 | Prozessende | `dos_exec(PE_GOTHENFREE, …)` gibt den Speicher des Desktopprozesses frei (`gemshlib.c:579`) — samt seiner RSC-Blöcke |
| 6 | `aes/gemshlib.c:650-657` | Falls das beendete Programm `AP_OPEN` hinterließ: `mn_cleanup`, Queue leeren, `AP_OPEN` löschen |
| 7 | `aes/gemshlib.c:668-669` | `if (wind_spb.sy_owner == rlr) unsync(&wind_spb)` — Bildschirmsperre lösen |
| 8 | `aes/gemshlib.c:676-677` | `wm_new()` (`gemwmlib.c:1875-1893`): alle `ml_ocnt`/`sy_tas`-Sperren lösen, Fenster 1…N schließen und löschen |
| 9 | `aes/gemshlib.c:717` | `wm_init()` — s. 2.2, inkl. `gl_newdesk = NULL` |
| 10 | `aes/gemshlib.c:719` | `sh_draw(D.s_cmd, TRUE)` — Schirm weiß |
| 11 | `aes/gemshlib.c:634-636` | `p_nameit`, `p_setappdir`, **`rlr->p_flags = 0`** |

Es sind also **elf** Schritte, nicht zwei. Die beiden im Auftrag genannten
(`deskmain.c:2166`, `gemshlib.c:636`) sind die auffälligsten, aber `wm_init`
(#9) ist der mit der größten Reichweite — und der einzige, der `gl_newdesk`
zurücksetzt.

---

## 3. Die NewTOS-Kette

### 3.1 Der reproduzierte Startweg ist der Autorun-Arm, nicht der Desktop

Aus dem Protokoll des Anwenders: `sh_ldapp: starting C:\APPS\TERADESK\DESKTOP.NAP,
sh_nextapp=2`. `sh_nextapp == 2` ist `AUTORUN_APP`
(`tos-api/src/aes/shell.rs:60`; `NORMAL_APP = 0`, `CONSOLE_APP = 1`,
`DESKTOP_APP = 3`, `:54-62`). TeraDesk läuft also als **erstes** Programm des
Bootlaufs über `kernel/src/shell_loop.rs:254-293`; der eingebaute Desktop
(`crate::desk::run()`, `:240`) läuft in diesem Lauf **überhaupt nicht**.

Ablauf bis zum Einsprung:

| Schritt | Ort |
|---|---|
| `sh_main` → `wm_init()` / `ratinit()` / `sh_draw_clear()` | `kernel/src/shell_loop.rs:82-87` (EmuTOS `:715-720`) |
| `sh_ldapp` Arm 3, `p_nameit_and_appdir(&cmd)` — **nur** `set_app_dir`, das EmuTOS-`rlr->p_flags = 0` fehlt ausdrücklich | `kernel/src/shell_loop.rs:278, 343-357` |
| `run_program_with_tail_rc` → `set_app_dir(path)` | `kernel/src/launch.rs:98` |
| `register_process(child.slot, path)` → `pd::register(slot+1, path)` | `kernel/src/launch.rs:102`, `tos-api/src/aes/mod.rs:516-522` |
| `set_current(basepage)`, `[launch] EL0: slot 0 entry …` | `kernel/src/launch.rs:116-117` |
| im Programm: `appl_init()` → `pd::open(current_pid())` | `tos-api/src/aes/appl.rs:243-269`, `pd.rs:295-311` |

`pd::register` (`tos-api/src/aes/pd.rs:528-544`) belegt einen freien Slot mit
`Aespd::EMPTY` (`:254-260`: `flags = 0`, leere Queue, Name Leerzeichen) und setzt
danach nur den Namen; bei **vorhandener** PID nur den Namen. `pd::open`
(`:295-311`) ODert seit dem A25-Fix nur noch `AP_OPEN` hinein.

### 3.2 Was `appl_init` in den `AESGLOBAL` schreibt

`tos-api/src/aes/appl.rs:243-269` füllt über `AesGlobalVals::write`
(`:118-184`) genau sechs Felder — `ap_version` [0], `ap_count` [1], `ap_id` [2],
`ap_private` [3…] (4 B ILP32 / 8 B LP64), `ap_planes` [10], `ap_3resv` [11-12] —
und **nichts** sonst; die Modulnotiz `:9-77` belegt das gegen `gemsuper.c:92-98`.
Das deckt sich mit EmuTOS. Gemessen bestätigt: der Referenzlauf
`measurements/measurement-2026-09-10-qemu/run3-qemu-rlist-all6/BOOT-SERIAL.txt` zeigt
`global after appl_init 832,1,1,4419,0,0,0,0,0,0,16,17320,18,0,0` — Index 5…9
(`ap_ptree`, `ap_rscmem`, `ap_rsclen`) sind 0.

### 3.3 Der Resourcezustand ist bei uns NICHT prozessgebunden

| Zustand | EmuTOS | NewTOS |
|---|---|---|
| Basisadresse der Resource | `pglobal->ap_rscmem` im App-Speicher (`gemrslib.c:787`) | Kernel-Static `RSC.obj_base`/`*_target`/`*_access` (`tos-api/src/aes/resource.rs:263-289`, gesetzt in `relocate_and_register_core`, `:322-460`) |
| Länge | `pglobal->ap_rsclen` (`gemrslib.c:788`) | nirgends abgelegt |
| Baumtabelle | `pglobal->ap_ptree` (`gemrslib.c:597`) | Kernel-Static `RSC.trees[…]` (`resource.rs:345-350`) |
| Freigabe-Handle | derselbe `ap_rscmem` (`gemrslib.c:699`) | Kernel-Static `USER_RSC_RAW` (`resource.rs:832, 1049, 1067`) |
| Anwendungsverzeichnis | `rlr->p_appdir` pro AESPD (`gempd.c:90-107`) | Kernel-Static `APP_DIR` (`resource.rs:840-870`) |
| Umschalten auf eine 2. Resource | Anwendung tauscht `global[5..8]` und ruft weiter `rs_gaddr` — `rs_sglobe` (`:675-679`) folgt ihr | nicht möglich: `rsrc_gaddr` liest immer `RSC` |

`tos-api/src/aes/resource.rs` enthält **keinen einzigen** Zugriff auf
`pb.global_ptr()` (selbst nachgegriffen: `grep -n "global_ptr\|global(" ` liefert
nichts). `rsrc_load` (`:892-974`) schreibt ausschließlich in `RSC`,
`USER_RSC_RAW` und den Pool.

**Wer liest diese Felder auf der Anwendungsseite?** TeraDesk, direkt und an sechs
Stellen:

* `sdk/apps/teradesk/rsc_load.c:417-423` — `xrsrc_fix` **schreibt** selbst
  `aes_global[5]` (`ap_ptree`) und `aes_global[7]` (`ap_rscmem`);
* `:485-486` — `xrsrc_free` gibt `Mfree(aes_global[7])` frei;
* `:504-508` — `xrsrc_gaddr` **liest** `aes_global[7]` als Resourcebasis;
* `sdk/apps/teradesk/icon.c:2692` — `aes_supports_coloricons` vergleicht
  `ob_spec.index` gegen `_AESrscmem`;
* `icon.c:2707-2708, 2742-2743, 2751-2752, 2762-2763` — `load_icons` sichert und
  stellt `_AESrscfile`/`_AESrscmem` um die eigene `icons.rsc`-Ladung herum wieder
  her;
* `icon.c:2771-2784` — `free_icons` desgleichen.

Die Makros stehen in `sdk/apps/teradesk/library/xdialog/library.h:65-83` bzw.
`sdk/gemlib/gemlib-0.44.0-m68k/gem.h:49-64`. Die Bauanleitung
`tools/make-sd-image.sh:272-283` bestätigt die Zweiteilung: `desktop.rsc` im
NRSC/LP64-Format für den Kernel-`rsrc_load`, `icons.rsc` im nativen RSHDR-Format
**für TeraDesks eigenen Parser**, `cicons.rsc` absichtlich nicht installiert.

Damit ist die Arbeitshypothese des Auftrags **teils bestätigt, teils widerlegt**:

* Bestätigt: `global[5..9]` sind bei GEM tatsächlich die prozessgebundenen
  Resourcefelder, und NewTOS füllt sie nie.
* Widerlegt für die Icons: die `ICONBLK`-Zeiger der TeraDesk-**Schreibtischicons**
  stammen **nicht** aus unserem `rsrc_load`, sondern aus TeraDesks eigenem
  `xrsrc_fix` (`rsc_load.c:340-434`), das `ib_pmask`/`ib_pdata` selbst aus dem
  `Malloc`-Block berechnet und `aes_global[5]/[7]` selbst setzt. Diese Kette
  hängt nicht davon ab, ob `appl_init` etwas erhält oder zurücksetzt.

### 3.4 Wo die Icons gezeichnet werden

`regen_desktop` (`sdk/apps/teradesk/icon.c:2509-2518`) meldet den
Schreibtischbaum mit `wind_set_ptr_int(0, WF_NEWDESK, desk_tree, 0)` an. Unser
`set_newdesk` (`tos-api/src/aes/window.rs:933-957`) merkt sich Zeiger, Wurzel,
Herkunft und ABI; `draw_desk_tree` (`:1097-1138`) zeichnet diesen Baum bei
**jeder** Hintergrundauffrischung mit `render_tree_app(…, MAX_DEPTH, …)`
(`object.rs:1013-1028`), also **einschließlich aller `G_ICON`-Kinder** →
`just_draw`-Arm `object.rs:2539-2555` → `draw_icon` (`:2906-2977`) →
`blit_plane` (`:2984-3026`). Das ist der einzige belegte Pfad, auf dem
anwendungseigene `ICONBLK`-Zeiger in die Panikstelle laufen; die
Fensterdekoration kann es nicht sein (EmuTOS `gl_watype`, `gemwmlib.c:61-82`,
enthält kein `G_ICON`/`G_IMAGE`, unsere Nachbildung ebenso wenig).

---

## 4. Die Abweichungen, benannt

Nach Schwere, jeweils EmuTOS-Beleg → NewTOS-Beleg → Beobachtbarkeit.

**A1 — `rsrc_load` füllt `ap_rscmem`/`ap_rsclen`/`ap_ptree` nicht.**
`gemrslib.c:597, 787-788` gegen `tos-api/src/aes/resource.rs:892-974` (kein
`global_ptr`-Zugriff). Über die dokumentierte API beobachtbar: ein Programm, das
nach erfolgreichem `rsrc_load` `global[5..9]` liest, bekommt bei TOS die
Resourcebasis, bei uns 0. TeraDesk liest sie (`icon.c:2692, 2707-2708`,
`rsc_load.c:504-508`). **Echter Fehler, keine hinnehmbare Abweichung.**
Für die Panik allein nicht hinreichend, weil TeraDesks Icon-Kette
`aes_global[5]/[7]` vor jeder Benutzung selbst belegt (`rsc_load.c:417-423`).

**A2 — Der Resourcezustand ist ein Kernel-Global statt prozessgebunden.**
`gemrslib.c:675-679` (`rs_sglobe` folgt dem übergebenen `pglobal`) gegen
`resource.rs:263-289` (ein `static RSC`), `:832` (ein `USER_RSC_RAW`), `:848`
(ein `APP_DIR`). Beobachtbar: zwei gleichzeitig gehaltene Resourcen sind bei uns
unmöglich; ein zweiter `rsrc_load` ersetzt den ersten für alle, und `rsrc_free`
gibt danach den zuletzt geladenen Block frei. TeraDesk umgeht das genau deshalb
mit einem eigenen Lader. **Echter Fehler**, sichtbar für jede Anwendung, die dem
in `rsc_load.h:82-89` beschriebenen GEM-Muster folgt.

**A3 — `rlr->p_flags = 0` an der Startgrenze fehlt.**
`gemshlib.c:636` gegen `kernel/src/shell_loop.rs:343-357` (schriftlich
zurückgestellt). Zusammen mit dem A25-Fix (`pd.rs:309`, nur noch `|= AP_OPEN`)
gibt es damit an der Grenze Desktop → Kind **keine** Rücksetzung mehr; das ist
der Befund B1 des A25-Audits (`TASK/A25-AUDIT-BEANSTANDET.MD`), den ich hier
bestätige. **Im hier reproduzierten Autorun-Lauf aber nicht erreichbar** (s. 5).

**A4 — `appl_exit()` an der Startgrenze fehlt.**
`desk/deskmain.c:2166` → `gemaplib.c:251-261` (leert die Queue) gegen
`kernel/src/desk/main.rs:529-548` (`menu_bar`, `save_windows`,
`wind_set_newdesk(0,0)`, `sh_draw_clear` — kein `appl_exit`). Ebenfalls B1.

**A5 — Deskriptorleck für Programme ohne `appl_init`.**
`gemshlib.c:650-657` löscht `AP_OPEN` nur, wenn es gesetzt war — bei EmuTOS
folgenlos, weil `:636` ohnehin alles nullt. Bei uns bleibt der Slot stehen
(`pd.rs:466-475` prüft `AP_OPEN`), weil A3 fehlt. Konkret in diesem Bootlauf:
`C:\AUTO\HELLO.NCP` läuft in Slot 0, `register_process` legt PID 1 an, `.NCP`
ruft nie `appl_init`, `close_client_if_open(1)` lässt den Deskriptor stehen —
TeraDesk erbt ihn. Hier harmlos (Flags 0, Queue leer), grundsätzlich aber ein
Leck: nach acht verschiedenen GEMDOS-Slots ist die Tabelle voll.

**A6 — `appl_init` kann bei uns scheitern, bei EmuTOS nicht.**
`gemaplib.c:54-63` (kein Fehlerpfad) gegen `appl.rs:246-250` (`-1` bei voller
8er-Tabelle). Mit A5 zusammen wird das erreichbar.

**A7 — `wm_init` setzt `NEWDESK_ABI` nicht zurück.**
`gemwmlib.c:1300` setzt `gl_newdesk = NULL`; unser `wm_init`
(`window.rs:3030-3043`) setzt `NEWDESK_TREE`/`_ROOT`/`_APP` zurück, `NEWDESK_ABI`
(`:923`) bleibt stehen. Derzeit **unbeobachtbar**, weil `NEWDESK_ABI` nur gelesen
wird, wenn `NEWDESK_APP` wahr ist, und das wird bei jeder Anmeldung neu gesetzt
(`:947-948`). Nebenbefund, kein Fehler.

**A8 — `rsrc_load` kappt bei 64 KiB.**
`gemrslib.c:775` (`dos_alloc_anyram(rslsize)`, unbegrenzt) gegen
`resource.rs:802` (`RSC_CAP = 64 * 1024`) und `:955` (`len < header` → nächster
Pfad). Für `desktop.rsc` (39 043 B im letzten Bau) unkritisch, für
`cicons.rsc` (63 788 B klassisch) grenzwertig. **Latente Abweichung**, hier nicht
auslösend, weil `cicons.rsc` nicht installiert wird
(`tools/make-sd-image.sh:274-276`).

---

## 5. Erklärt das die Panik? — **NEIN für `pd.rs`, UNENTSCHIEDEN insgesamt**

### 5.1 Was die Panik technisch aussagt

`object.rs:3006` ist `core::slice::from_raw_parts(addr as *const u16, total)`.
Die Vorbedingung lautet vollständig (Toolchain `nightly-2026-04-15`,
`library/core/src/slice/raw.rs:129-137`): *aligned and non-null* **und**
*total size ≤ isize::MAX*. Die zweite Hälfte kann hier nicht greifen:
`stride: u16 ≤ 65535`, `rows = src_y.max(0) + dst.h ≤ 65534`, also
`total ≤ 4,29e9` Elemente = 8,6 GB ≪ `isize::MAX`. Null ist bei `:3000`
ausgeschlossen. **Es bleibt genau eine Möglichkeit: `addr` ist ungerade.**
(Bewiesen durch Ausschluss, nicht reproduziert.)

Ein ungerader Zeiger entsteht nicht aus Geometrie. Er entsteht, wenn
(a) ein `ICONBLK`/`BITBLK` mit der falschen ABI-Breite gelesen wird,
(b) ein Objekt fälschlich als `G_ICON`/`G_IMAGE` behandelt wird, weil der
Baumlauf über das Array hinausläuft oder der Baumzeiger falsch ist, oder
(c) die Zeiger nie relokiert wurden.

### 5.2 Warum `pd.rs` als Ursache ausscheidet

Drei unabhängige Gründe:

1. **Der Startweg schließt sie aus.** `sh_nextapp=2` = `AUTORUN_APP`: TeraDesk
   ist das **erste** GEM-Programm des Bootlaufs, der eingebaute Desktop läuft
   nicht (`shell_loop.rs:229-247` wird nicht betreten). Es gibt keinen
   Vorbelegten, dessen Zustand `appl_init` erben könnte. Der Deskriptor für PID 1
   stammt aus `pd::register` (`pd.rs:531-541`, `*slot = Aespd::EMPTY`) bzw. aus
   dem `.NCP`-Lauf davor (Flags 0, Queue leer). `slot.flags |= AP_OPEN`
   (neu) und `slot.flags = AP_OPEN; queue.clear()` (alt) liefern auf einem
   Deskriptor mit `flags == 0` und leerer Queue **byteidentische** Ergebnisse.
2. **Der Regressionsmechanismus passt nicht.** Selbst wenn B1 aus dem
   A25-Audit greifen würde, ist seine Wirkung eine **geerbte `WM_REDRAW`-Nachricht**
   — ein falsches Rechteck, kein ungerader Zeiger. Von einer Nachricht in der
   Warteschlange führt kein Weg zu `ib_pdata`.
3. **Der Zeitstempel.** `tos-api/src/aes/pd.rs` wurde zuletzt am
   **2026-09-10 08:04** geändert (`stat`), also gestern früh. Der QEMU-Lauf
   `measurements/measurement-2026-09-10-qemu/run3-qemu-rlist-all6/BOOT-SERIAL.txt` vom
   **2026-09-11 00:31** enthält diesen Stand und bootet sauber durch
   `DUMPAES.NAP` und den Desktop.

**Die Hypothese des Auftrags ist für den reproduzierten Fall widerlegt.**
Das entwertet weder den A25-Fix noch den A25-Audit-Befund B1 — beide bleiben
gültig für den Desktop→Kind-Weg —, aber sie erklären diese Panik nicht.

### 5.3 Was übrig bleibt: `window.rs`, aber ohne belegten Mechanismus

Zeitstempel der beteiligten Dateien (`stat -f %Sm`):

| Datei | zuletzt geändert |
|---|---|
| `tos-api/src/aes/window.rs` | **2026-09-11 10:24** |
| `tos-api/src/aes/pd.rs` | 2026-09-10 08:04 |
| `tos-api/src/aes/object.rs` | 2026-09-02 09:40 |
| `kernel/src/dbg_qedtrace.rs`, `lib.rs`, `Cargo.toml`, `bootlog.rs`, `drivers/usb/**`, `hal/uart.rs` | 2026-09-09 |
| `tos-api/src/xbios/screen.rs`, `tools/make-sd-image.sh` | 2026-09-09 |

`window.rs` ist die **einzige** heute geänderte Quelldatei; ihre letzte Änderung
(10:24) fällt mit `TASK/A16-STATUS-UNFINISHED.MD` (10:29) zusammen. Zwei
Korrekturen am Auftragstext: (i) heute geändert wurde **eine** Datei, nicht zwei;
(ii) der Arbeitsbaum trägt darüber hinaus einen **dritten** uncommitteten Strang
im Kernel (USB/xHCI, `+625` Zeilen in `drivers/usb/xhci/mod.rs`, dazu
`bootlog.rs`, `lib.rs`, `Cargo.toml`), der in der Aufzählung fehlt und je nach
Baustand des „gestrigen" Kernels ebenfalls neu sein kann.

Was ich in `window.rs` **nicht** gefunden habe: irgendeinen Zeiger-, ABI- oder
Resourcezugriff. Der vollständige Diff (664 +, 130 −) enthält keine Zeile mit
`intin`/`addrin`/`ObjTree`/`abi`/`ob_spec`/`as *`-Konvertierung außerhalb der
entfernten `RList`-Statics (selbst durchgesehen). `window.rs` kann die Panik
daher nur **mittelbar** auslösen — indem es ändert, *welcher* Baum *wann*
zusammengesetzt wird. Der einzige Pfad dafür ist `draw_desk_tree`
(`:1097-1138`) über `NEWDESK_TREE`. Einen konkreten Auslöser dafür konnte ich
durch Lesen nicht belegen; `wm_init` (`:3030-3043`) setzt `NEWDESK_TREE`
ordnungsgemäß zurück, und `set_newdesk` (`:933-957`) ist unverändert.

**Verdikt: unentschieden.** Die Ursache liegt nach der Zeitstempel- und
Pfadanalyse mit deutlichem Abstand wahrscheinlicher in `window.rs` als in
`pd.rs`, aber ich habe für `window.rs` **keinen** Mechanismus, der einen
ungeraden Zeiger erzeugt. Eine gebastelte Erklärung liefere ich nicht.

---

## 6. Das entscheidende Experiment (nicht ausgeführt)

Drei Messungen, in dieser Reihenfolge; die erste allein entscheidet
wahrscheinlich schon.

**E1 — A/B über den einen heute geänderten Stand.**
`tos-api/src/aes/window.rs` aus `HEAD` (`70caee2`) per `git show
HEAD:tos-api/src/aes/window.rs` in eine **Kopie** des Baumes exportieren (nicht
`checkout`), Kernel bauen, mit demselben SD-Abbild und demselben `#Z`-Eintrag in
QEMU `raspi4b` booten. Startet TeraDesk, ist die Regression in `window.rs`
und `pd.rs` endgültig aus dem Spiel. Startet es nicht, ist keine der beiden
Dateien die Ursache und der dritte (Kernel-/USB-)Strang bzw. ein neu gebautes
`DESKTOP.NAP`/`icons.rsc` rückt in den Vordergrund. Gegenprobe mit
`pd.rs` aus `HEAD` bei heutigem `window.rs`.

**E2 — den Wert sichtbar machen, der die Panik auslöst.**
Genau eine Stelle, genau vier Werte: in `blit_plane`
(`tos-api/src/aes/object.rs:3000-3006`), **vor** dem
`from_raw_parts`, bei `addr & 1 != 0` einmal ausgeben:
`addr`, `stride`, `dst.w/h`, und — vom Aufrufer durchgereicht — ob der Aufruf aus
`blit_image` (`:2891`), `draw_icon`-Maske (`:2927`) oder `draw_icon`-Daten
(`:2928`) kam, dazu `ob_spec` und `t.abi()` des zeichnenden Objekts
(`object.rs:2550-2554`). Damit ist in einem Lauf entschieden, welcher der drei
Mechanismen aus 5.1 vorliegt:
* `addr` sieht aus wie eine kleine Zahl (< Dateigröße) → nicht relokiert;
* `addr` liegt im Bereich des Anwendungs-`Malloc`-Blocks, aber ungerade →
  TeraDesks `xrsrc_fix` oder ein ABI-Breitenfehler beim Lesen des `ICONBLK`;
* `abi` ist `Ilp32`, obwohl `DESKTOP.NAP` LP64 ist → `render_tree_app` liest den
  Baum mit 24-Byte-Schrittweite (`resource.rs:293`), dann ist `ob_spec` selbst
  schon Müll.

**E3 — den vollständigen seriellen Bootlog sichern.**
Der Auszug im Auftrag endet nach drei Zeilen. Der ganze Log zwischen
`[launch] EL0:` und der Panik sagt, wie weit TeraDesks `main()`
(`sdk/apps/teradesk/main.c:1747-1937`) gekommen ist — insbesondere ob
`rsrc_load(RSRCNAME)` (`:1847`) erfolgreich war und ob `load_icons()` (`:1924`)
überhaupt erreicht wurde. Scheitert `rsrc_load`, ist der nächste Schritt
`form_alert(1, msg_resnfnd)` (`:1850`), und die Fehlersuche verschiebt sich auf
`resource.rs` statt auf `window.rs`.

Ich habe E1–E3 **nicht** ausgeführt: E2 wäre Instrumentierung (untersagt), E1
und E3 brauchen einen Neubau des SD-Abbildes samt `DESKTOP.NAP`
(`tools/make-sd-image.sh` ist selbst uncommitted geändert) und lägen außerhalb
des Auftrags „lesende QEMU-Nutzung".

---

## 7. Was ich getrieben habe / was offen bleibt

**Belegt (selbst gelesen bzw. gemessen):**
* die vollständige EmuTOS-Kette `sh_main` → `wm_init` → `sh_ldapp` → `dos_exec`
  → `ap_init` → `rs_load`/`rs_readit`/`rs_fixit`, jede Zeile am gepinnten Stand;
* die elf Rücksetzungen an der Startgrenze;
* dass EmuTOS den Resourcezustand prozessgebunden im `AESGLOBAL` hält und
  NewTOS ihn in Kernel-Statics;
* dass TeraDesk `aes_global[5]/[7]` selbst liest **und schreibt**;
* dass der reproduzierte Fall der Autorun-Arm ist und dort `pd::open` alt wie neu
  dasselbe tut;
* dass der Paniktext nur durch einen **ungeraden** Zeiger erklärbar ist
  (Ausschluss über die Grenzen von `stride`/`rows`);
* dass `window.rs` die einzige heute geänderte Quelldatei ist (Zeitstempel);
* dass `measurements/measurement-2026-09-10-qemu/run3-qemu-rlist-all6` mit FIX-01…06 und dem
  A25-Fix sauber durchbootet (allerdings mit `DUMPAES.NAP`, nicht mit TeraDesk).

**Nicht verifiziert (offen):**
* der eigentliche Auslöser der Panik — kein Mechanismus in `window.rs` belegt;
* ob `rsrc_load` für TeraDesks `desktop.rsc` überhaupt erfolgreich ist
  (der Bootlog-Ausschnitt reicht nicht);
* ob `NEWDESK_TREE` zum Zeitpunkt der Panik TeraDesks Baum trägt;
* ob die ICONBLK-Zeiger aus `icons.rsc` (TeraDesks eigener Parser) oder aus
  `desktop.rsc` (unser Parser) stammen;
* QED: nicht untersucht, weil kein Protokoll vorlag. QED wird über den Desktop
  gestartet — dort ist B1 aus dem A25-Audit erreichbar, hier aber nicht.
