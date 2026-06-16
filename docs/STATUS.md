# STATUS — NewTOS

> Single source of truth for project state. Updated with every milestone.
> Edit only via PR; do not edit in random places. Date everything.

## Current Phase

**Phase 00 — EmuTOS Source Analysis** (✅ abgeschlossen 2026-05-02)

**Phase 02 — GEMDOS Console I/O** (in Arbeit, gestartet 2026-05-03)
- D0 Phase-Vorbereitung: done — Specs aus Phase 00 verifiziert.
- D1 Exception-Vector-Table + SVC-Dispatch: done —
  `kernel/src/arch/aarch64/exception_vectors.s` (16-Slot VBAR-Tabelle,
  ARM ARM D1.10.2-konform), `exceptions.rs` (ESR_EL1-Decoder,
  `rust_sync_handler`), `arch::aarch64::exceptions::init()` schreibt
  VBAR_EL1 vor erstem SVC. ABI: AAPCS64 nativ, Trap-Klasse via
  SVC-Immediate (1=GEMDOS, 13=BIOS, 14=XBIOS), Funktion-ID in `x8`,
  Argumente `x0..x7`, Rückgabe sign-extended `i32` in `x0`.
- D2 GEMDOS-Funktionstabelle: done — `tos-api` crate angelegt
  (`gemdos::dispatch`, `MAX_FN = 0x57`, statische Tabelle aus 12
  Slots).
- D3 Console-Leaf-Funktionen (12/15): done —
  Cconout 0x02, Cauxin 0x03, Cauxout 0x04, Cprnout 0x05, Crawio 0x06,
  Cconws 0x09, Cconis 0x0B, Cconos 0x10, Cprnos 0x11, Cauxis 0x12,
  Cauxos 0x13. Cconin 0x01 / Crawcin 0x07 / Cnecin 0x08 / Cconrs 0x0A
  blockiert auf Phase 06 (Pterm(-32) für Ctrl-C).
- D4 Host-Abstraktion: done — `tos_api::Host`-Trait + Lock-freie
  Installation (`AtomicPtr`); `kernel::host_env::KernelHost`
  delegiert an PL011-Konsole.
- D5 PL011-RX + IRQ-Ringpuffer: done — GICv2-Treiber
  (`kernel/src/drivers/gic.rs`), PL011 RX-IRQ (INT_RX|INT_RT, IFLS=1/8),
  SPSC-Ringpuffer in `console.rs`, Drain-Loop in `pl011_rx_drain`.
  IRQ-Vektor (`__vec_irq` → `rust_irq_handler`) acked GICC.IAR,
  dispatcht PL011-INTID, EOI an GICC.EOIR. Verifiziert in QEMU:
  Tastendruck → IRQ → Ringpuffer → `Crawio(0xFF)` → Echo via
  `Cconout` → `[selftest] RX-IRQ OK`.
- D6 Integrationstests in QEMU: pending (manueller Smoke-Test passed).
- D7 Real-Hardware EC: pending HW.
- **Smoke-Test** (2026-05-03, QEMU 11.0.0, `aarch64-virt,gic-version=2`,
  cortex-a72): SVC #1 / x8=0x09 dispatcht durch Vector-Table → Rust
  Sync-Handler → `gemdos::dispatch` → `cconws` und gibt
  `[selftest] hello from GEMDOS Cconws` zurück. Negativtest mit
  unbekannter Funktion-ID liefert `EINVFN`. RX-Pfad via PL011 IRQ
  end-to-end bestätigt (siehe D5).

**Phase 03 — Memory Management** (abgeschlossen 2026-05-03)
- D1 Memory-Map (`kernel/src/bios/machine/layout.rs`): done — board-spezifische
  Konstanten für QEMU `virt` und BCM2711 mit Quellenangaben,
  Linker-Symbole `__kernel_image_start/_end` (4-KiB-aligned), logische
  Trennung in ST-RAM (≤14 MiB ab `__kernel_end`) und TT-RAM
  (Rest-DRAM). Boot-Banner gibt die Map aus.
- D2 Page-Table-Builder (`kernel/src/arch/aarch64/mmu.rs`): done —
  4-KiB Granule, 48-bit VA, statische L0/L1/L2-Tabellen in BSS, 4 GiB
  Coverage in 2-MiB-Blöcken. Klassifizierung pro Block: DRAM = Normal
  Cacheable + Inner Shareable (AttrIdx 0); MMIO = Device-nGnRnE +
  PXN/UXN (AttrIdx 1); sonst invalid.
- D3 MMU-Enable: done — MAIR_EL1 (0xFF, 0x00), TCR_EL1 (T0SZ=16, TG0=4K,
  WBWA, Inner-Shareable, EPD1=1, IPS=40-bit), TTBR0_EL1, dann
  `dsb ishst → tlbi vmalle1 → dsb ish → isb → SCTLR_EL1.{M,C,I} → isb`
  per inline-asm. Pfad A (Identity, low-half) gewählt; Pfad B
  (High-Half via TTBR1) als Phase 05 vor Phase 06 nachzuholen
  (siehe Recent-Decisions 2026-05-03 + "Blocked"-Liste).
- D4 Frame-Allocator (`kernel/src/mm/frames.rs`): done — getrennte
  Bitmap-Allokatoren pro Pool (`Pool::Stram` / `Pool::Ttram`), 4-KiB
  Frames, `alloc_frame` / `alloc_frames_contig` / `free_frame` /
  `stats`. Bitmaps in BSS, 32 KiB pro Pool (worst case 1 GiB).
  Bits jenseits der Pool-Grenze stehen permanent auf "used", damit
  ein Suchüberlauf keine fremden Adressen zurückgibt. Nebenläufigkeit
  über `kernel::sync::SpinMutex` (test-and-set, `wfe`/`sev`-aufgewacht).
  Memory-Map-Refinement (2026-05-03): ST-RAM- und TT-RAM-Slot sind
  **additiv** abgebildet — ST-RAM-Slot 16 MiB ab `DRAM_START` (Atari
  `$000000–$FFFFFF`), davon obere 2 MiB als I/O-Reserve aus dem Pool
  ausgeschlossen; TT-RAM-Slot zusätzliche 256 MiB ab
  `DRAM_START + 16 MiB`. QEMU `-m 272M` entsprechend gesetzt. Smoke-
  Test bei Boot bestätigt: ST-RAM 3426 Frames frei (~13.4 MiB nach
  Kernel), TT-RAM exakt 65536 Frames (256 MiB), erste TT-RAM-Allokation
  liegt sauber an der 16-MiB-Marke (`0x41000000`).
- D5 Kernel-Heap (`kernel/src/mm/heap.rs`): done —
  First-Fit-Linked-List-Allocator mit Coalescing, 2 MiB Arena (512
  contig 4-KiB-Frames aus TT-RAM). Per-Block 8-Byte-Header mit
  Gesamtgröße; freie Blöcke speichern den `next`-Zeiger im Wort
  hinter dem Header; Free-Liste ist adressensortiert, Coalesce mit
  beiden Nachbarn beim Free. ZST (`size==0`) gibt `align as *mut u8`
  zurück und wird beim `dealloc` als Sentinel ignoriert. `KernelAllocator`
  ist via `#[global_allocator]` in `lib.rs` registriert; `extern crate
  alloc;` ist damit im Kernel verfügbar. `heap_stats()` liefert
  total/used/largest_free/free_blocks. Smoke-Test bei Boot: `Box<u64>`,
  `Vec<u32>` mit 1024 Elementen, 1000-Zyklen Alloc/Free — nach allem
  zurück auf `used=0`, `blocks=1`, `largest_free=2097152` (vollständiges
  Coalescing).
