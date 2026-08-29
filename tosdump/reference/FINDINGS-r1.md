# Round 1 findings — EmuTOS 1.4 vs TOS 3.06 on the same TT

Measured 2026-08-29 on one Atari TT, both runs in **640×480, 16 colours**
(`DEV_TAB[0..1] = 639,479`, `DEV_TAB[13] = 16` in both logs), case table
`cases/vdi-r1.tsv`, CRC `FFF2281E`, 24 cases. Both dumps validate and both logs
say `all 24 cases ran`.

Because the machine is held constant, **every difference below is EmuTOS
against Atari TOS, with no device component.** That is the one question reading
the EmuTOS source cannot answer.

## Is the 1.4 release usable as the reference?

Yes, for the VDI. The build that was flashed is EmuTOS **1.4 release**, not the
pinned `1a7484ab` — but between `VERSION_1_4` and `1a7484ab` (71 commits) there
are **zero** commits touching `vdi/`, `bios/font.c`, `bios/lineavars.S`,
`bios/linea.c`, `bios/lineainit.c` or `aes/gsx2.c`. The only `include/config.h`
changes in that range are hard-disk options (GPT, external disk driver, SCSI,
XHDI). The VDI these numbers came from is source-identical to the VDI the
specification in `docs/api/` was extracted from.

## What was noise, and whose fault it was

The first comparison showed 19 of 24 cases differing, all on `d0`. That was a
defect in **this project's field descriptor**, not in either TOS.

The VDI returns `flip_y` in `d0` as a **WORD**. EmuTOS clears the register
first — `vdi/vdi_asm.S:124-125`, `moveq #0,d0` then `move.w _flip_y,d0`. TOS
3.06 does not clear the high half, so it carries whatever the call path left
there: `0x0111` after every inquiry and setter, `0xFFFF` after `v_pline` and
`v_fillarea`, perfectly consistent per opcode. **The low half — the half the
contract defines — agrees in all 24 cases.**

`d0` had been recorded and compared as an `i32`. It is now compared as the WORD
it is; the full long stays in the record as evidence. The case was right, the
field width was wrong.

## The three real findings

### F1 — `vqt_attributes` (38) `INTOUT[5]`: EmuTOS 1, TOS 3.06 0

EmuTOS returns `vwk->wrt_mode + 1` (`vdi/vdi_text.c:1011`), i.e. the 1-based
`MD_*` code. After the reset the mode is replace, so EmuTOS returns 1 and TOS
3.06 returns 0.

**Not yet explained, and worth one more case before it is.** The other three
attribute inquiries return the same quantity — `vql_attributes` `INTOUT[2]`,
`vqm_attributes` `INTOUT[2]`, `vqf_attributes` — and those **agree** between
the two systems in this run. So TOS 3.06 is not uniformly reporting the mode
0-based; only `vqt_attributes` differs.

Round 2 must settle it: set `vswr_mode` to 3 and read all four inquiries back.
If TOS then returns 2 where EmuTOS returns 3, it is an off-by-one confined to
`vqt_attributes`. If it returns something unrelated, `INTOUT[5]` holds a
different quantity on TOS than EmuTOS believes.

This is the same family as `D-VDI-11`, where EmuTOS deliberately reproduces a
TOS off-by-one in `vqin_mode` — except here EmuTOS does *not* reproduce it.

### F2 — the TT has a fourth, much larger system font

`vq_extnd(0)`, which returns `DEV_TAB` in `INTOUT` and `SIZ_TAB` in `PTSOUT`:

| Field | Meaning | EmuTOS 1.4 | TOS 3.06 |
|---|---|---|---|
| `INTOUT[5]` = `DEV_TAB[5]` | number of character heights | 3 | **4** |
| `PTSOUT[2]` = `SIZ_TAB[2]` | maximum character width | 7 | **16** |
| `PTSOUT[3]` = `SIZ_TAB[3]` | maximum character height | 13 | **26** |

The same numbers appear in both run logs' `SIZ_TAB` lines, so this is a
property of the font ring, not of one call. TT TOS 3.06 offers four system
fonts in this resolution, the largest up to 16×26; EmuTOS 1.4 offers three, the
largest 7×13.

Consequence beyond the inquiry: anything that selects a font by size —
`vst_height`, `vst_point`, `vqt_extent`, and the AES text that sits on them —
resolves differently. This is a capability gap, not a formatting detail.

### F3 — three `INQ_TAB` constants EmuTOS guessed wrong

`vq_extnd(1)` returns `INQ_TAB`. EmuTOS builds it from a hardcoded template,
`vdi/vdi_control.c:76-123`:

| Field | EmuTOS's own comment | EmuTOS 1.4 | TOS 3.06 |
|---|---|---|---|
| `INTOUT[1]` | `number of background colors` | 1 | **4096** |
| `INTOUT[6]` | `performance factor????` | 50 | **5000** |
| `INTOUT[16]` | `number of buttons on MOUSE` | 1 | **2** |

`INTOUT[16]` is plainly wrong in EmuTOS: the Atari mouse has two buttons, and
TOS 3.06 says 2.

`INTOUT[6]` is the clearest vindication of the whole exercise. EmuTOS's source
comment is literally `performance factor????` — the value is an admitted guess.
No amount of reading that source produces 5000. Only measuring does.

## What NewTOS should be held to

`docs/api/` was extracted from this EmuTOS, so NewTOS inherits F1, F2 and F3
unless a decision is made otherwise. Each needs one:

- **F3** — no reason to keep EmuTOS's guesses. NewTOS should report the
  measured Atari values, with these dumps as the citation. `INTOUT[16] = 2` in
  particular.
- **F2** — a font-ring question, not a VDI-logic one. It decides what
  `vst_height` can resolve to and therefore how text metrics compare for the
  rest of the project.
- **F1** — do not decide before round 2 pins down the cause.

Following `docs/MEMORY.md`'s rule against rationalising EmuTOS deviations: none
of these is "benign". They are three places where the specification this project
implements against is measurably not Atari TOS.

## Files

| File | What |
|---|---|
| `EMUTOS14-VDI.BIN` / `.LOG` | EmuTOS 1.4, TT, 640×480×16 |
| `TOS306-VDI.BIN` / `.LOG` | TOS 3.06, same TT, same mode |

Reproduce with:

    host/build/tosdiff cases/vdi-r1.tsv \
        reference/EMUTOS14-VDI.BIN reference/TOS306-VDI.BIN
