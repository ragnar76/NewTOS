# EmuTOS Source Layout Map (D1)

> **Phase 00 deliverable D1.** Navigation index of the EmuTOS source
> tree at `references/emutos/`, pinned commit `1a7484ab`.
>
> Every later Phase-00 deliverable references locations recorded here.

---

## Pinned commit

- **Repo:** https://github.com/emutos/emutos
- **Branch:** master
- **Commit:** `1a7484ab` ("Update GitHub Actions to use Node 24")
- **Local path:** `references/emutos/`

Citation form in spec entries:

```
EmuTOS @ 1a7484ab — bdos/console.c:xconin()
```

---

## Top-level layout

| Directory       | Subsystem                                                 |
|-----------------|-----------------------------------------------------------|
| `bdos/`         | GEMDOS — file system, processes, memory, console (trap 1) |
| `bios/`         | BIOS + XBIOS + low-level device drivers (traps 13, 14)    |
| `vdi/`          | VDI — graphics primitives (trap 2 sub-call)               |
| `aes/`          | AES — windows, menus, forms, events (trap 2 sub-call)     |
| `desk/`         | EmuDesk — desktop application (uses AES/VDI)              |
| `cli/`          | EmuCON — command-line interpreter                         |
| `util/`         | Internal utilities (string, memmove, doprintf, intmath)   |
| `include/`      | Shared headers (types, defines, structures)               |
| `doc/`          | Project documentation (license, changelog, design notes)  |
| `tools/`        | Host-side build tools (mkrom, etc.)                       |
| `tests/`        | Test harness                                              |
| `extras/`       | Non-essential add-ons                                     |
| `po/`           | Translations                                              |

Root files of interest:

- `Makefile` — build system entry point
- `emutos.ld`, `tosvars.ld` — linker scripts (m68k + RAM-resident OS)
- `version.mk`, `country.mk` — build configuration knobs
- `readme.txt`, `release.mk` — project-level docs

---

## Trap dispatchers (entry points)

| Trap | Vector | Entry asm                      | C dispatcher                        |
|-----:|-------:|--------------------------------|-------------------------------------|
| 1    | 0x21   | `bdos/rwa.S:_enter`            | `bdos/bdosmain.c:osif()`            |
| 13   | 0x2D   | `bios/vectors.S:_biostrap`     | `bios/bios.c:bios_vecs[]` (indirect)|
| 14   | 0x2E   | `bios/vectors.S:_xbiostrap`    | `bios/xbios.c:xbios_vecs[]`         |

Both BIOS and XBIOS share the assembly trampoline `biosxbios:` in
`bios/vectors.S:1282`. Trap 1 has its own trampoline (`_enter`) because
it manages the basepage register-save area and the "supervisor mode
toggle" undocumented function 0x20.

Detailed dispatcher specs:
- [`docs/api/gemdos/_dispatcher.md`](api/gemdos/_dispatcher.md)
- [`docs/api/bios/_dispatcher.md`](api/bios/_dispatcher.md)
- [`docs/api/xbios/_dispatcher.md`](api/xbios/_dispatcher.md)

---

## `bdos/` — GEMDOS

| File             | Lines | Purpose                                                |
|------------------|------:|--------------------------------------------------------|
| `bdosmain.c`     |   705 | trap-1 dispatcher (`osif`), function table `funcs[]`   |
| `console.c`      |   582 | console functions 0x01–0x0B, 0x10–0x13                 |
| `proc.c`         |   634 | process control (Pexec, Pterm, ...)                    |
| `umem.c`         |   603 | user memory (Malloc, Mfree, Mxalloc, Mshrink)          |
| `iumem.c`        |   263 | internal memory allocator front-end                    |
| `osmem.c`        |   397 | OS-side small allocator (`xmgetblk`, `xmfreblk`)       |
| `time.c`         |   230 | date/time (Tgetdate, Tsetdate, Tgettime, Tsettime)     |
| `fsmain.c`       |   486 | filesystem entry points / driver registry              |
| `fsdir.c`        |  2074 | directory ops (Fsfirst/Fsnext, Dgetpath, ...)          |
| `fsopnclo.c`     |   722 | Fopen/Fclose/Fcreate                                   |
| `fsio.c`         |   557 | Fread/Fwrite/Fseek                                     |
| `fsfat.c`        |   367 | FAT cluster chain manipulation                         |
| `fsdrive.c`      |   249 | drive/BPB management                                   |
| `fsbuf.c`        |   235 | sector buffer cache                                    |
| `fshand.c`       |   123 | Fhandle table maintenance                              |
| `fsglob.c`       |    48 | global filesystem state                                |
| `kpgmld.c`       |   360 | program loader (used by Pexec)                         |
| `rwa.S`          |     — | trap-1 entry assembly (`_enter`, `_bdos_trap2`)        |

