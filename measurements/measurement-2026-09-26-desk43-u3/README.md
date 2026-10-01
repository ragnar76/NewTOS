# measurement-2026-09-26-desk43-u3

Belege zu **DESK43 / U3** — „Der Desktop wird ein Prozess, der endet (G1)"
(`docs/analysis/desktop-userspace-plan-v2.md` §5 U3).

Baum: `desk-rebuild` @ `d79e71f` + die U3-Änderung (unversioniert im Baum,
`method/product-u3.diff`). EmuTOS-Referenz: `references/emutos`.
Alles headless in QEMU `raspi4b` (640×480, Dev-Bau) bzw. `qemu-virt`.
**Keine echte Pi 4, nichts geflasht, `/Volumes/BOOT` nicht angefasst.**

```
serial/       die Seriellprotokolle aller Läufe
screendumps/  die QMP-Screendumps dazu
trace/        der svc-Trace des Doppelklicks (das U1-Instrument)
analysis/     Pixeltabelle + die beiden Trace-Ausschnitte
inf/          die drei NEWDESK.INF-Fassungen
method/       Messaufbau: Produktdiff, Scratch-Diffs, Injektorskript
build/        Bauen, clippy, Hosttests, Driftprobe
```

Kein `*.img` beigelegt (Auflage). Das Abbild entsteht reproduzierbar aus
`tools/make-sd-image.sh <img> 64` plus zwei Handgriffen, die
`method/put-inf.sh` bzw. `measurement-2026-09-20-a79/method/sd-put.sh`
ausführen (siehe „Messaufbau").

## Die Zahlen

| | |
|---|---|
| **Driftprobe** | **38** (A 20 · B 12 · C 6) — vorher 47 (A 21 · B 21 · C 5) |
| Bauen, beide Boards, je frisch | tos-api **64** / kernel **94** Warnungen (Basis 64/94, Delta 0) |
| clippy, beide Feature-Sätze | tos-api **80** / kernel **125** (Basis 80/125, Delta 0); **null** Treffer in `kernel/src/desk/`, `shell_loop.rs`, `launch.rs` |
| `cargo run -p xtask -- test` | **956 / 0 / 1** (Basis 956/0/1) |
| `fsck_msdos -n` | fünf Abbilder, Phase 1–3 ohne Befund, rc 0 |
| `EXC`/`PANIC`/`panic` in allen Seriellprotokollen | **0** |

### Die Driftzahl, aufgeschlüsselt

| Gruppe | vorher | nachher | Differenz |
|---|---:|---:|---|
| A `tos_api::` | 21 | 20 | −1: `tos_api::host::host()` (`main.rs`, `console_pause_for_key`) |
| B `crate::` | 21 | 12 | −6 `crate::shell_loop`, −2 `crate::launch`, −1 `crate::fbcon` |
| C Kernel → `desk` | 5 | 6 | **+1**: `crate::desk::reset()` im DESKTOP-Arm von `sh_ldapp` — genau der Aufruf, den U3 verlangt |

Summe 47 → **38**. Die einzige *steigende* Gruppe ist C, und zwar um die eine
Zeile, die der Plan selbst vorschreibt („`sh_ldapp`s DESKTOP-Arm ruft vor
`desk::run()` `desk::reset()`"). `desk::reset()` in `kernel/src/lib.rs:475`
(A106) bleibt stehen — U4s Abnahmekriterium listet es ausdrücklich als
Überlebenden.

## Die Läufe

| Lauf | Kernel | Abbild | Zweck |
|---|---|---|---|
| `locate` | Baum (U3) | `#W … C:\U3\@` | Icon-Position ausmessen (`screendumps/locate-desktop.png`) |
| `trace` | Scratch (U3 + svc-Tracer + IKBD-Injektor) | dito | **Beweispunkt 5**: Doppelklick auf `C:\U3\ARGVTEST.NAP` |
| `ncp` | dito | `C:\U3\HELLO2.NCP` | derselbe Weg für ein Konsolenprogramm (`isgraf = FALSE`) |
| `ztree` | Baum (U3) | `#Z 01 C:\HELLO2.NCP@` | **Beweispunkt 3a** |
| `zhead` | **d79e71f** | dito | Referenz für 3a |
| `boot-u3-placeholder` / `boot-head-placeholder` | U3 / d79e71f | Platzhalter-INF | Bootbild ohne Regression |
| `boot-u3-u3` / `boot-head-u3` | U3 / d79e71f | `#W … C:\U3\@` | Bootbild ohne Regression |
| `a106` | Scratch (U3 + A106-Injektor) | `#Z` TeraDesk + TERADESK.INF | **Beweispunkt 3b**: zwei AES-Neustarts |
| `qemu-virt-smoke` | Baum (U3), Board `qemu-virt` | — | zweites Board bootet |

## Beweispunkt 5 — der AES-Trace des Doppelklicks

`analysis/serial-doubleclick-excerpt.txt` (Seriell) und
`analysis/trace-doubleclick-excerpt.txt` (svc-Trace, Zeilen 548–640 von
`trace/QEDTRACE-doubleclick.log`).

Der Seriellauszug ist die kurze Fassung:

```
[h02] step 13 at +1300 -> mouse (64,124) mb=1     <- 1. Klick
[h02] step 15 at +1308 -> mouse (64,124) mb=1     <- 2. Klick
[shell] sh_ldapp: starting C:\U3\ARGVTEST.NAP, sh_nextapp=0, sh_isgem=1
[launch] EL0: slot 2 entry 0x1000
ARGVTEST: argc 1
[launch] returned (program Pterm'd)
[shell] sh_ldapp: 'C:\U3\ARGVTEST.NAP' exited with rc=0
[shell] sh_ldapp: starting NEWDESK, sh_nextapp=3, sh_isgem=1
[desk] desktop up (1 drives, 1 window(s) from the context)
```

**Der Desktop startet nichts mehr selbst.** `sh_ldapp` startet das Programm
(`sh_nextapp=0` = `NORMAL_APP`), und danach startet `sh_ldapp` den Desktop neu
(`sh_nextapp=3` = `DESKTOP_APP`) — genau `gemshlib.c:600-612,624-689,710-727`.

Der svc-Trace zeigt die Reihenfolge innerhalb des Desktops:

```
op=25  evnt_multi   -> 2   (fired=MU_BUTTON, mx=64 my=124, clicks=2)
op=106 wind_find / op=43 objc_find / op=42 objc_draw   (act_bsclick + Repaint)
Dsetdrv / Dsetpath path="C:\U3\"                        (set_default_path, desksupp.c:1785-1801)
op=78  graf_mouse                                        (desk_busy_on, deskpro.c:37)
op=121 shel_write   -> 1                                 (pro_exec, deskpro.c:41)
op=21  evnt_button                                       (deskmain.c:1003, Taste-hoch abwarten)
op=32/31 menu_ienable/menu_icheck x39                    (men_update, deskmain.c:1006)
op=104 wind_get handle=1 field=5 (WF_CXYWH)              (cnx_put,  deskmain.c:2154)
op=122 shel_get / op=123 shel_put                        (app_save(FALSE), :2155 -> deskapp.c:1123,1293)
op=30  menu_bar                                          (menu_bar(NULL,0), :2160)
op=105 wind_set                                          (wind_set(DESKWH,WF_NEWDESK,NULL), :2163)
op=19  appl_exit    -> 1                                 (:2166)
... dann der Neustart:  op=10 appl_init -> 0  (deskmain.c:1953)
```

**`Pexec` (GEMDOS `0x4b`) kommt im ganzen Trace null Mal vor** (744 Zeilen,
`/usr/bin/grep -c 0x4b` → 0).

**Eine Präzisierung zum Wortlaut des Plans.** Der Plan sagt, `shel_write(121)`
sei „die letzte Desktop-Aktion vor `appl_exit(19)`". Wörtlich stimmt das nicht
und darf es auch nicht: zwischen beiden liegen die Schritte, die EmuTOS dort
ebenfalls fährt — `evnt_button` (`deskmain.c:1003`), `men_update` (`:1006`),
`cnx_put`/`app_save` (`:2154-2155`), `menu_bar(NULL,0)` (`:2160`),
`wind_set(WF_NEWDESK,NULL)` (`:2163`). Richtig ist: **`shel_write` ist die
letzte Aktion der Ereignisschleife, `appl_exit` die letzte des Epilogs, und
dazwischen steht genau `deskmain.c:989-2166` und sonst nichts.**

## Beweispunkt 3 — headless

**3a `#Z 01 C:\HELLO2.NCP@`.** `serial/ztree-serial.log` gegen
`serial/zhead-serial.log` (Kernel `d79e71f`): identische Folge
(`sh_ldapp: starting C:\HELLO2.NCP, sh_nextapp=2, sh_isgem=1` → Programm →
`sh_ldapp: starting NEWDESK`), und die Desktopbilder danach sind **0 Pixel**
verschieden (`z-head-desktop.png` vs. `z-tree-desktop.png`).

**3b A106-Restart.** `serial/a106-serial.log`: zwei Runden
`sh_main: session over — shel_write(SHW_RESCHNG) pending` →
`AES restart (resolution change requested)` →
`sh_ldapp: starting C:\APPS\TERADESK\DESKTOP.NAP`. Alle sechs Vergleiche
**0 Pixel** (Kaltstart A vs. I vs. M, und die drei Dialogstufen C/E/F gegen
J/K/L) — dieselbe Tabelle wie in `measurement-2026-09-22-a106`.

**Bootbilder ohne Regression.** U3 gegen `d79e71f`, zweimal je **0 Pixel**
(Platzhalter-INF und Fenster-INF).

## Was der Plan als Beweispunkt 1 wollte — headless so weit wie möglich

Punkt 1 („Doppelklick auf `QED.NAP` → Vollbild → beenden → der Desktop steht
mit **denselben** Fenstern") gehört auf die echte Pi. Headless ist davon
nachgewiesen, was ohne Hand nachweisbar ist, und das ist die Kernaussage:

* `screendumps/t-on-icon.png` (unmittelbar vor dem Doppelklick) gegen
  `screendumps/t-after25.png` (nachdem `ARGVTEST.NAP` gelaufen und der Desktop
  **neu gebaut** ist): **0 abweichende Pixel**. Das Fenster `C:\U3\` steht mit
  Rechteck, Pfad, Infozeile und Inhalt wieder da — rekonstruiert aus
  `shel_put`/`shel_get` (U2) + `cnx_get`, nicht aus einem überlebenden
  Zustand: `desk::reset()` hat die Singleton-Struktur dazwischen fallen
  gelassen.
* Dasselbe für ein Konsolenprogramm (`ncp`-Lauf, `n-on-icon` vs. `n-after25`):
  **0 Pixel**, und `sh_isgem=0` im Protokoll zeigt, dass der Bildschirmwechsel
  über `sh_chgrf`/`sh_toalpha` läuft und nicht mehr über eine Desktop-Klammer.

Was headless **nicht** gezeigt ist: eine Anwendung, die *lebt* und den
Bildschirm hält (QED), und der Blick eines Menschen auf das Ergebnis.

## Messaufbau

**Abbild.** `tools/make-sd-image.sh "$SCRATCH/sd.img" 64` (einzige zulässige
Quelle, [[auto-folder-console-only]]), dann:
* `C:\U3\ARGVTEST.NAP` = Kopie von `C:\ARGVTEST.NAP` (ein einziges Icon im
  Fenster, damit die Klickkoordinate deterministisch ist); für den `ncp`-Lauf
  stattdessen `C:\U3\HELLO2.NCP`.
* `C:\NEWDESK.INF` je Lauf aus `inf/` (`method/put-inf.sh`).
* Für `a106`: `NEWDESK.INF` = `measurement-2026-09-20-a79/method/newdesk-tera.inf`
  und `C:\Apps\TERADESK\TERADESK.INF` =
  `measurement-2026-09-20-a79/method/teradesk-window.inf`, gesetzt mit
  `measurement-2026-09-20-a79/method/sd-put.sh`.

**Fahren.** `measurement-2026-09-22-a106/method/run.sh` (QEMU `raspi4b`,
`-display none`, QMP-Screendump am Seriellmarker) und
`measurement-2026-09-22-a106/method/pixdiff.py`.

**Scratch-Stände, beide von Hand zurückgedreht — kein `git`-Eingriff.** Sie
liegen ausserhalb des geteilten Baums (`rsync` der Crates in ein
Scratch-Verzeichnis); der Baum enthält **nur** die zehn Produktdateien.

1. `method/scratch-u3-trace-and-injector.diff` (Läufe `trace`, `ncp`):
   * `dbg_qedtrace.rs`: `QED_SVC_TRACE false → true`; die QED-Umleitung der
     Geometriezeilen in den `PRO`-Puffer abgeschaltet (sie stellt die
     Reihenfolge um, die hier geprüft wird — dieselbe Massnahme wie in U1);
     **und** das Leeren des Puffers bei jedem `evnt_multi`-Eintritt ersetzt
     durch eine Marke, damit der Lauf *in einer* Datei in Reihenfolge steht.
   * `host_env.rs`: der H02-IKBD-Injektor
     (`measurement-2026-09-20-a79/method/apply_injector.py` +
     `a78_patch2.py`), mit drei lokalen Korrekturen: `say()` schreibt direkt
     auf die UART statt über ein `Host::debug_write`, das es hier nicht gibt;
     `Step::Arm` ist ein No-op (die H02-AES-Spur wird nicht gebraucht); und
     die Uhr startet erst, wenn **unser** Desktop steht (`DESK_UP`) statt beim
     ersten EL0-Prozess — sonst armiert ein Accessory den Injektor lange
     vorher.
   * `desk/mod.rs`: eine Zeile, die `DESK_UP` setzt.
2. `method/scratch-a106-injector.diff` (Lauf `a106`): derselbe Injektor mit
   `measurement-2026-09-22-a106/method/a106_scripts.rs` und der Original-
   Doppelarmierung, **ohne** Tracer-Änderung.

**Klickskript.** `method/u3.actions` → `method/u3_scripts_full.rs`
(`measurement-2026-09-20-a79/method/gen_script.py`). Der Zeiger parkt zuerst
unten rechts (`parkbr`, kreuzt keine Menütitel —
[[injector-park-must-not-cross-menu-bar]]) und fährt dann auf (64,124), die aus
`screendumps/locate-desktop.png` ausgemessene Mitte des Icons.

## Offene Punkte / Beobachtungen

* Beweispunkte 1 und 2 des Plans brauchen die echte Pi 4; die Anweisung dafür
  steht in `TASK/DESK43-U3-STATUS-DONE.MD`.
* `console_pause_for_key` ist mit dem Console-Arm von `launch_native`
  weggefallen — die Ausgabe eines `.NCP` bleibt nach dem Doppelklick **nicht**
  mehr stehen (`screendumps/n-clicked.png`: 1,5 s nach dem Klick ist der
  Desktop schon wieder da). Das ist EmuTOS-konform und war STOP-3 des alten
  Plans; die Entscheidung gehört dem Anwender.
