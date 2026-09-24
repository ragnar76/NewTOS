# PORT01 — STATUS: URSACHE BEHOBEN (2026-09-25), Abnahme blockiert durch PORT02

**Auftrag:** Eine auf QED gezogene Datei startet QED, erreicht die Anwendung
aber nicht. Feststellen, ob es die crt0-`argv`-Konstruktion oder die
LP64-BasePage ist — **an der Quelle, vor jeder Aenderung**.

Basis: Branch `desk-rebuild`, HEAD `45de38b`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset` auf dem gemeinsamen Baum (das einzige
git-Kommando war das lesende `git show HEAD:sdk/libc/crt0.S`, mit dem die
„Vorher"-Programme gebaut werden, und `git diff`).

Messung: [`measurement-2026-09-25-port01/`](../measurement-2026-09-25-port01/).

---

## 1. Es war Kandidat 1

`sdk/libc/crt0.S` hat `main` ein festes `argc = 1`, `argv = { "", NULL }`
uebergeben und `p_cmdlin` nie angesehen. mintlibs `crtinit.c` `parseargs()`,
die `argv` aus dem Schwanz baut, wird nicht uebersetzt — die *andere* Haelfte
derselben Datei ist m68k-Assembler (`sdk/libc/expected-failures.txt:25`) — und
`crt0.S` hatte ihre Stelle eingenommen, ohne ihre Arbeit zu tun. Der Zustand
stand so auch in `docs/sdk/07-mintlib-limits.md` §5.

Betroffen war **jedes** libc-gebundene Programm, nicht nur QED.

## 2. Kandidat 2 ist ausgeschlossen — das LP64-Problem ist nicht real geworden

`sdk/libc/mintlib/include/mint/basepage.h` ist die LP64-korrigierte Fassung:
feste 32-Bit-Felder, `_Static_assert` auf `p_hitpa@4`, `p_env@0x2c`,
`sizeof(BASEPAGE) == 256`. `ARGVTEST.NAP` liest `p_cmdlin` durch genau diese
Struktur und bekommt in **beiden** Laeufen `len 13 "C:\README.TXT"`.

## 3. Aenderungssatz

| Datei | Art | Was |
|-------|-----|-----|
| `sdk/libc/newtos-arch/cmdargs.c` | neu | `__libc_parseargs()` — Argument-Haelfte von `parseargs()`, fuer AArch64; definiert `__libc_argc`/`__libc_argv` |
| `sdk/libc/crt0.S` | geaendert | ruft sie vor `main`, reicht `argc`/`argv` durch; `__crt0_argv` entfaellt |
| `sdk/libc/tests/cmdargs-host.c` | neu | Host-Pruefstand, 9 Faelle |
| `sdk/libc/tests/cmdargs-host.sh` | neu | dessen Bau-/Laufhuelle |
| `sdk/run-host-tests.sh` | geaendert | haengt ihn ein |
| `sdk/examples/argvtest.c` | neu | `ARGVTEST.NAP` — druckt Rohschwanz und `argc`/`argv` |
| `tools/make-sd-image.sh` | geaendert | legt `C:\ARGVTEST.NAP` ins Abbild |
| `sdk/libc/build-libc.sh` | geaendert | Uebersetzungsschritt fuer `cmdargs.c` |
| `sdk/lib-stale.sh` | geaendert | Kommentar (elf → zwoelf Dateien) |
| `docs/sdk/{01,03,05,07,08}*.md` | geaendert | die alte Einschraenkung richtiggestellt |

Kein Rust angefasst. Kein Kernel-Code angefasst. QED nicht angefasst.

## 4. Belege

**Host:** `sdk/run-host-tests.sh` → 8 passed, 0 failed, 0 skipped
(`cmdargs-host` neu, 9 Faelle: leerer Schwanz, die beiden DESK19-Schwaenze,
Leerzeichenlaeufe, Tabs, Name+Zeilennummer, MagX-Anfuehrung, verdoppeltes
`'`, und der dichteste 128-Byte-Schwanz = 64 Argumente).

**QEMU raspi4b**, gleiche Geste wie DESK19, ein Programm, ein Lauf:

```
vorher   ARGVTEST: cmdlin len 13 raw "C:\README.TXT"
         ARGVTEST: argc 1
nachher  ARGVTEST: cmdlin len 13 raw "C:\README.TXT"
         ARGVTEST: argc 2
         ARGVTEST: argv[1] "C:\README.TXT"
```

QED: vorher nur die Menueleiste, kein Fenster
(`screendumps/before-d-qed.png`); nachher handelt QED nach dem Argument
(`screendumps/after-d-qed.png`) — und laeuft in §5.

Null `[EXC]`, null `[PANIC]` in allen fuenf Logs; `fsck_msdos -n` sauber auf
jedem Kartenabbild (`serial/fsck.txt`).

## 5. Zweiter, eigener Defekt — blockiert die Abnahme, gehoert in ein neues Ticket

Gemessen: QED fragt `Fopen "C:\README.TX"` — ein Zeichen zu kurz.
cflibs `make_normalpath` (`sdk/cflib/cflib-21/fsnorm.c:85-95`) entfernt den
selbst angehaengten `\` wieder, wenn der Pfad kein existierendes Verzeichnis
ist (`path_exists` = `Fattrib(p,0,0) >= 0 && (attr&FA_DIR)`, fuer eine Datei
auf TOS 1-3 / TOS 4 / EmuTOS gleichermassen falsch); QED entfernt den Trenner
**bedingungslos** (`src/poslist.c:53-54`, `src/dd.c:99-101`).

Unsere `fsnorm.c` ist byte-identisch mit dem unveraenderten cflib-21-Tarball.
Im Scratch belegt (**nicht uebernommen**): laesst `make_normalpath` den
Trenner stehen, oeffnet QED die Datei — `Fopen len 13 "C:\README.TXT"`,
`screendumps/exp2-qed.png`.

Nicht uebernommen, weil die Aenderung jeden cflib-Aufrufer und damit jede
portierte cflib-Anwendung betrifft. Entscheidung des Leads.

## 6. Nur auf der echten Pi zu bestaetigen

Die Geste mit echter Maus, und der zweite Ziehweg ueber eine
Desktop-Verknuepfung (`kernel/src/desk/dir.rs:1885`). Beide Wege enden in
`do_aopen_drop` und uebergeben denselben Schwanz; die Startroutine sieht
ohnehin nur die BasePage.
