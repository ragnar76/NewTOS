# Round 2 — NewTOS against the EmuTOS reference

`cargo test -p tos-api --test vdi_reference` → **112 of 120 pass, 8 fail.**

*(First run: 107/13. The five that closed are recorded under each finding.
Every genuine NewTOS defect this round found is fixed; the eight that remain
are two deliberate profile differences, one harness artefact and five
reference-side observations, each named below.)*

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

> **Corrected 2026-08-29 after implementation.** Three of the entries below
> were wrong when first written, and the corrections are kept in place rather
> than edited away, because how the error happened matters more than the
> tidiness of the record: in each case a claim was made about the reference
> without reading the reference bytes or the EmuTOS source that produced them.
> That is the exact failure this whole exercise exists to prevent. See the
> **Withdrawn** section.

## Genuine defects

### R2-1 — WITHDRAWN, see below

### R2-1 (as originally written) — `vsm_color` (20) does not clamp

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

### R2-3 — `vqt_name` (130) does not walk the font ring — FIXED

    cases 54..57 (indices 0..3): 16 differing fields, now 1

**The premise as first written was wrong** and is corrected here: the claim
was "EmuTOS returns a different name per index". It does not. Records 19..22
of `EMUTOS14-VDI-r2.BIN` are byte-identical, all reporting `font_id = 1` and
`"6x6 system font"`, and that follows necessarily from the source —
`vdi_vqt_name` counts *distinct* `font_id`s, and all three built-in faces
carry `font_id = 1` (`bios/fnt_st_6x6.c:91`, `fnt_st_8x8.c:147`,
`fnt_st_8x16.c:275`), so element 1 is `fon6x6` and every other index falls
through the `if (!found)` fallback onto the same font. The four indices
*cannot* differ without deviating from EmuTOS.

The dump that disproves it was in hand when the claim was made. It was not
read.

There was a real defect underneath, though a different one: NewTOS returned a
hardcoded font id and name instead of walking the ring at all. Replaced with
the 1:1 ring walk (`vdi/vdi_text.c:1100-1134`, fallback `:1125-1126`), which
also closes **AUD-V1-011**. Cases 54..57 now differ only in `intout[33]`
(= R2-7).

### R2-4 — CORRECTED: the text inquiries ignore `cur_font`

    case 52   ptsout[0]   NewTOS = 8    reference = 6     (width of 'A')
    case 58   ptsout[0]   NewTOS = 8    reference = 6
              ptsout[5]   NewTOS = 8    reference = 3
              ptsout[9]   NewTOS = 13   reference = 4
    case 54   the name reads "8x16 system font" vs "6x6 system font"

**The mechanism as first written was wrong.** The claim was "NewTOS opens a
workstation whose default font is 8×16; EmuTOS opens one whose default is
6×6". Both open with 8×16: `text_init` picks the default by
`cell_height = (V_REZ_VT >= 400) ? 16 : 8` (`vdi/vdi_text.c:612`), and at 480
lines that is the 16-tall face. The reference reports 6×6 because **our own
reset sequence asks for it** — `td_reset_vdi` issues
`vst_height(0, SIZ_TAB[1])`, and `SIZ_TAB[1]` on this device is 4
(`EMUTOS14-VDI-r2.LOG`), so EmuTOS switches `cur_font` to the smallest face
it has.

So this is not a default-font question at all, and NewTOS is not missing the
small face — `FON_6X6` is present in `tos-api/src/vdi/font.rs`. The real
defect is that `vqt_width`, `vqt_extent` and `vqt_fontinfo` read the 8×16
module constants instead of `vwk.cur_font`, so they report the same metrics
whatever font the workstation actually selected. That is a plain conformance
bug and is fixable without any policy decision.

Fixing the inquiries to read `cur_font` is not a policy decision; it is
required either way.

**Fixed (`8b46b8f`), and it was three defects, not one.** The inquiries did
not read `cur_font` at all. `vst_height` was already correct — it walks the
ring and returns `FON_6X6` for a requested height of 4 — but was being asked
the wrong question, because `workstation.rs` folded `SIZ_TAB[0..3]` over a
one-element list holding only the 8×16 metrics and so reported `{7,13,7,13}`
where the reference reports `{5,4,7,13}`. The reset therefore asked for
height 13, not 4. Fixing the inquiries alone would have changed nothing.
Cases 52, 53, 58, 59 and 60 now pass.

