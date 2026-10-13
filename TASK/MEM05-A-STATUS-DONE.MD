# MEM05 Teil A (Kernel) — Quittung

**Status: ERLEDIGT · Schritte 1–6 des Entwurfs + Doku aus Schritt 9 (ohne
`docs/STATUS.md`, auf Anweisung) · Host-Tests, statische Prüfungen, QEMU
raspi4b on/off/strict/verify/on-asid und qemu-virt grün · nichts committet** ·
2026-10-13 · Basis `b87ecb2a` · Beleg
[measurements/measurement-2026-10-13-mem05-kernel/](../measurements/measurement-2026-10-13-mem05-kernel/RUNINFO.TXT)
(Abschnitt „CONTINUATION“)

Entwurf: `docs/analysis/mem05-design-2026-10-13.md` (E1–E8) + Nachentscheidung
**B** (`TASK/MEM05.md`): alle MEM05-Seiten zählen gegen ein Budget von 512
Seiten. Ersetzt `MEM05-A-STATUS-UNFINISHED.MD` (Halt vor Schritt 4).

## Umgesetzt

| Schritt | Inhalt | Dateien |
|---|---|---|
| 1–3 | (erste Sitzung) Zähler je Besitzer, Grenze 8192/`mdlimit=`, `xmfremd` O(1) | `owner_limit.rs`, `pool.rs`, `traps.rs`, `region.rs`, `mdpool.rs`, `kernel/src/bootargs.rs`, `lib.rs` |
| 4 | `MdPageSource` (Kernel-Heap), **ein** Seitenbudget 512 für MD/`RECS`/`SNAP`/`KEPT`; MD-Vorrat in Seiten zu 117 MDs, erste Seite statisch, nie verschoben | `tos-api/src/bdos/mem/pages.rs` (neu), `mdpool.rs`, `init.rs` |
| 5 | `RECS` dicht/seitenweise, `SNAP` als Budget-Lauf (wächst, Abschneiden gezählt statt still), `KEPT` dicht (`forget` 0 Schritte ohne `F_KEEP`), `VSCRATCH` fest 512; Reserve in `mxalloc` vor `ffit` | `memprot.rs`, `region.rs`, `traps.rs` |
| 6 | Heap 2 → 8 MiB; Bootzeile; MEM05-Statistikzeile beim Boot und nach jedem Shell-Programm | `heap.rs`, `layout.rs`, `kernel/src/lib.rs`, `shell_loop.rs` |
| 9 | `COMPATIBILITY.md` (Abweichung, Text nach B), `docs/api/structures/md.md`, `docs/sdk/05-abi-pitfalls.md:98`, `MEMORY.md` ADR-025 | — |

Tests (erst rot, dann grün): `tos-api/tests/mem05_growth.rs` (4),
`mem05_memprot_tables.rs` (3), dazu aus Sitzung 1 `mem05_owner_limit.rs` (7),
`mem05_mdpool.rs` (2), `bootargs`-Test (1).

## Zahlen

- Host 1340/0/1 (Basis 1333/0/1), Warnungen 64/94 beide Boards, Clippy 76/125,
  Drift 31, SDK 19/0/0.
- MDPROBE (aktuelle libc, 32-KiB-Hunks): `malloc(32)` × 3 000 000 ohne NULL
  (alt: NULL nach 147 605) → 5138 Hunks; 8189 erlaubt am Start, 3051 am Ende.
  Spitze 250/512 Seiten mit Schutz (1 MB Heap), 70/512 ohne.
- MDPAR (`mdlimit=64`): Eltern an der Grenze, Pexec-4-Kind holt seine volle
  Zuteilung; Pexec-0-Kind Grenze − TPA − Env. PASS in on/off/strict, unter
  `memprot=verify` mit `mdlimit=400` (0 Abweichungen).
- MDBUD (`mdlimit=0`): Budget endet bei 16 892 Blöcken (Schutz an) bzw.
  60 015 (aus); Heap danach 6,3 MB frei, Desktop zeichnet normal.
- APITEST 106/0/5 in allen Modi + on-asid, Zeilen identisch zur Abnahme
  `libc-tty-acceptance`; Stichprobe (Desktop, Accessory, TeraDesk, QED,
  TeraDesk→QED, EmuCON, S2ALLM, E9MEM) Screendumps byte-gleich, nur Doom
  (Demo-Timing) und der gewollte S2ALLS-Absturz in strict abweichend wie dort.
  qemu-virt on/off/strict/on-asid identisch zur Abnahme.

## Bekannte Grenzen / Abweichungen vom Entwurf

- **Kosten je Sync** mit Schutz O(N·64 + N log N): 8189 `Mxalloc`+`Mfree` bei
  bis zu 8192 Blöcken 23 s (QEMU) gegen 0,06 s ohne Schutz → Folgeticket
  [MEM10](MEM10.md). Dokumentiert in ADR-025 und `COMPATIBILITY.md`.
- `MdPageSource` hat `alloc_pages(n)`/`free_pages(p, n)` statt
  `alloc_page()`/`free_page()`: `SNAP` muss ein zusammenhängender Lauf sein
  (Sortieren/Binärsuche). Seiten 16-Byte- statt 4096-ausgerichtet (der Heap
  bedient 4096 nur per Überallokation; Schrumpfen ist nicht vorgesehen, E8).
- Zusätzliche Logzeile `[mm] MEM05 after program: …` nach jedem Programm,
  das die Shell startet (für die Messung der MD-/Heap-Zahlen; Entwurf §6
  „Statistik/Log“ nannte nur die Bootausgabe).
- Seiten werden nie zurückgegeben (E8): nach einer Spitze bleibt der Heap
  belegt (MDBUD: 2,1 MB), begrenzt durch das Budget.

## Offen (nicht Teil A)

- `docs/STATUS.md`-Eintrag (auf Anweisung nicht geschrieben).
- Teil B (SDK `MAXHUNK` 16 MiB) und QEMU-Test 15 (Accessory-Pfad).
- Nicht gelaufen: echter Pi 4; ACCFREE/TSRRES/PFLAGS/CRASHTST auf raspi4b
  (CRASHTST auf virt gelaufen); MDPROBE/MDBUD unter `memprot=verify`.
