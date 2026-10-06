# PORT09 — Quittung: `environ` aus `p_env` und die `ARGV=`-Konvention in der libc

Datum 2026-10-06 · Basis `desk-rebuild`, HEAD `50d4131`, Baum vorher sauber
· nicht committet · Belege [measurements/measurement-2026-10-06-port09/](../measurements/measurement-2026-10-06-port09/RUNINFO.TXT)
· Vorgeschichte: [PORT09-STATUS-UNFINISHED.MD](PORT09-STATUS-UNFINISHED.MD) (Schritt 1, Halt an MEM03)

Anwenderentscheidungen: 2026-10-02 `cmdargs.c` lernt die ARGV-Konvention;
2026-10-06 `environ` libc-weit aus `p_env` (mintlib-Paritaet, gebraucht von
LDG, `docs/analysis/ldg-after-arch03-2026-10-06.md` B1).

## Vorbedingung

Ein Kind liest seinen `p_env` jetzt (ARCH03 Stufe 1, VA = PA): die
unveraenderte Probe von 2026-10-02 laeuft auf dem unveraenderten Kern durch,
alle 17 Eintraege, kein Abort (`serial/pre-on-rpi4-envkid-serial.log`).
Vorher-Stand `ARGVTEST.NAP`: argc 12, Schwanz bei `kilo-el` abgeschnitten.

## Umsetzung

`sdk/libc/newtos-arch/cmdargs.c` ist jetzt ganz mintlibs `parseargs()`
(`crtinit.c:280-520`), Schritt fuer Schritt, mit AArch64-Zeigerbreiten:

* `environ` aus dem Block in `p_env`, Eintraege zeigen in den Block;
* `ARGV=` beendet die Umgebung und wird **an Ort und Stelle** gekappt;
  `argv[0]` = der Pfad dahinter, dann die Argumente — ohne Laengengrenze,
  Leerzeichen im Argument bleiben; Laengenbyte 127 **nicht** verlangt
  (mintlib: `STRICTLY_COMPATIBLE_WITH_STANDARD` aus, `crtinit.c:61-70`);
* `ARGV=NULL:i,j,...` setzt diese Eintraege auf `""` (`ARGV-blank.txt`);
* die Desktop-Form `"PATH=\0C:\\\0"` wird an Ort und Stelle zu `"PATH=C:\\"`
  (`crtinit.c:332-345`);
* ohne `ARGV=` der bisherige Schwanz-Parser (`argv[0]` = `""`);
* `program_invocation_name`/`_short_name` aus einem nichtleeren `argv[0]`;
* Ablage wie mintlib: `environ[]`, zerlegter Schwanz, `argv[]` im TPA direkt
  ueber dem BSS; `heap_init.c` zaehlt die Bytes zum behaltenen `m`
  (`crtinit.c:167-172`). Zusaetzlich wird der schlimmste Fall vor dem
  Schreiben gegen `p_hitpa` geprueft (sonst mintlibs „insufficient memory“).

Portierte Quellen unberuehrt. Weitere Dateien: `heap_init.c` (Platz),
`crt0.S`/`build-libc.sh` (Kommentare), Hosttest
`sdk/libc/tests/cmdargs-host.{c,sh}`, Doku `docs/sdk/07-mintlib-limits.md`
(Kopf, §1, §5), `01-overview.md`, `03-first-programs.md`, `05-abi-pitfalls.md`.

## Abnahme

„Ein Programm mit mehr Argumenten, als in 125 Zeichen passen, erhaelt sie
vollstaendig“ — **erfuellt** in QEMU raspi4b, `memprot` on **und** strict:
`ARGVTEST.NAP` argc **15**, `argv[0]` `C:\ARGVTEST.NAP`,
`"mike thirteen has blanks"` als **ein** Argument, `argv[14]` vollstaendig.
Fallreihe `P9PAR`/`P9KID` (ARGV 14 Argumente, `ARGV=NULL:`, ohne ARGV, leerer
Block, `env == NULL` erbt den verbundenen Block): 35 ok / 0 FAIL in beiden
Modi.

## Pruefzahlen

| Pruefung | vorher | nachher |
|---|---|---|
| Rust-Hosttests | 1165/0/1 | 1165/0/1 |
| Release-Warnungen tos-api/kernel, beide Boards | 64/94 | 64/94 |
| clippy rpi4/virt | 76/125 | 76/125 |
| Drift | 31 | 31 |
| SDK-Hosttests | 14/0/0 | 14/0/0 (cmdargs-host: 21 Faelle, vorher 11) |

Regression (memprot on): S2ALL, VFORKT allein, PPARENT AUTO/ACC/Z/Z-KID,
ENVRN PASS; APITEST 106/0/5; PORT10 44/44; QED und TeraDesk per `#Z`;
TeraDesk F1 → QED. Strict einmal: S2ALLS PASS. Bildschirmabzuege 10 von 10
bytegleich zu `measurement-2026-10-06-arch03-4`.

## Was sich fuer Programme sichtbar aendert (gewollt, mintlib-konform)

* Ein Programm, das nach dem Start seinen **rohen** `p_env`-Block liest, sieht
  mintlibs Umschrift (gekappt, `PATH=` verbunden), nicht die Kernbytes. Zwei
  Messproben taten genau das (MEM03-`envkid.c`, PPARENT-`pparent.c`); ihre
  Kopien im neuen Laufordner pruefen jetzt dieselbe Zustellung ueber `argv`/
  `environ`. Unveraendert gebaut wuerden sie an diesen Stellen **planmaessig**
  FAIL melden.
* Von `spawn*`/`system()` gestartete Kinder bekommen `argv[0]` = Pfad statt
  `""` (mintlibs `_spawnve` sendet immer `ARGV=`); argc unveraendert.
* `getenv("STACKSIZE")` im Start wirkt jetzt (mintlib-Verhalten).

## Offen

1. **TeraDesk mit Argumenten** nicht skriptbar gefahren: F1 startet ohne
   Kommandozeile, TeraDesk schaltet dann ARGV selbst ab (`applik.c:1470`) —
   belegt `serial/port09-on-rpi4-tdargv`. Mit Argumenten braucht es Ziehen/
   Oeffnen (Maus) — von Hand oder auf der Pi. Die Form, die TeraDesk dann
   sendet, faehrt `P9APAR`/`P9PAR` nach.
2. `docs/STATUS.md` (Zeile „PORT09 wartet auf MEM03“, Phasenzeile, Recent
   Decisions) und der Kopf von `TASK/PORT09.md` („BLOCKIERT“) sind nicht
   nachgezogen — beim Lead.
3. Doom wurde neu gelinkt, aber nicht gefahren (nicht in der Laufliste).
