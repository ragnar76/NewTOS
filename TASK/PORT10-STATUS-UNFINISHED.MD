# PORT10 — Quittung: `%f` und `intptr_t` behoben, `spawn*` laeuft, `system()` wartet auf eine Entscheidung

Datum 2026-10-02 · Basis `desk-rebuild`, HEAD `160d24b` bei Beginn · Belege
[measurements/measurement-2026-10-02-port10/](../measurements/measurement-2026-10-02-port10/RUNINFO.TXT)

**UNFINISHED heisst hier:** Punkt 2 (`printf("%f")`) und Punkt 3
(`intptr_t`/`uintptr_t`) sind erledigt und in QEMU `raspi4b` belegt; von
Punkt 1 laeuft die `spawn*`-Familie (synchroner `Pexec(0)`, wie mintlib auf
TOS ohne MiNT). **`system()` linkt weiterhin nicht**, und damit auch
`sdk/apps/build-doom.sh` nicht. Der Rest haengt an einem Kernmerkmal, das
NewTOS fehlt — laut Arbeitsanweisung angehalten, bevor `kernel/` oder
`tos-api/` angefasst wird.

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
* `spawnvp`s Suchendungen sind mintlibs `ttp/prg/tos`; ein Name ohne Endung
  findet keine `.NCP`/`.NTP`/`.NAP`.
* Mit einem echten `system()` liefe DOOMs `ZenityAvailable` durch
  `">/dev/null"` → `"null:"` (`mintlib/unx2dos.c`) und `Fcreate` — nicht
  gefahren.
* `docs/INVENTORY.md` §8.2 beschreibt den `uintptr_t`-Fehler noch als offen.
