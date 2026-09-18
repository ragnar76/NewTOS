# H05 — STATUS: DONE (Ursache festgestellt, kein Fix)

TeraDesk wird nach der Rückkehr eines Kindes **nicht** vom Kernel neu geladen und die
Shell-Schleife läuft **nicht** an — TeraDesk startet **selbst ein zweites TeraDesk als
Kind**, weil NewTOS' `shel_read` nicht den Puffer zurückgibt, den `shel_write(SHW_EXEC)`
beschrieben hat. Bearbeitet 2026-09-18, Branch `desk-rebuild`, Basis HEAD `051e7cc`.
Messlauf: [`measurement-2026-09-18-h05/`](../measurement-2026-09-18-h05/RUNINFO.TXT).
**Kein Produktcode geändert.** Gepinnte EmuTOS-Quelle: `references/emutos` @
`1a7484aba1bfba23e2c0899e243b6d8177fe41df`.

## 1. Welchen Startweg TeraDesk nimmt — gemessen

Single-TOS-Bau (`-D_TOS_ONLY` → `_MINT_ = 0`): `stask` ist **unbedingt** `TRUE`
(`startprg.c:462`, Initialisierer; die Zuweisung `:583` liegt in `#if _MINT_`), also
`startprg.c:632` → `exec_com()` (`:665`). Das ist **nicht** der `shel_write`-Weg.

`exec_com` (`startprg.c:190-402`), was TeraDesk *vor* und *nach* dem Start tut:

| Schritt | `startprg.c` | im Trace (`serial/run4-serial.log`) |
|---|---|---|
| `wd_tmpcls()` — `wd_deselect_all()` (`window.c:3617`, **Auswahl fällt per Bauart**), Fensterkonfiguration in eine **Speicherdatei** (`x_fmemopen`, reiner `malloc`, `xfilesys.c:1480`), alle Fenster schließen | `:230` | `wind_close`/`wind_delete` 2×, `menu_bar(…,0)` |
| `shel_write(SHW_EXEC, aptype, 0, pinfo.name, &pinfo.cml)` | `:260` | `[h05] shel_write(SHW_EXEC, isgem=0) D.s_cmd='C:\AUTO\HELLO.NCP'` (:780) |
| `wind_set(0, WF_NEWDESK, NULL)`, `close_windows()` = `wind_new()` | `:263`, `:278` | op 105 (0,14), op 109 |
| **`x_exec(0, …)` = `Pexec(0, …)`** (`xfilesys.c:1269-1277`) | `:312` | `[h05] Pexec mode=0 from_el0=true caller_slot=Some(0) caller_bp=0xffff000001000000` (:787) |
| Kind läuft, `Pterm` | — | `[h05] Pterm dying_slot=1 rc=0 resume_to_el0=true parent_slot=0 RUN_after=0xffff000001000000 run_slot=Some(0)` (:792) |
| `close_windows()` (GEM) bzw. `v_exit_cur`/`xd_mouse_on` (TOS) | `:337` / `:327-328` | op 78 (257) |
| **`shel_read(cmd, tail); if (strcmp(cmd, pinfo.name) && cmd[0]) { … pinfo.new = TRUE; }`** — die Kettenerkennung: hat das Kind per `shel_write` ein Nachfolgeprogramm bestellt? | `:344-355` | `[h05] shel_read -> returns launch_cmd='C:\APPS\TERADESK\DESKTOP.NAP' (D.s_cmd would be 'C:\AUTO\HELLO.NCP')` (:794) |
| `while (pinfo.new)` → nächste Runde: `shel_write(SHW_EXEC, 1, …)`, `Pexec(0, DESKTOP.NAP)` | `:241-312` | `[h05] shel_write(SHW_EXEC, isgem=1) D.s_cmd='C:\APPS\TERADESK\DESKTOP.NAP'` (:796), `[h05] Pexec … path='C:\APPS\TERADESK\DESKTOP.NAP'` (:804), dann `[aes] pid=2 op 10` = `appl_init` des **inneren** TeraDesk |
| erst nach Ende der Schleife: `shel_write(SHW_NOEXEC)`, `regen_desktop(desktop)`, **`wd_reopen()`** aus der Speicherdatei | `:386-391`, `window.c:3652-3668` | in run2 nach dem Ende des inneren TeraDesk (`run2-serial.log:1005-1077`) |

