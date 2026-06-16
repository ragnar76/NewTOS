# `Random` — XBIOS 0x11

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/xbios.c:421` (`random`).

## EmuTOS source

```c
static ULONG rseed;

static LONG random(void)
{
    if (rseed == 0) {
        rseed  = hz_200 << 16;
        rseed += hz_200;
    }
    rseed *= 3141592621UL;
    rseed += 1;
    return (rseed >> 8) & 0xFFFFFF;
}
```

`hz_200` is the OS variable at addr `0x4BA`, a 200 Hz tick counter
(LONG). Used as the entropy seed on first call.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x11` |
| WORD args | 0 |
| Stack frame | `+0: 0x0011` |
| Vector | `xbios_vecs[17]` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (24-bit pseudo-random) |

A 24-bit unsigned value `0..0xFFFFFF`. Bits 31..24 are always 0.

## Errors

None.

## Edge cases

### 1. Linear congruential — not cryptographic

The constant `3141592621` (≈ π × 10⁹) is the same multiplier used
by classic TOS. `(seed * a + 1) mod 2^32` is a standard LCG with
known weaknesses:
- Low bits have very short cycles (bit 0 alternates every call).
- Sequential outputs are linearly correlated.
- Period at most 2^32; effective bit-32-truncated period 2^32.

The `>> 8` discards the worst low bits. Output looks "random
enough" for game demos and AES timings, **never** for security.

### 2. Auto-seed from `hz_200`

First call seeds from `hz_200` shifted into bits 16..31 plus
ticks in bits 0..15. After boot but before user input, `hz_200`
is small (a few thousand ticks per second, system has been up <1
min typical), so seed has only ~16 bits of entropy.

### 3. Manual seeding

Atari's documented "set seed" mechanism: write a non-zero value
directly to `rseed` via `Setexc`-style memory access (`rseed` is
not exported but lives at a fixed offset in BIOS BSS — programs
historically patch it). NewTOS will not export this back-door;
programs needing reseed will use a NewTOS extension.

### 4. Re-entrancy

Two calls in quick succession both update `rseed` — not a bug but
a property: rapid consecutive calls produce a deterministic
sequence per seed.

### 5. AES uses `hz_200` directly for finer entropy

AES and many games actually mix `hz_200` themselves to compensate
for `Random()`'s weakness — implying the function is not trusted
even by Atari's own software.

## Dependencies

- `hz_200` (OS var `0x4BA`).
- `rseed` (BIOS BSS).

Used by: games, AES message timer jitter, screen-saver pattern
generation.

## Cross-references

- **Compendium:** "Random" — agrees on 24-bit return.
- **Profibuch:** "Random (0x11)" — agrees, notes seed mechanism.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: NewTOS will keep the same LCG for ABI determinism.
Better entropy is exposed via a NewTOS-extension XBIOS function
(reserved 0x2F..0x3F slot) backed by AArch64 `RNDR` instruction or
`/dev/random`-style entropy pool.
