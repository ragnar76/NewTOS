# Project inventory — 2026-10-08

State: branch `main` (HEAD `81a9dce3`, tag `ofam-2026`), working tree clean at start of the sweep.
Goal of this document: what is done, what is open, and what stands between now and a fixed/stable state ("fester Stand").

**Method and limits (read first).**
- Swept: `docs/STATUS.md` (phase table, Blocked, Recent Decisions), all 402 tickets in `TASK/` (header `Status:` line + presence of `-STATUS-DONE.MD` / `-STATUS-UNFINISHED.MD` / `-STATUS-BLOCKED.MD`, via a script over the whole directory, not a sample), `tools/check-tickets.sh`, `phases/PHASE-*.md`, `docs/INVENTORY.md` (2026-09-04), `docs/REQUIREMENTS.md`, `docs/COMPATIBILITY.md`, `docs/analysis/aes-audit/00-MASTER.md`, `docs/analysis/open-tasks-post-2a.md`, `docs/HW-TEST-LOG.md`.
- I did NOT run any build or test, and I did not read the product code beyond a few `grep` existence checks. Every "done" below is a claim taken from a ticket receipt, STATUS entry or measurement path; the evidence pointer says where to look. STATUS.md is a change log (memory: `status-md-is-not-evidence`), so where a claim matters I name the ticket/measurement behind it. Where I could not check a backing artefact I say "unchecked".
- Ticket header text is the ticket author's own classification; I counted by header prefix.

## 0. Numbers

| Item | Value | Source |
|---|---|---|
| Tickets in `TASK/` (excl. `_template`, 6 index/handover docs) | 402 | `ls TASK/*.md` |
| with a `-STATUS-DONE.MD` | 201 + 2 with both DONE and UNFINISHED (A35, PORT09) | file presence |
| with `-STATUS-UNFINISHED.MD` only | 12 | file presence |
| with `-STATUS-BLOCKED.MD` | 1 (A45) | file presence |
| no status file | 187 (31 header-ERLEDIGT, 80 BACKLOG, ~60 OFFEN/parked, 8+ withdrawn) | file presence + header |
| Header class: ERLEDIGT/BEHOBEN/GEKLAERT | ~217 | header |
| Header: OFFEN | 61 (P1: 3, P2: 15, P3: 35, P4: 5, others) | header |
| Header: IMPLEMENTIERT (HW/Abnahme open) | 12: A16 A25 A29 A34 A37 A78 DESK34 DESK35 DESK42 Q06 V17 X01 | header |
| Header: ERLEDIGT/BEHOBEN but "Pi-/Hardware-Abnahme offen" | 52 (A56–A109 range, DESK01–DESK13, DESK44, GUI06, R02–R05, SER01, T04, V11, V12) | header |
| Header: BACKLOG | 80, of which 79 `G-AES-*`/`G-VDI-*` "smallest measurement case" designs, 1 `NET06` | header |
| Header: GEGENSTANDSLOS/ZURUECKGEZOGEN | 11 | header |
| Header: ZURUECKGESTELLT/GEPARKT/ZUKUNFT/IDEE | 12: GUI01 GUI02 H01 H04 HW02 HW05 HW06 IDEA01 R01 SLB01 T13 ZVIEW03 | header |
| Host tests | 1268 pass / 0 fail / 1 ignored | `docs/STATUS.md:15-22` (unchecked, not re-run) |
| Build warnings `tos-api`/`kernel` | 64 / 94; clippy 76 / 125; `desk-drift-probe` 31 | `docs/STATUS.md:15-22` |
| Measurement runs in `measurements/` | 199 dirs | `ls measurements` |
| Phase files | PHASE-00 … PHASE-16 | `phases/` |

`sh tools/check-tickets.sh` currently reports 5 contradictions: **FS09, INPUT01, ZVIEW01, ZVIEW02, ZVIEW04** say OFFEN in the header while a DONE receipt sits next to them (details in §4).

## 1. Per subsystem

Convention: DONE = receipt/measurement claims it works; "Pi" = accepted on a real Pi 4 per ticket/STATUS text. OPEN = ticket id, one line, severity (P1..P4 from the header; "n/a" if none).