## 2. Reproduziert: ja

QEMU `raspi4b`, Scratch-Kopie mit Injektor + Trace (Methode H02/H03, plus ein
`drag`-Schritt mit gehaltener Taste, `method/gen_script.py`), Karte mit der
**`TERADESK.INF` und `NEWDESK.INF` des Users** (von `/Volumes/BOOT`, nur gelesen — die
`TERADESK.INF` definiert die Programmtypen `*.NAP`/`*.NCP`, ohne sie startet TeraDesk
kein `.NCP` per Doppelklick, `prgtype.c:346-362`).

Zustand vorher (`screendumps/run1-E-before-start.png`): **zwei Fenster** — W1
`C:\AUTO\*.*` an (7,143), per Titelbalken **verschoben** (INF-Vorgabe wäre (307,143)),
Infozeile „4531 bytes used in 2 items.", oben; W2 `C:\*.*` an (500,71) dahinter.
Dann Doppelklick `HELLO.NCP` (run1/run4) bzw. `MINIGEM.NAP` (run3, Schließer geklickt).

Nachher (`run1-F-after-1s.png`, `-G-`, `-H-after-8s.png`; `run3-G/H`): **kein Fenster**,
nur Menüleiste, vier HARD-DISC-Icons, TRASH — das Bild eines frisch gestarteten TeraDesk.
Fehlend: beide Fenster (Titel, Verzeichnis, Position, Infozeile). Das ist das Ticketbild.

## 3. Derselbe Prozess oder neu geladen?

**Derselbe Prozess — und zusätzlich ein zweiter obendrauf.** Belege (`run4-serial.log`):

- `Pexec` aus Slot 0 (Basepage `0xffff000001000000`), Kind Slot 1; `Pterm` des Kindes
  kehrt **nach EL0** in **Slot 0** zurück, `RUN` = dieselbe Basepage, `parent_elr=0x282f8`
  = TeraDesks Pexec-Aufrufstelle; die nächsten AES-Aufrufe tragen `pid=1`.
- Zwischen Kind-`Pterm` und dem Bild gibt es **keine** Zeile `[launch] returned`,
  `[shell] sh_ldapp`, `[desk]` — die Shell-Schleife (`kernel/src/shell_loop.rs`) wird nicht
  betreten, `launch::run_program_with_tail_rc` kehrt nicht zurück (`launch.rs:126-134`
  wartet auf das `Pterm` von Slot 0). **Hypothese 2 (Shell-Schleife) ist falsch.**
- `bdos/process/pterm.rs:188-195` setzt `RUN = parent`, `exceptions.rs:937-949` stellt
  TeraDesks EL0-Frame und `:988-990` sein `TTBR0` wieder her — wie EmuTOS `xterm` (`bdos/proc.c:609`
  `run = run->p_parent`) nach `proc_go` (`:541`). **Hypothese 1 (Pexec lädt den Eltern
  neu) ist falsch.**
- Der **neue** `appl_init` (`[aes] pid=2 op 10 … out=(2,…)`) gehört einem **zweiten**
  Prozess (Slot 1, pid 2), den **TeraDesk selbst** per `Pexec(0, "C:\APPS\TERADESK\
  DESKTOP.NAP")` startet (`:804`). Das äußere TeraDesk (pid 1) steckt derweil in seinem
  `Pexec` und behält seinen Zustand: in run2 wurde das innere TeraDesk per Menü beendet,
  danach lief das äußere weiter und `wd_reopen()` stellte **beide Fenster mit Pfad und
  Position** wieder her (`run2-R/S/T-after-inner-quit-*.png`).

**Dritter Fall (TeraDesk baut aus der `.INF` neu) ausgeschlossen:** `load_options` läuft
nur in `init()` (`main.c:1201`) und in `load_settings` (`va.c:1003`, `window.c:1710`:
AV-Protokoll bzw. Doppelklick auf eine `.INF`) — nicht auf dem Rückweg. Der Rückweg
liest die **Speicherdatei** (`wd_reopen`, `window.c:3663`), und run2 zeigt, dass sie den
Zustand trägt (verschobenes W1 kommt an (7,143) zurück, nicht an der INF-Position).

## 4. Was EmuTOS an der Stelle tut — und wo NewTOS abweicht

