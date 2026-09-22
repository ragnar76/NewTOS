# A106 — STATUS: DONE (Hostbeleg rot/gruen + QEMU: zwei Neustarts, Kaltstart-Bild 0 px) / Pi-Sichtabnahme offen

Die AES-Sitzung ist neu startbar. `sh_main` traegt beide Terme von
`aes/gemshlib.c:727`, gibt den Ausgangsgrund zurueck, und `kernel/src/lib.rs`
umschliesst den Aufruf mit NewTOS' `aes_restart` (`gemstart.S:86-124`):
`tos_api::aes::reinit()` setzt jede AES-Static auf den Einschaltzustand,
`kernel::desk::reset()` nimmt den Desktop herunter, `dos_sdrv(bootdev)`
stellt das Bootlaufwerk her, dann laufen `sh_tographic` / `process_inf2` /
`sh_main` erneut. Bearbeitet 2026-09-22 (die Uhr rollte waehrend des Laufs
auf den 23.; die vom Lead vorgegebenen Datumsnamen sind beibehalten), Branch
`desk-rebuild`, Basis HEAD `520662a`, **nicht committet, nichts geflasht**.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemshlib.c:187-191` (`sh_write`s `SHW_RESCHNG`-Arm, unbedingt — A48), `:727` (`while ((psh->sh_doexec != SHW_SHUTDOWN) && !gl_changerez)`), `:733` (`if (gl_shgem) sh_toalpha()`), `aes/geminit.c:693-731` (`run_accs_and_desktop`s Schwanz: `sh_tographic` → `process_inf2` → `sh_main`), `:753-899` (`gem_main`: `.INF` neu lesen `:766-773`, `process_inf1` **nur** wenn kein Rezwechsel `:775`, `Setscreen` `:778-793`, `gsx_wsclear` `:795`, `dos_sdrv(bootdev)` `:803`, `gl_changerez = FALSE` `:806`, `mn_init` `:808`, Prozesstabelle `:846-880`, `gl_mowner` `:888`), `:655-739` (`run_accs_and_desktop`: `gem_rsc_init` `:658`, Tastenvariablen `:661-664`, `mnext_init` `:672`, `init_3d` `:675`, `ev_dclick(3)` `:701`, `gem_rsc_fixit` `:704`, `ob_height` `:707-709`, `set_aes_background` `:712-721`, `wm_start` `:723`), `aes/gemstart.S:101-110` (`theglo` neu nullen), `:122-124` (`tst gl_changerez; jne aes_restart`), `aes/gem_rsc.c:624-627` (ROM→RAM-Kopie vor `gem_rsc_fixit`), `aes/gemmnlib.c:751-756,821-832` (`mn_cleanup`/`mn_init`), `aes/gemmnext.c:103-110` (`mnext_init`), `aes/gemoblib.c:1325-1343` (`init_3d`), `aes/gemwmlib.c:1247-1310,1320-1360` (`wm_init`/`wm_start`) |
| NewTOS (vorher) | `kernel/src/shell_loop.rs:109` Austritt nur auf `sh_doexec == SHW_SHUTDOWN`; `kernel/src/lib.rs:403-421` ruft `go_graphic`/`process_inf2`/`sh_main` **einmal** und `halt()`t danach; keine Reinit-Funktion, keine Static mit Ruecksetzpfad ueber die Sitzungsgrenze |
| NewTOS (nachher) | `shell.rs`: `ShExit`, `sh_main_continue()`, `sh_main_exit()`, `reinit()`; `shell_loop.rs`: `sh_main(..) -> ShExit`, Bedingung = `sh_main_continue()`; `lib.rs`: Sitzungsschleife; `aes/mod.rs`: `pub fn reinit()`; je Modul ein `reinit()`/`init_3d()`; `desk::reset()`; `bdos::fs::drive::set_boot_drive()` |

## Was gebaut ist

### Schleife und Austritt

* `tos-api/src/aes/shell.rs`
  * `pub enum ShExit { Shutdown, RezChange }` — welcher Term von `:727` fiel.
    EmuTOS gibt das nicht zurueck (`gem_main` ist `void`, `gemstart.S:122-124`
    liest `gl_changerez` danach noch einmal); derselbe Entscheid an derselben
    Stelle, nur als Wert, damit der Wrapper **und ein Hosttest** ihn lesen
    koennen. `gl_changerez` gewinnt, wenn beide Terme fallen — das ist der,
    auf den `gemstart.S` verzweigt.
  * `pub fn sh_main_continue() -> bool` = `sh_doexec() != SHW_SHUTDOWN &&
    gl_changerez() == 0`, woertlich `:727`.
  * `pub fn sh_main_exit() -> ShExit`.
* `kernel/src/shell_loop.rs` — `sh_main` gibt `ShExit` zurueck, die
  Schleifenbedingung ist `sh_main_continue()`, und der Austritt bekommt eine
  Seriellzeile (`[shell] sh_main: session over — …`). Der Kopfkommentar
  benennt beide Erzeuger von `gl_changerez` wie bisher, jetzt mit dem
  Verbraucher statt der Vertagung.
* `kernel/src/lib.rs` — aus `go_graphic(); process_inf2(); sh_main(); halt();`
  wird eine Schleife, deren **ein Durchlauf eine AES-Sitzung ist**: der
  Schwanz von `run_accs_and_desktop` (`sh_tographic` `:700` → `process_inf2`
  `:734` → `sh_main` `:739`). `ShExit::Shutdown` bricht aus (das `halt()`
  bleibt, auf diesem Profil so unerreichbar wie EmuTOS' `Pterm0` nach
  `ui_start` — `can_shutdown()` ist FALSE, `bios/bios.c:754-770`);
  `ShExit::RezChange` fuehrt `aes::reinit()`, `desk::reset()` und
  `set_boot_drive()` aus und laeuft die Schleife erneut.

### `tos_api::aes::reinit()` — NewTOS' `theglo`-Nullen

Eine einzige Funktion in `tos-api/src/aes/mod.rs`, die in EmuTOS' Reihenfolge
die Modul-`reinit()`s ruft. Der einzige ausgelassene Schritt ist der physische
Moduswechsel (`Setscreen`/`Vsetmode`, `geminit.c:778-793`) — ein Modus,
ADR-017 rev 2, D-101; `gl_nextrez` wird aufgezeichnet, gemeldet und nie an
einen Moduswechsel gereicht. `gsx_wsclear()` (`:795`) und
`dos_conws("\033f\033E")` (`:763`) brauchen keinen eigenen Schritt:
`sh_main`s eigener Abschluss `sh_toalpha()` (`:733`) fuellt den Schirm mit
Hintergrund und setzt den Cursor nach Hause (`vdi::escape::clear_and_home`).

### Die Static-Liste — jede geprueft, mit Entscheid

Vollstaendige Inventur aller Modul-Statics unter `tos-api/src/aes/`
(`/usr/bin/grep -n "^\(pub\(([a-z()]*)\)\? \)\?static "`, 65 Treffer) plus
`kernel/src/desk/`. **Zurueckgesetzt:**

| Static (Datei) | EmuTOS-Entsprechung | Zurueckgesetzt auf |
|---|---|---|
| `INFBUF` (`init.rs:144`) | `infbuf`, `geminit.c:110` | Datei neu gelesen (`:766-773`) |
| `DESKTOP` (`init.rs`) | `rs_obj`/`rs_trees[DESKTOP]` | **ROM-Kopie `DESKTOP_ROM`** (neu) + `rs_obfix` + `ob_height` (`gem_rsc.c:624-627,636-641`, `geminit.c:707-709`) |
| `DTNAME_TED` (`init.rs:151`) | `rs_tedinfo[12]` | `te_ptext = 0`, Template/Valid neu verlinkt (`gem_rsc.c:626-630`) |
| `BG` (`init.rs`, **neu**) | `bg[]`/`bgfound`, `geminit.c:112-113` | **bleibt** — `process_inf1` laeuft auf diesem Pfad nicht (`:775`), der Wert der vorigen Sitzung wird wieder angewandt (`:719-728`) |
| `ACCS` (`menu.rs:1057`) | `desk_ppd[]`/`D.g_acctitle[]`/`g_accreg` | leer (`mn_init`, `gemmnlib.c:821-832`) |
| `SUBMENU_TAB` (`menu.rs:1634`) | `gl_submenu[]` | `Smib::EMPTY` (`mn_cleanup`, `:751-756`) |
| `SETTINGS` (`menu.rs:1517`) | `settings` | `INIT_*_VALUE` via `clamp_height` (`mnext_init`, `gemmnext.c:103-110`) |
| `MNTREE`/`MNPPD`/`CTWAIT_RECT`/`CTWAIT_BAR` (`menu.rs:51,66,86,87`) | `gl_mntree`/`gl_mnppd`/`gl_ctwait` | `remove_bar()` = der Zustand nach `ctlmgr`s Start (`gemctrl.c:438`, `geminit.c:888`) |
| `MNHOLD` (`menu.rs:200`) | `ml_mnhold`, `gemfmlib.c:56` | 0 |
| `MENU_SAVE`/`SUBMENU_SAVE` (`menu.rs:880,882`) | `menu_sr`/`blitsave` | `take()` + `drop` → der transiente Poolblock von `popup_blit` wird freigegeben (`gemmnext.c:212`) |
| `TABLE` (`pd.rs:299`) | `D.g_int[].a_pd` + Warteschlangen | `Aespd::EMPTY` (`gemstart.S:101-110` + `geminit.c:846-880`) |
| `TABLE`/`SAVE`/`UPDATE_LOCK`/`MCTRL_CNT` (`window.rs:621,639,771,778`) | `D.w_win[]`, `ml_ocnt`, `wind_spb.sy_tas` | **geblankt** (`Window::EMPTY`, `next_z = 1`) + `init_desk_window()`, Zaehler 0 — bewusst **nicht** `wm_init`/`wm_new`: das ist der Reset *pro Programm* innerhalb einer Sitzung (`gemshlib.c:717`) und **zeichnet** (`draw_change` → `w_drawdesk` + `WM_REDRAW`-Faecher, `gemwmlib.c:1216-1222,1453`); an der Sitzungsgrenze nullt EmuTOS die Tabelle einfach (`gemstart.S:101-110`) und `wm_start` baut auf. Ein Sweep wuerde hier einen Schirm neu malen, der gleich ohnehin neu gezeichnet wird, und die gerade geleerten Warteschlangen wieder fuellen |
| `NEWDESK_*` (`window.rs:1221-1254`) | `gl_newdesk`/`gl_newroot` | `clear_newdesk()` (`gemwmlib.c:1300`) — der Baum eines Programms ist eine VA eines verschwundenen Adressraums (A97) |
| `GL_WCOLORS` (`window.rs:797`) | `gl_wtcolor`/`gl_wbcolor` | Vorgaben (`wm_start`, `gemwmlib.c:1346-1357`) |
| `ML_PMOWN` (`window.rs:784`) | `ml_pmown`, `gemfmlib.c:58` | `true` |
| `BUTTON`/`GL_BDELY`/`XRAT`/`YRAT` (`input.rs:52,99,58,59`) | `gl_btrue`/`gl_bdely`/`xrat`/`yrat` | 0 (`geminit.c:661-664`) |
| `GL_MOWNER_APP`/`GL_CTMOWN`/`CT_MSGUP` (`input.rs:71,81,91`) | `gl_mowner`/`gl_ctmown`/`ct_msgup`s Warte | `true`/`false`/`false` (`geminit.c:888`, `gemctrl.c:50,100-101`) |
| `IND_*`/`ACT_*`/`BACKGR_COL` (`object.rs:1588-1594`) | `indtxtmove` … `backgrcol` | `init_3d()` **neu implementiert**, inkl. Palettenzweig (`gemoblib.c:1325-1343`, `geminit.c:675`) |
| `DCLICK_INDEX` (`event.rs:433`) | `ev_dclick` | 3 (`geminit.c:701`), danach ueberschreibt `process_inf2` die `#E`-Zeile |
| `RSC` (`resource.rs:684`) | die Resourceliste des Prozesses | jeder Eintrag ueber `on_process_exit(owner)` — derselbe gepruefte Pfad wie beim Prozessende |
| `APP_DIR` (`resource.rs:2099`) | `rlr->p_appdir` | leer (`geminit.c:871`, `gempd.c:95-97`) |
| `S` (`shell.rs:165`), `GL_SHGEM`, `SHOW_NAME` | `SHELL sh`, `D.s_cmd`, `ad_stail`, `D.g_scrap`, `D.g_shelbuf`, `ad_envrn`, `gl_shgem` | Einschaltzustand; `gl_changerez = FALSE` (`geminit.c:806`), `gl_nextrez = 0`, `ad_envrn` wie `gemstart.S:90-96` es wiederherstellt |
| `PENDING_SWEEP` (`mod.rs:44`) | — (A107, kein EmuTOS-Gegenstueck) | 0 — es gibt kein Fenster mehr zu sweepen |
| `DESK` (`kernel/src/desk/global.rs:519`) | `G` des Desktops | `None` → naechster `global::init` baut neu (EmuTOS startet `deskstart` neu) |
| `SESSION` (`kernel/src/desk/inf.rs:260`) | `G.g_ahead` u. a. | `None` → `load_and_apply` liest neu |
| Bootlaufwerk (`bdos/fs/drive.rs`) | `dos_sdrv(bootdev)`, `geminit.c:796-803` | erste Festplatte aus der BIOS-Laufwerkskarte, dieselbe Ableitung wie `$446` im Loader |

