# PORT10 — Quittung: `%f`, `intptr_t`, `spawn*` und `system()` erledigt

Datum 2026-10-02 · Basis `desk-rebuild`, HEAD `160d24b` bei Beginn,
`94622ad` beim Abschluss · Belege
[measurements/measurement-2026-10-02-port10/](../measurements/measurement-2026-10-02-port10/RUNINFO.TXT)
(`%f`, `intptr_t`, `spawn*`) und
[measurements/measurement-2026-10-02-port10b/](../measurements/measurement-2026-10-02-port10b/RUNINFO.TXT)
(`system()`, DOOM ohne Ersatz).

**DONE heisst hier:** die Abnahme des Tickets ist erfuellt —
`sdk/apps/build-doom.sh` linkt ohne Diagnose-Ersatz (81/81, 0 undefined),
`sh sdk/run-host-tests.sh` 11/0/0, und `sdk/examples/port10.c` ruft auf
NewTOS (QEMU `raspi4b`) `system()` und `printf("%f")` erfolgreich auf.
`port10.c` meldet trotzdem **2 von 29 FAIL**: beide messen Kernverhalten,
das `system()`s Umleitung (`>`/`<`) braucht und das von EmuTOS abweicht
(unten „Offen", Punkt 1). Bis zur Datei unten „Entscheidung noetig" ist der
Text der Stand vor der Anwenderentscheidung und bleibt so stehen.

## Nachtrag 2026-10-02: `system()` per SDK-Overlay (Entscheidung Anwender)

* **Overlay** `sdk/libc/overlay/posix/system.c` = mintlibs Datei ohne den
  POSIX-Zweig (`/bin/sh`-/`$SHELL`-Probe, `sigaction`, `vfork`, `execve`,
  `waitpid`); alles andere woertlich. `system()` ist immer der TOS-Zweig
  `system_hack` → `spawnvp(P_WAIT)` → synchroner `Pexec(0)`. Abweichung von
  mintlib nur dort, wo `/bin/sh` oder `$SHELL` existierten. Kopfkommentar
  sagt warum und dass TASK/PROC01 das Overlay wieder entfernt.
* **Host:** `sdk/libc/tests/system-host.{sh,c}` + `system-host-gemdos.h`,
  in `sdk/run-host-tests.sh` verdrahtet: 1 Symbolpruefung (keine
  POSIX-Zweig-Symbole mehr; Gegenprobe mit der Originaldatei trennt) + 9
  Verhaltenspruefungen.
* **Ziel:** `port10.c` +8 `system()`-Pruefungen, `port10kid.c` schreibt mit
  Argument `fwrite` eine Zeile ueber `Fwrite(1, …)`.
* **DOOM:** `build-doom.sh` linkt; der Hinweistext auf die Luecke ist
  entfernt. timedemo: `timed 5026 gametics in 2254 realtics (78.043480 fps)`,
  danach laeuft `I_Error` → `ZenityErrorBox` → echtes `system()` →
  `/dev/null: …` (Rueckgabe 2), DOOM endet mit rc 0. Ohne IWAD: „No IWAD
  file was found", `system()` → 2, rc 0, Desktop zurueck — kein Haenger,
  kein Absturz.
* **Endungen:** `system()` geht durch `spawnvp`s Liste `ttp/prg/tos`
  (Quelle und Messung: `system("C:\P10\P10KID s3")` → `-1/ENOENT`, mit
  `.NCP` laeuft es). Liste **nicht** geaendert — `.NCP/.NTP/.NAP` sind
  NewTOS-eigen; ob sie hinein gehoeren, ist eine Entscheidung (Offen, 2).
* `docs/sdk/07-mintlib-limits.md` §1/§8 neu geschrieben,
  `sdk/libc/build-libc.sh` Kopfkommentar, `TASK/PORT06-STATUS-DONE.MD`
  datierte Korrektur zu „`system()` wird nie erreicht".

### Offen nach dem Nachtrag

1. **Kern, Umleitung (EmuTOS-Abweichung, gemessen):** (a) Kinder erben
   umgelenkte Standardhandles nicht — `tos-api/src/bdos/process/loader.rs:676`
   `p_uft = [-1; NUMSTD]` gegen EmuTOS `bdos/proc.c:386-397`
   `init_pd_files`/`ixforce`. (b) `Fforce` ohne Benutzungszaehler — nach
   `Fforce(1, dup)` + `Fclose(dup)` liefert `Fwrite(1)` `EIHNDL` (-37);
   EmuTOS `bdos/fshand.c:69-73` erhoeht `f_use`. Folge fuer
   `system("prog >datei")`: Datei leer, Kind schreibt auf die Konsole,
   Aufrufer verliert Handle 1. Braucht ein Kernticket (gesperrt fuer diese
   Arbeit).
2. **Entscheidung:** NewTOS-Endungen in `spawnvp`s Suchliste — ja/nein.
3. mintlib-eigen, unveraendert uebernommen: `system_hack` reserviert `argv`
   mit `argc` Plaetzen und schreibt bei einer Zeile ohne Umleitung
   `argv[argc] = NULL` dahinter (`posix/system.c`, `malloc` vs.
   `argv[i] = (char *)0`). Mit mintlibs `malloc` (Block vom Ende des freien
   Stuecks, 24-Byte-Kopf) ueberschreibt das das `valid`-Wort des zuletzt
   davor vergebenen Blocks → dessen `free` ist still wirkungslos (Leck je
   Aufruf, kein Absturz; aus dem Quelltext, nicht gemessen). Nicht im
   Overlay behoben (Auftrag: nur den POSIX-Zweig entfernen).
4. `perror` in `system_hack` zeigt ein liegengebliebenes `errno`
   (`Fcreate` per Trap-Makro setzt keins) — mintlib-Verhalten.

## Nachtrag 2026-10-02 (2): Suchendungen `ntp`, `nap`, `ncp` (Entscheidung Anwender)

Beleg [measurements/measurement-2026-10-02-port10c/](../measurements/measurement-2026-10-02-port10c/RUNINFO.TXT).

* **Entscheidung:** die Endungsliste, mit der `spawnvp` (und damit
  `system()`) einen Namen ohne Endung aufloest, ersetzt mintlibs `ttp`,
  `prg`, `tos` durch `ntp`, `nap`, `ncp` — Reihenfolge und Zuordnung
  `ttp→ntp`, `prg→nap`, `tos→ncp`. Nur die nativen Programme sind auf NewTOS
  ausfuehrbar; die m68k-Endungen kommen mit dem Emulator **hinter** die
  nativen (in den Overlays so kommentiert). `.NAC` kommt nicht hinein.
* **Drei Kopien der Liste, alle drei geaendert** (nur Overlays):
  `overlay/mintlib/spawnvp.c` (`spawnvp`/`spawnlp`/`system()`),
  `overlay/mintlib/spawn.c` (neu als Overlay, Suche des `#!`-Interpreters,
  sonst woertlich) und `overlay/posix/execvp.c` (neu als Overlay,
  `{ "", .ntp, .nap, .ncp }`; mintlibs `.app`/`.gtp` fallen mit weg, beide
  m68k; `__mint == 0`, also ist das die Liste, die `execvp` laeuft).
  `sdk/libc/build-libc.sh` Kopfkommentar ergaenzt.
* **Gefunden, nicht geaendert** (anderer Zweck: Ausfuehrbits, keine
  Namenssuche; erledigt in Nachtrag (3) unten): `mintlib/quickstat.c` und das ausgeschlossene
  `mintlib/do_stat.c` setzen `0111` nur fuer `.app/.gtp/.ttp/.prg/.tos`,
  dazu mit Kleinbuchstaben-Vergleich — `access(X_OK)` ist fuer eine `.NCP`
  falsch (aus dem Quelltext, nicht gemessen).
* **Gross/klein:** `_buffindfile` haengt `.ntp` usw. klein an; GEMDOS macht
  auf FAT Grossbuchstaben daraus (`tos-api/src/bdos/fs/find.rs`,
  `dir.rs`), `P10KID.ntp` trifft `P10KID.NTP`.
* **Gemessen (QEMU raspi4b):** `system("C:\P10\P10KID s3")` → **42** (vorher
  `-1`/`ENOENT`); `system("C:\P10\ONLYNCP s3 s4")` → 43;
  `system("C:\P10\LEGACY s5")` mit nur `LEGACY.PRG` (= dieselbe Datei wie
  das Kind) → `-1`/`ENOENT`. `port10.c` 29/31, die 2 FAIL unveraendert die
  Kern-Umleitung (unten „Offen", Punkt 1).
* **Hosttest:** `sdk/libc/tests/spawn-host.sh` baut zusaetzlich
  `spawn-search-host` (`spawn-search-host.c`, `spawn-search-fs.c`) ueber
  mintlibs echtem `buffindf.c`: 19 Pruefungen + Objektpruefung `spawn.c`;
  Gegenprobe mit der alten Liste: 13 FAIL. `sh sdk/run-host-tests.sh`
  11/0/0. QED, TeraDesk, DOOM bauen mit 0 undefined; QED.NAP und
  DESKTOP.NAP byte-gleich mit port10b.
* `sdk/examples/port10.c`: die ENOENT-Pruefung ist jetzt eine 42-Pruefung,
  dazu `ONLYNCP` und der `LEGACY.PRG`-Koeder.
* Doku: `docs/sdk/07-mintlib-limits.md` §8 (und Kopfzeilen).

## Nachtrag 2026-10-02 (3): Ausfuehrbits fuer `.NTP`/`.NAP`/`.NCP` (Entscheidung Anwender)

Beleg [measurements/measurement-2026-10-02-port10d/](../measurements/measurement-2026-10-02-port10d/RUNINFO.TXT).

* **Entscheidung:** `stat()`/`access()` halten eine Datei nur fuer
  ausfuehrbar (`0111`), wenn sie auf `.ntp`, `.nap` oder `.ncp` endet,
  ohne Ruecksicht auf Gross/Klein. mintlibs `.app/.gtp/.ttp/.prg/.tos`
  werden **ersetzt**, nicht ergaenzt; sie kommen mit dem Emulator hinter die
  nativen (im Overlay kommentiert). `.NAC` ist nicht ausfuehrbar.
* **Wo entschieden wird** (Quelltext): `mintlib/quickstat.c` `__quickstat`,
  im `Fsfirst`-Zweig, `strrchr(_path, '.')` + `strcmp` — also nach dem Namen,
  wie der Aufrufer ihn schreibt, nur Kleinbuchstaben. Erreicht, weil
  `__sys_stat` (`newtos-arch/do_stat.c`) `Fstat64` (0x14b) ruft und NewTOS
  dafuer `EINVFN` = mintlibs `-ENOSYS` liefert. Daran haengen `stat`/`lstat`
  (`__do_stat`), `access` (`newtos-arch/access.c`), `_buffindfile`, `open`,
  `glob`. Weitere Kopien, beide **nicht gebaut**: `mintlib/do_stat.c`
  (dieselbe Liste, ausgeschlossen) und `mintlib/do_fstat.c` (Ausfuehrbit
  nach m68k-Magic `0x601a`/`#!`, ausgeschlossen; `newtos-arch/do_fstat.c`
  setzt keins).
* **Fehler zuerst belegt:** neuer Hosttest `sdk/libc/tests/quickstat-host.sh`
  (`quickstat-host.c`, `quickstat-host-fs.c`, `quickstat-host-gemdos.h`):
  mintlibs `__quickstat` + `newtos-arch/access.c` ueber ein falsches
  FAT-Verzeichnis (Grossbuchstaben, `Fsfirst` vergleicht wie GEMDOS). Mit
  der unveraenderten Datei aus dem Tarball **15 von 27 FAIL** (`X.NCP` usw.
  `0100644`, `x.prg` usw. `0100755`) — `host/quickstat-host-pristine.txt`.
* **Fix (nur Overlay):** `sdk/libc/overlay/mintlib/quickstat.c` = pristine
  plus `strcasecmp` gegen `.ntp`/`.nap`/`.ncp`; sonst woertlich.
  `sdk/libc/build-libc.sh` Kopfkommentar ergaenzt. Danach **27 von 27**.
* **Gemessen (QEMU raspi4b, Abschnitt 5 in `sdk/examples/port10.c`):**
  `access(…, X_OK)` = 0 fuer `C:\P10\P10KID.NCP`, `c:\p10\p10kid.ncp`,
  `P10KID.NTP`, `APP.NAP`; `-1`/`EACCES` fuer `LEGACY.PRG`, `legacy.prg`,
  `ACC.NAC`, `PLAIN.TXT`; `stat()` `0100755` bzw. `0100644`. Alle 13 neuen
  Pruefungen PASS; `port10.c` gesamt 42/44, die 2 FAIL unveraendert die
  Kern-Umleitung (unten „Offen", Punkt 1).
* **Hosttests:** `sh sdk/run-host-tests.sh` **12/0/0** (neu `quickstat-host`).
* **Apps:** QED, TeraDesk, DOOM bauen, je 0 undefined. `DESKTOP.NAP`
  byte-gleich (linkt `__quickstat` nicht); `QED.NAP` und `DOOM.NAP` aendern
  sich (beide linken `__quickstat`). Verhalten der Apps nicht gefahren.
* Doku: `docs/sdk/07-mintlib-limits.md` §8 (und Kopfzeilen).
* Offen: QEDs eigener Quelltext (`src/av.c:81`) prueft `.app`/`.prg` selbst
  — App-Code, nicht angefasst.

## Achtung: fremder Kommit mit halber PORT10-Arbeit

Waehrend dieser Arbeit kam `685cd68` („docs(task): PORT06 scales …") in den
Zweig. Er enthaelt **Teile dieses Tickets** aus dem gemeinsamen Arbeitsbaum:
`sdk/libc/expected-failures.txt`, `sdk/libc/newtos-arch/cmdargs.c`,
`sdk/libc/tests/cmdargs-host.{c,sh}`, `sdk/run-host-tests.sh` — aber **nicht**
die zugehoerigen neuen Dateien (Overlays, `gmp-host`/`spawn-host`). `HEAD`
allein ist damit inkonsistent: `build-libc.sh` bricht auf einem sauberen
Checkout ab (spawnve.c/spawnvp.c scheitern, stehen aber nicht mehr in der
Liste), `run-host-tests.sh` meldet zwei fehlende Tests. Mit dem Arbeitsbaum
zusammen ist alles stimmig. Nichts per git angefasst.

## Ursachen

| Punkt | Ursache (Quelle) | Beleg |
|---|---|---|
| `printf("%f")` falsche Ziffern / Data Abort in `__printf_fp` | `stdlib/gmp-mparam.h:23-25` sagt `BITS_PER_MP_LIMB 32`, `BYTES_PER_MP_LIMB 4`; `mp_limb_t` ist `unsigned long` (`stdlib/gmp.h:57`, weder `_SHORT_LIMB` noch `_LONG_LONG_LIMB` gesetzt) = 64 Bit auf LP64. `printf_fp.c`, `_itoa.c`, `strtod.c`, `fpioconst.c`, `gmp/dbl2mpn.c` waehlen damit 32-Bit-Code und -Tabellen und rechnen sie in 64-Bit-Limbs. Jede dieser Dateien hat den `== 64`-Zweig schon (glibc-Code). | vorher `1.5 → "1.?00000"`, `1e100 →` Muell (`serial/fp-before-pristine-gmp-mparam.log`); Host: 17/21 falsch mit dem Original-Header, 21/21 richtig mit dem Overlay (`host/gmp-host-*.txt`) |
| `intptr_t`/`uintptr_t` 32 Bit | `include/stdint.h:132-144` kennt nur `__MSHORT__` (long) und sonst `int`; `<unistd.h>` definiert `intptr_t` dagegen als `long` (`bits/types.h:141`) — je nach Include-Reihenfolge verschieden | `p_saveg.c:187/192` und TeraDesk `environm.c:57/92` warnen nicht mehr |
| `spawnve.c`/`spawnvp.c` uebersetzen nicht | `spawnl`/`spawnle`/`spawnlp` casten die `va_list` nach `char **` (m68k: Zeiger in den Stapel; AAPCS64: Struktur) | `obj/*.err` vorher, jetzt fehlerfrei |
| `spawnvp()` haette Adresse 0 gelesen | `_spawnve` (`mintlib/spawn.c`) ersetzt `envp == NULL` durch `environ` und indiziert es ohne Pruefung; mintlibs `crtinit.c parseargs()` setzt `environ` immer (bei `p_env == 0` auf `{NULL}`), unser `cmdargs.c` setzte es gar nicht → `environ == NULL` | `cmdargs-host`: environ jetzt die leere Umgebung |
| `system()` linkt nicht | `posix/system.c` enthaelt neben dem TOS-Zweig (`system_hack` → `spawnvp`) den POSIX-Zweig mit `vfork`/`execve`/`waitpid`. `vfork` ist m68k-Assembler (`unix/vfork.S`, nie assembliert), `__waitval`/`__waittime` stehen in `mintlib/thread.c` (ausgeschlossen). Auf TOS ist mintlibs `vfork` = `tfork()` = `Pexec(PE_CBASEPAGE)` + `Pexec(PE_GO)` auf einen Basepage, dessen Text der eigene Code des Elternprozesses ist. NewTOS beantwortet die Modi 5/6/7 mit `EINVFN` (`tos-api/src/bdos/process/pexec.rs:177-179`) und `PE_GO` nimmt nur ein Handle aus `PE_LOAD`. | Link-Log `method/build-doom-delivered.log`: genau `vfork`, `__waitval`, `__waittime` |

## Was laeuft (QEMU raspi4b, `serial/port10-serial.log`)

`port10: ALL PASS (21 checks)` — `%f/%e/%g`, DOOMs Zeilenformat, 64-Bit-
Ganzzahlen, `strtod`, `intptr_t`-Rundweg und Grenzen, `PRI*PTR`; dazu
`spawnv`, `spawnvp` (= TOS-Zweig von `system()`), `spawnl` (auch 11
Argumente, also Register **und** Stapel), `spawnlp`, `spawnle` mit eigener
Umgebung, `spawnvp` auf fehlende Datei → `-1/ENOENT`. Jeder Rueckgabewert ist
der `Pterm`-Code des Kindes.

DOOM `-timedemo demo1` (Diagnose-Link aus PORT06, Skripte unveraendert):
`timed 5026 gametics in 2302 realtics (76.416161 fps)`, DOOM endet mit rc 0,
der Desktop kommt zurueck (`screendumps/31`, `32`).

## MEM03-Wechselwirkung

`_spawnve` baut **immer** einen Umgebungsblock (`ARGV=`), also bekommt jedes
per `spawn*`/`system()` gestartete Kind `p_env` = physische Adresse
(gemessen `0x1804000`), nicht im EL0-Fenster. Unsere libc liest `p_env` nicht
(`cmdargs.c`, dokumentiert), deshalb laufen libc-Kinder. Ein Kind, das
`p_env` selbst liest, faellt wie PORT09s `envkid`. Nicht behoben (MEM03).

## Befund gegen PORT06

`ZenityErrorBox` wird bei **jedem** `I_Error` erreicht: `i_system.c:410`
prueft `exit_gui_popup && !I_ConsoleStdout()`, und doomgenerics
`I_ConsoleStdout` liefert ausserhalb von `ORIGCODE` immer 0
(`i_system.c:210-221`). Nur `-nogui` verhindert den Aufruf. Gemessen: die
Diagnose-Ersatzfunktion meldete sich nach der timedemo-Zeile. PORT06s
„nie erreicht, weil stdout ein Terminal ist" stimmt nicht; der `%f`-Absturz
lag nur davor. Text in `build-doom.sh` korrigiert; PORT06-Dateien unveraendert.

## Entscheidung noetig (fuer `system()`)

1. **Kern:** `Pexec`-Modi 5 (`PE_BASEPAGE`) und 4 (`PE_GO` auf einen vom
   Aufrufer gebauten Basepage, Text im eigenen Raum) nach EmuTOS — dann
   `unix/vfork.S` nach AArch64 (wie `setjmp.S`) und `mintlib/thread.c` mit
   LP64-Casts uebernehmen. mintlib bliebe unveraendert im Verhalten.
2. **SDK:** Overlay von `posix/system.c`, das den POSIX-Zweig auf NewTOS
   nicht mitbaut — `system()` ist dann immer der TOS-Zweig. Abweichung von
   mintlib, solange `/bin/sh`/`$SHELL` dort nie erfolgreich waeren.

Bis dahin linkt `build-doom.sh` nicht. Eine `vfork`, die nur `-1` liefert,
waere ein Stub und wurde nicht gebaut.

## Dateien

* `sdk/libc/overlay/stdlib/gmp-mparam.h` (neu) — Limb-Breite aus `long`.
* `sdk/libc/overlay/include/stdint.h`, `inttypes.h` (neu) — LP64-Zweig fuer
  `intptr_t`/`uintptr_t`, `INTPTR_*`/`UINTPTR_MAX`, `PRI*PTR`/`SCN*PTR`.
* `sdk/libc/overlay/mintlib/spawnve.c`, `spawnvp.c` (neu) — `va_arg` statt
  `va_list`-Arithmetik; `expected-failures.txt` um beide gekuerzt.
* `sdk/libc/newtos-arch/cmdargs.c` — veroeffentlicht `environ` als leere
  Umgebung (mintlibs `p_env == 0`-Fall), liest `p_env` weiterhin nicht.
* Hosttests: `sdk/libc/tests/gmp-host.{sh,c}` + `gmp-probe.c`,
  `spawn-host.{sh,c}`, `cmdargs-host.{sh,c}` erweitert; in
  `sdk/run-host-tests.sh` verdrahtet.
* `sdk/examples/port10.c`, `port10kid.c` (neu) — Abnahmeprogramm + Kind.
* Doku: `docs/sdk/02`, `05` (B6, B7), `07` (§1, §5, §6, neu §8), `08`;
  `sdk/libc/build-libc.sh` (Kopfkommentar), `sdk/apps/build-doom.sh`
  (Meldungstext), `sdk/overlays/coremark/newtos/core_portme.h` (nur
  Kommentar, der jetzt falsche Behauptung).

## Grundwerte

Kein Rust-Quelltext angefasst. `cargo test -p tos-api -p tos-types -p xtask
--no-fail-fast`: **964 / 0 / 1** (unveraendert). `sdk/run-host-tests.sh`:
**10 / 0 / 0** (vorher 8, neu `gmp-host`, `spawn-host`). Builds/Clippy/Drift
nicht gefahren (kernel/tos-api unberuehrt); Release-Bau rpi4 fuer die Laeufe
64/94 Warnungen. QED und TeraDesk neu gebaut: je 0 undefined, TeraDesk zwei
Warnungen weniger, keine neue; Verhalten nicht gefahren. Nebenwirkung:
`lib_ensure` hat `gemlib.a`/`cflib.a`/`libm.a` (unversionierte Bauprodukte)
neu gebaut.

## Offen, nicht angefasst

* `PTRDIFF_MIN/MAX` und `SIZE_MAX` in mintlibs `stdint.h` sind auf LP64
  weiter die 32-Bit-Werte.
* `libc.a` hat `__mpn_add`/`__mpn_sub_1` … doppelt (`fpioconst.o` und
  `mpn-inlines.o`, C99-`extern inline` aus `gmp.h`) — Link-Konflikt, sobald
  beide gezogen werden.
* ~~`spawnvp`s Suchendungen sind mintlibs `ttp/prg/tos`; ein Name ohne Endung
  findet keine `.NCP`/`.NTP`/`.NAP`.~~ Erledigt 2026-10-02, Nachtrag (2) oben.
* Mit einem echten `system()` liefe DOOMs `ZenityAvailable` durch
  `">/dev/null"` → `"null:"` (`mintlib/unx2dos.c`) und `Fcreate` — nicht
  gefahren.
* `docs/INVENTORY.md` §8.2 beschreibt den `uintptr_t`-Fehler noch als offen.
