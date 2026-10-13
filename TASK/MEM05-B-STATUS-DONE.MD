# MEM05 Teil B (SDK) — Quittung

**Status: ERLEDIGT · Schritt 7 des Entwurfs (Entscheidung E6) · Host-Tests,
statische Prüfungen, Relink aller libc-Programme, QEMU raspi4b
on/off/strict/on-asid grün · nichts committet** · 2026-10-13 · Basis
`c06d370c` · Beleg
[measurements/measurement-2026-10-13-mem05-sdk/](../measurements/measurement-2026-10-13-mem05-sdk/RUNINFO.TXT)

Entwurf: `docs/analysis/mem05-design-2026-10-13.md` §4; Teil A:
[MEM05-A-STATUS-DONE.MD](MEM05-A-STATUS-DONE.MD).

## Umgesetzt

| Inhalt | Dateien |
|---|---|
| Overlay = Original `malloc.c` + `__newtos_malloc_maxhunk(siz)` (setzt nur `MAXHUNK`) | `sdk/libc/overlay/mintlib/malloc.c` (neu) |
| Aufruf mit 16 MiB ganz am Anfang von `__libc_crtinit`, schwach referenziert, vor dem Accessory-Zweig | `sdk/libc/newtos-arch/heap_init.c` |
| Overlay-Liste / Kopfkommentar | `sdk/libc/build-libc.sh` |
| Undefined-Symbol-Prüfung der LDG-Module zählt die eine schwache Referenz nicht (`ls_undefined_count`) | `sdk/libshare/libshare-link.sh`, `sdk/zview/build-zview.sh`, `sdk/ldg/build-ldg-{demo,client}.sh` |
| Host-Test Chunkfolge (30 Prüfungen; rot mit leerem Setter: 10 FAIL) | `sdk/libc/tests/malloc-host.{sh,c}`, `malloc-host-gemdos.h`, `sdk/run-host-tests.sh` |
| Doku | `docs/sdk/07-mintlib-limits.md` §5 + §10, `docs/COMPATIBILITY.md` (MEM05-Abweichung Punkt 4), `docs/MEMORY.md` ADR-025 Status |

## Zahlen

- Host 1340/0/1, Warnungen 64/94 beide Boards, Clippy 76/125, Drift 31 —
  unverändert; SDK 20/0/0 (+1 `malloc-host`).
- Kernel unverändert (nur Git-Stempel verschieden); QEMU mit Byte-Kopien der
  Teil-A-Kernel.
- MDSTEP `malloc(32)` × N → Hunks (= GEMDOS-Blöcke) in allen vier Modi:
  100→1, 1 000→3, 10 000→7, 147 605→10, 1 Mio→14, 3 Mio→21, 10 Mio→44 —
  Modell §4.4 an jedem Punkt getroffen (Teil A bei 3 Mio: 5138).
- Laufzeit 3 Mio: 0,9 s (on) / 4,4 s (off) / 1,1 s (strict) statt ~2000 s;
  10 Mio: 37–46 s.
- Accessory (Test 15): MDACC PASS in on/off/strict — 10 Hunks 8 KiB … 4 MiB.
- Hello-World: ein 8-KiB-Hunk, alte und neue libc identisch.
- APITEST 106/0/5 in allen Modi + on-asid, Zeilen identisch zu Teil A;
  Stichprobe byte-gleich zu Teil A außer Doom (Timing) und dem gewollten
  S2ALLS-Absturz in strict.

## Abweichung vom Entwurf

- Die schwache Referenz bleibt in jedem LDG-Modul undefiniert (Module laufen
  auf libshares Allokator und dürfen `malloc.o` nicht ziehen). Die
  Undefined-Symbol-Schranke von `build-zview.sh` schlug darauf an; sie zählt
  jetzt alles außer genau diesem schwachen Symbol. Belegt: kein dynamischer
  Relocation-Eintrag, Aufruf wird übersprungen.

## Offen / nicht gelaufen

- `docs/STATUS.md` (auf Anweisung nicht geschrieben); Commit.
- Echter Pi 4; qemu-virt; `memprot=verify`; zView/LDG/present & Co. nur
  gelinkt, nicht gestartet.
