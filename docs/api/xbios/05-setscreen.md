# `Setscreen` — XBIOS 0x05

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/screen.c:1102` (`setscreen`).

## EmuTOS source (essence)

```c
WORD setscreen(UBYTE *logLoc, const UBYTE *physLoc, WORD rez, WORD videlmode)
{
    WORD oldmode = 0;

    if ((LONG)logLoc  > 0)  v_bas_ad = logLoc;          /* set logical */
    if ((LONG)physLoc > 0)  setphys(physLoc);           /* set physical */

    if (rez_was_hacked || rez == -1)  return 0;          /* leave rez alone */
    if (rez < MIN_REZ || rez > MAX_REZ)  return -1;

    /* (CONF_WITH_VIDEL Falcon mode reallocation omitted — disabled in NewTOS) */

    vsync();                              /* wait for end-of-frame */
    atari_setrez(rez, videlmode);         /* poke shifter registers */

    vblsem = 0;
    linea_init();
    vt52_init();
    vblsem = 1;
    return oldmode;                       /* 0 except on Falcon mode swap */
}
```

The function is an **all-in-one display reconfigurator**:
- Optionally update logical buffer.
- Optionally update physical (visible) buffer.
- Optionally change resolution.
- Always re-initialise line-A and VT52 if rez changed.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x05` |
| WORD args | 6 (LONG + LONG + WORD + WORD) |
| Stack frame | `+10: videlmode (W)`, `+8: rez (W)`, `+4: physLoc (L)`, `+0+2: logLoc (L)`, `+0: 0x0005` |
| Vector | `xbios_vecs[5]` |

(Frame layout matches the Atari TOS 1.04+ ABI; pre-1.04 had no
`videlmode` argument. EmuTOS unconditionally consumes 4 args.)

## Inputs

| Pos | Name        | Type    | Range / meaning                              |
|-----|-------------|---------|----------------------------------------------|
| 0   | `logLoc`    | UBYTE * | new logical base, or `0/-1` to leave alone   |
| 1   | `physLoc`   | UBYTE * | new physical base, or `0/-1` to leave alone  |
| 2   | `rez`       | WORD    | new resolution, or `-1` to leave alone       |
| 3   | `videlmode` | WORD    | Falcon mode word (ignored without Videl)     |

### `rez` values: see `docs/api/xbios/04-getrez.md`.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (zero-extended WORD) |

| Case                                 | Return                              |
|--------------------------------------|-------------------------------------|
| `rez == -1` or line-A vars hacked    | `0`                                 |
| `rez` out of range                   | `-1`                                |
| Falcon mode change (with Videl)      | previous Videl mode word            |
| ST/TT mode change                    | `0`                                 |

## Errors

`-1` for invalid `rez`. No GEMDOS-style error code.

## Edge cases

### 1. Pointer-vs-flag overload

`logLoc` / `physLoc` of type `UBYTE *`, but the source casts to
`(LONG)` for the `> 0` check. **Address `0` (NULL) is treated as
"don't change"**. So is **address `0xFFFFFFFF` (-1)**, because
`(LONG)0xFFFFFFFF` is negative. A program wanting to set the base
to a low ST-RAM page near 0 would have its call ignored — but no
real screen ever lives there.

### 2. `rez_was_hacked` veto

If a TSR or hacker has poked line-A variables to use a
non-standard mode, EmuTOS refuses to change resolution (returns
0). The flag is set in `linea.c:linea_init()` when post-init
mutations to `V_CEL_HT` etc. are detected.

### 3. Vsync before rez change

`vsync()` is called before the shifter register write to avoid the
ST plane-shift bug (writing rez mid-line corrupts the next
display frame). NewTOS HDMI has no such bug but the call should
still wait for vblank to avoid tearing.

### 4. line-A + VT52 re-init

Resolution change zeroes the cursor position, font glyph tables,
and line-A globals. Programs maintaining their own VT52 cursor
state must re-establish it after `Setscreen`.

### 5. `vblsem` race window

`vblsem = 0` disables VBL processing during init; `linea_init()`
and `vt52_init()` mutate state that the VBL handler reads.
Restoring `vblsem = 1` re-enables it. The window is on the order
of microseconds.

### 6. NewTOS double-buffering

A common idiom is:

```c
Setscreen(buf, -1, -1, -1);  /* draw to off-screen */
/* ... draw ... */
Vsync();
Setscreen(-1, buf, -1, -1);  /* flip on vblank */
```

The `physLoc=-1` first call leaves the physical base untouched;
the `physLoc=buf` second call flips. Per the pointer-vs-flag rule,
real `-1` is treated as "no change", so this exact idiom requires
careful checking — most programs use a no-op flag of `0`, not `-1`.

### 7. `vfixmode` clamping (Videl, not in NewTOS)

The Falcon path passes `videlmode` through `vfixmode` which clamps
it to a supported mode. NewTOS does not implement this; calling
with `rez = FALCON_REZ` returns `-1` (out of range — `MAX_REZ <
3` in TT-only profile).

## Dependencies

- `v_bas_ad`, `setphys()`, `atari_setrez()`.
- `vsync()`, `linea_init()`, `vt52_init()`.
- `vblsem` flag.

Used by: AES on resolution change, GEM control panel, double-
buffered games.

## Cross-references

- **Compendium:** "Setscreen" — agrees on signature & semantics.
- **Profibuch:** "Setscreen (0x05)" — agrees, notes 0/-1 idiom.
- `docs/api/xbios/02-physbase.md`, `03-logbase.md`,
  `04-getrez.md`, `25-vsync.md` (pending).

## Configuration

- `CONF_WITH_ATARI_VIDEO`, `CONF_WITH_TT_SHIFTER` — define
  `MIN_REZ`/`MAX_REZ`. NewTOS profile: `MIN_REZ=0`, `MAX_REZ=2`
  (ST modes) extended to 7 if TT_LOW/TT_HIGH activated.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: NewTOS will support `rez ∈ {ST_LOW, ST_MEDIUM, ST_HIGH,
TT_MEDIUM, TT_HIGH, TT_LOW}` by reconfiguring the VideoCore
framebuffer (different geometry / depth) and re-allocating the
buffer in the screen-RAM region. Buffers passed by `logLoc`/
`physLoc` must be in the framebuffer-mapped region; otherwise
return `-1`.