| | EmuTOS | NewTOS |
|---|---|---|
| `shel_write(SHW_EXEC)` | `strcpy(D.s_cmd, pcmd); memcpy(ad_stail, ptail, 128)` — `aes/gemshlib.c:169-170` | `cstr_in(S.s_cmd, …); tail_in(S.s_tail, …)` — `tos-api/src/aes/shell.rs:414-415` |
| `shel_read` | `strcpy(pcmd, D.s_cmd); memcpy(ptail, ad_stail, 128)` — **derselbe Puffer** — `:102-106` | `cstr_out(…, S.launch_cmd); cstr_out(…, S.launch_tail)` — **ein anderer Puffer** — `shell.rs:331-338` |
| Wer füllt den gelesenen Puffer | `sh_ldapp` startet mit `dos_exec(PE_LOADGO, D.s_cmd, ad_stail, …)` (`:645`) — der Startbefehl **ist** `D.s_cmd` | `kernel/src/launch.rs:115` `set_launch_info(path, tail)` (`shell.rs:351-357`), nur beim Start durch den Kernel-Launcher; `shel_write` fasst `launch_cmd` nie an |

Auf EmuTOS (und jedem Atari-TOS) gibt `shel_read` nach TeraDesks eigenem
`shel_write(SHW_EXEC, …, "C:\AUTO\HELLO.NCP")` genau diesen String zurück — es sei denn,
das Kind hat selbst `shel_write` gerufen (dann ist die Kette gewollt, das ist das
Single-TOS-Kettenprotokoll, das TeraDesk hier nachbildet). `strcmp(cmd, pinfo.name) == 0`
→ `pinfo.new` bleibt `FALSE` → `shel_write(SHW_NOEXEC)`, `regen_desktop`, `wd_reopen()`:
Fenster mit Verzeichnissen und Positionen zurück, Auswahl weg (`wd_deselect_all`, per
Bauart, auch auf TOS).

Auf NewTOS liefert `shel_read` den Startbefehl von TeraDesk selbst (`launch_cmd`), der
ungleich `pinfo.name` ist → TeraDesk startet ihn. Die Spec (`docs/api/aes/scrap-shell.md`
§ `shel_read`) zitiert die EmuTOS-Zeilen richtig (`strcpy(pcmd, D.s_cmd)`), umschreibt sie
aber als „the command that launched **this** app" — das ist nur der Normalfall, nicht die
Semantik; die Spec sollte den Puffer als **denselben** benennen, den `shel_write`
beschreibt, sonst hält die nächste Implementierung `launch_cmd` wieder für 1:1
(Researcher). Herkunft der Abweichung: `884dcc1` (WP8b, AUD-P6-006) hat „`shel_read`
liefert leere Strings" mit einem **zweiten** Puffer repariert statt `D.s_cmd` zu füllen.
Kein Hosttest deckt `shel_read` (`tos-api/tests/` kennt weder `shel_read` noch
`shel_write`).

**Zweiter Defekt, vom ersten verdeckt (`run2-serial.log`):** nachdem das innere TeraDesk
`rsrc_load` (op 110) und `rsrc_free` (op 111, `:1003`) gerufen hat, liefern **alle 36**
`rsrc_gaddr` des **äußeren** TeraDesk `0` (vorher 44/44 = `1`): leere Infozeile, andere
Datei-Icons (`run2-S-after-inner-quit-4s.png` gegen `run1-E-before-start.png`).
`tos-api/src/aes/resource.rs` hält den geladenen Ressourcenzustand in **einem globalen
Static** (`RSC`, `:375`; `USER_RSC_RAW`, `:1302`), den jedes `rsrc_load` überschreibt und
jedes `rsrc_free` löscht (`:1645-1698`). EmuTOS hält ihn **je Anwendung** in deren
`AESGLOBAL` (`ap_ptree`/`ap_rscmem`/`ap_rsclen`) und setzt bei jedem Aufruf
`rs_global = pglobal` des Aufrufers (`rs_sglobe`, `aes/gemrslib.c:673-677`; `rs_free`
`:685-704`; `rs_gaddr` `:715-721`). Sobald Defekt 1 behoben ist, trifft das jedes GEM-Kind
aus TeraDesk (QED, DUMPVDI, …): das äußere TeraDesk kommt zurück, kann aber seine Strings
und Objekte nicht mehr auflösen.

## 5. Ursache in einem Satz, und die Schicht