- D6–D8 (Mxalloc/Malloc/Mfree, Stack-Guard, heap_stats-API): pending.
- **Smoke-Test** (2026-05-03, QEMU 11.0.0, `aarch64-virt,gic-version=2`,
  cortex-a72, MMU enabled): Boot-Banner, Memory-Map-Dump,
  `[mm] MMU enabled (identity, TTBR0)`, danach Phase-02-Selftests grün
  (Cconws, EINVFN, RX-IRQ). EC1 erfüllt: Cconws nach `enable_mmu()`
  weiter funktional.

- D1 Source layout map: done — `docs/emutos-map.md`
- D2 Trap dispatchers (3 specs): done —
  `docs/api/gemdos/_dispatcher.md`, `docs/api/bios/_dispatcher.md`,
  `docs/api/xbios/_dispatcher.md`
- D3 GEMDOS console group (15 functions, 0x01–0x0B + 0x10–0x13): done —
  `docs/api/gemdos/console/*.md`
- D4 BIOS character-device specs (Bconstat 0x01, Bconin 0x02,
  Bconout 0x03, Bcostat 0x08): done — `docs/api/bios/0?-*.md`
- D4 BIOS remaining specs (Getmpb 0x00, Rwabs 0x04, Setexc 0x05,
  Tickcal 0x06, Getbpb 0x07, Mediach 0x09, Drvmap 0x0A,
  Kbshift 0x0B): done — BIOS group **100%** complete.
- D7 BIOS state (chardev vector tables, boot_status flags): done —
  `docs/api/bios/_state.md`
- D6 Structures (PD, DTA, BPB, MD/MPB, OFD/DFD/FCB, DND/DMD, OSHEADER,
  IOREC): done — `docs/api/structures/*.md`
- D3 GEMDOS process group (Pterm0 0x00, Ptermres 0x31, Pexec 0x4B,
  Pterm 0x4C): done — `docs/api/gemdos/process/*.md`
- D3 GEMDOS memory group (Maddalt 0x14, Mxalloc 0x44, Malloc 0x48,
  Mfree 0x49, Mshrink 0x4A): done — `docs/api/gemdos/memory/*.md`
- D3 GEMDOS time group (Tgetdate 0x2A, Tsetdate 0x2B, Tgettime 0x2C,
  Tsettime 0x2D): done — `docs/api/gemdos/time/*.md`
- D3 GEMDOS drive/DTA/version (Dsetdrv 0x0E, Dgetdrv 0x19, Fsetdta 0x1A,
  Fgetdta 0x2F, Sversion 0x30): done —
  `docs/api/gemdos/{drive,dta,version}/*.md`
- D3 GEMDOS file group (Fcreate 0x3C, Fopen 0x3D, Fclose 0x3E, Fread 0x3F,
  Fwrite 0x40, Fdelete 0x41, Fseek 0x42, Fattrib 0x43, Fdup 0x45,
  Fforce 0x46, Fsfirst 0x4E, Fsnext 0x4F, Frename 0x56, Fdatime 0x57):
  done — `docs/api/gemdos/file/*.md` (14 specs).
- D3 GEMDOS directory group (Dfree 0x36, Dcreate 0x39, Ddelete 0x3A,
  Dsetpath 0x3B, Dgetpath 0x47): done —
  `docs/api/gemdos/directory/*.md` (5 specs).
- D3 GEMDOS Misc (Srealloc 0x15): done — `docs/api/gemdos/15-srealloc.md`.
- D7 GEMDOS state: done — `docs/api/gemdos/_state.md`.
- D7 XBIOS state: done — `docs/api/xbios/_state.md`.
- D10 Line-A overview: done — `docs/api/_line-a.md`
  (implementation deferred to PHASE-12).
- D13 Phase 00 completion report: done —
  `docs/api/_phase00-completion.md`. **Phase 00 closed.** D8 (AES) and
  D9 (VDI) deferred to PHASE-00b.
- D5 XBIOS — **100%** spec coverage of NewTOS dispatch table
  (`LAST_ENTRY=0x57`, 88 entries; 51 routed via `xbios_unimpl`,
  37 implemented entries fully specified):
  - **Batch 1** (display + system): Physbase 0x02, Logbase 0x03,
    Getrez 0x04, Setscreen 0x05, Setpalette 0x06, Setcolor 0x07,
    Random 0x11, Cursconf 0x15, Settime 0x16, Gettime 0x17,
    Bioskeys 0x18, Kbdvbase 0x22, Supexec 0x26.
  - **Batch 2** (IKBD/MIDI/serial + floppy): Midiws 0x0C, Iorec 0x0E,
    Rsconf 0x0F, Keytbl 0x10, Ikbdws 0x19, Kbrate 0x23, Vsync 0x25,
    plus `_floppy-stubs.md` (8 functions).
  - **Batch 3** (mouse, screen-dump, PSG, blitter, console-map,
    TT shifter): Initmous 0x00, Scrdmp 0x14, Bconmap 0x2C,
    Blitmode 0x40, plus `_psg-group.md` (4 functions) and
    `_tt-shifter.md` (8 functions).
  Files: `docs/api/xbios/_triage.md`, `_floppy-stubs.md`,
  `_psg-group.md`, `_tt-shifter.md`, `xbios/*.md`.
- D11 Discrepancy log: live — `docs/api/_discrepancies.md` (D-001..D-008)
- D12 Dependency graph: live — `docs/api/_dependency-graph.md`
- Working list: `docs/api/_working-list.md`
- Next: D3 GEMDOS Misc (Srealloc 0x15), D7 GEMDOS + XBIOS state docs,
  D10 Line-A overview, D13 completion report.
**Phase 01 — Boot + UART** (complete pending hardware EC2)
- D1 Workspace skeleton: done
- D2 Linker scripts + boot stub: done
- D3 HAL UART trait: done
- D4 PL011 drivers (bcm2711 + qemu_virt): done
- D5 Panic handler: done
- D6 `kernel_main` entry: done
- D7 `cargo xtask` build/run/test: done
- EC1 (boot in QEMU, serial output): **passed** (2026-05-02, QEMU 11.0.0,
  `aarch64-virt`, cortex-a72) — expected text appears immediately.
- EC2 (real RPi4): pending — manual hardware step.
- EC3 (panic via UART): **passed** (2026-05-02) — `[PANIC] ...` line
  printed with file:line:column before halt.
- EC4 (zero warnings, clippy clean): **passed**.

## Phase Status

> **Phase plan reorganised 2026-05-03** to mirror the EmuTOS module
> layout (`bios/` / `bdos/` / `vdi/` / `aes/` / `desk/`). The
> previous numbering is preserved in `phases/_archive/`. m68k user
> emulator phases removed from scope.