### 1.1 Boot / MMU / memory protection
DONE
- Boot, EL entry, exception vectors, GIC, high-half migration, per-process TTBR0, page tables: PHASE-01..05 marked complete in `docs/STATUS.md:293-299` (Phase 01 EC2 only partially; see OPEN). Boot screen with EmuTOS `initinfo` wait loop (`STATUS.md:293`).
- ARCH03 (one shared address space + FreeMiNT-style per-process rights `PROT_P/G/S/PR/I`): stages 0c, 0d, 1, 2, 3a, 4 have DONE receipts (`TASK/ARCH03-*-STATUS-DONE.MD`), 3b/3c have UNFINISHED receipts; default `memprot=on`, `asid=on` default off (`TASK/ARCH03.md:53-69`). Evidence: `measurements/measurement-2026-10-05-arch03-*`, `measurement-2026-10-06-post-e9`.
- MEM03, MEM04, PROC01, PORT09 closed with ARCH03 stages (`TASK/MEM04.md`, `PROC01.md`, `PORT09-STATUS-DONE.MD`).
- MEMPROT-FAST (2026-10-07): sync only where rights change (`STATUS.md`, Recent Decisions; `docs/analysis/zview-sluggish-2026-10-07.md`).
- Framebuffer kept outside ST/TT pools (memory note; `docs/analysis/memory-layout-and-reported-free-memory.md`).

OPEN
- ARCH03 (header still "IN ARBEIT", P1): 3b/3c Tester acceptance and Pi runs open; **S2ALL/TSRPAR FAIL under protection on/strict** (cause E9+E16, decision needed, `ARCH03-3c-STATUS-UNFINISHED.MD`); stage "2b FreeMiNT Pexec modes 100+" not started (`ARCH03.md:65`). Severity: high for a stable state because memprot is default-on.
- ARCH03-POSTE9 point 1 (`p_parent` as EmuTOS) halted for a decision; PROC03 (desktop/AES as real EL0 process with its own PD) P2 open.
- MEM01 (`Mxalloc(-1)` reports same value for ST and TT) P2; MEM02 (`MX_PREFTTRAM` returns ST-RAM) P2; MEM05 (254 memory descriptors systemwide exhausted by many small `malloc`) P2.
- ARCH02 (`Pexec` parents lose FP/SIMD regs) P2.
- Multi-bank RAM detection: only first `/memory` bank used (768 MiB TT-RAM on a 4 GB Pi), `STATUS.md` Blocked section; user says 768 MiB is enough (memory note) — future.
- Phase 01 EC2 real-HW rest-verification list in STATUS Blocked is old (June); not re-based.
- B1 `os_magic=0`, `os_date` placeholder (`open-tasks-post-2a.md` §4, unchecked whether changed).

### 1.2 Drivers
SD/EMMC
- DONE: SD host read/write/CSD, MBR → drives C:+ (STATUS Phase 08); multi-block + 4-bit bus (FS08 part 1, `FS08-EMMC-STATUS-DONE.MD`), FAT cluster-hint + direct-to-buffer I/O (`FS08-FAT-STATUS-DONE.MD`), 32 KiB stdio buffer (`FS08-STDIO-STATUS-DONE.MD`), ARM clock to firmware max (`FS08-CPUCLK-STATUS-DONE.MD`), 32 MiB block read cache (FS09, `docs/analysis/fs09-block-cache-2026-10-07.md`), partition-boundary copy 2182 → 467 ms on Pi (FS04, `FS04-FIX-STATUS-DONE.MD`), write errors reach the application (HW03, `HW03-STATUS-DONE.MD`). 
- OPEN: HW04 (init/transfer form vs. spec, from EMMC audit) P2; HW05 (write throughput 89 KB/s, deferred by user) P2; HW06 (directory writes 176 ms, unexplained) P3; HW07 (half clock from block 3?) P3; HW02 (clock reported vs measured) P4 deferred; FS08 header itself still OFFEN (see §4). Unverified: after FS08/FS09 the effect on zView image load on the Pi is covered only by the user's overall "läuft alles" of 2026-10-08, not by a measurement.

USB / HID
- DONE: xHCI + VL805 stack, HID keyboard/mouse, mouse wheel, key repeat, on Pi (STATUS Phase 16; mouse+wheel and key repeat Pi-accepted 2026-09-30/10-03).
- OPEN (unclear): the "mouse works only in one of four ports / hub treated as mouse" defects (STATUS Phase 16 row and Blocked section, 2026-09-04/09-09) have no ticket in `TASK/` and no closing entry that I found; I did not trace whether they were fixed. Mark as unchecked, severity potentially high (input is the demo path).
- KEYS02 / D7: only `kbdvec` is delivered; `midivec`, `statvec`, `mousevec`, `clockvec`, `joyvec`, `ikbdsys` hooks are not called (`STATUS.md` Blocked first entry). Severity P3 (no real app known to need them).