**The deferred decision was posed on the wrong premise.** It was put to the
user as "should NewTOS adopt EmuTOS's small default font", and the answer was
no, with a sound reason: NewTOS targets a Full-HD framebuffer, and a 6×6 face
that is legible on a 640×480 TT screen is not legible at 1920×1080.

That reasoning stands, but the question does not arise. Both systems open at
8×16, so the desktop default was never at risk. What the measurement actually
shows is an application explicitly asking for a 4-pixel height and being given
the nearest face — which is what `vst_height` is for, and which no Full-HD
argument bears on.

What that leaves open is *how* to express it. The options are not equivalent
and none is chosen yet: keep NewTOS's 8×16 default and accept that these
inquiries diverge from the reference for a documented reason; ship EmuTOS's
font ring and select a larger member as the default, which keeps
`vqt_name`/`vqt_fontinfo` conformant while the default differs; or scale.
Until it is decided, the tests that depend on font metrics stay red on
purpose — they are recording a known, understood divergence, not an unnoticed
one.

Note that the reference was measured at 640×480 because that is what the TT
offers. A NewTOS dump at Full HD would differ in every resolution-dependent
field regardless of fonts, so this decision is entangled with what geometry
future reference rounds are taken at.

### R2-5 — WITHDRAWN: not a defect, a measurement artefact

    case 51   intout[0]   NewTOS = 0   reference = 1

EmuTOS initialises **all four** input modes to 0 (`vdi/vdi_mouse.c:575-578`),
so two initial values would both read 0. The reference's 1 is not an initial
value: the AES's keyboard poll calls `gsx_char()`, which issues
`vsin_mode(device 4, mode 2)` before any application runs
(`aes/gemgsxif.c:459-461`), and EmuTOS stores `mode - 1` = 1. `loc_mode`
stays 0 because its only AES writer runs under `appl_tplay` — which is
exactly the asymmetry the dump shows (cases 49/50 = 0, case 51 = 1).

The dumper ran as a GEM application under a live AES; the Rust harness has no
AES. NewTOS reports the untouched 0 correctly. Initialising `str_mode = 1`
would contradict `vdi_mouse.c:578`.

**Open question for the AES, not the VDI:** does NewTOS's AES keyboard poll go
through `vsin_mode(4,2)` + `v_string` the way `chkkbd`/`gsx_char` does? If it
does, the real system agrees with the reference and only the AES-less harness
differs.

## Withdrawn

**R2-1 (`vsm_color`) is not a defect.** `attributes::vsm_color` already calls
`validate_color_index`, matching `vdi/vdi_marker.c:72-79`. The difference is
`numcolors`, not the validation: `DEV_TAB[13] = (v_planes<8) ? (1<<v_planes)
: 256` (`vdi/vdi_control.c:206`), and the reference log reports 16 on the TT
against NewTOS's 256, so index 99 is invalid there and valid here. This is
**R2-8**, the deliberate profile difference, which this document had already
warned would produce exactly such cases.

The supporting argument was also unfounded. It read "`vsl_color` (17),
`vsf_color` (25), `vst_color` (22) all pass their out-of-range cases, so this
is one missing validation". `cases/vdi-r2.tsv` contains **no** out-of-range
case for 17, 25 or 22 — the only one in the table is line 89. A pattern was
asserted from tests that do not exist.

**R2-5 is not a defect** — see above.

## Reference-side observations, not NewTOS defects

### R2-6 — `v_dspcur` (escape 18) clobbers the caller's `INTIN[0]`

    case 169   intin[0]   NewTOS = 0xDEAD (untouched)   reference = 0

**OPEN — decision deferred to a later pass (2026-08-30).**

The case passes no `intin` at all, and the reference came back with a zero in
it: EmuTOS wrote into its caller's input array.

Not a quirk worth preserving. `vdi/vdi_esc.c:274-278`:

```c
static void escfn18(Vwk * vwk)
{
    INTIN[0] = 0;           /* show regardless */
    vdi_v_show_c(vwk);      /* display the graphics cursor */
}
```

