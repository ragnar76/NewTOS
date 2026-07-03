# XBIOS floppy & DMA stub group

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/floppy.c`, `bios/acsi.c`, `bios/xbios.c`.
>
> NewTOS profile (`CLAUDE.md`): no floppy controller, no Atari
> ACSI bus. The functions in this group are exposed for ABI
> compatibility but return `EUNDEV` (-15) immediately.

This group spec covers:

| ID    | Name      | EmuTOS func | Args | Return on NewTOS |
|------:|-----------|-------------|------|------------------|
| 0x08  | Floprd    | `floprd`    | rec, fill, dev, sect, track, side, cnt | `EUNDEV` |
| 0x09  | Flopwr    | `flopwr`    | (same) | `EUNDEV` |
| 0x0A  | Flopfmt   | `flopfmt`   | buf, fill, dev, spt, track, side, interleave, magic, virgin | `EUNDEV` |
| 0x12  | Protobt   | `protobt`   | buf, serialno, disktype, execflag | implemented (PHASE-04) |
| 0x13  | Flopver   | `flopver`   | buf, fill, dev, sect, track, side, cnt | `EUNDEV` |
| 0x29  | Floprate  | `floprate`  | dev, rate | `EUNDEV` |
| 0x2A  | DMAread   | `DMAread`   | sect, cnt, buf, dev | `EUNDEV` (or PHASE-04 SD-card map) |
| 0x2B  | DMAwrite  | `DMAwrite`  | sect, cnt, buf, dev | `EUNDEV` (or PHASE-04 SD-card map) |

## Trap interface (common)

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | per-call (see table) |
| Vector | `xbios_vecs[N]` |

Stack frames follow the EmuTOS / Compendium signatures verbatim
so source-port code links unmodified.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG |

`EUNDEV` (-15) — "device not present". This is the canonical TOS
error for "the requested floppy / DMA hardware is missing or
broken".

`Protobt` is the only function in this group that does meaningful
work on NewTOS — it builds a 512-byte boot-sector image from
serial-number and disk-type parameters; entirely software-side, no
hardware needed.

## Errors

| Code     | Value | When                                       |
|----------|------:|--------------------------------------------|
| `EUNDEV` |   -15 | always (NewTOS profile)                    |

## Per-function notes

### `Floprd` / `Flopwr` / `Flopver` / `Floprate`

Atari floppy I/O on WD1772 + DMA. Hardware not present. NewTOS
returns `EUNDEV` so programs can detect-and-fall-back to alt-paths
(many games attempt floppy, fall back to ROM or HDD).

### `Flopfmt`

Disk formatting. `EUNDEV`. Programs that need to "create a
filesystem" should use `Dcreate`-driven directory creation on an
existing volume, not low-level format.

### `Protobt`

Signature (`include/xbiosbind.h:36`, `xbios_v_llww(18,…)`):
`Protobt(UBYTE *buf, LONG serialno, WORD disktype, WORD execflag)`.
Generates an Atari boot-sector image in `buf`:
- `serialno = -1`: leave existing serial; otherwise stamp.
- `disktype`: 0..3 (40-track SS / DS, 80-track SS / DS) — sets
  BPB fields per layout.
- `execflag`: 0 = non-bootable (clears bytes 0..1); ≠0 = bootable
  (sets `0x60 0x1E` BRA.S to load code at offset 30).

Pure data-shuffling; NewTOS implements verbatim per Compendium
table. Useful for SD-card image preparation tools.

### `DMAread` / `DMAwrite`

ACSI/SCSI raw-sector access by device number. NewTOS may map
device 0..1 onto the SD-card backing store in PHASE-04, returning
real success codes; for now `EUNDEV`. Programs using DMA bypass
filesystem so the spec is straightforward but use cases are rare
on NewTOS targets.

## Edge cases

### 1. Detection idiom

The Compendium-recommended detection sequence is to call
`Floprd(buf, 0, 0, 1, 0, 0, 1)` and check for `EUNDEV`. NewTOS
returns it immediately, so detection is honest.

### 2. Compatibility with `Rwabs`

`Rwabs(0, ...)` (BIOS) goes through `hdv_bpb`/`hdv_rw` vectors and
*will* succeed for SD-card-backed drives. Programs should use
`Rwabs`, not `DMAread`/`Floprd`, for portable raw I/O.

### 3. `Protobt` is the exception

Per-function spec required at PHASE-04 implementation time. Will
be promoted out of this group spec into its own
`docs/api/xbios/12-protobt.md` once PHASE-04 starts.

## Dependencies

- BIOS char-device error subsystem (returns `EUNDEV`).
- (`Protobt`) BPB layout (`docs/api/structures/bpb.md`).

Used by: legacy Atari programs probing for floppy, disk-image
creation tools.

## Native ABI

Shapes below are cited from `include/xbiosbind.h` where a macro exists
(Floprd/Flopwr/Flopfmt/Protobt/Flopver), and otherwise inferred from the
EmuTOS handler signatures (Floprate/DMAread/DMAwrite — no binding macro in
`xbiosbind.h`, so the shape is read from the C prototype).

| ID | Name | Shape | Source |
|---:|------|-------|--------|
| 0x08 | Floprd | buf:L, fill:L, dev/sect/track/side/cnt:W → WORD | `xbios_w_llwwwww(8,…)` (`xbiosbind.h:27`) |
| 0x09 | Flopwr | (same) → WORD | `xbios_w_llwwwww(9,…)` (`xbiosbind.h:28`) |
| 0x0A | Flopfmt | buf:L, skew:L, dev/spt/track/side/interlv:W, magic:L, virgin:W → WORD | `xbios_w_llwwwwwlw(10,…)` (`xbiosbind.h:29`) |
| 0x12 | Protobt | buf:L, serialno:L, disktype:W, execflag:W (void) | `xbios_v_llww(18,…)` (`xbiosbind.h:36`) |
| 0x13 | Flopver | buf:L, fill:L, dev/sect/track/side/cnt:W → WORD | `xbios_w_llwwwww(19,…)` (`xbiosbind.h:37`) |
| 0x29 | Floprate | dev:W, rate:W → LONG | handler `LONG floprate(WORD,WORD)` |
| 0x2A | DMAread | sect:L, cnt:W, buf:L, dev:W → LONG | handler `LONG DMAread(LONG,WORD,UBYTE*,WORD)` |
| 0x2B | DMAwrite | sect:L, cnt:W, buf:L, dev:W → LONG | handler `LONG DMAwrite(LONG,WORD,const UBYTE*,WORD)` |

## Cross-references

- **Compendium / Profibuch:** not present in `references/`; cross-check
  deferred. (Historically documents all eight signatures + error codes; the
  `EUNDEV` return for a missing controller is EmuTOS/driver behaviour.)
- `docs/api/bios/04-rwabs.md` — preferred portable raw-I/O entry.

## Configuration

- All EmuTOS floppy/DMA-controlling configs disabled in NewTOS.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-04 plan: `Protobt` implemented as pure software; DMAread/
DMAwrite optionally map to SD-card backing for compatibility.
The remaining six remain `EUNDEV` stubs.
