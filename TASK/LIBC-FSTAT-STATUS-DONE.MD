# LIBC-FSTAT — `fstat` erkennt umgelenkte Standard-Handles — Quittung

**Status: ERLEDIGT (Developer, Entscheidung 2026-10-17) · QEMU raspi4b in memprot on/off/strict belegt ·
Tester-Abnahme offen · nichts committet · Pi offen** · 2026-10-17 · Zweig `main`, Basis HEAD
`70a2e69b` · Beleg
[measurements/measurement-2026-10-17-libc-fstat/](../measurements/measurement-2026-10-17-libc-fstat/RUNINFO.TXT)

Ticket: [LIBC-FSTAT](LIBC-FSTAT.md), Analyse `docs/analysis/libc-fstat-2026-10-17.md` (§3 Änderung, §5 Tests).

## Änderung

* `sdk/libc/newtos-arch/do_fstat.c` `__do_fstat`: Zeichengerät genau dann, wenn
  `fd >= 0 && fd < 6 && __isatty(fd)` (gleicher `__open_stat[]`-Cache wie `isatty()`,
  `stdio_init.c`, `main.c`); 32 KiB `st_blksize` für jedes `fd >= 0`, dessen `Fseek(END)`
  gelingt und das kein Terminal ist. Negative Handles, ungültige Handles und Handles ≥ 6
  unverändert. Kopfkommentar um den Abschnitt „STANDARD HANDLES“ ergänzt.
* Hosttest `sdk/libc/tests/fstat-host{.c,-fs.c,.sh}`: `fh_redirect(h, size)`,
  `fh_reset_cache()`; Std-Handles 0–5 im `Fseek`-Modell standardmäßig Gerät. 44 → 92 Checks.
* `docs/sdk/07-mintlib-limits.md`: `do_fstat`-Zeile §2, 32-KiB-Absatz §2, Start-Zeile §5
  (Textmodus statt „bleibt binär“), Vererbungsabsatz §8 (seit PROC02 `c2cd746b` erledigt).

## Ergebnisse

| | Soll | Ist |
|---|---|---|
| Hosttest fstat-host rot → grün | rot | 32/92 rot → 0/92; alter Shim erneut 32 rot |
| Hosttests | 1376/0/1 | 1376/0/1 |
| Warnungen tos-api/kernel, beide Boards | 64/94 | 64/94 |
| Clippy | 76/125 | 76/125 (rpi4, qemu-virt, selftest-desk) |
| Drift / unsafe | 4 / 20 | 4 / 20 |
| SDK-Hosttests | 20/0/0 | 20/0/0 (fstat-host jetzt 92 Checks) |
| Programme / libc-Relink | 0 Warnungen | 0 Warnungen, alle libc-Programme neu gelinkt |

| QEMU raspi4b (EL0) | ON | OFF | STRICT |
|---|---|---|---|
| FSTATP neu (alte libc: FAIL 4) | PASS | PASS | PASS |
| OUT.TXT Text / `UNIXMODE=b` | `61 0D 0A 62 0D 0A` / `61 0A 62 0A` | gleich | gleich |
| stdin aus Datei `x\r\ny\r\n` | `"x\n"`, `"y\n"`, EOF | gleich | gleich |
| Konsole Kind | CHR, 1024, isatty 1 | gleich | gleich |
| STDOUTDB, STDERRP neu/alt crt0, STDERRA | A = B | A = B | A = B |
| TTYA | T1–T3 PASS | PASS | PASS |
| APITEST | 109/0/5 | 109/0/5 | 109/0/5 |
| Views + Serie vs PROC06 | 15/15 identisch | 15/15 | 15/15 |
| Doom | VERIFIED | VERIFIED | VERIFIED |

Erster Serienlauf: APITEST-Karte ohne `C:\PTERMCH.NCP` (T33 SKIP, 106/0/8) und OFF-Hänger unter
Host-Last; Karte korrigiert, die drei APITEST-Läufe wiederholt (RUNINFO 5).

## Beobachtbare Verhaltensänderungen

* Vom Elternprozess per `Fforce` auf eine Datei gelenktes stdout/stdin/stderr eines libc-Kindes:
  `fstat` → `S_IFREG` mit echter Größe, Puffer 32 KiB (vor der ersten Ausgabe), **Textmodus**:
  `\n` → `\r\n` beim Schreiben, `\r\n` → `\n` beim Lesen; Binär nur mit `UNIXMODE=b`.
  TTYA-Kinddatei endet jetzt `0D 0A` statt `0A`.
* `fstat(3..5)` am Standardgerät: `S_IFCHR`, 1024 (vorher `S_IFREG`, Größe 0); umgelenkt
  `S_IFREG`, 32 KiB.
* `isatty(fd)` und `S_ISCHR(fstat(fd))` stimmen für 0–5 überein.
* Konsole, Desktop, TeraDesk, QED, Doom, APITEST: unverändert.

## Nicht gelaufen / offen

Echter Pi 4; qemu-virt-Läufe (nur gebaut); desk=kernel; asid; `system("prog >datei")` als eigener Fall.

## Out-of-scope-Beobachtungen

* Der erste `printf` auf stdout (noch ohne Puffer) setzt `__bufsize` auf 0, der folgende Puffer
  ist dann BUFSIZ (1 KiB) — Konsole wie Datei, Upstream-stdio-Pfad; ob das auch `fopen`-Dateien
  trifft (FS08), ist nicht geprüft.
* `Fclose(fh)` des Elternteils nach `Fforce(n,-1)` liefert −65 (EINTRN, zweites Schließen eines
  geteilten OFD, `3e-fclose.md`); Dateiinhalt vollständig.
* NewTOS `Cconws` schreibt immer auf CON (`console.rs`), auch bei umgelenktem Handle 1 (nicht
  gegen EmuTOS geprüft; die Sonde nutzt es nur als Kontrollkanal).
