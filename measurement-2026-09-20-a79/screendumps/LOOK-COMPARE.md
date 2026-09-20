# A79 — the two frame looks, side by side (QEMU raspi4b, 640x480, 2026-09-20)

Two scratch kernels that differ in **one line** — the initialiser of
`tos_api::aes::window::FRAME_LOOK` (`method/scratch-lit-vs-gray.diff`):

| kernel | `FRAME_LOOK` default | sha256 (kernel8.img, scratch, dev profile) |
|---|---|---|
| `gray` | `FrameLook::Tos4Gray` — today's look for every gadget whose stored word equals its compiled default | `a9d7f2347d791fb2…` |
| `lit`  | `FrameLook::Literal` — every gadget painted from its colour word as EmuTOS `just_draw` paints the `W_ACTIVE` tree | `934263596a72e128…` |

Same SD image, same injector script, same marks for both. The pointer is
parked bottom-right (639,479) before every dump. `pixdiff.py` results are in
`../RUNINFO.TXT`.

## Pair 1 — the kernel desktop with two windows (sd-base, no `#Z`)

Mark `B_TWO_WINDOWS`: the boot-time `C:\` window (untopped, behind) and a
second `C:\` window opened by a double-click on the drive icon (topped,
staggered by 24 px). Both windows carry every gadget incl. `INFO`; all colour
words are the compiled defaults.

| Tos4Gray (today) | Literal |
|---|---|
| `desk-gray-B-two.png` | `desk-lit-B-two.png` |

Side by side: `pair-desk-two.png` (left gray, right literal). Zooms (8x /
4x): `zoom-gray-topleft.png` / `zoom-lit-topleft.png` (closer, name band,
info band of the topped window over the title band of the untopped one),
`zoom-gray-botright.png` / `zoom-lit-botright.png` (down arrow, sizer corner,
right arrow, the shared bar borders).

What differs (the ✗ rows of the design table): gadget faces (closer, fuller,
arrows, sizer, elevators) white with a 1 px black border on the 3D-grown slot
and the bevel inside it instead of grey face without border; info band white
with a black box instead of grey; scroll-bar columns/bands white with black
box borders instead of grey; slider tracks `IP_1PATT` black dither instead of
solid grey. What is the same (the ✓ rows): outer border, title bottom row,
topped-name `IP_2PATT` stipple, black glyphs, drop shadow (`DROP_SHADOW_COLOR`
stays a documented look decision in both).

Also `desk-*-A-idle.png` (one window, before the double-click) and
`desk-*-C-two-2s.png` (2 s later, byte-identical to B in both looks).

## Pair 2 — the probe window with default words (sd-probe, `#Z A79COLOR.NAP`)

`sdk/examples/a79color.c`: one window `(30,40,320,240)`, kind `0x0FEF`, name
`A79 T7`, default colour words. Mark `A_DEFAULT` (+5.5 s).

| Tos4Gray (today) | Literal |
|---|---|
| `probe-gray-A-default.png` | `probe-lit-A-default.png` |

Side by side: `pair-probe-default.png`; zooms `zoom-probe-gray-A-default-tl.png`
/ `zoom-probe-lit-A-default-tl.png` (closer + start of the name band).

## Pair 3 — the same window after T7 (`wind_set(WF_COLOR, W_NAME, 0x1172, 0x1172)`) and `wind_set(WF_COLOR, W_VSLIDE, 0x1171, 0x1171)`

Mark `B_T7` (+11 s; the probe applies both words at +7 s). A changed word is
rendered literally under **both** looks, so the name band and the slider track
are the same in the pair; the other gadgets still show the look's default.

| Tos4Gray (today) | Literal |
|---|---|
| `probe-gray-B-t7.png` | `probe-lit-B-t7.png` |

Side by side: `pair-probe-t7.png`; zooms `zoom-probe-gray-B-t7-tl.png` /
`zoom-probe-lit-B-t7-tl.png`. Expected per spec § (4): name band interior
100 % colour 2 (RED), 1 px black border, black `A79 T7` glyphs in transparent
mode (red between the strokes, no white glyph cells); vertical slider track
solid black outside the elevator. Before this change the band showed the
stipple and the track the grey (0 pixels changed by T7).

`probe-*-C-t7-2s.png` (2 s later) are byte-identical to B in both looks.