Framebuffer / VideoCore
- DONE: framebuffer, fbcon, page flipping measured on Pi (`HW-TEST-LOG.md` 2026-06-17, 09-05); truecolor VDI (`open-tasks-post-2a.md` §0).
- OPEN: X02 (resolution change on the Pi, Track-B decision paper) P3; BOOT01 (boot screen layout on high resolutions) P3; HW-TEST-LOG notes "`Setscreen` does not lock to vsync" (2026-09-09) — no ticket found, unchecked.

Timer / UART
- DONE: 200 Hz tick (`_hz_200`), `clock()` accurate (STATUS Phase 07, 2026-09-04); PL011 console + RX IRQ in QEMU; serial output off by default on Pi, `serial=on` in cmdline (SER01).
- OPEN: H01 (DS3231 RTC) parked, waits for hardware, P2; SER01 header says Pi-Abnahme open (P1 header); `etv_timer`/`etv_critic` hooks exist in `tos-api/src/bios/{vectors,tick}.rs`, whether installed vectors are actually invoked is not verified here (INVENTORY §1.6 said no; later EL0-upcall work in `open-tasks-post-2a.md` §4 delivered `etv_term` only — treat as unclear).
- Sound: H04/SND01/SND02 (DMA sound, HDMI audio) P3, after the meeting; SND01 spec and Track-B notes exist (`SND01-SPEC-STATUS-DONE.MD`, `SND01-TRACKB-STATUS-DONE.MD`); SND02 says HDMI audio source situation unresolved.
- Networking: NET01–NET06 (GENET, netif, inet4, mintsock, HighWire, WLAN) all not started; future.

### 1.3 BIOS (trap 13)
DONE: 12/12 functions per triage (`docs/analysis/bios-audit/bios-xbios-feasibility-triage.md`, via `open-tasks-post-2a.md` §4); B1 low-memory sysvars + OSHEADER + cookie jar written per process (`b1-lowmem-feasibility.md`); Getbpb hardening, Rwabs physical mode; `$5A0` mirror live (STATUS Phase 08); BIOS group proven from EL0 on Pi (`HW-TEST-LOG.md` 2026-09-05, APITEST 86 PASS/0 FAIL/1 SKIP).
OPEN: etv_critic wiring Rwabs → handler + `fm_show`/`eralert` (P4-012, `open-tasks-post-2a.md` §2; state after July unchecked); KEYS02 vectors (above); STRAM/TTRAM split "BB2" (MEM01/MEM02).

### 1.4 XBIOS (trap 14)
DONE: dispatcher with EmuTOS `xbios_unimpl` convention, screen group, clock, `Random`, `Protobt`, `Kbdvbase`, `Kbrate`, `Ssystem`/cookie jar; HW-absence contracts for PSG/MIDI/serial/blitter (STATUS Phase 11 row). `Super`/`Supexec` solved on the caller side in the toolchain overlay (X01, `measurements/measurement-2026-09-14-x01/`).
OPEN: X01 two decisions + Pi (header "IMPLEMENTIERT", P2); Videl group 0x58–0x5F and `Srealloc` deferred until an app needs them (user decision, `open-tasks-post-2a.md` §4); Falcon audio/DSP not present. Severity low unless a target app needs them.

### 1.5 GEMDOS / filesystem
DONE: console (15/15, per INVENTORY correction §1.11), time, memory (`Malloc`/`Mxalloc`/`Mfree`/`Mshrink`, Phase 04), process (`Pexec` modes, `Pterm`, `Ptermres`, PROC01 modes 5/6/7, PROC02 handle inheritance + `Fforce`), FAT16 read/write, long names (FS01), `Fattrib` trailing backslash (Q07), FreeMiNT `Ssystem`. Evidence: Phase rows in `STATUS.md:293-305`, `TASK/PROC0*-STATUS-DONE.MD`, `TASK/FS0*-STATUS-DONE.MD`.
OPEN: FS06 (`Frename` to an empty name returns E_OK and creates an empty directory entry) P2 — data-integrity relevant; FS07 (`Fseek` on a console handle) P2; FS02 (`Frename` leaks long-name chain) P3; FS05 (`fwrite` walks cluster chain from start per sector) P3; PROC04 (remaining handle deviations) P3; FS03 withdrawn. Filesystem scope: FAT16 only; FAT12 write is an honest "unsupported" (memory note); FAT32/exFAT is listed in REQUIREMENTS FR-5 ("FAT16 / FAT32") but no ticket implements FAT32 — see §4 and the SD-card note that FAT16-vs-FAT32 on the physical card was left unresolved 2026-09-30.

