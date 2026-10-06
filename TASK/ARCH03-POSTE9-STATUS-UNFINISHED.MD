# ARCH03 Nacharbeiten nach E9 — Quittung

**Status: TEILWEISE · Punkte 3, 4, 5 und die MD-Sonde erledigt und in QEMU
in allen drei Schutzmodi belegt · Punkt 2 umgesetzt, legt aber einen
vorhandenen vfork-Defekt frei (S2ALL unter ON FAIL in VFORKT) · Punkt 1 nach
Vorgabe angehalten (Optionen unten) · Abnahme durch den Tester offen · Pi
offen (nur auf Ansage)** · 2026-10-05 · Zweig `desk-rebuild`, Basis HEAD
`2d33c69`, nichts committet · Belege
[measurements/measurement-2026-10-06-post-e9/](../measurements/measurement-2026-10-06-post-e9/)
(`RUNINFO.TXT`, `checks/`, `serial/`, `screendumps/`, `method/`, `bin/`,
`red/`, `diag-libc-main/`).

## 1. `p_parent` wie EmuTOS — ANGEHALTEN (Entscheidung nötig)

EmuTOS (`references/emutos`, Pin `1a7484ab`): `p_parent` wird nur in
`proc_go` gesetzt, `p->p_parent = run` (`bdos/proc.c:541`); `init_pd_fields`
nullt den PD vorher (`:368`).

| Start | `run` beim Start → `p_parent` | Beleg |
|---|---|---|
| AUTO-Programm | `&initial_basepage` (statischer BDOS-PD, `run` seit `osinit`) | `bios/bios.c:693`, `bdos/bdosmain.c:339-342` |
| AES selbst (`exec_os`) | `&initial_basepage` | `bios/bios.c:862-865` |
| EmuDesk (ROM-Programm) | AES-PD | `aes/gemshlib.c:574-579` (`PE_GOTHENFREE`) |
| Programm vom Desktop / `#Z` | AES-PD (der Desktop ist beendet, `run` = AES) | `aes/gemshlib.c:645` (`dos_exec(PE_LOADGO)`) |
| Accessory | **0** (nur `Pexec(PE_LOAD)`, nie `proc_go`) | `util/gemdos.c:34`, `aes/geminit.c:251-255` |

NewTOS: der Kernel-Starter lädt mit `pexec::load` und springt direkt nach
EL0 (`kernel/src/launch.rs:80,144`), `set_parent` gibt es nur im
`Pexec`-Trap (`kernel/src/arch/aarch64/exceptions.rs:2016`); der Lader
schreibt `p_parent = 0` (`tos-api/src/bdos/process/loader.rs:841`). Weder
ein AES-Prozess noch ein `initial_basepage` existieren — `desk/` und die AES
laufen im Kernel, `RUN` ist dabei `null`. Ein **exaktes** Gegenstück gibt es
also nicht; nach Vorgabe nicht gebaut. Die Accessory-Erkennung bleibt bei
`EGSBF`.

Optionen:

* **(A) Stellvertreter-PDs.** Beim Booten zwei PDs in einer
  Kernel-Seite im Pool (`PROT_PR`, für alle lesbar, 32-Bit-Adresse): ein
  `initial_basepage` (genullt, `p_flags = PF_STANDARD`, `p_env` → doppeltes
  NUL in derselben Seite) und ein „AES-PD“ (`p_parent` = initial, `p_env`
  = AES-Umgebung). Der Starter schreibt `p_parent` = AES-PD für Desktop/`#Z`,
  = initial für AUTO. `RUN` bleibt `null`, `parent_slot`/Rückkehr unverändert.
  Beobachtbar wie EmuTOS (`p_parent ≠ 0` außer bei Accessories, lesbare
  Kette, `getppid` ≠ 0); Abweichung: diese PDs sind nie `run`, besitzen
  keinen Speicher, `*(os_run)` während des Desktops bleibt wie heute.
  Danach mintlibs Regel (`x0`-Basepage, `p_parent == 0`) und `EGSBF` weg.
  Aufwand klein.
* **(B) Desktop/AES als echter EL0-Prozess** (`desktop-must-run-in-user-scope`)
  — exakt, groß, eigener Auftrag.
