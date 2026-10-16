# MEM02 — STATUS: DONE (2026-10-17, Host + QEMU) — `Mxalloc` bedient alle vier Pool-Modi

**Auftrag:** `Mxalloc` beantwortet `MX_STRAM`/`MX_TTRAM`/`MX_PREFSTRAM`/`MX_PREFTTRAM` wie EmuTOS `xmxalloc`
(`references/emutos/bdos/umem.c:234-330`). Abnahme neu (Anwender 2026-10-17): Hosttests aller vier Modi, inklusive
`MX_TTRAM` ohne TT-Pool → 0, plus QEMU-Sonde `MXPROBE`. Die Ball-Demo ist kein Abnahmekriterium.

## Ursache und Behebung

Behoben durch ARCH03 Stufe 1 (`a9279fe9`, VA = PA). Der Befund vom 2026-09-28 war kein Pool-Fehler. Die Bloecke kamen
schon damals aus dem TT-Pool, zurueck kam aber die ADR-021-Fenster-VA ab 4 MiB. Die liegt numerisch unter `phystop`, und
deshalb las die Demo „ST“. Belege dafuer: `docs/analysis/mem02-mxalloc-pools-2026-10-17.md` und die Gegenmessung auf
`c806a645` in `measurements/measurement-2026-10-17-mem02/RUNINFO.TXT`. Kein Produktcode geaendert.

## Abnahme (`3a417b40`)

| | |
|---|---|
| `tos-api/tests/mem02_mxalloc_modes.rs` | 3/3: Groessenabfragen je Modus (unbekannter Modus → ST), jeder Modus aus seinem Pool (+ `F_PROT_G`), Verweigerung bei `MX_STRAM`/`MX_TTRAM`, Rueckfall `MX_PREFSTRAM` → TT |
| `tos-api/tests/mem02_mxalloc_no_alt.rs` | 1/1: ohne alternate RAM liefert `MX_TTRAM` 0 (Groesse und Block), `MX_PREFTTRAM` nimmt ST |
| `sdk/libc/tests/mxprobe.c` | QEMU raspi4b, memprot an: **MXPROBE: PASS** (29 ok, 0 FAIL), adressgleich mit dem Lauf vom selben Morgen |

Ueberschneidung mit `arch03_shared_space.rs` geprueft: dort je ein ST- und ein TT-Block, `Malloc`, `Mshrink`/`Mfree`.
Die neuen Dateien bringen die Vier-Modi-Matrix und den Fall ohne TT-Pool. Gebaut wird die Sonde mit
`sdk/libc/build-libc.sh sdk/libc/tests/mxprobe.c sdk/libc/build/MXPROBE.NCP`. `tools/make-sd-image.sh` kopiert sie,
wenn sie gebaut ist; mit `SD_MXPROBE_AUTO=1` kommt sie zusaetzlich nach `\AUTO\`.

Belege: `measurements/measurement-2026-10-17-mem02-tests/RUNINFO.TXT`. Basislinien: Hosttests **1378/0/1** (+4), SDK 20/0/0,
Bau 64/94 beide Boards (frisch), Clippy 76/125, Driftprobe 0.

## Mitkorrigiert

* `TASK/PORT06.md` §3: Berichtigung, die Voraussetzung (TT-RAM-Praeferenz) galt; die Lesart „ST-RAM verbraucht“ ist widerlegt.

## Offen

* Nichts fuer die Abnahme. Optional und nur auf echter HW moeglich: dieselbe Sonde aus `C:\AUTO\` auf der Pi 4
  (erwartet `ramtop` 0x2FE00000, alle TT-/PREFTT-Bloecke „alt“).
