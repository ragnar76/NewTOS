# NewTOS on real hardware vs EmuTOS on a TT — round 2

First run of `DUMPVDI.NAP` on a Raspberry Pi 4. All 120 cases ran; the dump
validates. **27 of 120 cases differ.**

    NewTOS  DEV_TAB 1919,1079,...,256,...   1920x1080, 256 colours
    TT      DEV_TAB  639, 479,..., 16,...    640x480,   16 colours
    both    SIZ_TAB 5,4,7,13,1,0,40,0,15,11,120,88

`SIZ_TAB` is **identical** on both machines — the ring fold fixed in `8b46b8f`
holds on real hardware, not just in the harness.

## Why this run mattered: it saw what no host test can

The Rust suite compares the same `tos-api` code against the same reference and
reports **10** round-2 failures. The hardware run reports 27. The 17 extra are
not noise: **16 of them differ in `d0` alone**, and `d0` is structurally
invisible to the host harness, because `tos_api::vdi::dispatch` returns `()` —
the value does not exist through that entry point. The harness says so itself.

## R-HW-1 — `d0` (`flip_y`) is never produced

    opcodes 12, 19, 21, 107, 116, 117, 131
    EmuTOS returns 1, NewTOS returns 0, in all sixteen cases

That is exactly the set `docs/api/vdi/_working-list.md` **D-080** names:
`vsm_height` (19), `vst_height` (12), `vst_point` (107) and — as a leaked side
effect — `vst_font` (21) set the global `flip_y`, plus the inquiries that
return magnitudes: `vqt_extent` (116), `vqt_width` (117), `vqt_fontinfo` (131).

`docs/api/vdi/_dispatcher.md` carries `d0 = flip_y` as part of the native trap
contract (`vdi/vdi_asm.S:124-125`). D-080 logged this as an **open question** —
"whether NewTOS's selector-based ABI needs an equivalent signal at all". It is
now answered by measurement rather than by reasoning: EmuTOS sets it for these
seven opcodes and NewTOS's GEM trap always leaves 0.

Whether to produce it is a decision, and it is not obviously yes: an
application that reads `d0` to tell a coordinate from a magnitude gets the
wrong answer on NewTOS today, but nothing in the tree reads it, and gemlib's
own trap primitive discards it.

What is *not* open: the measurement. This is the one finding of the whole
exercise that only the real trap path could produce.

## R-HW-2 — `vq_chcells` is pure geometry, and both are right

    case 155   intout[0] rows     TT 30    NewTOS 67
               intout[1] columns  TT 80    NewTOS 240

480/16 = 30 rows and 640/8 = 80 columns; 1080/16 = 67 and 1920/8 = 240. Both
correct for their screen. This is the class of difference that genuinely is
computable from the resolution, and here the arithmetic closes exactly.

## Everything else was already known

The remaining ten were all already red in the Rust suite and are already
classified: the colour-count profile (46, 89), the AES-versus-no-AES artefact
(51), `vqt_name`'s `intout[33]` (54-57, R2-7), `v_get_pixel` in different
colour modes (140, 141) and `v_dspcur` writing `INTIN[0]` (169, R2-6).

**Nothing new appeared in the drawing primitives** — but that proves less than
it looks. Those cases write nothing back into the parameter block, so a match
means the block survived, not that a pixel landed correctly. See
`FINDINGS-r3.md`.

## Files

| File | What |
|---|---|
| `NEWTOS-VDI-r2.BIN` / `.LOG` | NewTOS on a Pi 4, 1920x1080, 256 colours |
| `EMUTOS14-VDI-r2.BIN` / `.LOG` | EmuTOS 1.4 on a TT, 640x480, 16 colours |

    host/build/tosdiff cases/vdi-r2.tsv \
        reference/EMUTOS14-VDI-r2.BIN reference/NEWTOS-VDI-r2.BIN
