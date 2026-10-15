# `TEDINFO` — editable / formatted text object payload

> Phase 00 spec (AES Object/Form batch). Source: EmuTOS @ `1a7484ab` —
> `include/obdefs.h:170-183` (struct), `obdefs.h:248-255` (`ED*`/`TE_*`
> constants). Consumed by `just_draw` (`aes/gemoblib.c:567-757`) and by
> `ob_edit` (`aes/gemobed.c`). Pointed to by `ob_spec` for `G_TEXT`,
> `G_BOXTEXT`, `G_FTEXT`, `G_FBOXTEXT` (`obdefs.h:170`).
>
> **Contract vs. reference** — the field layout, the `te_*` semantics, the
> `te_just` values, the colour word in `te_color`, and the validation-string
> characters are **contract** (a resource / program supplies these).

## Purpose

A `TEDINFO` describes an editable or formatted text field. It carries the
live text (`te_ptext`), a template that positions the text
(`te_ptmplt`, with `_` marking data slots), a validation string that limits
which characters may be typed (`te_pvalid`), and formatting attributes
(font, justification, colour word, border thickness).

`te_ptext` **must be the first field** (comment in `obdefs.h:172`) because
some code takes `&TEDINFO == te_ptext`.

## Binary layout (m68k: 28 bytes)

```c
typedef struct {
/* 0x00 */ char *te_ptext;     /* -> live text string (must be 1st)   */
/* 0x04 */ char *te_ptmplt;    /* -> template string ('_' = data slot)*/
/* 0x08 */ char *te_pvalid;    /* -> validation characters            */
/* 0x0C */ WORD  te_font;      /* font id (IBM=3, SMALL=5)            */
/* 0x0E */ WORD  te_junk1;     /* unused ("junk word")               */
/* 0x10 */ WORD  te_just;      /* TE_LEFT 0 / TE_RIGHT 1 / TE_CNTR 2  */
/* 0x12 */ WORD  te_color;     /* colour word (same layout as box)    */
/* 0x14 */ WORD  te_junk2;     /* unused ("junk word")               */
/* 0x16 */ WORD  te_thickness; /* border thickness (signed; see note) */
/* 0x18 */ WORD  te_txtlen;    /* max length of te_ptext incl. NUL    */
/* 0x1A */ WORD  te_tmplen;    /* max length of te_ptmplt incl. NUL   */
/* 0x1C — end (28 bytes) */
} TEDINFO;
```

> **Native (AArch64) variant.** The three leading pointers are 64-bit, so the
> native `TEDINFO` grows accordingly; the WORD fields keep the same relative
> order and meaning (COMPATIBILITY "Native vs m68k struct variants").
> `te_junk1`/`te_junk2` are reserved padding words in the original layout,
> preserved for m68k struct compatibility.
>
> Native offsets (source: `tos-types/src/aes.rs:524-535` assertions,
> `repr(C)` struct `:63-90`): **40 bytes**, 8-byte aligned; `te_ptext` 0x00,
> `te_ptmplt` 0x08, `te_pvalid` 0x10, `te_font` 0x18, `te_junk1` 0x1A,
> `te_just` 0x1C, `te_color` 0x1E, `te_junk2` 0x20, `te_thickness` 0x22,
> `te_txtlen` 0x24, `te_tmplen` 0x26. The m68k table above agrees with
> `include/obdefs.h:170-183` (28 bytes). (SPEC01 cross-check 2026-10-16: no
> earlier native number in this file; added.)

## Field semantics

| Field | Meaning (from `just_draw`/`ob_edit`) |
|-------|--------------------------------------|
| `te_ptext`     | current field contents; `ob_edit` mutates this in place (`gemobed.c:444`) |
| `te_ptmplt`    | template; each `_` is a slot filled from `te_ptext`, other chars are literal (`ob_format`, `gemoblib.c:361-408`) |
| `te_pvalid`    | one validation code per slot; `check()` maps a code (`9AaNnFfPpXx`) to an allowed-character set and optional up-casing (`gemobed.c:252-309`) |
| `te_font`      | `IBM = 3` (system) or `SMALL = 5` (`obdefs.h:62-63`) |
| `te_just`      | `TE_LEFT=0`, `TE_RIGHT=1`, `TE_CNTR=2` (`obdefs.h:253-255`) |
| `te_color`     | colour word, decoded by `gr_crack` exactly like a box `ob_spec` (border/text/mode/pattern/inside — see [`object.md`](object.md)) |
| `te_thickness` | border thickness; **signed**: `ob_sst` converts >128 to negative (`gemobjop.c:68-69`). **Positive = inside border, negative = outside** (corrected per [`object-form.md`](../aes/object-form.md) D-089 — traced through `gr_box`/`just_draw`; an earlier version of this table had the sign backwards) |
| `te_txtlen`    | buffer size of `te_ptext`; `ob_edit` never grows past `te_txtlen-2` data chars (`gemobed.c:394,407`) |
| `te_tmplen`    | buffer size of `te_ptmplt`; bounds the validation-string expansion (`gemobed.c:354`) |

## Validation codes (`check()`, `gemobed.c:252-309`)

Each character of `te_pvalid` validates the corresponding editable slot. The
per-code allowed sets (EmuTOS notes most external documentation is wrong;
these match TOS 2/3/4, `gemobed.c:41-49`):

| Code | Allowed | Up-cases? |
|------|---------|-----------|
| `9` | `0..9` | no |
| `A` | `A..Z`, space, uppercase non-Roman | yes |
| `N` | `0..9`, `A..Z`, space, uppercase non-Roman | yes |
| `a` | `a..z`, `A..Z`, space, `0x80..0xff` | no |
| `n` | `0..9`, `a..z`, `A..Z`, space, `0x80..0xff` | no |
| `F` | `:?*` + DOS filename chars | yes |
| `f` | DOS filename chars (no `:?*`) | yes |
| `P` | `.?*` + DOS pathname chars | yes |
| `p` | DOS pathname chars (no `.?*`) | yes |
| `X` | anything | no |
| `x` | anything | yes (lower→upper) |

If `te_pvalid` is shorter than the number of slots, `ob_edit` **repeats the
last validation character** to fill the remaining slots up to `te_tmplen`
(`gemobed.c:353-356`).

## Related data structures

- [`object.md`](object.md) — the `OBJECT` node that references a `TEDINFO`
- The `@` sentinel: `ob_format` treats a `te_ptext` beginning with `'@'` as
  an empty string (`gemoblib.c:367-368`).

## Cross-references

### Atari Compendium / Profibuch

- **Not present in `references/`** (no PDFs). Field layout and the `te_pvalid`
  code table are documented in Compendium; cross-check **deferred**. EmuTOS
  explicitly warns its validation table differs from most documentation
  (`gemobed.c:39-41`) — EmuTOS wins per CLAUDE.md Rule 1.
