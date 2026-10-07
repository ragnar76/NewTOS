# FS08-STDIO — Quittung (FERTIG, QEMU): größerer stdio-Puffer für Dateien

Datum 2026-10-07 · Basis `desk-rebuild` `f14942c` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-07-stdiobuf](../measurements/measurement-2026-10-07-stdiobuf/RUNINFO.TXT)
· Ticket: [FS08](FS08.md), Teil 4. Vorher: [FS08-EMMC](FS08-EMMC-STATUS-DONE.MD), [FS08-FAT](FS08-FAT-STATUS-DONE.MD), [FS08-CPUCLK](FS08-CPUCLK-STATUS-DONE.MD).

## Auftrag

Entscheidung des Anwenders 2026-10-07: „größerer Buffer ist kein Problem, da
wir mehr Speicher haben als ein realer Atari“. Auf der echten Pi las stdio mit
1-KiB-`fread` nur 1,35 MB/s gegen 11,7 MB/s bei einem großen `Fread`
(`measurement-2026-10-07-fs08-pi-after`), weil `do_fstat.c` `st_blksize =
BUFSIZ` (1024) meldete und mintlibs stdio daraus die Puffergröße nimmt.

## Was geändert ist (nur Toolchain; Kernel und tos-api unverändert)

* `sdk/libc/newtos-arch/do_fstat.c`: `st_blksize` = **32 KiB** für ein
  GEMDOS-Datei-Handle (≥ 6), dessen Größe `Fseek` lesen konnte; Konsole (0–2),
  Standard-Handles 3–5 und BIOS-Geräte behalten 1024. Benannte Abweichung im
  Kopfkommentar.
* `docs/sdk/07-mintlib-limits.md` §2: Tabellenzeile + neuer Abschnitt mit
  Begründung, Größenwahl und Kostentabelle.
* `sdk/examples/fsbench.c`: Kopf der L3-Seite zeigt das gemessene
  `st_blksize` (außerhalb jeder Zeitmessung) statt „default BUFSIZ“.
* Neu: `sdk/libc/tests/fstat-host.sh` (+ `.c`, `-fs.c`, `-gemdos.h`), in
  `sdk/run-host-tests.sh` verdrahtet: 25 Prüfungen über mintlibs eigenes
  `stdio_init.c`; gegen den alten Shim 6 rot.

## Warum 32 KiB und nicht 64

| Puffer | 10 MiB per `fread(1 KiB)`, Pi-Modell | DOOM 90 s: gelesen / I/O-Zeit Pi-Modell |
|---|---|---|
| 1 KiB (bisher) | 6,3–7,6 s (gemessen 7,57 s) | 3,0 MB / ~1,6 s |
| 16 KiB | 1,21–1,29 s | 15 MB / ~3,5 s |
| **32 KiB** | **1,04–1,08 s** | **28 MB / ~4,6 s** |
| 64 KiB | 0,96–0,98 s | 55 MB / ~6,9 s |

DOOM liest jeden Lump mit `fseek` + `fread`; nach jedem `fseek` füllt stdio
einen ganzen Puffer nach. 32 KiB ist die kleinste Größe, die beim
sequentiellen Lesen bis auf ~0,1 s pro 10 MiB an 64 KiB herankommt, bei gut
der halben DOOM-Mehrlast. Eine Konstante, falls der Anwender anders
entscheidet.

## Messwerte (QEMU raspi4b, memprot an)

| Messung | Vorher | Nachher |
|---|---|---|
| FSBENCH Lesekommandos gesamt / L3 allein | 195 769 / 45 899 | **151 492 / 1 621** |
| FSBENCH gelesene Blöcke | 548 939 | 548 939 |
| FSBENCH 100 Lesungen / D:\FSBENCH / fsck | ok / 10/10 / sauber | **ok / 10/10 / sauber** |
| zView BIG.JPG (2,2 MB), Lesekommandos des Laufs | 2 644 | **566**, Dumps bytegleich |
| zView Browser, 15 Beispielbilder | 754 | 752, Dumps gleich (nur Größenangaben in C:\ anders) |
| QED neu/sichern/ändern/sichern | PASS | **PASS**, Dumps bytegleich |
| TeraDesk Datei- und Ordnerkopie | PASS | **PASS** (nur „bytes used“ in C:\ anders) |
| LDGDEMO (EX_LIB + POPUP.NLD) | 10 Dumps | **bytegleich** |
| APITEST | — | **106/0/5** |
| S2ALL | — | FAIL nur ENVPAR — **mit HEAD-libc identisch** (Probe veraltet seit PORT09) |
| SDK-Hosttests | 17/0/0 | **18/0/0** |

In QEMU ändert sich die Zeit kaum (keine Kommandolatenz modelliert); auf der
Pi zählen die Kommandos.

## Speicher

32 KiB pro offenem Datei-Stream, beim ersten Zugriff, bei `fclose` frei;
fehlt der Speicher, halbiert mintlib bis 128 Byte (kein Fehler). Höchstens 16
offene Dateien systemweit → ≤ 512 KiB. QED ≤ 3 Konfigdateien gleichzeitig,
TeraDesk kopiert über eigenes `Fread`/`Fwrite` (unberührt), zView ein Stream
pro dekodiertem Bild, DOOM einer (WAD). LDG-Module (libshare): der Puffer
eines Codecs kommt per `ldg_Malloc` im Kontext von zView und wird bei
`fclose` zurückgegeben; die Konsolenströme beim Modulende bleiben 1 KiB
(LDG-M3-B2 unverändert). SYSACC.NAC linkt keine libc — unberührt.

## Pi-Paket

`measurements/measurement-2026-10-07-stdiobuf/bin/card/` (Kartenlayout,
gitignored), `bin/SHA256SUMS`, `bin/LIESMICH.TXT`: QED, TeraDesk, CoreMark
(2), DOOM, zView + 10 Codecs, `AUTO\FSBENCH.NC` (geparkt). Nicht kopiert.
Kernel unverändert.

## Offen

* Bestätigung auf der echten Pi (FSBENCH L3, DOOM-Ladezeit).
* DOOM: Abhilfe für den Zufallszugriff gehört in den Port (`setvbuf` auf dem
  WAD-Stream) — nicht Teil dieser Aufgabe.
* S2ALL-Probe ENVKID erwartet `ARGV=` im rohen `p_env`; seit PORT09 schneidet
  der Start das ab — vorbestehend, unberührt.