**Bewusst nicht zurueckgesetzt, jeweils mit Grund:**

| Static | Grund |
|---|---|
| `BUF_KVA`/`BUF_PA`/`BUF_LEN` (`gsxif.rs:76-78`) — der AES-Schirmpuffer | Kernel-eigener Poolblock, **einmal** geholt und nie freigegeben (Moduldoku, `docs/COMPATIBILITY.md`, A54/A57). EmuTOS gibt ihn in `sh_toalpha` zurueck und holt im naechsten `sh_tographic` einen neuen (`gemshlib.c:236,258`); NewTOS hat **kein** `gsx_mfree`, ein Neuholen wuerde pro Neustart einen schirmgrossen Block verlieren. Die Adresse ist stabil und gehoert dem Kernel, also kann nichts baumeln: der naechste `sh_tographic` findet denselben Puffer, `WF_SCREEN` meldet ihn weiter. **Beleg im Lauf:** `[aes] gsx_malloc: …` steht genau einmal im Seriellog, ueber zwei Neustarts. (Ticketauflage „STOP statt lecken" — hier ist Behalten die leckfreie Wahl; das Ticket nennt diesen Block ausdruecklich.) |
| `CLIPPED` (`gsxif.rs:84`) | Diagnosezaehler fuer den ganzen Boot, kein Sitzungszustand |
| `WM_READY` (`window.rs:646`) | Lazy-Init-Latch; was er bewacht (`init_desk_window`) laeuft in `wm_init` ohnehin |
| `thread.rs:113-181` (AES-Faser, Stall-Zustand) | Bei Sitzungsende leer (das Programm ist beendet, `sh_main` kehrt auf dem Bootstack zurueck); kein AES-Sitzungszustand |
| `trace.rs:422-533` | Diagnose-Ringpuffer (feature-gated), dessen Wert gerade das Ueberspannen des Boots ist |
| `SYS` (`fsel.rs:143`) | Kernel-BSS-Blob der `GEM.RSC`, einmal reloziert und zwischengespeichert (EmuTOS macht `gem_rsc_init`/`fs_start` pro Sitzung auf einer frischen ROM-Kopie). Kein Zeiger nach ausserhalb des Kernels; der Dialogschmierzettel (Pfad, Auswahl, Liste) wird von jedem `fsel_input` neu geschrieben |
| `NIL_RECORD`, `DTNAME_TEXT`, Fonts/Muster/Icons, `MENU` (`kernel/src/desk/menu.rs:320`) | Konstante Tabellen bzw. ein Baum, dessen Haken-/Sperrzustand `men_update` bei jedem Desktopstart neu ableitet |
| VDI (`tos-api/src/vdi/*`) | Ausserhalb des Auftrags **und** ausserhalb der EmuTOS-Sequenz an dieser Stelle: der Neustart oeffnet keine Workstation neu, weil NewTOS gar keine oeffnet/schliesst. Die `vex_*`-Vektoren (`vdi/mouse.rs:92-96`) koennten eine tote EL0-Adresse halten, werden aber von NewTOS **nie aufgerufen** (AUD-V6-002) — inert, kein Neustartrisiko. Als Nebenbefund notiert, nicht angefasst |

## Hosttests (`measurement-2026-09-22-a106/host-tests/`)

Neu: `tos-api/tests/aes_a106_restart.rs` (3 Faelle, Referenzharness).

| Datei | Ergebnis |
|---|---|
| `RED-1-compile-vs-HEAD-520662a.txt` | Der neue Test gegen HEAD: `E0432` (`sh_main_continue`/`sh_main_exit`/`ShExit` gibt es nicht) + 7x `E0425` (`tos_api::aes::reinit` gibt es nicht) |
| `RED-2-behaviour-vs-HEAD-520662a.txt` | Mit HEADs eigener API: `head_loop_ends = false` (ein wartendes `gl_changerez` beendet HEADs Schleife nicht — der zweite Term von `:727` fehlt) und das Fenster der Sitzung ist danach **noch in Benutzung** (`Some(16)`, die Desktop-Identitaet) — es gibt keinen Sitzungsreset |
| `GREEN-a106-after.txt` | 3/3 |
| `xtask-test-AFTER.log` | `cargo run -p xtask -- test`: **919 / 0 / 1**, rc 0 |

Die roten Laeufe liefen in einem **losgeloesten Scratch-Worktree**
(`git worktree add --detach <scratch> HEAD`, danach `git worktree remove`) —
der gemeinsame Arbeitsbaum wurde nicht angefasst, kein `checkout`/`restore`/
`stash`. Der Scratchtest liegt als `a106_red_behaviour-scratch.rs` daneben
(nicht im Repo). **Basis fuer die Gesamtzahl:** derselbe Lauf im
HEAD-Worktree ergibt 911/5 — die fuenf Fehler sind reine Worktree-Artefakte
(`references/emutos/aes/gem.rsc` ist dort nicht ausgecheckt), also
916 + 3 neue = 919, Delta = genau die neue Datei.

## Build / Clippy (`measurement-2026-09-22-a106/build/`)

| Lauf | Ergebnis |
|---|---|
| `cargo xtask build --board rpi4` | rc 0, tos-api **64** / kernel **94** (Baseline 64/94, Delta 0), `kernel8.img` `8b19cd1e…` |
| `cargo xtask build --board qemu-virt` | rc 0, tos-api **64** / kernel **94** |
| `cargo clippy -p tos-api` | **80** (Baseline 80, Delta 0) |
| `cargo clippy --package kernel --features board-rpi4` | tos-api 80 / kernel **125** (Baseline 125) |
| `cargo clippy --package kernel --features board-qemu-virt` | tos-api 80 / kernel **125** |

Kein Clippy-Treffer auf einer neuen Zeile: in den angefassten Modulen sind
alle Treffer die vorbestehenden `Aespb`/`Vdipb`-Sichtbarkeitshinweise und
die bekannten Schleifenindex-Notizen (`shell.rs` 15 → 15); der einzige
Treffer in einer angefassten Kerneldatei steht auf `kernel/src/lib.rs:9`,
dem crate-weiten `#![warn(missing_docs)]`.

## QEMU raspi4b (`screendumps/`, `serial/`, `method/`)

Injektor-Scratchkern nach der A48/A87-Methode (`method/make_scratch_a106.sh`
= A48s Skript, `rsync` der Crates in den Scratch — nie der gemeinsame Baum —
plus A79-Serialtrace und H02-Injektor; tos-api 67 / kernel 105 = 64/94 + das
Injektorgeruest). Bild: `tools/make-sd-image.sh` (64 MiB) + `NEWDESK.INF`
mit `#Z` TeraDesk und `TERADESK.INF` (zentrierte Dialoge) aus a79/method.
Skript `method/tera-a106.actions` = A48s Sequenz, **zweimal** gefahren, mit
dem Neustart dazwischen; Koordinaten unveraendert aus A48.

* **Serial** (`serial/a106-serial.log`, 0 Treffer fuer `EXC|PANIC|panic`):

  ```
  [aes] pid=0 op 121 in=(5,2,0,0,0,0) out=(1,0,0,0,0,0,0)     shel_write(SHW_RESCHNG, ST_LOWRES+2, 0)
  [aes] op 25 … op 24 … op 19                                  wait_to_quit / wait(500) / appl_exit
  [launch] returned (program Pterm'd)
  [shell] sh_ldapp: 'C:\APPS\TERADESK\DESKTOP.NAP' exited with rc=0
  [shell] sh_main: session over — shel_write(SHW_RESCHNG) pending, restarting the AES
  [shell] AES restart (resolution change requested)
  [shell] sh_ldapp: starting C:\APPS\TERADESK\DESKTOP.NAP, sh_nextapp=2, sh_isgem=1
  ```

  … und derselbe Block ein zweites Mal (Zeilen 131-134 und 377-380).
  `[aes] gsx_malloc: …` steht **einmal** (Zeile 41) — der Schirmpuffer wird
  ueber beide Neustarts behalten, kein Poolblock pro Neustart verloren.
  Dass nach dem Neustart wieder **TeraDesk** startet und nicht der
  Kernel-Desktop, ist EmuTOS-treu: `process_inf2` liest die `#Z`-Zeile pro
  Sitzung neu (`geminit.c:734`).

* **Screendumps** (10 Marken, alle getroffen):

  | Vergleich | Unterschied |
  |---|---|
  | `A-idle-teradesk` (Kaltstart) vs `I-restarted-teradesk` (nach Neustart 1) | **0 px** |
  | `A-idle-teradesk` vs `M-restarted-2` (nach Neustart 2) | **0 px** |
  | `I-restarted-teradesk` vs `M-restarted-2` | **0 px** |
  | `C-video-dialog` vs `J-video-dialog-2` | **0 px** |
  | `E-alert` vs `K-alert-2` | **0 px** |
  | `F-confirmed` vs `L-confirmed-2` | **0 px** |

  Das Kaltstartbild ist TeraDesks Desktop (eigene Leiste
  Desk/File/View/Window/Options, vier `HARD DISC`-Symbole, `TRASH`); das
  Bild nach jedem Neustart ist bitgleich dasselbe. `F`/`L` sind TeraDesks
  eigener Abbau waehrend `wait_to_quit` (Leiste weg, Flaeche geraeumt).
  Der zweite Video-Dialog zeigt wieder TT-Medium vorgewaehlt und
  ST-Low/ST-Medium aktiv — die **neue** TeraDesk-Instanz liest `Getrez()`
  frisch; der Modus hat sich absichtlich nicht geaendert (D-101).

* **qemu-virt-Rauchprobe** (`serial/qemu-virt-smoke.log`): bootet
  unveraendert bis `NewTOS console (EmuCON)`, 0 `EXC`/`PANIC` — auf einem
  Board ohne Raster ist `go_graphic()` in der Schleife weiterhin ein No-op
  und `sh_ldapp` faellt wie bisher in den Konsolenarm.

## Spezifikation / Dokumentation nachgezogen

* `docs/api/aes/shell-loop.md` § 7 — der Absatz „NewTOS deviation in the exit
  condition" ist durch die Beschreibung des vollstaendigen Terms, des
  `ShExit`-Rueckgabewerts und der Sitzungsschleife ersetzt (mit dem einen
  ausgelassenen Schritt und seiner Begruendung).
* `docs/api/aes/scrap-shell.md` — der `SHW_RESCHNG`-Aufzaehlungspunkt sagt
  nicht mehr „der Shell-Loop ignoriert es bis A106", sondern beschreibt
  Aufzeichnung **und** Verbrauch; die Zeilenangabe `:730` → `:727`
  richtiggestellt.
* `docs/STATUS.md` — Phase-13-Zeile ergaenzt, dazu eine datierte Zeile in
  *Recent Decisions* (2026-09-22).
* `docs/api/desktop/_hw-test-2026-09-17.md` — neuer **Punkt 33** (was auf der
  Pi 4 zu tun und zu erwarten ist, inklusive „die Aufloesung aendert sich
  absichtlich nicht" und „`gsx_malloc` nur einmal").

## Nicht angefasst (Nebenbefunde, je eine Zeile)

1. `geminit.c:671-672` setzt `D.g_scrap` pro Sitzung auf `C:\CLIPBRD`;
   NewTOS' Boot tut das nicht — der Neustart setzt den Einschaltzustand
   (leer) wieder her, die Luecke selbst bleibt unberuehrt.
2. `geminit.c:685` setzt den Shellpuffer pro Sitzung auf `"#a000000\r\n"`
   (`CP_SHELL_INIT`); NewTOS' Boot tut das nicht, der Neustart nullt wie der
   Boot.
3. `vdi/mouse.rs:92-96`: ein per `vex_*` gesetzter EL0-Vektor ueberlebt jedes
   Programmende — inert, weil NewTOS die Vektoren nie aufruft
   (AUD-V6-002); nicht neustartspezifisch.
4. `aes/init.rs`' `init()` galt im Doc-Kommentar als „idempotent"; das war
   fuer den `rs_obfix`-Teil falsch (nicht idempotent) — mit der ROM-Kopie ist
   es jetzt zutreffend.

## Offen

Pi-Sichtabnahme (Tester/Lead; nichts geflasht), HW-Checkliste **Punkt 33**
(`docs/api/desktop/_hw-test-2026-09-17.md`).