| Phase | Title                                  | Status        | Started    | Completed |
|-------|----------------------------------------|---------------|------------|-----------|
| 00    | EmuTOS Source Analysis & Extraction    | ✅ Completed   | 2026-05-02 | 2026-05-02 |
| 01    | Boot + Early Console                   | EC1/EC3/EC4 passed; EC2 pending HW | 2026-05-02 |       |
| 02    | GEMDOS Console (Trap 1, console group) | D1–D5 done (12/15 Konsolen-Funktionen, RX-IRQ verifiziert); D6/D7 offen | 2026-05-03 |           |
| 03    | BIOS HW Layer                          | ✅ Completed (D1–D9: Memory-Map, MMU, Frame-Allocator, Kernel-Heap, Stack-Guard, kprintf, DTB-RAM-Detect, Refactor `bios/machine/`) | 2026-05-03 | 2026-05-03 |
| 04    | BDOS Memory Layer (`bdos/mem.c`+`umem.c`+`iumem.c`) | ✅ Completed (D0–D13 + EC1–EC9 in QEMU-Selftest verifiziert: 10/10 grün, baseline-restored) | 2026-05-03 | 2026-05-03 |
| 05    | High-Half Migration & Process ABI      | ✅ Completed (D1–D6 + D8 + EC4 grün; Pseudo-Process EL0-Round-Trip in `selftest_ec4` validiert: TTBR0-Install, eret nach EL0, SVC `#0xEC4` Trap zurück, Handler-Redirect mit SP_EL0-Restore, Resume in EL1; Phase-04-/02-Selftests bleiben grün); D4 (TTBR0-Clear) als Architektur-Entscheidung auf Phase 06 verschoben — TTBR0 wird dort per Pexec mit Per-Process-L0 atomar installiert; D7 Supexec deferred bis ADR-Supexec | 2026-05-04 | 2026-05-07 |
| 06    | BDOS Process (`bdos/proc.c`)           | D1 + D2 + D3 + D8 + D9 done (BasePage-Struct + Process-Tabelle + ELF-Loader mit Parser/Arena/load_to_slot + Per-Process-Page-Tables + m_own-RUN-Wiring; D3-Selftest grün — embedded ELF parsed, segmente in Slot-Page kopiert, EL0 fetcht User-Code, SVC trap-back validiert); D4–D7 offen | 2026-05-07 |   |
| 07    | BDOS Misc (Time + RX completion)       | Not started                           |            |           |
| 08    | BIOS Block Devices (Trap 13)           | Not started                           |            |           |
| 09    | BDOS File System                       | Not started                           |            |           |
| 10    | BIOS Screen                            | Not started                           |            |           |
| 11    | XBIOS (Trap 14)                        | Not started — Specs in Phase 00 fertig|            |           |
| 12    | VDI                                    | Not started — PHASE-00b Spec offen    |            |           |
| 13    | AES                                    | Not started — PHASE-00b Spec offen    |            |           |
| 14    | Desktop                                | Not started                           |            |           |
| 15    | First C Source Port                    | Not started                           |            |           |

## Open ADRs

Folgende ADRs blockieren spätere Implementierungsphasen und müssen vor
Phasenbeginn entschieden sein (siehe `docs/api/_phase00-completion.md`):

| ID            | Topic                                   | Blockiert |
|---------------|-----------------------------------------|-----------|
| ADR-Supexec   | AArch64-Substitut für `Supexec`          | PHASE-06  |
| ADR-BDOS-Lock | BDOS-Serialisierungsmodell auf SMP       | PHASE-05  |
| ADR-Line-A-01 | C-ABI-Form des Line-A-Exports            | PHASE-12  |
| ADR-Line-A-02 | Mauszeiger-Eigentümerschaft              | PHASE-08  |

## Blocked

- **EC2 (PHASE-01)**: Real-hardware verification on RPi4 still pending.
  Requires SD card with RPi firmware + `kernel8.img`, USB-TTL adapter on
  GPIO 14/15. Manual step.
- **PHASE-02 (GEMDOS Console I/O)**: spec for the 15 console functions
  is complete (Phase 00 D3 console group). Implementation can start for
  the leaf functions (Cconout, Cauxout, Cprnout, Cauxin, Crawcin,
  Crawio, Cconis, Cconos, Cprnos, Cauxis, Cauxos, Cconws); Cconin /
  Cnecin / Cconrs require `Pterm(-32)` (Phase 06) for Ctrl-C handling.
  See `docs/api/_dependency-graph.md`.
- **Phase 05 — High-Half-Kernel-Migration** (folgt auf Phase 03/04,
  blockiert Phase 06): Kernel von Identity (TTBR0) nach High-Half
  (TTBR1, VA `0xFFFF_0000_xxxx_xxxx`) umziehen. Schritte:
  (a) Linker-Skripte: VA-Basis high, LOAD-Adressen physisch.
  (b) `boot.s` Trampolin: nach MMU-Enable per absoluter Sprung in High-Half.
  (c) TTBR1_EL1 setzen, TCR_EL1.EPD1 = 0, T1SZ = 16.
  (d) TTBR0_EL1 leeren (TCR_EL1.EPD0 = 1) — pro-Prozess Mappings ab Phase 06.
  Begründung dieser Trennung siehe Recent-Decisions-Eintrag 2026-05-03.
  Eigenes Phasendokument: `phases/PHASE-05-high-half-migration.md`.

- **Phase 03 D8 — Runtime-RAM-Detection** (folgt auf Phase 03 D5,
  vor Phase 04 abgeschlossen):
  `kernel/src/mm/layout.rs::board::DRAM_SIZE` ist aktuell hartcodiert
  (QEMU 272 MiB, RPi4 1 GiB). Wir bauen ein eigenes OS, kein
  Atari-Emulator — der TT-RAM-Slot soll bis ans physische DRAM-Ende
  reichen, egal wieviel RAM die Hardware tatsächlich hat. Aufgabe:
  (a) Auf QEMU `virt`: DTB-Pointer aus `x0` beim Boot übernehmen,
      `/memory@…/reg`-Property parsen, daraus `dram.end` ableiten.
  (b) Auf RPi4: VideoCore-Mailbox `GET_ARM_MEMORY` (tag 0x00010005)
      über die Property-Channel-Schnittstelle (BCM2711 §1.3) abfragen.
  (c) `discover()` so umbauen, dass `dram.end` zur Laufzeit gesetzt
      wird; ST-RAM-Slot bleibt fix bei 16 MiB ab `DRAM_START`,
      TT-RAM-Slot = `[DRAM_START + 16 MiB, dram.end)`.
  (d) Feature-Flag oder Build-Konstante als Fallback, falls die
      Detektion vor `panic` fehlschlägt.
  Bis dahin sind die Konstanten konservativ und QEMU `-m 272M` /
  RPi4 1 GiB sind die getesteten Konfigurationen.

## Recent Decisions