* **(C) So lassen** — `p_parent == 0` für Kernel-Starts, `EGSBF`-Erkennung
  bleibt (heute belegt).

## 2. libc-crt0 ruft `exit()` — umgesetzt, mit Nebenwirkung

`sdk/libc/crt0.S`: `exit(main(__libc_argc, __libc_argv, environ))` — so
endet mintlibs `__libc_main` (`mintlib/main.c:302`), das `_crtinit`
(`crtinit.c:257`) und `_acc_main` (`:147`) als `_main` erreichen; `exit`
ruft die atexit-Funktionen rückwärts und schließt alle Streams
(`stdlib/exit.c:43-59`), dann `_exit` = `Pterm`.

**Nicht** `_main` selbst: zuerst so gebaut — dann scheitert jede Ausgabe
über `stdout` (`printf` −1, `EBADF`). Ursache (Quelltext + 9 Läufe,
`diag-libc-main/README.TXT`): `init_stream_mode` setzt die
Stream-Funktionen neu, damit wieder eine Seek-Funktion; der erste Flush
fragt per `Fseek(0, 1, SEEK_CUR)` die Position, NewTOS antwortet auf einem
Konsolen-Handle `EIHNDL` (`tos-api/src/bdos/fs/file.rs` `fseek`), EmuTOS
mit 0 (`bdos/bdosmain.c:571-641`: auf einem Zeichengerät nur Fread/Fwrite,
sonst `return 0`). **Abweichung von EmuTOS, nicht behoben** (außerhalb des
Auftrags) — Vorschlag eigener Auftrag; danach kann crt0 `_main` rufen.

Sonde ATEXITP/ATEXITK: Datei enthält `main-body;atexit-handler;`,
Exitcode 7, die Zeile ohne Zeilenende erscheint — PASS ON/OFF/STRICT. Rot:
mit dem HEAD-crt0 gebaut → Datei leer, FAIL.

**Nebenwirkung — S2ALL unter ON FAIL in VFORKT.** VFORKT stürzt jetzt in
seinem `exit()` ab: `__malloc` (`malloc(BUFSIZ)` beim Schließen der
Streams) liest `0x01a57010`, **freien** Poolspeicher. Deterministisch (auch
Crash-Kernel: `S2ALL: FAIL 1`); dieselbe S2ALL-Fahrt mit einem VFORKT vom
HEAD-crt0: PASS; OFF und STRICT (S2ALLS): PASS; VFORKT allein per `#Z`:
kein Absturz. Vermutung (nicht bewiesen): seit E9 holt mintlibs `malloc`
Speicher per `Malloc`; ein vfork-Kind (`Pexec` 4, gemeinsamer Raum) holt
einen Hunk, **besitzt** ihn und gibt ihn mit seinem `Pterm` frei, während die
`malloc`-Listen des Elternteils (gemeinsame Daten) weiter darauf zeigen —
bisher fiel das nie auf, weil nach `main` direkt `Pterm` kam. EmuTOS
verhält sich gleich (Besitzer = `run` = Kind), nur ohne MMU unsichtbar;
FreeMiNT behält Speicher eines Threads im gemeinsamen Raum
(`FORK_SHAREVM`). **Entscheidung:** (a) `Malloc` eines `Pexec`-4-Kindes
gehört dem Raum (FreeMiNT, Erweiterung von E6/E15), (b) so lassen und
Ursache erst belegen, (c) Punkt 2 zurücknehmen.

## 3. `free_accs` gibt den Speicher frei — erledigt

`kernel/src/accs.rs` `free_accs`: je Accessory AES-Ressourceneinträge,
`free_all_owned(PD)` (TPA, Umgebung, eigene `Malloc`s), nie gestartete
`PE_LOAD`-Kinder, dann `release_slot`. EmuTOS: `free_accs` `dos_free`t die
TPA (`aes/geminit.c:296-303`), das Ende von `run_accs_and_desktop` gibt den
Rest frei (`bdos/proc.c:133-139`; `PE_LOAD` macht `run` zum Besitzer,
`:305-307`). Serie: „2 block(s) … freed“. Sonde ACCFREE (zwei AES-Sitzungen
per `shel_write(SHW_RESCHNG)`): freier ST/TT-Speicher gleich → PASS in allen
Modi; rot auf dem E9-Kernel: 0x12000 pro Sitzung weg → FAIL.
Hosttest `tos-api/tests/post_e9_free_accs.rs` (3). Doku
`docs/api/aes/accessories.md` (Zuordnungstabelle).