Headers under `bdos/`:
- `console.h` — `H_Console`/`H_Aux`/`H_Print`, `HXFORM`
- `fs.h` — KBBUFSZ, filesystem internals
- `mem.h`, `proc.h`, `pghdr.h` — internal layout for memory/process/program
- `time.h`, `bdosstub.h` — time, glue declarations

---

## `bios/` — BIOS + XBIOS + drivers

155 files. High-traffic ones:

| File          | Purpose                                                         |
|---------------|-----------------------------------------------------------------|
| `bios.c`      | BIOS dispatch table `bios_vecs[]`, BIOS-12 implementations      |
| `xbios.c`     | XBIOS dispatch table `xbios_vecs[]`, function-number constants  |
| `vectors.S`   | trap-13/14 entry, exception trampolines (`_int_vbl` etc.)       |
| `chardev.c`   | character-device vector tables (`bconin_vec`, `bconout_vec` …)  |
| `conout.c`    | screen output (`cputc`, `ascii_out`), VT52 state                |
| `vt52.c`      | VT52 escape state machine                                       |
| `ikbd.c`      | IKBD (keyboard) protocol                                        |
| `mouse.c`     | mouse handling (via IKBD)                                       |
| `mfp.c`       | MFP (interrupts, timers)                                        |
| `serport.c`   | serial / RS-232 (`bconin1`/`bconout1`/`bcostat1`)               |
| `parport.c`   | parallel printer port (`bconin0`/`bconout0`)                    |
| `midi.c`      | MIDI in/out                                                     |
| `floppy.c`    | floppy drive                                                    |
| `ide.c`       | IDE                                                             |
| `acsi.c`, `scsi.c` | ACSI / SCSI                                                |
| `sd.c`        | SD card                                                         |
| `clock.c`     | RTC / system tick                                               |
| `screen.c`    | screen base/resolution                                          |
| `videl.c`     | Falcon VIDEL controller                                         |
| `nvram.c`     | NVRAM                                                           |
| `kprint.c`    | kernel printf (debug only)                                      |
| `lineainit.c` | Line-A init                                                     |
| `sound.c`, `dmasound.c` | YM2149 / DMA sound                                    |
| `font.c`, `fnt_*.c` | fixed-width fonts                                         |
| `xhdi.c`      | XHDI (extended hard disk interface)                             |
| `natfeats.c`  | NatFeats hypervisor calls (Hatari/ARAnyM)                       |

Boot/glue:
- `lowstram.c`, `biosmem.c`, `memory2.c` — low-RAM layout, ST-RAM init
- `bootparams.c`, `initinfo.c` — boot info
- `cookie.c` (in `util/`) — cookie jar
- `country.c` — locale defaults

Architecture-specific (NOT relevant for NewTOS but must be skipped cleanly):
- `amiga.c`, `lisa.c`, `coldfire.c` — non-Atari hardware paths
- `nova.c` — Nova graphics card

---

## `vdi/` — VDI

| File                | Purpose                              |
|---------------------|--------------------------------------|
| `vdi_main.c`        | VDI dispatch                         |
| `vdi_control.c`     | v_opnwk / v_clswk / control opcodes  |
| `vdi_line.c`        | line drawing                         |
| `vdi_fill.c`        | polygon/area fill                    |
| `vdi_text.c`        | text rendering                       |
| `vdi_mouse.c`       | mouse cursor                         |
| `vdi_raster.c`      | raster ops, vro_cpyfm                |
| `vdi_marker.c`      | marker primitives                    |
| `vdi_bezier.c`      | Bezier curves                        |
| `vdi_blit.S`, `vdi_tblit.S`, `vdi_tblit_cf.S` | bit-blit asm |
| `vdi_textblit.c`    | text bit-blit                        |
| `vdi_input.c`       | input device                         |
| `vdi_col.c`         | color management                     |
| `vdi_gdp.c`         | generalized drawing primitives       |
| `vdi_misc.c`        | misc                                 |
| `vdi_esc.c`         | escape opcodes                       |
| `vdi_asm.S`         | misc asm helpers                     |
| `vdi_defs.h`        | internal types                       |
| `vdi_inline.h`      | hot-path inlines                     |
| `vdistub.h`         | glue for bios/aes                    |

