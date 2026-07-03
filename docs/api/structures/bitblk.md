# `BITBLK` — raw raster image object payload (`G_IMAGE`)

> Phase 00 spec (AES Object/Form batch). Source: EmuTOS @ `1a7484ab` —
> `include/obdefs.h:219-227`. Consumed by `just_draw`'s `G_IMAGE` case
> (`aes/gemoblib.c:759-775`, via `gsx_blt`). Pointed to by `ob_spec` for
> `G_IMAGE`. Also used directly by the alert builder for the NOTE/QUEST/STOP
> icons (`rs_bitblk[]`, `aes/gemfmalt.c:217,272`).
>
> **Contract vs. reference** — the field layout and the monochrome-blit
> semantics are **contract**.

## Purpose

A `BITBLK` is the simplest image: a single monochrome bitmap blitted to the
object's position in one foreground colour, transparently (background is left
untouched). Unlike `ICONBLK` there is no mask, no text, no character.

## Binary layout (m68k: 14 bytes)

```c
typedef struct {
/* 0x00 */ void *bi_pdata;  /* -> bitmap data                        */
/* 0x04 */ WORD  bi_wb;     /* width of form in BYTES                */
/* 0x06 */ WORD  bi_hl;     /* height in lines (pixels)              */
/* 0x08 */ WORD  bi_x;      /* source x offset within the form       */
/* 0x0A */ WORD  bi_y;      /* source y offset within the form       */
/* 0x0C */ WORD  bi_color;  /* foreground colour of the blit         */
/* 0x0E — end (14 bytes) */
} BITBLK;
```

> **Native (AArch64) variant.** `bi_pdata` is a 64-bit pointer; the WORD
> fields keep order/meaning (COMPATIBILITY "Native vs m68k struct variants").

## Draw semantics (`just_draw`, `gemoblib.c:759-774`)

`just_draw` copies the `BITBLK` locally, then calls:

```
gsx_blt(bi.bi_pdata, bi.bi_x, bi.bi_y,   /* source form + src x/y */
        t.g_x, t.g_y,                    /* dest = object screen x/y */
        bi.bi_wb * 8, bi.bi_hl,          /* width in PIXELS = bytes*8, height */
        MD_TRANS, bi.bi_color, WHITE);   /* transparent, fg colour, bg white */
```

- **`bi_wb` is a byte width**; the blit width in pixels is `bi_wb * 8`
  (`gemoblib.c:772-773`).
- The mode is **`MD_TRANS`** (transparent) — the image's set bits are drawn in
  `bi_color`, cleared bits leave the background.
- **SELECTED handling** (`CONF_WITH_3D_OBJECTS`, default on): a selected
  `G_IMAGE` first XOR-fills its rect solid (making the background black), then
  the image colour is XOR-complemented (`xor_color`) before the blit
  (`gemoblib.c:765-770`). Selecting/deselecting a `G_IMAGE` in `ob_change`
  uses XOR but **forces a redraw** because `xor_is_ok()` returns FALSE for
  `G_IMAGE` (`gemoblib.c:1295-1303`).

## Related data structures

- [`object.md`](object.md) — the `OBJECT` node; `G_IMAGE` payload
- [`iconblk.md`](iconblk.md) — the richer masked icon payload

## Cross-references

### Atari Compendium / Profibuch

- **Not present in `references/`** (no PDFs). The 14-byte layout and the
  `bi_wb`-in-bytes convention are documented in Compendium; cross-check
  **deferred**.
