# LIBC-MAIN — libc-crt0 ruft wieder mintlibs `_main` — Quittung

**Status: ERLEDIGT (Developer) · QEMU raspi4b in memprot on/off/strict belegt ·
Tester-Abnahme offen · nichts committet · Pi offen (nur auf Ansage)** ·
2026-10-13 · Zweig `main`, Basis HEAD `f3970e1e` · Beleg
[measurements/measurement-2026-10-13-crt0-main/](../measurements/measurement-2026-10-13-crt0-main/RUNINFO.TXT)

Folgeschritt aus [ARCH03-POSTE9](ARCH03-POSTE9-STATUS-UNFINISHED.MD) Punkt 2 und
[FS10](FS10.md) („gibt frei: crt0 kann danach mintlibs `_main` rufen“).

## Änderung

`sdk/libc/crt0.S`: nach `__libc_crtinit` (unverändert) jetzt
`_main(__libc_argc, __libc_argv, environ)` statt `exit(main(...))` — so enden
mintlibs `_crtinit` (`mintlib/crtinit.c:257`) und `_acc_main` (`:147`); ein
Pfad für Programme und Accessories, weil `__libc_crtinit` beide Fälle schon
vor dem Sprung erledigt (Accessory: `Malloc`-Stack, `argv = environ =
{"", NULL}`). `_main` ist der schwache Alias von `__libc_main`
(`mintlib/main.c:305`), ein Programm kann weiter ein eigenes liefern.
Kommentar in `sdk/libc/build-libc.sh` nachgezogen; Doku
`docs/sdk/07-mintlib-limits.md` §5 (neue Zeile „start-up (`_main`)“, Zeile
`exit`) und `docs/sdk/03-first-programs.md` (crt0-Ablauf; nannte noch das alte
`__libc_init_heap`). Kein Kernel-/tos-api-Code, mintlib-Quelltext unverändert.

## Was `_main` zusätzlich tut (Beleg `mintlib/main.c`)

