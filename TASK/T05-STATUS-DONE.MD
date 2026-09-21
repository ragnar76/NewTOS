# T05 — STATUS: DONE (Hosttests + QEMU-Selbstbeleg) / Pi-Lauf nicht erforderlich

Die sechs Basepage-Segmentfelder `p_tbase/p_tlen/p_dbase/p_dlen/p_bbase/p_blen`
sind gefuellt; mintlibs `_base` ist gesetzt. Bearbeitet 2026-09-21, Branch
`desk-rebuild`, Basis HEAD `75b2cc3`, **nicht committet, nichts geflasht**.
Kein Kernel-Produktcode geaendert (nur ein `#[cfg(test)]`-Modul im Lader).

## Entscheid, wie umgesetzt

Lead-Entscheid **zwei `PT_LOAD`s in `sdk/newtos.ld`**: `text` (`FLAGS(5)`,
R-X, `.text` + `.rodata`) und `data` (`FLAGS(6)`, RW-, `.init_array`/
`.fini_array` + `.data`, `.bss` als `NOLOAD`-/memsz-Schwanz). Der Lader
(`populate_basepage`) ist unveraendert und fuellt aus den Flags alle sechs
Felder. Mapping geprueft: `populate_and_map` mappt jede TPA-Seite
`UserRwExec` — **das Fenster bleibt, nur die Felder kommen aus den
Segmenten.** Ruecken-an-Ruecken-Layout ohne Seiten-Padding; die 16-Byte-ALIGNs
liegen IM letzten Abschnitt jedes Segments, damit `p_dbase = p_tbase + p_tlen`,
`p_bbase = p_dbase + p_dlen`, `p_bbase + p_blen = _end` byte-genau gelten
(EmuTOS `bdos/kpgmld.c:131-132`). `.init_array` (mintlibs `init_clock`) wird
explizit vor `.data` platziert — als lld-Waise lag es hinter `.bss` und machte
das BSS zu Datei-Nullen (memsz == filesz).

## Was getan wurde

| Bereich | Datei | Inhalt |
|---|---|---|
| Linkerskript | `sdk/newtos.ld` | zwei PHDRS, Segment-Layout im Kopf, `.init_array`/`.fini_array` vor `.data`, ALIGN(16) in `.rodata`/`.data`/`.bss` |
| `_base` | `sdk/libc/crt0.S` | `__libc_init_heap(basepage, p_hitpa)` (x1 = Basepage-Register, x9 = `p_hitpa`); Layoutkommentar |
| `_base` | `sdk/libc/newtos-arch/heap_init.c` | `_base = (BASEPAGE *) basepage` mit Beleg der mintlib-Leser (`startup/crt0.S:69`, `crtinit.c:167`, `unix/getpid.c:21`, `unix/getppid.c:22`, `gmon.c:201,237`); `<mint/basepage.h>` |
| Bau | `sdk/libc/build-libc.sh` | `heap_init.c` mit `${INCS[@]}` (voller mintlib-Include-Satz) uebersetzt |
| Hosttest (neu) | `tos-api/src/bdos/process/loader.rs` `basepage_segment_tests` | zwei Segmente → alle sechs Felder + drei Relationen; ein RWX-Segment → wie bisher (Regression); Daten ohne BSS → `p_blen` 0 |
| Zieltest | `sdk/apps/apitest/apitest.c` | **T32**: sechs Felder, `_end`, `_base`; T32.1 `p_tbase < p_dbase`, T32.2 `p_dbase <= p_bbase`, T32.3 `p_bbase + p_blen == _end`, T32.4/5 die kpgmld-Relationen, T32.6 Laengen ≠ 0, T32.7 `_base == p_lowtpa` |
| Spec/Docs | `docs/api/structures/basepage.md` (NewTOS-Notizen), `docs/sdk/03-first-programs.md`, `05-abi-pitfalls.md`, `07-mintlib-limits.md`, `sdk/README.md`, `sdk/build.sh` | Segmentlayout, `_base`, Transkriptzeile |
| Status | `docs/STATUS.md` (Phase-15-Zeile + Recent Decisions 2026-09-21), `TASK/T05.md` Abschluss | |

## Belege (`measurement-2026-09-21-t05-t06/`)

* `host-tests/xtask-test-AFTER.log` — `cargo run -p xtask -- test`: **909 passed / 0 failed / 1 ignored** (906 + 3 Ladertests).
* `serial/apitest-serial.log` — QEMU raspi4b, `C:\AUTO\APITEST.NCP` (BATCH):
  `T32 p_tbase=0x00001000 p_tlen=0x000123A0 p_dbase=0x000133A0 p_dlen=0x00000070`,
  `p_bbase=0x00013410 p_blen=0x00001E40 _end=0x00015250 _base=0x00000000`,
  T32.1–T32.7 `[OK]`; Summary 106/0/5; 0 × EXC/PANIC.
* `screendumps/` — TeraDesk und QED per `#Z`, vorher (HEAD-Binaries aus
  `before-bin/`, SHA-256 dort) vs. nachher: **0 px** (A +5 s, B +10 s; SHA-256
  in `SHA256.txt`). `tera-*-t04inf-*`: mit den INF-Dateien des T04-Laufs,
  gegen `measurement-2026-09-21-t04/screendumps/tera-after-A-idle.png`
  98 px = Mauszeiger (Injektor-Parkposition vs. Bildmitte).
* `build/after-program-headers.txt`, `after-bin-sha256.txt` — Segmente/Hashes
  der neuen QED.NAP / DESKTOP.NAP / APITEST.NCP; `build-teradesk-AFTER.log`
  48/48, `build-qed-AFTER.log` 37/37 (Kette libc → gemlib → cflib → libm neu
  gebaut, `lib_ensure` hat `heap_init.c` erkannt), je 0 undef.
* `build/xtask-build-rpi4.log`, `xtask-build-qemu-virt.log` — tos-api 64 /
  kernel 94 (Baseline 64/94, Delta 0); `clippy-tos-api-lib.log` 80,
  `clippy-tos-api-lib-tests.log` 82 (Baseline 80/82, keine Treffer in `loader.rs`).
* `method/` — `prep-card.sh`, `run.sh` + `qmp_multi.py` (T04-Verfahren),
  `pixdiff.py`, INF-Dateien; `product-t05-t06.diff`.

## Offen

* Datenlose Ein-Segment-Programme (`tools/mk-hello-ncp.py`, `sdk/crt0.S` ohne
  `.data`/`.bss`) bekommen weiterhin `p_dbase = p_bbase = 0`; die TOS-Form
  (`p_dbase = p_bbase = p_tbase + p_tlen`, Laengen 0) waere ein 4-Zeilen-
  Fallback im Lader — nicht gemacht, Lead-Entscheid.
* Pi-Lauf: nicht erforderlich; beim naechsten Pi-Lauf mit APITEST in `\AUTO\`
  erscheint T32 auf dem Schirm/im `C:\APITEST.LOG`.
