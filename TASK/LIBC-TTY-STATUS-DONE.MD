# LIBC-TTY — mintlibs eigenes `isatty` statt NewTOS-Shim — Quittung

**Status: ERLEDIGT (Developer, Entscheidung 1) · QEMU raspi4b in memprot on/off/strict belegt ·
Tester-Abnahme offen · nichts committet · Pi offen** · 2026-10-13 · Zweig `main`, Basis HEAD
`ad789346` · Beleg
[measurements/measurement-2026-10-13-libc-tty/](../measurements/measurement-2026-10-13-libc-tty/RUNINFO.TXT)

Ticket: [LIBC-TTY](LIBC-TTY.md), Analyse `docs/analysis/libc-tty-2026-10-13.md`.
Entscheidungen 2 (ioctl) und 4 (`do_fstat`) liegen in [LIBC-IOCTL](LIBC-IOCTL.md) /
[LIBC-FSTAT](LIBC-FSTAT.md) und sind hier nicht angefasst; Entscheidung 3 (Doppellesen im
RAW-Weg bleibt) ist in `docs/sdk/07-mintlib-limits.md` §5 vermerkt.

## Änderung

* `sdk/libc/newtos-arch/isatty.c` gelöscht (Löschung im Index vorgemerkt).
* `sdk/libc/build-libc.sh`: `mintlib/isatty.c` aus `EXCLUDE`, Overlay-Kompilierschritt und
  Kommentar entfernt; `__open_stat[]` kommt wieder aus `mintlib/isatty.c:14`.
* `sdk/libc/newtos-arch/access.c`: Kommentar nennt den Shim nicht mehr.
* Hosttest `sdk/libc/tests/fstat-host.sh` (+ `-gemdos.h`, `-fs.c`, `.c`): baut
  `mintlib/isatty.c`, gefaktes `Fcntl` → −32, Std-Handles 0–2 im `Fseek`-Modell als Gerät
  (0); neue isatty-Prüfungen. 25 → 44 Checks; gegen den alten Shim 9 rot.
* Doku: `docs/sdk/07-mintlib-limits.md` (isatty-Zeile §2, Start-Zeile §5, neue Zeile
  „console `read()`“ mit RAW-Weg/Doppellesen = Upstream, veralteter `getenv("PATH")`-Satz),
  `docs/sdk/04-porting-anatomy.md:81`, `docs/sdk/05-abi-pitfalls.md` B6, `docs/MEMORY.md`
  (Eintrag Shim-Liste).

## Ergebnisse

| | Soll | Ist |
|---|---|---|
| Hosttests | 1323/0/1 | 1323/0/1 |
| Warnungen tos-api/kernel, beide Boards | 64/94 | 64/94 |
| Clippy beide Boards | 76/125 | 76/125 |
| Driftprobe | 31 | 31 |
| SDK-Hosttests | 19/0/0 | 19/0/0 (fstat-host 44 Checks) |

| QEMU raspi4b | ON | OFF | STRICT |
|---|---|---|---|
| TTYA: isatty(0/1/2)=1, Datei-Handle=0 (Position bleibt), Kind nach `Fforce(1,Datei)`: isatty(1)=0, stdout voll gepuffert | PASS | PASS | PASS |
| TTYPROBE (Keys-Kernel): zusätzlich `getchar`='a', `fgets`="b\n", "cop\n" ohne Flag-Setzen | PASS | PASS (Wiederholung, 1. Lauf Host-Stall beim Booten) | PASS |
| STDOUTDB | wie aux-Abnahme | wie aux-Abnahme | wie aux-Abnahme |
| STDERRP neue crt0, STDERRA | identisch | identisch | identisch |
| APITEST | 106/0/5 | 106/0/5 | 106/0/5 |
| Doom | VERIFIED | VERIFIED | VERIFIED |
| Screendumps Desktop/TeraDesk/QED/APITEST-t40 vs aux-Abnahme | gleich | gleich | gleich |

Doom-Bilder unterscheiden sich (Demo-Timing), APITEST-t25 in ON (Timing; t40 gleich).

## Beobachtbare Verhaltensänderungen

* `getchar`/`fgets`/`read(0)` auf der Konsole: `Fread`-Zeilenweg mit Echo und Editieren
  statt RAW-`Crawcin` (kein Echo, zwei Tasten pro Byte, `fgets` endete nie).
* Vom Elternprozess per `Fforce` auf eine Datei gelenktes stdout: `isatty(1)=0` → voll
  gepuffert, Binärmodus (kein CRLF, weil `fstat` noch `S_IFCHR` meldet → LIBC-FSTAT);
  stderr auf Datei: kein `Fforce(2,-1)` mehr.
* `isatty(-1)` = 1; jedes `open()` kostet `Fcntl` + bis zu 3 `Fseek` mehr.

## Nicht gelaufen

Echter Pi 4; qemu-virt-Läufe; asid=on; alte crt0 und pre/prefs11-Kernel der stderr-Matrix;
EmuCON; restliche cm-Regressionsstichprobe (tdkeys, KBDVEC, S2ALL, PPARENT, PORT10).

## Offen / Hinweise

* `docs/api/gemdos/file/3f-fread.md` (Hinweis „mintlib tests“) und die FS11-Notiz, dass
  `getchar` Fread nicht erreicht, sind jetzt überholt — nicht angefasst.
* `docs/STATUS.md` nicht geändert (Auftrag).