| Zeile | Wirkung | NewTOS |
|---|---|---|
| 87, 294 | `errno = 0` | — |
| 95-98 | `Getcookie(C_MiNT)` → `__mint` | kein MiNT-Cookie → 0 (TOS-Pfad) |
| 100-101 | Programm: `Pdomain(1)` | `EINVFN` wie EmuTOS über `MAX_FNCALL` (`bdos/bdosmain.c:444-445`), nur gespeichert |
| 107-108 | `Pgeteuid==Pgetuid && Pgetegid==Pgetgid` → nicht „secure“ | alle `EINVFN` → gleich → `UNIXMODE` gilt |
| 115-142 | `UNIXMODE` (`b`, `r<c>`) | — |
| 147-158 | nur MiNT (`Dgetdrv` U:) | übersprungen |
| 161-162 | `__open_stat[]` löschen | — |
| 168-169 | `argv[0] == ""` und `isatty(2)` → `Fforce(2, -1)` | Spec `46-fforce.md`: BIOS-Handle wird gespeichert, `E_OK` |
| 191-198 | `setvbuf` stdin `_IOFBF` / stdout `_IOLBF` / stderr `_IONBF` | mintlib-Eigenheit: Größe 0 → `EINVAL` nach dem Setzen der Modus-Bits, auf TOS gleich |
| 201-203 | `init_stream_mode`: Terminal unter TOS → Textmodus, Stream-Funktionen neu (mit Seek) | erster Flush `Fseek(0,h,SEEK_CUR)` = 0 seit FS10 |
| 226-291 | `PATH` (bzw. `PCONVERT`-Liste) in `environ` → Unix-Form (`_path_dos2unx`, `unx2dos.c:217`) | `PATH=C:\` → `/dev/c/`; `p_env` unberührt |
| 303 | `exit(main(argc, argv, envp))` | wie bisher |

`__main()` (globale Konstruktoren, `crtinit.c:142-145, 252-255`) gibt es nur
unter `__GCC_HAVE_INITFINI_ARRAY_SUPPORT` — clang definiert das für
`aarch64-unknown-none-elf` nicht (`-dM -E` geprüft), `mintlib/__main.c` ist
bei uns leer: nichts zu rufen. **Doppelungen:** keine — `__libc_crtinit`/
`cmdargs.c` machen `_base`, parseargs, `_stksize`, Mshrink/Accessory-Stack,
`_app`; nichts davon steht in `__libc_main`. NewTOS beantwortet nichts, was
`_main` braucht, falsch (nach FS10).

## Ergebnisse (QEMU raspi4b, `checks/summary-cm.txt`)

| | ON | OFF | STRICT |
|---|---|---|---|
| STDOUTDB (Rot-Fall der Diagnose) | grün | grün | grün |
| FSEEKCON | PASS | PASS | PASS |
| ATEXIT (Datei `main-body;atexit-handler;`, Exitcode 7) | PASS | PASS | PASS |
| E9MEM ×2 / E9PLAIN | PASS | PASS | PASS |
| ACCFREE | PASS | PASS | PASS |
| libc-Accessory PPACC (`_acc_main`-Pfad) | PASS | PASS | PASS |
| VFORKT allein | PASS | PASS | PASS |
| ENVRN, PPARENT AUTO, PPZJ | PASS | PASS | PASS |
| S2ALL(S) mit ENVKIDM | PASS | PASS | PASS |
| PORT10 | 44/44 | 44/44 | 44/44 |
| APITEST | 106/0/5 | 106/0/5 | 106/0/5 |
| Doom | VERIFIED | VERIFIED | VERIFIED |
| KBDVEC + EmuCON | PASS | PASS | PASS |
| Desktop, TeraDesk, QED, TeraDesk→QED, Desktop+ACC | Dumps = a13 | = a13 | = a13 |

Rot→Grün: STDOUTDB mit neuem crt0 auf dem a13-Kernel (vor FS10):
`fputs` −1, `errno` 37 (EBADF); auf HEAD: `fputs` 0, `printf` 0x1a.
Keine `[EXC]`/`[PANIC]`-Zeile. Dumps gegen a13: on 24/0, off 24/0,
strict 20/4 (nur Doom-Jitter und die beabsichtigten Strict-Abstürze
`s2allsj-crash1/2` mit verschobenen Adressen).

**Sonde angepasst:** S2ALLJ (ENVKIDJ aus dem a13-Run) meldet jetzt MEM03 FAIL,
weil `envkidj.c` `getenv("PATH") == "C:\"` erwartet — mintlibs `_main`
liefert `/dev/c/` (gewolltes mintlib-Verhalten auf TOS). Neue Sonde
`method/tests/envkidm.c` prüft `getenv("PATH") == "/dev/c/"` **und** rohes
`p_env` = `PATH=C:\` plus argc/argv: PASS in allen Modi. PPARENT Z/Z-KID und
ENVKID (Original) scheitern weiter wie in a13 (veraltete Sonden).

Basislinien: Hosttests 1277/0/1, Release tos-api 64 / kernel 94 beide Boards,
Clippy 76/125 beide Boards, Driftprobe 31, SDK-Hosttests 19/0/0 — keine
Abweichung.

## Beobachtbare Verhaltensänderungen

* stdout zeilengepuffert im Textmodus, stderr ungepuffert und (bei leerem
  `argv[0]`) per `Fforce(2,-1)` auf die Konsole.
* `getenv("PATH")` liefert die Unix-Form; `spawnvp`/`execvp`/`system()`
  wandeln zurück (PORT10 grün).
* Vor `main` ein paar `Malloc`-Blöcke mehr (stdio-Puffer, PATH-Kopie) —
  spätere Adressen verschieben sich (Doom-Zone 0x1c01000 statt 0x1bff000).

## Offen

* Tester-Abnahme; echte Pi; virt-Läufe, `asid=on`, EmuCON-Fünferlauf, desk3a/
  ppdesk, MDPROBE, Crash-Sonden nicht gefahren (siehe RUNINFO).
* `docs/STATUS.md`-Eintrag mit Commit-Hash beim Commit (Lead).
