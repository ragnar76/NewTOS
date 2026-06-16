# `Logbase` — XBIOS 0x03

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/screen.c:1071` (`logbase`).

## EmuTOS source

```c
UBYTE *logbase(void) { return v_bas_ad; }
```

`v_bas_ad` is an OS variable at addr `0x44E` (line-A / OS-header
exposes it). It points at the buffer where the VT52, line-A, and
GEMDOS console direct their drawing.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x03` |
| WORD args | 0 |
| Stack frame | `+0: 0x0003` |
| Vector | `xbios_vecs[3]` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (`UBYTE *`) |

The current logical screen base — destination of all software
drawing.

## Errors

None.

## Edge cases

- **Same as physbase by default**: at boot, `Setscreen(NULL,
  NULL, rez, vmode)` sets both to the same hardware framebuffer.
  Programs implementing double-buffering call `Setscreen(buf2,
  -1, …)` to redirect logical drawing while leaving the hardware
  base untouched.
- **`v_bas_ad` is mutable from any context** — it's a plain global
  with no sync. EmuTOS is single-threaded so this is fine.
- **Line-A reads `v_bas_ad` directly** — changes take effect
  immediately on the next line-A draw.

## Dependencies

- `v_bas_ad` (OS variable `0x44E`).

Used by: line-A drawing, VDI, VT52 console.

## Cross-references

- **Compendium:** "Logbase" — agrees.
- **Profibuch:** "Logbase (0x03)" — agrees.
- `docs/api/xbios/02-physbase.md`
- `docs/api/xbios/05-setscreen.md`
- `docs/api/structures/osheader.md` — `v_bas_ad` exposure.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)