### 1.6 VDI
DONE: all ~70 opcodes, truecolor workstation, raster ops, Gate green (`docs/analysis/vdi-audit/00-GATE.md` via `open-tasks-post-2a.md` §0); V-series classified as device parameters with proof (`docs/analysis/vdi-v-series-classification-2026-09-21.md`); R02–R05 raster with screen operand (`R02-R05-STATUS-DONE.MD`); `vr_trnfm`/`vq_scrninfo` EdDI for Doom (PORT06-EDDI); ZVIEW01 `vrt_cpyfm` byte-in-word fix.
OPEN: R01 (`vro_cpyfm` oversized destination, D-VDI-25) deferred P3; T08 (`vex_*` 8-byte pointer slots) P4; 79 `G-VDI-*`/`G-AES-*` BACKLOG measurement-design tickets (P3) — these are test-coverage gaps (functions never measured against a reference), not known defects (see §2b); rotated `v_justified` render (`open-tasks-post-2a.md` §2). R02–R05, V11, V12 are "Pi-Abnahme offen" (P1 header).

### 1.7 AES
DONE: 79/79 opcodes present (AES-MASTER dashboard); WP1–WP8 landed (`open-tasks-post-2a.md` §0); the whole A-series (A01–A109) of measured divergences vs EmuTOS (`TASK/A*-STATUS-DONE.MD`); keystate at event time, click counting EmuTOS model (ZVIEW02), keyboard routed to mouse owner (KEYOWNER); cooperative scheduler fix (SCHED01); accessories (ACC01–ACC06, ACC10: load, desk menu, `AC_OPEN/CLOSE`, SYSACC dialog); AES owns a physical workstation + `graf_handle` (ZVIEW-M4-K1). Pi-accepted: desktop, TeraDesk, QED, EmuCON (`STATUS.md:8-19`; `HW-TEST-LOG.md` 2026-09-11, 09-18, 09-20).
OPEN:
- A45 BLOCKED (P1 header): `appl_tplay`/`appl_trecord` are stubs because the AES forkqueue is missing; no target app uses them (`STATUS.md` Blocked). This is the one remaining declared stub in the production AES against CLAUDE.md rule 2 / NFR-3.
- A16, A25, A29, A34, A37, A78: implemented, HW acceptance open (P1/P2). A13/A14/A15/A28/A32: header ERLEDIGT, UNFINISHED receipt = "HW acceptance missing" per the receipts.
- A62 (AES process table half-size, investigation done, decision on rebuild with the user); A111 (click counting/caret) P3; ACC07/ACC08 (accessory titles in an app's menu tree; accessory with own window) P3; ACC09 `CONFIG.NAC` P3; GUI03/GUI05 (loadable cursors, alt desktop graphics) P4; GUI01/GUI02 (show file, viewer support) deferred P4.
- M-series deferrals from AES-MASTER: custom `G_USERDEF` upcall into EL0 apps, full CICON colour (A41 done for tool; check AES side), `fm_own`/`fm_show` — state after July unchecked.
- `docs/analysis/aes-audit/00-MASTER.md` is dated 2026-07-22 and predates the A-series; its WP list is superseded by `TASK/A*`. Do not use it as a current backlog.

### 1.8 Desktop (EmuDESK, `kernel/src/desk/`)
DONE: desk rebuilt 1:1 against EmuTOS (ADR-022), DESK01–DESK38, DESK41, DESK44, DESK46, DESK47, DESK50 closed; DESK43 steps U1–U4 accepted; X03 done; drift probe 47 → 31 (`STATUS.md:23-30`; `TASK/DESK43.md:122-132`); Pi: window open via `do_wopen`/`do_diropen` (DESK50), `.NTP` parameter dialog (DESK47), four native extensions from TeraDesk (DESK41).
OPEN:
- **DESK43 (P1, header OFFEN)**: desktop must run in user space like every other app; U5–U10 after the meeting; X03 must precede U8. Largest remaining structural deviation from EmuTOS in the desktop; also the premise for PROC03 and the A28/A29/A62 family.
- DESK48 (four extra `redraw_desktop()` calls EmuTOS lacks) P2; DESK34, DESK35, DESK42 implemented, HW acceptance open (P1/P1/P3 headers); DESK39–DESK40, DESK45 (4 desk selftests wired in no start path), DESK49, DESK51–DESK56 (side findings of DESK50) P3/P4.
- BALL01 (ball painted before the background on the first frame, user saw on the real Pi 2026-10-08, cause unknown) P3, "after the meeting".
- Nine desktop menu items were disabled in INVENTORY §7.2 (2026-09-04); DESK-series work since then closed most of the file-menu items (DESK01–DESK04, DESK13, DESK30, DESK33…). I did not recount which menu items remain greyed out — unchecked.

### 1.9 Process model / scheduler
DONE: `Pexec` all EmuTOS modes, `Pterm`/`Ptermres`, per-process address space → shared address space (ARCH03), cooperative scheduler with sleep/wake (`selftest_sleepwake*`, SCHED01), EL0-upcall primitive + `etv_term` (July, `open-tasks-post-2a.md` §4), accessories coexisting with the desktop (ACC series), `vfork` ownership (`docs/analysis/vfork-malloc-ownership-2026-10-06.md`).
OPEN: PROC03 (P2), ARCH02 (P2), ARCH03 stages 2b/3b/3c remainder, T11 (flaky `sched_pexec_parent_suspend`, shared global state) P3, preemption/multitasking = future (FreeMiNT direction, see §2c). Cooperative multi-process kernel (AES-MASTER §3: 44 internal functions) is partly realised through SCHED01/ACC work; I did not compare against the 44-function list — unchecked whether "gemasync/gemdisp/gemflag/geminput/gemctrl" are complete.

### 1.10 SDK / toolchain
DONE: LP64 mintlib + gemlib + cflib, crt0 argc/argv + `ARGV=` convention (PORT09), `<sys/wait.h>` fix, `Super`/`Supexec` overlay (X01), overlays keep upstream `src/` pristine; libs zlib/libpng/libjpeg/giflib/libexif (zView M2), libldg + WinDom + libshare (LDG M1–M3, `docs/analysis/libshare-2026-10-06.md`); SDK host tests 14/0/0 (`STATUS.md` 2026-10-06).
OPEN: PORT04 (keep mintlib/gemlib at maintained state) P3; PORT11 (SDK docs after E9) P3; T09 (loader gives data-less program four null fields; `.init_array` never run) P4 — INVENTORY §8.4 flagged `.init_array` not executed; ticket still open; T13 test programs should leave their result on screen (deferred); W01 RSC viewer as real GEM app P2; SDL01 (SDL backend) P3 after the meeting; DESK49 (TeraDesk `filetypes` list lacks native extensions) P3.

### 1.11 Ported apps
DONE (Pi unless noted): QED (from TeraDesk), TeraDesk start/quit, EmuCON, Doom 40–75 fps with exact keyboard (PORT06/KEYS02), CoreMark, Ball demo (PORT08/SCHED01), PRESENT.NAP slide show (`aec187e4`), zView + 10 codecs built (ZVIEW-M4) with browser names (ZVIEW01), double-click (ZVIEW02), mouse during load (INPUT01), File > Quit (ZVIEW04) — fixed in QEMU, then on the demo card accepted by the user "es laeuft alles wie es soll" (2026-10-08, `STATUS.md` Recent Decisions first rows). HighWire: directory `sdk/apps/highwire` exists, NET05 not started.
OPEN: PORT02 (what else can be ported, rating) P2; ZVIEW header tickets stale (§4); ZVIEW03 thumbnail cache future; SLB01 FreeMiNT shared libraries future; `LDG-M3` UNFINISHED receipt (two non-WinDom blockers, then closed by B1/B2 per later receipts — header state unclear); ZVIEW-M4 UNFINISHED receipt.

### 1.12 Test infrastructure
DONE: `cargo xtask test` now runs `tos-api`, `tos-types`, `xtask` (`tools/xtask/src/main.rs:630`) — INVENTORY §0.3/§1.2 ("command runs zero tests") is no longer true; 1268 host tests; QEMU raspi4b/injector runs with measurements (`measurements/`, 199 dirs), Hatari TT/Falcon references (memory notes), dumper reference tables, drift probe `tools/desk-drift-probe.sh`, `tools/check-tickets.sh`.
OPEN: DESK45 (desk selftests wired nowhere) P3; `kernel/src/selftest_*.rs` (28 modules) — INVENTORY §0.2 said none runs at boot; I did not verify whether that changed, DESK45 and the "unwired selftests rot silently" note imply it did not; T10/T11/T15 (injector arming, flaky test, shared pointer state) P3; M08 (one table unwired, depends on product defect), M09/M10 P3; the 79 BACKLOG `G-*` measurement designs.
Hardware logging: `docs/HW-TEST-LOG.md` has its last real entry on 2026-09-20 and a `_pending_` serial row; the later Pi runs (09-24, 09-28, 09-30, 10-01..10-08) are recorded only in STATUS/ticket receipts — the INVENTORY recommendation §11.3 (one logged Pi run per week) is not followed in the log file itself.

### 1.13 Documentation
DONE: 188+ API spec docs `docs/api/`; `docs/COMPATIBILITY.md` (905 lines of sanctioned deviations); `docs/MEMORY.md` (2125 lines of ADR/quirks); README rewritten 2026-10-08; presentation `docs/presentation/`; SDK docs `docs/sdk/`.
OPEN: REF03 (three wrong spec citations, two in product code) P3; `docs/STATUS.md` top block is stale (§4); `docs/INVENTORY.md` is a 2026-09-04 snapshot and its "Behauptung ohne Deckung" list has partly been overtaken; `docs/api/xbios/26-supexec.md` was corrected (DESK27 done). NFR-6 license: `LICENSE` file exists, `docs/MEMORY.md:450` lists ADR-001 (MIT vs Apache) under pending ADRs — I did not verify which license file is present or that ADR-001 was written.

## 2. Open tickets, sorted

### (a) Blocking a stable state: visible bug, crash, data risk, or unverified on the real Pi
P1 headers:
- DESK43 desktop in user space — structural; U5–U10 (L).
- ARCH03 remainder: S2ALL/TSRPAR FAIL under memprot on/strict, Tester acceptance 3b/3c, Pi runs 0c–4 (M–L). memprot default-on makes this the highest-risk item for stability.
- A45 `appl_tplay`/`appl_trecord` stubs (L: needs forkqueue; or an explicit decision to document it as sanctioned honest-unsupported).
- FS08 (header P1) — read speed; parts 1–4 + FS09 done; measured on the real Pi on 2026-10-07 by the user with FSBENCH (raw ~0.7 → 10.5 MB/s, stdio 10 MB 20.8 s → 1.05 s, photos in this session; not yet filed as a measurement run). Remaining: file that run and fix the header (S).
- A16, A25, A29, A37, Q06, V17, DESK34, DESK35 — implemented, "HW offen" (P1 headers); folded into one Pi acceptance sweep (S each, one run).
- R02–R05, V11, V12, SER01 — header "Pi-Abnahme offen" with P1 (S, same sweep).
P2 with user-visible or data effect:
- FS06 `Frename` empty name creates an empty directory entry (S) — data integrity.
- MEM02 `MX_PREFTTRAM` ignored, MEM01 `Mxalloc(-1)` wrong, MEM05 descriptor exhaustion (M) — allocation behaviour that real apps hit (zView/Doom class).
- ARCH02 FP/SIMD lost across `Pexec` parent (M) — silent corruption class.
- PROC03, DESK48, HW04, HW05 (write 89 KB/s), W01 (tool), PORT02, X01 decisions.
- 52 ERLEDIGT tickets "Pi-/Sichtabnahme offen" (A56–A109, DESK01–DESK13, DESK44, GUI06, T04) — not defects, but unverified-on-HW per their own text.
- USB multi-port mouse issue (no ticket; see 1.2).

### (b) Conformance gaps vs EmuTOS without a visible effect (or P3/P4 side findings)
DESK39, DESK40, DESK45, DESK51–DESK56, FS02, FS05, FS07, PROC04, REF03, R01, T08, T09, A111, GUI03, GUI05, ACC07, ACC08, MOD01 (research), KEYS02 D7 vectors, HW06, HW07, HW02, X02, BOOT01, PORT04, PORT11, M09, M10, T10, T11, T15, BALL01 (visible once, first frame only), the 79 BACKLOG `G-AES-*`/`G-VDI-*` measurement designs.

### (c) Future / extensions
- m68k userland emulator (REQUIREMENTS FR-4; only `build-doom-m68k.sh` and the "own extensions" notes exist; no ticket).
- SLB01 (FreeMiNT shared libraries), ZVIEW03 (thumbnail cache), LDG already done.
- Multitasking beyond EmuTOS-cooperative (FreeMiNT preemption, multiple main apps), multi-core prep (post-2a roadmap memory).
- Networking NET01–NET06; sound H04/SND01/SND02; RTC H01; IDEA01 power management; SDL01; ACC09 `CONFIG.NAC`; GUI01/GUI02; Videl/audio XBIOS; multi-bank RAM; ARCH03 stage 2b (Pexec 100+).
- ARCH03 stage 4 `asid=on` default.

## 3. Proposed definition of "fester Stand"

Derived from the project's own gates, not invented:
- AES-MASTER §4/§6 and memory `base-emutos-clean-on-pi-before-extension`: the gate is "2a (all bugs) + 2b (all missing EmuTOS functions, including cooperative kernel and accessories) **run clean on the real Pi**", and only then extension work.
- `STATUS.md:8-19` demonstration gates (nothing hangs, visible errors before conformance) and `COMPATIBILITY.md` (contract surface must match).
- CLAUDE.md rule 2 / NFR-3: no stubs in production.

Proposed definition (for the user to confirm):

> **Fester Stand = tag `base-1`:** (1) no open P1 ticket and no open "implemented, HW pending" ticket; (2) one scripted Pi acceptance sweep, recorded in `docs/HW-TEST-LOG.md`, covering the gestures of the 52 "Pi-Abnahme offen" tickets plus the demo path; (3) memory protection default-on has no known FAIL (ARCH03 S2ALL/TSRPAR resolved or the default consciously set to off); (4) no declared stub left (A45 implemented or recorded as sanctioned deviation in COMPATIBILITY); (5) no known data-integrity defect (FS06, HW03 done); (6) `cargo xtask test` + both-board build + drift probe at the STATUS baseline and the 28 `selftest_*` either wired or removed from the claims; (7) STATUS/ticket headers reconciled (§4); (8) the Pi image reproducibly built and its sha recorded (kernel + SD layout).

Tickets between now and that state, with rough size (S = under a day, M = days, L = a week or more; my estimate, not from the tickets unless ARCH03/DESK43 give one):

| Gate | Tickets | Size |
|---|---|---|
| Hardware acceptance sweep | A16, A25, A29, A34, A37, A78, DESK34, DESK35, DESK42, Q06, V17, R02–R05, V11, V12, SER01, 52 header-"Pi offen" tickets, GUI06, DESK44 → one script + one user session + log entry | M (preparation S, user time on the Pi) |
| Memory protection stable | ARCH03 3b/3c remainder (S2ALL/TSRPAR), Tester acceptance, Pi runs; MEM01, MEM02, MEM05; ARCH02 | L |
| Desktop conformance | DESK43 U5–U10 (+ X03 done), DESK48, PROC03 | L (DESK43 plan lists U5–U10 after the meeting; `ARCH03.md` estimated 35–51 days for stages 0–3, mostly done) |
| Stub-free AES | A45 forkqueue → `appl_tplay`/`appl_trecord`, or decide sanctioned deviation | L / S |
| Data and file system | FS06, FS02, FS05, FS07, PROC04, HW04 | M |
| Input/USB | confirm or ticket the multi-port mouse defect; KEYS02 D7 (optional) | S–M |
| Perf confirm | FS08 Pi run to file, HW05 (write speed) | S / M |
| Test/doc hygiene | DESK45, T11, T15, STATUS top block, 5 ticket headers, HW-TEST-LOG, INVENTORY refresh (rerun, not edit) | S–M |
| Decisions only the user can make | X01 (two decisions), A62 rebuild, ARCH03-POSTE9 point 1, ARCH03 E9/E16 | S each |

## 4. Inconsistencies found

1. **Header OFFEN but fixed (check-tickets finds 5):** FS09, INPUT01, ZVIEW01, ZVIEW02, ZVIEW04 — DONE receipts exist and `STATUS.md` Recent Decisions (2026-10-07/08) describes the fixes. FS08 has four part receipts (`FS08-EMMC/FAT/STDIO/CPUCLK-STATUS-DONE.MD`) but no `FS08-STATUS-*` file and an OFFEN header; the script does not catch this because the filename pattern differs.
2. **STATUS.md top block is stale** (`docs/STATUS.md:8-60`, "Stand 2026-09-30"): says branch `desk-rebuild` (dissolved 2026-10-08, `main` carries the work); lists MEM03/PORT09 as open or "after the meeting" items although closed 2026-10-05/06; says ARCH03 "Freigabe offen" while stages 0c–4 were built and DONE receipts exist; says PORT06 Doom "Schritt 1 laeuft in QEMU" while Doom runs on the Pi (also stated 3 lines above); "Kein P1 offen" while DESK43 (P1), FS08, ZVIEW01 headers say P1 OFFEN and A45 is a P1 BLOCKED. Also the "Phase 14 — Desktop" row still says "In Arbeit … Offen: Maus-Input HW (IKBD/UART2)" which predates USB mouse on the Pi; "Phase 02 D6/D7 offen", "Phase 01 EC2 pending HW", "Phase 15 In Arbeit" are old row texts.
3. **Pi-acceptance vs header:** 52 tickets are ERLEDIGT with "Pi-/Hardwareabnahme offen"; the user's 2026-10-08 card acceptance ("es laeuft alles wie es soll") is general, not a per-ticket receipt; none of the 52 headers were updated. Either mark them accepted by reference to that run, or run the sweep.
4. **Stale UNFINISHED receipts next to done tickets:** A35 (DONE + UNFINISHED; the UNFINISHED text says "WEITERHIN NICHT IMPLEMENTIERT", header ERLEDIGT), PORT09 (DONE 2026-10-06 + older UNFINISHED), A13/A14/A15/A28/A32/A62/FS04/M08/Q06/X01 (header ERLEDIGT or IMPLEMENTIERT with UNFINISHED receipt meaning "HW acceptance missing" — the filename suffix means two different things). `LDG-M3-STATUS-DONE.MD` and `-UNFINISHED.MD` coexist; `ZVIEW-M4-STATUS-DONE.MD` and `-UNFINISHED.MD` coexist.
5. **Header IMPLEMENTIERT/BEHOBEN where a DONE file exists:** A16, A34, A37, A78, DESK34, DESK35, DESK42, DESK44, V17 (DONE file present, header says HW/acceptance open) — consistent with their text but not counted by `check-tickets.sh`, so the board reads as "open" for 9 tickets that only await the Pi.
6. **Tickets without a ticket file:** `ACCTITLE-STATUS-DONE.MD`, `FIX-01..06`, `DESK43-PLAN`/`-U1..U4`, `R3-TABELLE`, `PORT-WAITH`, `SND01-SPEC/-TRACKB`, `ZVIEW-M4(-K1)`, `LDG-M*` are sub-receipts of other tickets or tickets without a `.md`; 187 tickets have no status file at all, of which 31 are header-ERLEDIGT (A06, A08, A09, V01–V15 device-parameter proofs, R02–R05, DESK47, REF04, MEM04, PROC01 ...) — their closure is only in the header text.
7. **`ARCH03.md` header "IN ARBEIT … Umsetzung nach der OFAM"** vs. the table inside showing stages already built; MEM04/PROC01 point to ARCH03 stages as closure. The ticket is both "plan awaiting approval" (STATUS) and "built" (table).
8. **`INVENTORY.md` (2026-09-04) superseded in several points:** (a) "`cargo xtask test` runs zero tests" → now `HOST_TEST_PACKAGES = ["tos-api","tos-types","xtask"]` (`tools/xtask/src/main.rs:630`), 1268 tests; (b) `Super`/`Supexec` "report success, do nothing" (§1.4/1.5) → caller-side overlay (X01) but kernel-side `Supexec` remains "needs decision"; (c) `MAX_VERTICES` 512 → code comment at `tos-api/src/vdi/primitives.rs:562-572` now documents 1024 (value not re-read); (d) argc/argv and `uintptr_t` (§8.2/8.3) → PORT09 and LP64 pivot; (e) "nothing measured on hardware" (§11.3) → Pi runs through 2026-10-08; (f) "cooperative kernel and accessories not by October" → accessories and SCHED01 are done; (g) "nine disabled menu items" → mostly closed by DESK-series. Not superseded and still true: BIOS had zero host tests (unchecked now), selftests not wired into a boot path (DESK45 for the desk ones).
9. **AES-MASTER (2026-07-22) and `open-tasks-post-2a.md` (2026-07-23)** read as "current backlog" but predate A01–A109, DESK01–DESK56 and ARCH03; they should carry a "superseded by TASK/" banner.
10. **REQUIREMENTS.md vs reality:** FR-4 m68k emulator (no ticket; PHASE plan removed it from scope, `STATUS.md:290`) — the requirement text still lists it as functional; FR-5 FAT32 and virtio-blk (only FAT16 on SD; QEMU uses emulated EMMC per memory note, so virtio-blk is probably moot); FR-7 "keyboard via UART (initial) and USB HID (later)" — met on the Pi, UART path is console-only; C-3 "cooperative multitasking acceptable" matches; NFR-3 "no stubs" vs A45 stub (the only declared one I found); NFR-6 license "decision pending".
11. **HW-TEST-LOG.md** last entry 2026-09-20 plus a `_pending_` row for a serial adapter ordered 2026-06-16 that has since been replaced by working serial (SER01); later Pi acceptances (09-24, 09-28, 09-30, 10-01, 10-03, 10-04, 10-07, 10-08) are not logged there.
12. **Phase files:** `PHASE-02`/`PHASE-04` checklists carry unticked `[ ]` / unmarked D9–D13 items although STATUS says Phase 02 D1–D5 done and Phase 04 complete; `phases/` has no per-phase status field — a phase's state exists only in the STATUS table.

## 5. What I could not determine (flagged, not guessed)
- Whether the USB multi-port mouse defect (2026-09-04/09) was fixed; no ticket, no closing STATUS line found.
- Whether `etv_timer`/`etv_critic` installed handlers are now invoked (grep shows only `etv_term` upcall work and the vector definitions).
- Whether any `selftest_*.rs` is wired into a boot path after 2026-09-04.
- Exact count of still-greyed desktop menu items.
- Whether ADR-001 (license) was written.
- Whether the AES-MASTER 99 NO-COUNTERPART functions (cooperative core group) are fully realised.
These need a targeted read of product code, outside this sweep's read-only-docs scope.