It sets `INTIN[0]` as an argument for `v_show_c` — and `vdi_v_show_c` does not
read it. It calls `linea_show_mouse()` and nothing else
(`vdi/vdi_mouse.c:321-324`). Historically `v_show_c` took a reset flag; this
implementation dropped it and the assignment stayed behind. The only surviving
effect is the clobber.

### R2-7 — `vqt_name` (130) writes one word past its declared count

    case 54..57   intout[33]   NewTOS = 0xDEAD (untouched)   reference = 0

**OPEN — decision deferred to a later pass (2026-08-30).**

An off-by-one, not a convention. `vdi/vdi_text.c:1128-1133`:

```c
*int_out++ = tmp_font->font_id;                        /* INTOUT[0] */
for (i = 0, name = tmp_font->name; (*int_out++ = *name++); i++)
    ;
while (i++ < FONT_NAME_LEN) *int_out++ = 0;
```

The copy loop writes each character **including the terminating NUL** — the
assignment happens before the test — but counts only the non-NUL characters in
`i`. For a name of length L that is `1 + (L+1) + (FONT_NAME_LEN - L)` = **34**
words with `FONT_NAME_LEN = 32` (`include/fonthdr.h:28`), i.e. `INTOUT[0..33]`.
The jump table declares `nintout = 33` for opcode 130 (`vdi/vdi_main.c:105`),
i.e. `INTOUT[0..32]`.

It survived because real GEM applications size `intout` generously — gemlib
uses fixed maxima and the VDI arrays are conventionally 128 words. An
application that sized `INTOUT` at exactly 33 words would have the word after
it overwritten.

### What the decision on R2-6 and R2-7 actually is

Both are EmuTOS writing outside its own contract, into memory the caller owns.
Reproducing them means:

- **R2-6** — adding an assignment whose only effect is destroying one of the
  caller's input words.
- **R2-7** — deliberately writing one word past a declared output count, i.e.
  overrunning a caller's buffer.

**Recommendation on record, decision not taken:** reproduce neither; keep both
documented here with the citations, and leave the five tests (54–57, 169) red
so they stand as a permanent marker of a known and understood divergence.

This is deliberately *not* filed under `docs/MEMORY.md`'s rule against
rationalising EmuTOS deviations, and the distinction should be checked when
the decision is made rather than assumed. That rule guards behaviour an
application can rely on. Neither of these is behaviour: R2-6 is a dead write,
and R2-7 is a question of whether NewTOS writes past a buffer boundary it
declared. If the answer is nonetheless "match EmuTOS exactly", each is a
one-line change.

A third option exists and was deliberately not built: `mkrusttests` could emit
`#[ignore = "reason"]` for a documented exception list, which would leave the
suite green with the divergence still visible. It costs the property that
every red test is an open question, which is worth more right now than a green
count.

## Deliberate profile difference

### R2-8 — colour count

    case 46   vq_color pen 255   NewTOS = 255   reference = -1

NewTOS reports a Falcon truecolor workstation: 256 colours over 16 planes
(`kernel/src/vdi_backend.rs`). The TT reports 16 over 4, so pen 255 is
invalid there and EmuTOS returns −1. This follows from the chosen target
profile (`docs/MEMORY.md`, Falcon030/TOS4) and is not a defect. Any case
whose result depends on the colour count will differ for the same reason.

## What the suite does not cover

Worth stating, because a green suite invites the wrong conclusion. `vst_font`
(opcode 21) is an echo with no effect in NewTOS: EmuTOS sets `cur_font` to the
face it found and drives the size through `vst_point`/`vst_height` with the
old `top` (`vdi_text.c:939-986`), while we only write `INTOUT[0]`. That is a
real behaviour gap, and **no case in `vdi-r2.tsv` catches it** — the table has
`vst_font` cases, but nothing afterwards reads back a metric that would expose
the missing effect.

The lesson generalises: a case that only checks a setter's echo proves the
echo, not the effect. Round 3 should pair state-changing calls with an
inquiry that reads the state back.

## Reproducing

    cargo test -p tos-api --test vdi_reference -- --test-threads=1

Regenerate the cases after a new measurement:

    tosdump/host/build/mkrusttests \
        tosdump/cases/vdi-r2.tsv \
        tosdump/reference/EMUTOS14-VDI-r2.BIN \
        ../../../tosdump/reference/EMUTOS14-VDI-r2.BIN \
        tos-api/tests/vdi_reference/cases_r2.rs