VDI deferred until D9 (or PHASE-00b).

---

## `aes/` — AES

Layered as `gem*<group>lib.c` files (≈30 modules). Highlights:

| File             | Group                                     |
|------------------|-------------------------------------------|
| `gemaplib.c`     | appl_*                                    |
| `gemevlib.c`     | evnt_*                                    |
| `gemoblib.c`     | objc_*                                    |
| `gemfmlib.c`     | form_*                                    |
| `gemfmalt.c`     | form_alert                                |
| `gemfslib.c`     | fsel_*                                    |
| `gemmnlib.c`     | menu_*                                    |
| `gemwmlib.c`     | wind_*                                    |
| `gemrslib.c`     | rsrc_*                                    |
| `gemgrlib.c`     | graf_*                                    |
| `gemshlib.c`     | shel_*                                    |
| `gemsclib.c`     | scrp_*                                    |
| `gemctrl.c`      | central control loop                     |
| `gemdisp.c`      | dispatcher                                |
| `gemdosif.S`     | GEMDOS-trap binding                       |
| `gemasm.S`       | misc assembly                             |
| `gem.def`, `gem.rsc`, `gem.def` | resources                  |

AES deferred until D8 (or PHASE-00b).

---

## `include/` — Shared Headers

| Header            | Contents                                        |
|-------------------|-------------------------------------------------|
| `portab.h`        | `WORD`/`LONG`/`UBYTE`/... typedefs              |
| `bdosdefs.h`      | `PD` (basepage), `MD`, `OFD`, `DTAINFO`         |
| `bdosbind.h`      | GEMDOS binding inline asm + numeric IDs         |
| `biosdefs.h`      | BPB, OSHEADER                                   |
| `biosbind.h`      | BIOS binding inline asm + numeric IDs (0x0–0xB) |
| `biosext.h`       | extended BIOS prototypes                        |
| `gemdos.h`        | function-name macros (Cconout=`gemdos1(2,c)` …) |
| `gemerror.h`      | error codes (`E_OK`=0, `EFILNF`=-33, ...)       |
| `gemerror.h`      | TOS error code table                            |
| `tosvars.h`       | `_run`, `_bootdev`, `_dskbufp`, `os_header` etc.|
| `lineavars.h`     | Line-A variable layout                          |
| `osheader` ref.   | (in `biosdefs.h`)                               |
| `cookie.h`        | cookie-jar API                                  |
| `obdefs.h`, `aesdefs.h`, `aesext.h` | AES types                  |
| `gsxdefs.h`, `vdiext.h` | VDI types                                 |
| `fonthdr.h`       | font header struct                              |
| `rsdefs.h`        | resource file format                            |
| `scancode.h`      | IKBD scancodes                                  |
| `sysconf.h`, `config.h` | build-time configuration                  |
| `has.h`           | feature-presence flags                          |
| `nls.h`           | translation                                     |
| `setjmp.h`        | setjmp/longjmp prototypes                       |
| `string.h`        | str* / mem* prototypes                          |
| `version.h`       | EMUTOS_VERSION etc.                             |
| `intmath.h`       | divmod, mul                                     |
| `kprint.h`        | debug printf                                    |
| `optimize.h`, `optimopt.h` | hot-path helpers                       |
| `rectfunc.h`      | rectangle operations                            |
| `miscutil.h`      | misc                                            |
| `blitter.h`       | BLiTTER chip                                    |
| `ahdi.h`          | AHDI / hard-disk driver glue                    |
| `asm.h`, `asmdefs.h`, `asmpd.h` | inline-asm helpers, basepage asm offsets |
| `ctrycodes.h`     | country code table                              |
| `doprintf.h`      | doprintf internals                              |