**`shel_read` (`tos-api/src/aes/shell.rs:331-338`) gibt den vom Kernel-Launcher gesetzten
Puffer `launch_cmd` zurück statt `D.s_cmd`, das `shel_write(SHW_EXEC)` beschreibt; TeraDesks
Kettenerkennung nach der Rückkehr (`startprg.c:344-355`) liest deshalb seinen eigenen
Startbefehl `C:\APPS\TERADESK\DESKTOP.NAP`, hält ihn für einen Kettenwunsch des Kindes und
startet ein zweites TeraDesk als Kind — das ist der „Neustart".** Schicht: **AES-Shell-Manager**
(`tos-api/src/aes/shell.rs`), nicht das Prozessmodell, nicht die Shell-Schleife, nicht
TeraDesk. Zweite Schicht dahinter: **AES-Ressourcenmanager** (`resource.rs`, globaler statt
je-Anwendung-Zustand). Kein Fix in diesem Auftrag.

**Regressionstests, die der Befund verlangt:** (a) Hosttest über den Dispatcher:
`shel_write(SHW_EXEC, X, tail)` gefolgt von `shel_read` liefert `X`/`tail`; dazu die Probe,
dass ein per Launcher gestartetes Programm ohne eigenes `shel_write` weiterhin seinen
Startbefehl liest (der Launcher muss dann `D.s_cmd`/`ad_stail` füllen, wie `sh_ldapp`).
(b) Hosttest: `rsrc_load`/`rsrc_gaddr` von pid 1, dann `rsrc_load`+`rsrc_free` von pid 2,
dann `rsrc_gaddr` von pid 1 liefert weiterhin `1` und dieselbe Adresse.

## Offen / Nebenbefunde (nicht verfolgt)

* **Hardwareabnahme** nach dem Fix: aus TeraDesk `HELLO.NCP` und ein `.NAP` starten und
  beenden; Soll: dieselben Fenster mit denselben Verzeichnissen und Positionen, Infozeile
  gefüllt, Icons wie vorher; **Auswahl ist auch auf TOS weg** (`wd_deselect_all`).
* `launch.rs` ruft `set_launch_info` nur auf dem Kernel-Launcher-Pfad; ein per EL0-`Pexec`
  gestartetes Kind (der TeraDesk-Weg) bekommt weder `register_process` (`p_nameit`) noch
  `set_launch_info` — für `appl_find` nach dem Kindnamen ein eigener Punkt (A28/A62-Nähe).
* Das innere TeraDesk erhält beim zweiten `rsrc_load` `0` (`run2-serial.log:880`, wohl
  `cicons.rsc`, laut `tools/make-sd-image.sh` absichtlich fehlend) — unverfolgt.
* `docs/api/aes/scrap-shell.md` § `shel_read`: Wortlaut schärfen (siehe 4).

## Aufräumen

Injektor und Traces nur in der Scratch-Kopie (`<scratchpad>/h05/ws`); Diffs unter
`measurement-2026-09-18-h05/method/scratch-*.diff`. `git status --short` zeigt nur
`measurement-2026-09-18-h05/` und diese Datei; `kernel/src/host_env.rs` sha256
`287ff7415805b9cf0b7efe74487a5f24dd06f121ddd61cdb5f9e2d8a15618e3e` vorher wie nachher.
QEMU beendet, QMP-Sockets gelöscht, Images ausgehängt, `/Volumes/BOOT` nur gelesen.
Keine Hardware-/Outward-Aktion.

## Verifikation (Tester, 2026-09-18)

Unabhängig nachgemessen, nicht nur den Developer-Selbstreport gelesen. Alle Belege
unter [`measurement-2026-09-18-h05/verify/`](../measurement-2026-09-18-h05/verify/RUNINFO.TXT).

**Hosttests.** `cargo test -p tos-api --test aes_shel_roundtrip` im Hauptbaum (Fix):
**3/3 grün**. Rot-Gegenprobe in einem `git worktree` auf HEAD `f9d7608` (ohne den
uncommitted Fix, Testdatei hineinkopiert): **1 passed, 2 FAILED** — genau Fall a und c
(`shel_write_exec_then_shel_read_returns_the_written_pair`,
`shel_write_after_launch_wins_shel_read`), Assertion zeigt den TeraDesk-eigenen
Startbefehl statt des `shel_write`-Werts — der F1-Defekt, reproduziert. `cargo run -p
xtask -- test`: **570 passed, 0 failed, 1 ignored** (567 + die 3 neuen H05-Tests, exakt
konsistent mit A67s Zeile in `docs/STATUS.md`).

