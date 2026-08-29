# Round 2 — NewTOS against the EmuTOS reference

`cargo test -p tos-api --test vdi_reference` → **107 of 120 pass, 13 fail.**

The reference is `EMUTOS14-VDI-r2.BIN`, measured on an Atari TT running
EmuTOS in 640×480×16, case table `cases/vdi-r2.tsv` (CRC `A0706A6D`, 120
cases). `docs/api/` was extracted from that same EmuTOS source, so a
difference here has one reading: NewTOS is wrong — unless it is one of the
two deliberate profile deviations named below.

## What the suite does and does not cover

The dump records the five parameter-block arrays, not pixels, so these tests
exercise everything **above** the `RasterBackend` seam: dispatch, `Vwk`
attribute state, validation and clamping, clipping, the inquiries. Whether
the rasterizer puts the right pixels down needs a pixel comparison the record
format does not carry.

The harness backend is a plain memory buffer at the reference geometry, not
the kernel's rasterizer (which is pinned to AArch64 and reaches the board
framebuffer through the HAL). These tests say nothing about the kernel's
rasterizer.

## One failure was the harness's own fault

`vq_color` on pen 0 reported `(0,0,0)` where every real machine reports
white. The memory backend was not seeding the default palette; the kernel's
backend seeds pens 0..15 from EmuTOS' `videl_palette1`. Fixed in the harness,
and named here because a harness artefact counted as a finding would be worse
than no finding at all.

## Genuine defects

### R2-1 — `vsm_color` (20) does not clamp

    case 89   intout[0]   NewTOS = 99   reference = 1

Every VDI attribute setter validates its argument and echoes **the value it
actually accepted**, not the request (`docs/api/vdi/_state.md`). Asked for
colour index 99 on a workstation that has no such pen, EmuTOS clamps and
echoes 1; NewTOS echoes 99 back. An application reading the echo to learn
what it got is misled.

Note the shape of this: `vsl_color` (17), `vsf_color` (25), `vst_color` (22)
and `vsm_type` (18) all pass their out-of-range cases. Only `vsm_color` is
wrong, so this is a single missing validation, not a systematic one.

### R2-2 — `vqt_width` (117) writes output slots it must not touch

    case 52   ptsout[1], ptsout[3], ptsout[5]
              NewTOS = 0   reference = 0xDEAD (untouched)

The reference left those three words at the poison pattern: EmuTOS writes
only the x components and never touches the y ones. NewTOS zeroes them.
Invisible without poisoning the arrays first, which is exactly why the dumper
does.

### R2-3 — `vqt_name` (130) appears to ignore its font index

    cases 54, 55, 56, 57 (indices 0..3) all produce the same NewTOS output

EmuTOS returns a different name per index. NewTOS returns one name for every
index, and it is the name of a different font besides (see R2-4).

### R2-4 — the default font differs

    case 52   ptsout[0]   NewTOS = 8    reference = 6     (width of 'A')
    case 58   ptsout[0]   NewTOS = 8    reference = 6
              ptsout[5]   NewTOS = 8    reference = 3
              ptsout[9]   NewTOS = 13   reference = 4
    case 54   the name reads "8x16 system font" vs "6x6 system font"

NewTOS opens a workstation whose default font is 8×16; EmuTOS on this device
opens one whose default is 6×6. Everything downstream of font choice —
`vqt_width`, `vqt_extent`, `vqt_fontinfo`, `vqt_name`, and the AES text that
sits on them — differs as a consequence. This is the NewTOS-side counterpart
of round 1's F2 (the TT has four system fonts to EmuTOS's three).

Whether to match EmuTOS here is a decision, not a bug report: it changes text
metrics across the whole system.

### R2-5 — `vqin_mode` (115) on the string device

    case 51   intout[0]   NewTOS = 0   reference = 1

`vqin_mode` returns the stored mode minus one (`D-VDI-11`, an off-by-one
EmuTOS reproduces from TOS deliberately). The reset does not set the string
device's mode, so this compares two initial values, and they differ.

## Reference-side observations, not NewTOS defects

### R2-6 — EmuTOS writes `INTIN[0]` on `v_dspcur` (escape 18)

    case 169   intin[0]   NewTOS = 0xDEAD (untouched)   reference = 0

The case passes no `intin` at all, and EmuTOS wrote a zero into it — it
modified its caller's input array. NewTOS correctly leaves it alone.

Under the project's 1:1-EmuTOS rule this still needs a decision, because an
application could in principle observe it. Recorded rather than resolved.

### R2-7 — `vqt_name` writes one word past its declared count

    case 54   intout[33]   NewTOS = 0xDEAD (untouched)   reference = 0

The jump table declares `nintout = 33` for opcode 130 (`vdi_main.c`), so
`INTOUT[0..32]`. The reference has `INTOUT[33]` written as well.

## Deliberate profile difference

### R2-8 — colour count

    case 46   vq_color pen 255   NewTOS = 255   reference = -1

NewTOS reports a Falcon truecolor workstation: 256 colours over 16 planes
(`kernel/src/vdi_backend.rs`). The TT reports 16 over 4, so pen 255 is
invalid there and EmuTOS returns −1. This follows from the chosen target
profile (`docs/MEMORY.md`, Falcon030/TOS4) and is not a defect. Any case
whose result depends on the colour count will differ for the same reason.

## Reproducing

    cargo test -p tos-api --test vdi_reference -- --test-threads=1

Regenerate the cases after a new measurement:

    tosdump/host/build/mkrusttests \
        tosdump/cases/vdi-r2.tsv \
        tosdump/reference/EMUTOS14-VDI-r2.BIN \
        ../../../tosdump/reference/EMUTOS14-VDI-r2.BIN \
        tos-api/tests/vdi_reference/cases_r2.rs