| Date       | Decision                                            |
|------------|-----------------------------------------------------|
| 2026-05-02 | Project structure adopted, all phase docs drafted   |
| 2026-05-02 | TOS API spec template (`docs/api/_template.md`) set |
| 2026-05-02 | Rust toolchain pinned to `nightly-2026-04-15`       |
| 2026-05-02 | Per-package `forced-target` for `kernel` (cargo unstable `per-package-target`) — keeps `xtask` host-buildable in the same workspace |
| 2026-05-02 | EmuTOS sources under `references/emutos/` are **read-only** primary source for Track A; not part of the Cargo workspace |
| 2026-05-02 | EmuTOS reference checkout pinned: upstream `emutos/emutos` master @ `1a7484ab` ("Update GitHub Actions to use Node 24"). Update procedure documented in `references/README.md`. |
| 2026-05-02 | `references/` directory established per Source-Analysis-Protocol; gitignored except `README.md`. Acquisition status tracked in that README. |
| 2026-05-02 | EmuTOS-Checkout von `./emutos/` nach `references/emutos/` verschoben; `newtos/`-Duplikatbaum entfernt. |
| 2026-05-03 | QEMU-Test-Harness: `xtask run` verwendet `-M virt,gic-version=2 -display none -serial stdio`. Begründung: unser GIC-Treiber ist GICv2 (QEMU-Default mit cortex-a72 ist GICv3); `-serial mon:stdio` puffert Pipe-stdin im Mux-Chardev und blockiert PL011 RX. |
| 2026-05-03 | **Phase-03 R3 superseded:** ST-RAM und TT-RAM werden als **getrennte logische Pools** geführt, nicht als ein uniformer Heap. Begründung: Source-portierte TOS-Programme rufen `Mxalloc(size, mode)` mit Mode 0–3 (ST-only / TT-only / ST-pref / TT-pref); ein uniformer Pool würde diese Semantik unterlaufen und Programme brechen, die `Mxalloc(-1, 0)` zur ST-RAM-Erkennung verwenden. ST-RAM-Pool: 14 MiB hart (Atari TT max) ab `__kernel_end`. TT-RAM-Pool: restliches nutzbares DRAM. Auf RPi4/QEMU physisch identisch, logisch getrennt. Die Atari-BIOS-Variablen bei `$420`/`$432`/`$436` werden NICHT auf physische AArch64-Adressen gemappt; sie leben in einem Kernel-Struct, Zugriff nur über BIOS-Calls (Phase 07). Direkt-Poke auf `$420` ist out-of-scope für Source-Porting; nur via m68k-Emulator (Phase 10) erreichbar. |
| 2026-05-03 | **Phase-03 D2/D3 — Pfad A (Identity-Mapping über TTBR0)** gewählt. Kernel-VA = PA, 4 KiB Granule, 48-bit VA, TTBR0_EL1 hält die Tabellen, TTBR1_EL1 deaktiviert (TCR_EL1.EPD1=1). MAIR_EL1: Index 0 = Normal Cacheable WBWA inner+outer / Inner Shareable; Index 1 = Device-nGnRnE. Page-Tables statisch in `.bss.page_tables`, ein L0 + ein L1 + vier L2 (4 GiB Coverage in 2-MiB-Blöcken) — reicht für QEMU virt (2 GiB used) und RPi4 (4 GiB). Begründung: Pfad B (High-Half-Kernel über TTBR1 + Trampolin-Sprung) hätte die Phase-03-Risikofläche etwa verdoppelt, ohne D4–D8 (Frame-Allocator, Heap, Mxalloc, Stack-Guard) inhaltlich zu bessern. **Folge-Aufgabe: Phase 05 (High-Half-Migration)** muss VOR Phase 06 (Prozesse) erledigt sein — User-TTBR0 setzt voraus, dass der Kernel bereits in TTBR1 sitzt. Tracking unter "Pending blockers". |
| 2026-05-02 | Phase 00 gestartet. D1 (`docs/emutos-map.md`), D2 (3 Dispatcher-Specs) und D3 Konsolengruppe (15 GEMDOS-Funktionen) extrahiert. Discrepancies D-001..D-004 erfasst. |
| 2026-05-02 | D5 XBIOS-Triage + 13 Kernspezifikationen extrahiert (Display-Grundpfeiler, Zeit-Paar zu D3, Kbdvbase, Supexec). D-008 (xbios_unimpl gibt Funktionsnummer statt EINVFN) dokumentiert. |
| 2026-05-02 | D5 XBIOS Batch 2: 7 IKBD/MIDI/Serial-Specs (Midiws 0x0C, Iorec 0x0E, Rsconf 0x0F, Keytbl 0x10, Ikbdws 0x19, Kbrate 0x23, Vsync 0x25) plus Gruppen-Spec für Floppy/DMA (0x08, 0x09, 0x0A, 0x12, 0x13, 0x29, 0x2A, 0x2B — alle EUNDEV außer Protobt). |
| 2026-05-02 | D5 XBIOS Batch 3: Initmous 0x00, Scrdmp 0x14, Bconmap 0x2C, Blitmode 0x40 plus Gruppen-Specs `_psg-group.md` (Giaccess/Offgibit/Ongibit/Dosound) und `_tt-shifter.md` (Eset/Eget-Familie 0x50..0x57). D5 XBIOS damit **100%** abgedeckt (37 implementierte Einträge + 51 unimpl-Filler im Triage-Doc). |
| 2026-05-02 | D3 GEMDOS Datei- und Verzeichnis-Gruppen extrahiert (19 Specs): `gemdos/file/` (Fcreate 0x3C, Fopen 0x3D, Fclose 0x3E, Fread 0x3F, Fwrite 0x40, Fdelete 0x41, Fseek 0x42, Fattrib 0x43, Fdup 0x45, Fforce 0x46, Fsfirst 0x4E, Fsnext 0x4F, Frename 0x56, Fdatime 0x57) und `gemdos/directory/` (Dfree 0x36, Dcreate 0x39, Ddelete 0x3A, Dsetpath 0x3B, Dgetpath 0x47). D3 GEMDOS damit bis auf Srealloc 0x15 vollständig. |
| 2026-05-02 | **Phase 00 abgeschlossen.** Letzter Kleinkram: Srealloc 0x15 (`gemdos/15-srealloc.md`), GEMDOS-State-Doc (`gemdos/_state.md`), XBIOS-State-Doc (`xbios/_state.md`), Line-A-Übersicht (`_line-a.md`, Implementierung später) und Abschlussbericht (`_phase00-completion.md`). D8 (AES) und D9 (VDI) auf PHASE-00b verschoben. PHASE-02 freigegeben. |
| 2026-05-02 | Ziel-Build-Konfiguration für TOS-API-Surface (TBD-ADR): `CONF_WITH_TT_SHIFTER` + `CONF_WITH_VT52`; Falcon (DSP/DMASOUND/VIDEL) und `CONF_ATARI_HARDWARE` werden nicht abgebildet. Endgültige Festlegung mit ADR vor Phase 02. |
| 2026-05-03 | PHASE-02 D1+D2+D3 implementiert: neue Crate `tos-api` (Workspace-Member), `Host`-Trait, GEMDOS-Dispatcher, AArch64-Exception-Vektoren, Self-Test in QEMU bestanden. ABI festgelegt: AAPCS64 nativ; Funktion-ID in `x8`; SVC-Immediate kodiert Trap-Klasse. |
| 2026-05-03 | **PHASE-04 D9–D11 wired:** `Malloc` (0x48), `Mfree` (0x49), `Mshrink` (0x4A) durch den GEMDOS-Dispatcher freigeschaltet. `bdos::mem::traps::mshrink` neu (Alignment per `PoolConfig::AARCH64.align_mask=15`, Drei-Wege-Vergleich grow/equal/zero/shrink, Zero-Length-Pfad delegiert an `freeit`). Vorherigen Build-Bruch behoben (`tos_types::error::ERR` existierte nicht — die Konstante heißt `ERROR`). `cargo build` + `cargo build --release` (kernel + tos-api, board-qemu-virt) sauber. **Offen:** D12 (`m_own` an aktiven Prozess binden — pending PHASE-06), D13 (`pool_stats(pool)`), Phase-04-Selftest (EC1–EC9 sind spezifiziert aber nicht in `kernel::selftest` instrumentiert). |
| 2026-05-03 | **PHASE-04 cleanup + D12 (Owner-Attribution):** D9 `Malloc`-Default von `MX_STRAM` auf `MX_PREFTTRAM` korrigiert (matcht Spec-NewTOS-Note "kernel sentinel PD has `PF_TTRAMMEM=1`" — vorher hätten Programme nur 14 MiB ST-RAM gesehen statt 270 MiB). `find_mpb` aufgeräumt (toter Fallback weg, Doc-Bug korrigiert: Funktion gibt nie null zurück; Routing-Korrektheit über Maddalt-Overlap-Reject dokumentiert). Toter Helper `total_alt_ram` gelöscht (gehört in D13 als `pool_stats`). **D12 strukturell fertig:** statischer `KERNEL_PD_SENTINEL: u8` als rodata-Cookie, `current_owner()` und `is_kernel_owned()` API; `mxalloc` stempelt jeden Erfolg jetzt mit dem Sentinel statt `null`. Invariante: `m_own == null` ⇔ frei (auf `mp_mfl`); `m_own == current_owner()` ⇔ kernel-owned (auf `mp_mal`). Phase 06 muss nur noch den Body von `current_owner()` durch `run.cast::<c_void>()` ersetzen — keine weiteren Call-Site-Änderungen nötig. Boot-/Driver-Allokationen bleiben permanent kernel-owned und überleben jeden zukünftigen `Pterm`. |
| 2026-05-03 | **PHASE-04 D13 (`pool_stats`-Diagnostik):** Neues `tos_api::bdos::mem::stats`-Modul mit `PoolKind`-Enum (`Stram`/`Ttram`), `PoolStats`-Struct (`free_bytes`, `used_bytes`, `largest_free`, `free_blocks`, `allocated_blocks`, plus `total_bytes()`-Helfer), `pool_stats(PoolKind)`-Convenience-Wrapper und `pool_stats_raw(*mut MPB)` für Tests. Walks `mp_mfl` und `mp_mal` jeweils einmal, O(n) in MD-Anzahl, Snapshot konsistent unter BDOS-Lock. Kernel druckt nach `bring_up_bdos_pools` automatisch eine Ein-Zeile-Übersicht pro Pool (analog zum bestehenden `print_heap_stats`). QEMU-Smoke-Test bestätigt: ST-RAM `total=10403840 free_mds=1 alloc_mds=0`, TT-RAM `total=266338304 free_mds=1 alloc_mds=0` — Byte-Counts matchen die `bring_up_bdos_pools`-Ausgabe exakt. **Phase 04 D0–D13 vollständig**, nur EC1–EC9 (Selftest-Instrumentierung mit 1000-Cycle-Churn, `Mxalloc(-1, …)`-Probe, Wild-Pointer-EIMBA, Mshrink-Pfade) sind noch offen. |
| 2026-05-03 | **PHASE-04 EC1–EC9 verifiziert — Phase 04 abgeschlossen.** Neue `selftest::run_phase04_self_test()` (~270 Zeilen in `kernel/src/selftest.rs`) fährt jeden der fünf Memory-Traps via `svc #1` durch den GEMDOS-Dispatcher; gegliedert in 9 unabhängige Test-Funktionen plus eine Baseline-Restored-Final-Assertion. **QEMU-Lauf (`virt`, cortex-a72, 272 MiB DRAM): 10/10 passed, 0 failed, baseline restored.** Coverage: EC1 (1000-Cycle-Alloc/Free-Churn → Pool exakt zurück auf Baseline inkl. `free_blocks=1`), EC2 (`Mxalloc(-1, MX_STRAM/TTRAM/PREFSTRAM)` matchen `pool_stats().largest_free` und das Maximum über beide Pools), EC3 (PREFSTRAM-Fallback nach Drain der ST-RAM), EC4 (Wild-Pointer + Double-Free → `EIMBA`), EC5a/b (Grow → `EGSBF`, Shrink → `E_OK`), EC6 (`Mshrink(0,blk,0)` ≡ `Mfree`, follow-up `Mfree` → `EIMBA`), EC7 (`Maddalt` ST-RAM-Overlap → −1), EC8 (alle 5 Traps via Dispatcher routen — implizit durch jeden anderen Test bewiesen), EC9 (Phase-02 RX-IRQ-Smoke nach Phase-04-Tests weiter grün). Keine neuen Clippy-Lints im Patch (die 13 vorhandenen Kernel-Lints aus Phase 03 sind weiterhin in der Blocked-Liste). |
| 2026-05-03 | **Kernel-Clippy aufgeräumt** (PHASE-03 EC6 wieder ehrlich): alle 13 Pre-existing-Lints in `kernel/src/{arch/aarch64/mmu.rs,bios/machine/{heap,layout}.rs,console.rs,drivers/gic.rs,lib.rs}` behoben. Substantive Refaktorierung in `mmu.rs::build()`: indizierte Schleifen `for i in 0..NUM_L2 { L2[i]… }` durch `iter_mut().enumerate()` und `zip()` ersetzt, statisch-mut-Zugriff über `&raw {const,mut}` → `&{,mut} *(…)` mit `#[allow(clippy::deref_addrof)]` (Lint ist False-Positive auf `&raw`-Syntax — die Form ist die idiomatic Alternative zu `&static_mut`, das in Edition 2024 hart fehlerhaft ist). MAIR/TCR-Konstanten ohne `<< 0`/`<< 14`-No-Ops umgeschrieben (Bit-Position bleibt im Kommentar). `is_multiple_of` (1.87+) statt `% n == 0` in heap-init- und layout-asserts. Inline-`const { UnsafeCell::new(0u8) }`-Block in `RxRing::new`. `# Safety`-Sektion auf `mmu::init` und `kernel_main` ergänzt. RPi4-spezifische `1 * 1024…`-No-Op in `layout.rs::board::DRAM_SIZE` entfernt. **Verifikation:** `cargo clippy --package kernel --no-default-features --features board-{qemu-virt,rpi4} --lib --bins -- -D warnings` clean auf beiden Boards; `cargo clippy --package tos-api --lib -- -D warnings` clean; QEMU-Smoke-Test nach Clean-Build bestätigt unveränderten Boot-Pfad und 10/10 Phase-04-Selftests grün. |
| 2026-05-04 | **PHASE-05 D1–D3 + D8 — High-Half-Migration läuft.** Plan-A zweistufig umgesetzt: Stage 1 in `arch::aarch64::boot._start` (asm-only Mini-MMU mit board-spezifischer 1-GiB-Block-Tabelle in `__BOOT_L0`/`__BOOT_L1`, MMU enable, absoluter Sprung in high-half VA von `kernel_main`), Stage 2 in `arch::aarch64::mmu::reinstall` (volle 2-MiB-Block-Tabellen mit `MemoryMap`-Klassifikation, atomarer TTBR1-Swap + TLBI). **D1** Linker-Skripte mit `VA_OFFSET = 0xFFFF_0000_0000_0000` und `AT()`-Klauseln pro Section; ELF entry `_start_phys` (LMA) statt `_start` (high-half VA); phys-Aliase `__bss_phys_start/_end_exclusive`, `__boot_l0_phys`, `__boot_l1_phys` als `<sym> - VA_OFFSET`. **D2** boot.rs komplett neu (~250 Zeilen): per-board `BOOT_L1_ENTRIES` (QEMU virt: Block 0 Device, 1–3 Normal/Device; RPi4: Blocks 0–2 Normal, 3 Device — deckt 1/2/4-GiB-SKUs). **D3** Stage 2 in mmu.rs refactored, kritischer Fix: `build()` setzte Tabellen-Pointer als VA statt Phys (im Phase-03-Identity-Mapping zufällig korrekt, post-Phase-05 fatal); `va_to_pa()`-Konvertierung bei `L0[0] → L1` und `L1[i] → L2[i]` ergänzt. **D4 deferred to Phase 06**: `TTBR0_EL1 = 0` lässt Walker auf QEMU virt phys 0 lesen (= unmapped flash → hang); `TCR_EL1.EPD0 = 1` mid-flight ohne TTBR0-Re-Write triggert ARM-ARM-K11.5-asymmetric-Race. Pragmatische Lösung: TTBR0 weiter auf Stage-1-Boot-Tables zeigen lassen, Kernel greift sowieso nicht mehr auf Low-Half zu, Phase 06 schreibt TTBR0 atomar mit `EPD0`-Clear bei erstem `Pexec`. **D8 verifiziert**: QEMU Boot durchläuft komplett, alle 10 Phase-04-Selftests grün, Phase-02 RX-IRQ + Cconws + EINVFN unverändert grün. **Offen**: D5 (EL0-Stack-Region — kleines Linker-Detail), D6 (SP_EL0 im Trap-Frame — 8 Byte ergänzen), EC4 (Pseudo-Process-Test — bevor Phase 06). D7 Supexec laut Vorgabe deferred. |
| 2026-05-07 | **PHASE-06 D3 — ELF-Loader + Drop-zu-EL0 mit echtem Programm.** Drei neue tos-api-Module: [`bdos::process::elf`](../tos-api/src/bdos/process/elf.rs) (~250 Zeilen, minimaler ELF64-Parser für ET_EXEC AArch64; `MAX_LOAD_SEGMENTS = 8`; präzise `ParseError`-Enum-Diagnostik für jedes Reject-Kriterium — BadMagic, NotElf64, NotLittleEndian, NotExecutable, NotAArch64, BadPhEntSize, TooManySegments, SegmentOutOfRange etc.; rein no_std no_alloc, byte-by-byte LE-Parsing); [`bdos::process::arena`](../tos-api/src/bdos/process/arena.rs) (~70 Zeilen, per-slot 4-KiB-aligned `__PROCESS_USER_PAGES`-BSS-Array, 64 KiB total; `page_bytes_mut(slot)` + `page_phys(slot)` für Loader-Konsumenten; Phase-09-Replacement mit aligned-BDOS-Allocator vermerkt); [`bdos::process::loader`](../tos-api/src/bdos/process/loader.rs) (~150 Zeilen, `load_to_slot(elf_bytes, slot) -> Result<LoadedImage, LoadError>` parst ELF, kopiert PT_LOAD-Segmente in Slot-User-Page, zero'd BSS-Tail, ruft init_slot_tables + map_page; aktuelles D3-Limit: ein 4-KiB-Page-Mapping mit `UserRwExec`, p_vaddr in `[USER_VA_BASE, USER_VA_BASE + 4 KiB)`; D4/Phase-09 erweitern auf multi-page mit text/data/bss-Trennung). Neuer Selftest [`kernel::selftest_d3`](../kernel/src/selftest_d3.rs) mit hand-crafted 264-Byte AArch64-ELF (ELF64 Ehdr + 1× PT_LOAD-Phdr + 8 Byte Code: `svc #0xEC4; b .` an VA 0x100 = e_entry); Drop-zu-EL0 wiederverwendet EC4-Sentinel-Mechanismus (Pterm ist D6); Slot 1 (D3) statt Slot 0 (EC4) damit beide Tests parallel laufen ohne Interferenz. **QEMU-Test 4/4 Asserts grün**: entry_va=0x100 (Parser stimmt), captured ELR = entry_va+4 (User-Code lief, SVC nach 4 Bytes), SP_EL0 = USER_STACK_TOP_EXCLUSIVE (D6-Wiring durchgängig), SPSR.M = 0 = EL0t (Trap kam echt aus EL0). Build sauber für tos-api/qemu-virt/rpi4 + Clippy clean. **D3-Selftest läuft auf der gleichen Code-Pfad-Maschinerie (Parser → Arena → Loader → Slot-Tables → install_ttbr0 → eret) die Pexec-D4/D5 verwenden werden** — keine Stubs, kein "platzhalter"-Layer. |
| 2026-05-07 | **PHASE-06 D8 — Per-Process-Page-Table-Maschinerie + EC4-Refactor.** Neues Modul [`tos-api/src/bdos/process/mmu.rs`](../tos-api/src/bdos/process/mmu.rs) (~270 Zeilen) liefert die Page-Table-Helpers die Pexec-D3 verwenden wird: pro Slot 4× 4-KiB-aligned `UserTable` (L0/L1/L2/L3 = 16 KiB) im static `__USER_TABLES`-Array (16 Slots × 16 KiB = 256 KiB BSS); `PageFlags`-Enum mit `UserRwExec`/`UserRoExec`/`UserRwNx`-Presets (alle haben PXN=1, EL1 fetcht nie aus User-Pages); `init_slot_tables(slot)` zeroes + verdrahtet die L0→L1→L2→L3-Kette idempotent; `map_page(slot, va, pa, flags)` setzt L3-Entry mit Validation (Slot-Range, 4-KiB-Alignment, VA in [0, 2 MiB)); `l0_pa_for(slot)` returnt die TTBR0-Phys; `install_ttbr0(l0_pa)` ist der minimale `msr+isb`-Pfad ohne TLBI (auf QEMU virt cortex-a72 wedge-bekannt). **EC4-Spike auf D8 refactored**: 4× lokale Page-Table-Statics + Descriptor-Bit-Konstanten + `ec4_l0_pa()`+`ec4_l1_pa()`+... Helper-Funktionen weg, durch `init_slot_tables(EC4_TEST_SLOT) + map_page(EC4_TEST_SLOT, USER_VA_BASE, user_code_pa, UserRwExec) + tos_api::bdos::process::mmu::install_ttbr0(l0_pa)` ersetzt. **EC4 läuft auf der gleichen Code-Pfad-Maschinerie die Pexec-D3 verwenden wird** — kein Stub, alles End-to-End validiert. Build sauber für tos-api/qemu-virt/rpi4 + Clippy clean (3× `#[allow(clippy::deref_addrof)]` für die `&raw {const,mut}`-Pattern-False-Positives wie in `mmu::build`); QEMU-Boot 4/4 Asserts grün. **Damit löst sich Phase-05 D4 (TTBR0-Install) auf** — der gleiche Mechanismus den Phase 06 D3 Pexec verwendet, ist heute schon mit EC4 produktiv. |
| 2026-05-07 | **PHASE-06 D9 — `m_own` an `RUN` gebunden.** [`tos-api::bdos::mem::traps::current_owner()`](../tos-api/src/bdos/mem/traps.rs) konsultiert jetzt `process::current()`: bei aktivem Prozess wird dessen `BasePage`-Kernel-VA als Owner-Cookie zurückgegeben, sonst Fallback auf `KERNEL_PD_SENTINEL`. Damit ist die Pre-Phase-04 D12 Sentinel-Allokationen (Boot, Driver-Init, Between-Process-Kernel-Work) als "kernel-owned" markiert und überleben jeden zukünftigen `Pterm` — Per-Process-Allokationen kriegen den BasePage-VA der `Pexec` aufgesetzt hat. `is_kernel_owned()` vergleicht direkt gegen die Sentinel-Adresse (unabhängig von `RUN`-State), damit MDs die VOR einem `Pexec` allokiert wurden auch nach `Pexec` als kernel-owned erkennbar sind. Build sauber, Phase-04 Selftests + EC4 weiter grün (RUN ist im Test null → Cookie = Sentinel → identisches Verhalten wie pre-D9). |
| 2026-05-07 | **PHASE-06 D2 — Process-Tabelle + run/nextrun.** Neues Modul [`tos-api/src/bdos/process.rs`](../tos-api/src/bdos/process.rs) (~250 Zeilen) liefert die kernel-seitige Prozess-Verwaltung: `MAX_PROCESSES = 16` statisches `PROCESS_TABLE` Array mit `Process { state: AtomicU8, basepage_kva: AtomicPtr<BasePage>, basepage_pa: AtomicU64, l0_pa: AtomicU64, parent_slot: AtomicU32 }`-Slots; `RUN`/`NEXTRUN` als `AtomicPtr<BasePage>`-Globals (lock-free Read aus dem SVC-Sync-Handler); `State`-Enum (Free/Loaded/Running/Terminated); `allocate_slot()` mit CAS gegen Doppel-Claim (Phase-08+ SMP-vorbereitet); `free_slot`, `slot_for(bp)` Lookup, `slot(idx)` Borrow. `l0_pa`-Feld ist reserviert aber leer — wird von Phase-06 D8 (Per-Process-Page-Table-Install) gefüllt. Slot-Allokation ohne BDOS-Lock möglich, Mutationen unter dem implicit-single-CPU-Lock (Phase-06 D6 ersetzt das durch expliziten SpinMutex). **Build sauber für tos-api/qemu-virt/rpi4 + Clippy clean; Phase-04/05-Selftests + EC4 weiter grün.** |
| 2026-05-07 | **PHASE-06 D1 — `BasePage`-Struct mit bit-genauem Atari-Layout.** Neues Modul [`tos-types/src/basepage.rs`](../tos-types/src/basepage.rs) (~250 Zeilen) liefert die 256-Byte-`BasePage` exakt nach Compendium / `<tos.h>` `BASEPAGE`-Konvention: alle Pointer-shaped Felder als `Long` (i32, 4 Byte) wie auf m68k, alle Field-Offsets matchen das `bdos/bdosdefs.h` Phase-00-Extract. **23 `core::mem::offset_of!`-compile-time-Asserts** auf jeden dokumentierten Offset (0x00 `p_lowtpa` … 0x80 `p_cmdlin`) plus `size_of == 256` + `align_of == 4` Asserts garantieren dass jede zukünftige Layout-Änderung den Build sofort bricht. Felder: 8 Memory-Layout-Felder (TPA/text/data/bss + Längen), 7 Process-State-Felder (DTA/parent/flags/env/uft/lddrv/curdrv), 3 reserved-Filler (1fill/2fill/3fill), 16-Byte-`p_curdir` (per-drive cwd), `p_dreg`+`p_areg` Pexec-Save-Slots (Phase-06-D2-relevant), 128-Byte-`p_cmdlin`. Konstanten `PF_FASTLOAD` (0x1), `PF_TTRAMLOAD` (0x2), `PF_TTRAMMEM` (0x4), `PF_STANDARD` exportiert. Re-Exports von `tos-types`-lib: `BasePage`, `NUMSTD=6`, `NUMCURDIR=16`, `PDCLSIZE=0x80`, alle `PF_*`. **Build sauber, Phase-04/05-Selftests + EC4 weiter grün.** Damit ist die ABI-Brücke gelegt für Source-Ports: ein Atari-Compendium-Programm das `((BASEPAGE *)bp)->p_cmdlin[1]` direkt liest, sieht auf NewTOS exakt das gleiche Byte. |
| 2026-05-07 | **Spec-Audit (Cross-Check Implementation vs. Phase-00-Spec) durchgeführt.** Alle 5 GEMDOS-Memory-Traps, 12 Console-Funktionen, Dispatcher und Error-Codes systematisch gegen `docs/api/`-Specs verglichen. **Zwei Implementation-Bugs gefunden + behoben**: (1) `Cconout` returnt jetzt `1` statt `0` (D-001/D-011-konform); (2) `Crawio`-Write-Pfad returnt jetzt `0` statt des geschriebenen Bytes (06-crawio.md "Write: always 0"-konform). **Drei neue Discrepancies** ins Log: D-009 (`Cconws`-return-count: bewusste Compendium-Treue statt EmuTOS-source-undefined), D-010 (`Maddalt`-Overlap-Predicate: strict half-open intersection statt EmuTOS-overly-broad), D-011 (Drift-Korrekturen). **Zwei Spec-Doku-Bugs gefixt**: `structures/md.md` "Validate ownership"-Schritt war falsch (Mfree macht keinen Ownership-Check, nur Pterm); `tos-types/src/memory.rs`-Doc behauptete "32B/24B"-AArch64-Layout, tatsächlich 24B/24B. Implementation jetzt durchgehend spec-konform unter "ported-app"-Lens (Compendium-erwartete Returns, App-sichtbare Strukturen bit-genau). |
| 2026-05-07 | **PHASE-05 EC4 grün — Pseudo-Process-EL0-Round-Trip läuft.** Neuer Selftest `kernel/src/selftest_ec4.rs` (~330 Zeilen): vier 4-KiB-aligned Static-Page-Tables (L0/L1/L2/L3) plus 4-KiB User-Code-Page in BSS; Tabellen-Builder verdrahtet `L0[0] → L1[0] → L2[0] → L3[0] → user_page` (die einzige gemappte VA ist `[0, 0x1000)`, alles andere im Low-Half faulted), User-Code wird mit zwei Instruktionen `svc #0xEC4` + `b .` initialisiert plus `dsb ish`+`ic iallu`+`isb` für I-Cache-Konsistenz. TTBR0-Install minimal: nur `msr ttbr0_el1` + `isb` (TLBI vmalle1 wedget QEMU cortex-a72 — Phase-06-Tracking-Item). EL0-Drop via `eret` mit SP-Juggling: Kernel-`SP_EL0` (= aktiver SP wegen SPSel=0) wird via `mov xN, sp` gesichert (MRS auf SP_EL0 ist UNPREDICTABLE wenn SP_EL0 aktiv ist — ARM ARM D17.2.130), dann `msr spsel, #1` umschalten auf SP_EL1, dann sicheres `msr SP_EL0, user_stack`, dann eret. Sync-Handler erkennt das Sentinel-Immediate, capturet Diagnostics (User-SP_EL0, ELR, SPSR.M, alle für Asserts), restored Kernel-SP_EL0 in `frame.sp_el0`, redirected `frame.elr` auf den `1f`-Resume-Label und setzt `frame.spsr.M = EL1t`. **Test-Ergebnis QEMU**: 4/4 Asserts grün — User-SP_EL0 = `USER_STACK_TOP_EXCLUSIVE` (Phase-05 D6 wiring durchgehend), ELR = `USER_VA_BASE + 4` (eret korrekt + SVC trap-after sauber), SPSR.M = `0b0000` (Trap kam echt aus EL0), Running-Flag vom Handler gecleart. **Damit ist der ganze Phase-04-D12 + Phase-05-D5/D6 + Phase-05-Foundation Stack End-to-End validiert** — ready für Phase-06 D1 (BasePage-Struct in tos-types) als nächsten Schritt. |
| 2026-05-04 | **PHASE-05 D5 + D6 nachgereicht.** D5 (EL0-Stack-Region) als neues Modul `kernel/src/arch/aarch64/el0.rs`: Konstanten `USER_VA_BASE`/`USER_VA_END_EXCLUSIVE`/`USER_STACK_TOP_EXCLUSIVE`/`USER_STACK_BOTTOM_INCLUSIVE`/`USER_STACK_DEFAULT_SIZE` definieren das per-Prozess-Low-Half-Layout (TPA mit Basepage am Anfang, EL0-Stack growing-down vom Top, 64-KiB-Default, 4-KiB-aligned, Top-Page reserviert als SP-Underflow-Guard). Compile-time-Asserts gegen Constant-Drift. Phase-06 Pexec mappt diese VAs in eine per-Prozess-TTBR0. D6 (SP_EL0 im Trap-Frame): `exception_vectors.s` Frame-Größe 272 → 288 Byte (zusätzliches `stp x12, xzr; mrs x12, sp_el0` save plus symmetrischer restore via `ldp x12, x13; msr sp_el0, x12`). `ExceptionFrame`-Struct um `sp_el0: u64` + reservierten 8-Byte-Pad ergänzt (Pad ist Phase-06-Slot für TPIDR_EL0 / Process-Tag). Compile-time-Assert auf Frame-Größe 288. **Phase-04 + -02 Selftests bleiben grün** (10/10 BDOS-Tests, Cconws, EINVFN). |
| 2026-06-16 | **PHASE-06 D3 `Pexec(PE_LOADGO)` + D6 `Pterm` parent round-trip grün; EmuTOS `run = run->p_parent` Fix.** Der Pexec-Selftest hängte nach dem Kind-`Pterm` im Eltern-Resume. Zwei Ursachen: (1) **EmuTOS-Divergenz** — `pterm::terminate` setzte `RUN = null` statt `run = run->p_parent` (EmuTOS `xterm`, bdos/proc.c:609), wodurch das eigene `Pterm` des wieder-laufenden Elternprozesses `NoRunningProcess`→EINVFN sah und im Leerlauf hing. Fix: `terminate` restauriert `RUN` auf den Eltern-BasePage über das neue kernel-seitige `parent_slot`-Linkage, das `handle_pexec` jetzt setzt (EmuTOS `proc_go`: `p->p_parent = run`, proc.c:541); `parent_slot == NO_PARENT` (boot-gelauncht, D6) clear't weiter auf null. (2) **QEMU-Wedge** — TTBR0-Switch in den VA-aliasierten, ASID-0-Elternraum vor dem `eret`-zu-EL0 verklemmt QEMUs cortex-a72-TCG; interim per Device-Traffic-Barrier zwischen `restore_boot_ttbr0` (TLBI) und `install_ttbr0(parent)` umgangen (identisch zu `handle_pexec`). Dokumentiert als **QUIRK-001** in `docs/MEMORY.md`. Per-Prozess-ASIDs (`TTBR0_EL1[55:48]`) wurden als „eigentlicher" Fix versucht und **verschlechtern dieses QEMU-Modell** (cortex-a72-TCG wedget bereits beim ASID-getaggten TTBR0-Write, EC4 hängt) → komplett revertiert. Erkenntnis: der Wedge ist ein reines QEMU-Artefakt (reale PIPT-Caches aliasen nie), der Barrier ist die korrekte Langzeitlösung für das QEMU-Target; ASIDs nur später für reale-HW-TLB-Effizienz (auf RPi4/neuerem QEMU validieren). **Verifikation:** QEMU 4/4 deterministisch — Kind druckt `Hello`, `Pterm(42)`, Eltern resümieren mit `x0=42`, Eltern-`Pterm`, `D3 PEXEC PASS`; Phase-04 (10/10), EC4, D3-Loader, D6 bleiben grün. **phase02-Cconws-Data-Abort root-caused + gelöst (kein Kernel-Bug):** `run_phase02_self_test()` löste beim `Cconws` via `svc #1` einen Data Abort (Permission fault L2, READ, FAR im Kernel-rodata) aus — **nicht durch Pexec verursacht** (faultet identisch vor den EL0-Tests). Ursache: [`cconws`](../tos-api/src/gemdos/console.rs) liest den String absichtlich per **unprivilegiertem Load** (`ldtrb`), damit der Zugriff als EL0 geprüft wird — die korrekte, TOS-treue Semantik „Syscall-Pointer sind User-Pointer". Das Pexec-Kind (EL0, User-String) funktioniert; der phase02-Selftest übergab einen **Kernel**-Pointer (AP=EL0-none) → `ldtrb` faultet zu Recht. Der phase02-EL1-Cconws-Subtest ist also **per Design ungültig** (Relikt aus Phase 02 vor echten EL0-Prozessen); entfernt — die Cconws-Round-Trip-Abdeckung liegt jetzt beim Pexec-Kind. EINVFN- und RX-Subtests (kein User-Pointer) bleiben. **Boot läuft jetzt komplett sauber durch, 0 Exceptions.** **Pexec liest `path` jetzt aus dem User-Pointer** (kein hartcodiertes `"C"` mehr): neuer `read_user_cstr`-Helfer in `exceptions.rs` liest die NUL-terminierte Pfad-Zeichenkette via `ldtrb` (unprivilegiert, gleiche EL0-Semantik wie `cconws`) aus dem aktiven Eltern-TTBR0 in einen 128-Byte-Kernel-Puffer; Registry-Lookup matcht `b"C"`. **Clippy** (`unused_unsafe` in `mmu.rs:245`) behoben — kernel + tos-api wieder clean auf beiden Boards. **Endstand verifiziert (QEMU 3×, deterministisch):** p04 10/10, EC4/D3/D6/PEXEC PASS, EINVFN OK, 0 Exceptions, sauberer Boot-Durchlauf. |

## Notes

The project is intentionally over-documented at the start. The premise:
spec-first development against a fixed, finite, well-documented API
prevents the implementation drift that ruins long-running OS hobby
projects.