**Build/Clippy, beide Boards frisch** (`cargo clean -p kernel/tos-api --target
aarch64-unknown-none-softfloat` vor jedem Bau): rustc `tos-api` 64 / `kernel` 94
Warnungen auf **beiden** Boards identisch; clippy `tos-api` 81 / `kernel` 125 identisch.
Referenz A67 (`docs/STATUS.md`, 2026-09-18): "rustc 64/94, Clippy 81/125" — **Delta 0**.
`shell.rs`/`launch.rs` selbst tragen 0 neue Warnungen.

**H05-Abnahme in QEMU raspi4b**, eigene Scratch-Kopie + eigene Injektor/Trace-
Instrumentierung (dieselbe Methode wie oben; für `shell.rs` von Hand an die neuen
`S.s_cmd`-Stellen nachgezogen, der alte Diff zielte auf den gelöschten
`launch_cmd`-Puffer). **run1 (`HELLO.NCP`)**: `shel_read` liefert nach der Rückkehr
`s_cmd='C:\AUTO\HELLO.NCP'` — TeraDesks eigenen `shel_write`-Wert, nicht mehr
`launch_cmd='C:\APPS\TERADESK\DESKTOP.NAP'`. Genau ein `[h05] Pexec` im ganzen Lauf,
kein zweites `DESKTOP.NAP`, kein `pid=2`, kein zweites `appl_init` — **kein zweites
TeraDesk**. Screendumps vor/1s/3s/8s nach der Rückkehr pixelgleich: beide Fenster mit
Pfad und Position zurück, Auswahl weg. **run3 (`MINIGEM.NAP`)**: gleiches Bild für den
Rückweg (ein legitimes `pid=2 op 10` gehört MINIGEMs eigenem `appl_init`, nicht einem
zweiten TeraDesk; danach nur noch `pid=1`). **Abweichung vom Auftrag, gemessen, keine
Fehlmessung:** der Auftrag erwartete hier A39 (leere Infozeile/andere Icons) als
bekannten Nebenbefund — gemessen ist die Infozeile ("18755 bytes used in 3 items") und
alle drei Icons nach der Rückkehr unverändert korrekt, weil `MINIGEM.NAP` nie
`rsrc_load`/`rsrc_free` ruft (kein `op 110`/`op 111` im gesamten Log). A39 greift laut
dieser Messung nur, wenn das gestartete Kind selbst eine `.rsc` lädt (wie TeraDesk
selbst in run2 oben) — eine Präzisierung von A39s Reichweite, kein neuer Befund.

**Rauchtests ohne Injektor** (frischer `--release`-Bau ohne jede Instrumentierung):
`#Z` TeraDesk hoch (`sh_ldapp: starting …DESKTOP.NAP`, Desktop-Screendump sauber), `#Z`
QED hoch (`sh_ldapp: starting …QED.NAP`, QEDs eigene Menüleiste vollflächig — cflibs
`appinit`-Weg über `shel_read`/`gl_appdir`/`rsrc_load` bleibt intakt), unser Desktop ohne
Autorun hoch (`sh_ldapp: starting NEWDESK`, `[desk] desktop up (1 drives)`). Keine
Exception, kein Panic in allen fünf QEMU-Läufen (run1, run3, drei Rauchtests).

`git status --short` zeigt nur die drei Fix-/Test-Dateien plus
`measurement-2026-09-18-h05/`; `kernel/src/host_env.rs` sha256
`287ff7415805b9cf0b7efe74487a5f24dd06f121ddd61cdb5f9e2d8a15618e3e` vor und nach der
Messung identisch (nur die Scratch-Kopie wurde instrumentiert). QEMU beendet, QMP-
Sockets gelöscht, alle Image-Attaches ausgehängt, `git worktree` für die Rot-Gegenprobe
entfernt, `/Volumes/BOOT` nicht angefasst. **Verdikt: PASS** — F1 (H05s Ursache) ist
behoben, keine Regression in den 570 Hosttests, beiden Boards oder den drei
Rauchtest-Szenarien. A39 (Ressourcenmanager, zweiter Defekt) bleibt offen wie
dokumentiert, jetzt mit einer schärferen Reichweitenangabe.
