# NewTOS vs EmuTOS — the complete run

One table, both machines, same 182 cases.

    cases/vdi-all.tsv   182 cases, 34 preludes, CRC D135ADAF
    EMUTOS14-VDI-all    Atari TT, EmuTOS 1.4,  640x480,   16 colours
    NEWTOS-VDI-all      Raspberry Pi 4,       1920x1080, 256 colours

Both dumps validate. **54 of 182 cases differ**, and every one of them falls
into a class that is already understood. **Nothing unclassified remains.**

## Where the 54 come from

| Cause | Cases | Status |
|---|---:|---|
| `d0` / `flip_y` never produced | **39** | measured here for the first time |
| Screen geometry | 6 | correct on both machines |
| Colour profile (256/16 planes vs 16/4) | 6 | deliberate |
| `MAX_VERTICES` 512 vs 1024 | 3 | open decision (R3-3) |
| `v_get_pixel` colour model | 5 | different modes, no oracle possible |
| `vqt_name` writes past `nintout` | 4 | EmuTOS defect (R2-7), parked |
| `vqin_mode` string device | 1 | AES ran on the reference, not here |
| `v_dspcur` writes `INTIN[0]` | 1 | EmuTOS defect (R2-6), parked |

(Several cases appear under more than one cause; 54 is the count of distinct
cases.)

## R-ALL-1 — `d0` (`flip_y`) accounts for 39 of the 54

Opcodes **12, 19, 21, 36, 38, 100, 102, 107, 116, 117, 131**. EmuTOS returns
1, NewTOS returns 0, without exception.

The full run widens the set found in round 2 by three: `vqm_attributes` (36),
`vqt_attributes` (38) and `v_opnvwk` (100) also set it. All of them return
magnitudes or open a workstation, which is exactly when EmuTOS raises
`flip_y` (`vdi/vdi_asm.S:124-125`, and `docs/api/vdi/_working-list.md`
**D-080** for the four attribute setters).

**No host test can see this.** `tos_api::vdi::dispatch` returns `()`; the
value does not exist through that entry point. Only the real trap produces it,
which is why three rounds of `cargo test` never mentioned it.

D-080 had this as an open question — "whether NewTOS's selector-based ABI
needs an equivalent signal at all". It is now measured rather than reasoned:
EmuTOS sets it for eleven opcodes and NewTOS's GEM trap always leaves 0.
`docs/api/vdi/_dispatcher.md` carries `d0 = flip_y` as **contract**.

## Everything else, and why it is not a defect

**Geometry** (cases 33, 34, 173, 250, 252, 274). `DEV_TAB[0..1]` 639/479
against 1919/1079; clip bounds likewise; `vq_chcells` 30x80 against 67x240,
which is 480/16 by 640/8 against 1080/16 by 1920/8. Correct on both.

**Colour profile** (33, 34, 42, 72, 113, 250, 252, 274). `DEV_TAB[13]` 16
against 256, `INQ_TAB[4]` 4 planes against 16, `INQ_TAB[5]` 1 against 0,
`DEV_TAB[39]` 4096 against 0. All follow from the Falcon truecolor target
profile. The two colour-clamp cases belong here too: index 99 is invalid on a
16-colour TT and valid on a 256-colour NewTOS, so echoing 1 and echoing 99 are
both right.

**`v_get_pixel`** (54, 160, 161, 270, 272). The TT is in palette mode, NewTOS
in truecolor, so `INTOUT[0]` is a PEL there and 0 here by contract
(`vdi_fill.c:1104-1113`). These cases cannot pass and are detectors, not
oracles — and they detected a real bug earlier, which is now fixed.

## What this run does not tell us

46 of the 182 cases write nothing into the parameter block. For the no-ops
that is the whole contract, but for the drawing primitives — `v_pline`,
`v_fillarea`, `v_gtext`, `v_bar`, the GDP sub-ops, `v_contourfill` — a match
means the block survived, not that a pixel landed correctly. Drawing
correctness needs screendumps or prelude-paired `v_get_pixel` probes.

So: **the parameter-block behaviour of the VDI is now measured end to end on
both machines, and the only unexplained difference is `d0`.** What the drawing
actually puts on screen is a separate question this format cannot answer.

## Reproducing

    host/build/tosdiff cases/vdi-all.tsv \
        reference/EMUTOS14-VDI-all.BIN reference/NEWTOS-VDI-all.BIN