## 4. TSRPAR semantisch — erledigt

`tsrpar.c`: „ein freier Block beginnt bei `PD + blkln`“ (auf 16 B bzw. eine
Seite gerundet), geprüft per `Mxalloc` in Adressreihenfolge; TSRRES meldet
`_PgmSize`. ON und OFF: TSR PASS. Strict-Varianten P567S/TSRPARS/S2ALLS
prüfen die Verweigerungen als erwartetes Ergebnis (`MPF_STRICT`); S2ALLS auf
dem Crash-Kernel: **PASS**. S2ALL: OFF PASS, ON FAIL nur durch VFORKT (Punkt 2).

## 5. `syscall__main.o` — erledigt

`sdk/libc/build-libc.sh` überspringt `syscall/main.c` (Wirt-Generator,
`syscall/Makefile` `gen_objs`); `libc.a` ohne `syscall__main.o`; kein
`why.txt` (11 Builds) zieht ein `syscall__`-Objekt.

## MD-Sonde

MDPROBE (ON/OFF/STRICT gleich): 254 freie MDs; `malloc(32)` × 10 000 /
100 000 belegt 19 / 173 MDs; nach **147 605** Blöcken (4,7 MB Nutzlast)
liefert `malloc` NULL — alle 254 MDs systemweit verbraucht, ~930 MB frei.
Gemeldet, nicht umgebaut.

## Prüfungen

| | E9 | jetzt |
|---|---|---|
| Hosttests | 1135/0/1 | **1138/0/1** |
| Release frisch, tos-api/kernel, beide Boards | 64/94 | 64/94 |
| Clippy rpi4/qemu-virt | 76/125 | 76/125 |
| Driftprobe | 31 | 31 |
| SDK-Hosttests | 13/0/0 | 13/0/0 |

QEMU ON/OFF/STRICT: Desktop, TeraDesk, QED, APITEST (106/0/5), Accessory,
TeraDesk→QED, KBDVEC+EmuCON — Bilder bytegleich mit E9; Doom VERIFIED;
EmuCON-Folge wie E9; E9MEM/E9PLAIN, ATEXIT, ACCFREE PASS; CRASHTST, UACC3A,
MPROT3B, E10USR PASS (raspi4b + virt); virt-Selbsttests PASS; PORT10 42/44
wie E9; desk3a ON = OFF = STRICT. Einzelheiten `RUNINFO.TXT`.

## Für die Pi-Karte (nur auf Ansage)

Der Kernel wurde geändert (`free_accs`), alle libc-Programme neu gelinkt
(crt0). Hashes `bin/SHA256SUMS`, `repo-binaries.sha256`.

| Karte | Quelle |
|---|---|
| `kernel8.img` | `bin/kernel8-p1.img` (`34f5ee52…`) |
| `C:\Apps\QED\QED.NAP` (+ `qed.rsc`) | `sdk/apps/qed/build/` |
| `C:\Apps\TERADESK\DESKTOP.NAP` | `sdk/apps/teradesk/build/dist/` |
| `C:\DOOM\DOOM.NAP` | `sdk/apps/doom/build/` |
| `C:\APITEST.NCP` | `sdk/apps/apitest/` |
| `C:\ACC\SYSACC.NAC` | `bin/SYSACC.NAC` |
| weitere SDK-Programme | gleichnamig in `bin/`; `TESTGEM.NAP`, `CHAINTST.NAP`, `CORE.NCP`/`COREVAL.NCP` unter `sdk/apps/…` |

## Außerhalb des Auftrags (unverändert)

- NewTOS-`Fseek` auf Konsolen-Handle `EIHNDL` statt 0 wie EmuTOS (oben, 2).
- `syscall/{check,generate,syscallparser,traps}.c` (Rest des Wirt-Generators)
  liegen weiter in `libc.a`; ungenutzt.
- `docs/sdk/07-mintlib-limits.md` Zeile `malloc` beschreibt noch das
  Vor-E9-Modell; `docs/sdk/03-first-programs.md` den Vor-E9-Plain-crt0.