---

## `util/` — Utilities

| File           | Purpose                                |
|----------------|----------------------------------------|
| `doprintf.c`   | shared printf implementation           |
| `string.c`, `stringasm.S`, `memmove.S`, `memset.S` | libc-ish |
| `intmath.c`, `miscasm.S` | integer math helpers         |
| `setjmp.S`     | setjmp / longjmp                       |
| `cookie.c`     | cookie-jar lookup                      |
| `nls.c`        | translation runtime                    |
| `optimize.c`, `optimopt.S` | hot-path helpers           |
| `rectfunc.c`   | rectangle operations                   |
| `gemdos.c`     | C-side GEMDOS bindings (used by AES)   |
| `boot.c`, `bootsect.S`, `bootram.S` | boot helpers      |
| `minicrt.S`    | minimal C runtime                      |
| `lisaboot.S`, `lisautil.S`, `amigaboot.S`, `ramtos.S` | platform glue |

---

## Build configuration knobs (CONF_WITH_*)

`include/config.h` defines build-time switches that change the API
surface or implementation. Affecting the TOS API:

| Knob                       | What it gates                                      |
|----------------------------|----------------------------------------------------|
| `CONF_WITH_ALT_RAM`        | GEMDOS 0x14 (`Maddalt`) availability               |
| `CONF_WITH_VIDEL`          | GEMDOS 0x15 (`Srealloc`) + XBIOS 0x58–0x5F         |
| `CONF_WITH_TT_SHIFTER`     | XBIOS 0x50–0x57 (TT shifter)                       |
| `CONF_WITH_DSP`            | XBIOS 0x60–0x7F (Falcon DSP)                       |
| `CONF_WITH_DMASOUND`       | XBIOS 0x80–0x8D (Falcon DMA sound)                 |
| `CONF_WITH_MFP`            | XBIOS 0x0D, 0x1A, 0x1B, 0x1F (MFP-dependent calls) |
| `CONF_WITH_NVRAM`          | XBIOS 0x2E (`NVMaccess`)                           |
| `CONF_WITH_PRINTER_PORT`   | XBIOS 0x21 (`Setprt`) + parport bcon* paths        |
| `CONF_SERIAL_CONSOLE`      | redirects bconout5 to serial                       |
| `CONF_SERIAL_CONSOLE_ANSI` | enables ANSI escape pre-processing on serial out   |
| `BCONMAP_AVAILABLE`        | XBIOS 0x2C (`Bconmap`) availability                |
| `CONF_ATARI_HARDWARE`      | gates the bulk of XBIOS hardware paths             |

**NewTOS target configuration** (TBD ADR — see `docs/MEMORY.md`): we
will track the **CONF_WITH_TT** baseline plus `CONF_WITH_VT52`, and
disable Falcon-specific (DSP, DMASOUND, VIDEL) and m68k-only
(`CONF_ATARI_HARDWARE`) paths. The XBIOS surface visible to TOS programs
will therefore include the TT shifter additions but not Falcon DSP/sound.

---

## Cross-references and gotchas

- **`gemdos.h`** wraps every GEMDOS call as a CPP macro that expands into
  a `trap #1` invocation with the right function ID. Useful as a sanity
  check on parameter ordering when reading EmuTOS application code.
- **`bdos/bdosstub.h`** declares `_run` (current basepage), `_iorec`, and
  the GEMDOS-side prototypes for `osif`. Read together with `console.c`
  when tracing console state.
- **Hardware-driven globals** are declared `extern` in `tosvars.h` and
  defined in `bios/tosvars.S` (assembly). Many are at fixed memory
  addresses for application compatibility.
- **VT52 state** lives in `lineavars.h` (as part of the Line-A var
  block) — a TOS application may read these directly. Layout-critical.

---

## What this map does NOT cover

- Function-by-function specs (D3–D5) — those live in
  `docs/api/<trap>/<group>/<id>-<name>.md`
- Data-structure layouts (D6) — those live in `docs/api/structures/`
- Subsystem state details (D7) — those live in `docs/api/<trap>/_state.md`

The map only points at where to look. Spec entries do the deep extraction.
