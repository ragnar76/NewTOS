# `Blitmode` (XBIOS 0x40)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/xbios.c:886` (`blitmode`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the status-word bit meanings, the `mode == -1`
> query, the `0x0000` "no blitter" return. Reference: `has_blitter`,
> `blitter_is_enabled`.

## EmuTOS source

**Primary location:** `bios/xbios.c:886` — `static WORD blitmode(WORD mode)`,
slot 0x40 of `xbios_vecs[]` (`bios/xbios.c:1371`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:** none.

**Headers / type definitions used:**
- `bios/processor.h` / `bios/machine.c` — `has_blitter`, `blitter_is_enabled`.

**System variables / globals read:**
- `has_blitter` (`bios/machine.c:300`), `blitter_is_enabled`
  (`bios/machine.c:301`).

**System variables / globals written:**
- `blitter_is_enabled = mode & 1` when `mode != -1` and a blitter is present.

**Build configuration affecting this function:**
- `CONF_WITH_BLITTER` — without it, `blitmode` always returns `0x0000`.

---

## Source extraction summary

`Blitmode` gets or sets whether the Atari BLiTTER chip is used and reports its
presence/state. With `CONF_WITH_BLITTER`: if `has_blitter`, the status word is
`0x0003` when the blitter is enabled, `0x0002` when present-but-disabled; and
if `mode != -1` it sets `blitter_is_enabled = mode & 1` (only the LSB
matters). It always returns the (pre-update? — see below) status. Without a
blitter, or with `CONF_WITH_BLITTER` off, it returns `0x0000`.

Status-word bits: **bit 0** = blitter currently on/off, **bit 1** = blitter
hardware present. So `0x0000` = absent, `0x0002` = present+off, `0x0003` =
present+on. `mode == -1` is the pure query (no state change).

On NewTOS there is no Atari BLiTTER, so `Blitmode` returns `0x0000` — programs
that probe for a blitter correctly see "absent" and fall back to CPU blits.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x40
- **Stack frame on entry (m68k):** `Blitmode(mode)` = `xbios_w_w(64, a)`
  (`include/xbiosbind.h:58`):
  ```
  +2: mode (word)
   0: function_id (word) = 0x0040
  ```
- **Total parameter bytes:** 4 (W).
- **Native (AArch64) ABI:** ID `0x40` in `x8`; arg in `x0`; the WORD status
  **sign-extended** into `x0`.

  | m68k stack | AArch64 reg | Name   | Type / size |
  |------------|-------------|--------|-------------|
  | +2         | `x0`        | `mode` | `WORD` (16) — `-1` query; else `& 1` sets enable |

---

## Inputs

| Position | Name   | Type   | Size | Source location    | Notes |
|----------|--------|--------|------|--------------------|-------|
| arg0     | `mode` | `WORD` | 2    | `bios/xbios.c:895` | `-1` = query only; else bit 0 sets `blitter_is_enabled` |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** WORD (status)
- **Encoding:**

  | Status | Meaning |
  |-------:|---------|
  | 0x0000 | no blitter (absent, or `CONF_WITH_BLITTER` off) |
  | 0x0002 | blitter present, disabled |
  | 0x0003 | blitter present, enabled |

  bit 0 = enabled, bit 1 = present.
- **Source location:** `bios/xbios.c:889-899` (`CONF_WITH_BLITTER`) / `:901`
  (`#else`).

### Side effects on global state

- `blitter_is_enabled = mode & 1` when `mode != -1` and `has_blitter`.

### Side effects on hardware

- None on the NewTOS profile (no blitter). On Atari, enabling routes VDI blits
  through the BLiTTER.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

`Blitmode` cannot fail; "no blitter" is reported as `0x0000`, not an error.

---

## Verification criteria

- **No-blitter profile:** `Blitmode(-1)` returns `0x0000` (bit 1 clear =
  absent), and `Blitmode(1)` also returns `0x0000` (no state to change).
- **Query does not change state:** on a blitter machine, `Blitmode(-1)`
  returns the status without altering `blitter_is_enabled`.
- **Enable/disable:** on a blitter machine, `Blitmode(1)` enables (status bit
  0 set), `Blitmode(0)` disables; only the LSB of `mode` matters.

---

## Edge cases discovered in source

1. **`0x0000` means "absent", not "unimplemented".** This is the
   `CONF_WITH_BLITTER`-off / `!has_blitter` return — a real status value with
   bit 1 clear, distinct from the `xbios_unimpl` function-number path. A
   program checking bit 1 sees "no blitter" and falls back correctly.
2. **Only the LSB of `mode` sets the enable.** `blitter_is_enabled = mode & 1`
   (`bios/xbios.c:897`); higher bits are ignored.
3. **`mode == -1` is the query.** No state change (`:895`).

---

## Dependencies

### Requires (must be initialized / functional)

- `has_blitter`/`blitter_is_enabled` (boot detection) — trivially "no blitter"
  on NewTOS.

### Used by

- VDI (routes blits through the blitter when enabled), performance-tuning
  tools, blitter-presence probes.

---

## Related data structures

- `docs/api/xbios/_state.md` — (no dedicated blitter section; state is the two
  `machine.c` globals).

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the status
  bits and the `-1` query.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `CONF_WITH_BLITTER` off → always `0x0000`. NewTOS profile: no Atari blitter,
  so `0x0000`.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07/08: `Blitmode` returns `0x0000` (no blitter); NewTOS uses CPU /
VideoCore blits. If a future NewTOS "accelerated blit" path is exposed it
would be a separate extension, not this bit.

---

## Pi / BCM2711 binding

> Re-points the reference from the **Atari BLiTTER chip** (`has_blitter` /
> `blitter_is_enabled`) to **no blitter** on the Pi. The status-word contract
> (`0x0000` = absent, `0x0002` = present+off, `0x0003` = present+on; `-1` =
> query) is unchanged; the value is pinned to `0x0000`. See
> `docs/HARDWARE-PROFILE.md` §3.9.

### There is no blitter → `Blitmode` should report `0x0000`

The BCM2711 has no Atari BLiTTER; NewTOS does CPU / VideoCore blits. The
contract answer for a blitter-less machine is **`0x0000`** (bit 1 clear =
absent), for both `Blitmode(-1)` (query) and any `Blitmode(mode)` (nothing to
enable).

| Atari reference | Pi-4 model today |
|-----------------|------------------|
| `has_blitter` from bus probe | always false (no blitter) |
| `0x0003`/`0x0002` present+on/off | never — bit 1 always clear |
| Enabling routes VDI blits through the BLiTTER | N/A — VDI blits are CPU/VideoCore |

### Current-code gap (Step-2 candidate)

- **`Blitmode` (0x40) is not wired in the XBIOS dispatcher.** There is no `0x40`
  entry in `tos-api/src/xbios/mod.rs:33-55`, so the call hits the `_ => EINVFN`
  catch-all and returns **`EINVFN`**, not the contractual **`0x0000`**. A program
  probing bit 1 for a blitter sees a generic error instead of a clean "absent"
  status. Step-2: add the `0x40` slot returning `0x0000` (a one-line
  no-blitter constant, not a stub — it is the real status value).
