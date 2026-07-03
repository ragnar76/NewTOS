# `Random` (XBIOS 0x11)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/xbios.c:421` (`random`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the 24-bit LONG result (bits 24-31 zero), the
> deterministic LCG sequence from a given seed. Reference: the `rseed`
> storage and the `hz_200` auto-seed.

## EmuTOS source

**Primary location:** `bios/xbios.c:421` — `static LONG random(void)`, slot
0x11 of `xbios_vecs[]` (`bios/xbios.c:1307`). Uses the file-static seed
`rseed` (`bios/xbios.c:419`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:** none.

**Headers / type definitions used:**
- `include/tosvars.h` — `hz_200` (the 200 Hz tick counter).

**System variables / globals read:**
- `hz_200` (`0x4BA`) — used to seed `rseed` on the first call.

**System variables / globals written:**
- `rseed` (`bios/xbios.c:419`) — advanced on every call.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Random` returns a 24-bit pseudo-random number using a linear congruential
generator. On the first call (`rseed == 0`) it seeds `rseed = (hz_200 << 16)
+ hz_200`. Each call then does `rseed = rseed * 3141592621 + 1` (a 32-bit LCG,
multiplier ≈ π×10⁹) and returns `(rseed >> 8) & 0xFFFFFF` — the middle 24 bits,
with bits 24-31 always zero.

The sequence is fully deterministic given a seed, which is contract: two runs
that reach the same `rseed` produce the same sequence. The `>> 8` discards the
worst low bits (bit 0 of an LCG alternates every step), but the result is not
cryptographic. Well-behaved code that needs a specific starting sequence sets
`rseed` (historically by patching the BIOS BSS location); NewTOS does not
expose that back door.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x11
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0011
  ```
  No parameters (`Random()` = `xbios_l_v(17)`, `include/xbiosbind.h:35`).
- **Total parameter bytes:** 2.
- **Native (AArch64) ABI:** ID `0x11` in `x8`; no argument registers; the
  LONG result **sign-extended** into `x0` (top byte is 0, so the value is a
  clean 0..0xFFFFFF).

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | (none)     | —           | —    | —           |

---

## Inputs

None.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG (24-bit value)
- **Encoding:** `(rseed >> 8) & 0xFFFFFF` — bits 0-23 random, bits 24-31 zero.
- **Source location:** `bios/xbios.c:429`.

### Side effects on global state

- Advances `rseed` (and seeds it from `hz_200` on the first call).

### Side effects on hardware

None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

`Random` cannot fail.

---

## Verification criteria

- **Range:** every result satisfies `0 <= r <= 0xFFFFFF` (bits 24-31 zero).
- **Determinism:** from a fixed `rseed`, the sequence of results matches the
  LCG `rseed = rseed*3141592621 + 1; return (rseed>>8)&0xFFFFFF`.
- **Auto-seed:** the first call after boot seeds from `hz_200`, so it is not a
  constant across boots (but is a function of uptime at first call).
- **Advances each call:** two consecutive calls return different values (for
  any non-degenerate seed).

---

## Edge cases discovered in source

1. **First-call seed from `hz_200`.** `rseed = (hz_200 << 16) + hz_200`
   (`bios/xbios.c:423-426`); early after boot `hz_200` is small, so the
   effective entropy is low (~16 bits). AES/games often mix `hz_200`
   themselves for this reason.
2. **24-bit result, LCG weaknesses.** The `>> 8` masks the worst low bits, but
   sequential outputs remain linearly correlated; never use for security.
3. **Seed is internal.** `rseed` is a BIOS static; the classic "poke the seed"
   trick relies on a fixed BSS offset. NewTOS keeps the LCG for determinism
   but does not expose the poke; a reseed is a NewTOS extension.

---

## Dependencies

### Requires (must be initialized / functional)

- `hz_200` running (for the seed).

### Used by

- Games, AES timer jitter, screen-saver pattern generation, sample shuffling.

---

## Related data structures

- `docs/api/xbios/_state.md §7` — `rseed`.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the 24-bit
  return.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- The multiplier `3141592621` and the `>> 8`/`& 0xFFFFFF` shaping match
  classic TOS, so the sequence is bit-identical to real hardware for a given
  seed.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: NewTOS keeps this exact LCG for ABI determinism. Higher-quality
entropy (AArch64 `RNDR`) is exposed via a separate NewTOS-extension slot, not
by changing `Random`.
