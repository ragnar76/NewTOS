# `Vsync` — XBIOS 0x25

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/screen.c:1206` (`vsync`).

## EmuTOS source

```c
void vsync(void)
{
    LONG a;
#if CONF_WITH_ATARI_VIDEO
    WORD old_sr = set_sr(0x2300);   /* allow VBL interrupt */
#endif
    a = frclock;
    while (frclock == a) {
#if USE_STOP_INSN_TO_FREE_HOST_CPU
        stop_until_interrupt();
#endif
    }
#if CONF_WITH_ATARI_VIDEO
    set_sr(old_sr);
#endif
}
```

`frclock` is incremented by the VBL handler (`bios/vectors.S`
trampoline calls `int_vbl`). Spin-wait until it ticks.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x25` |
| WORD args | 0 |
| Stack frame | `+0: 0x0025` |
| Vector | `xbios_vecs[37]` |

## Inputs

None.

## Outputs

`void`. D0 == 0 from dispatcher default.

## Errors

None.

## Edge cases

### 1. SR clobber to 0x2300

The function temporarily lowers IPL to 3 by writing `0x2300` to
SR — supervisor mode + IPL 3. This **enables** any pending IRQ ≤
level 3, including the VBL (level 4). The comment in source warns
this also enables MFP interrupts as a side effect — callers
disabling MFP via SR must reconfigure after `Vsync`.

### 2. Spin-wait with optional STOP

If `USE_STOP_INSN_TO_FREE_HOST_CPU` is set (typically for QEMU /
ARAnyM hosts), the wait uses `stop #0x2300` instead of busy-spin
to yield to the host CPU. NewTOS on aarch64 will use `wfi` (wait
for interrupt) for the same effect.

### 3. Race window

Reading `frclock = a`, the next instruction is the test
`while (frclock == a)`. If a VBL fires exactly between, the loop
exits immediately — correct. If `frclock` was about to wrap
around 2³² boundary mid-test, the wraparound is harmless because
of `==` (not `<`).

### 4. Up to 1 VBL latency

Worst case: called 1 µs before VBL — returns almost immediately.
Best case: called 1 µs after VBL — waits ~16.6 ms (60 Hz) /
~20 ms (50 Hz). Programs requiring exact timing budget the
upper bound.

### 5. Re-entrant callers from MFP IRQ

If a MFP timer ISR calls `Vsync` (rare but possible), the IPL-
lowering may cause unexpected IRQ recursion. EmuTOS does not
guard. NewTOS should detect "called from IRQ" and either reject or
adjust.

## Dependencies

- `frclock` global, incremented by VBL.
- VBL interrupt handler.

Used by: animation pacing, palette/screen-flip synchronisation,
rate-limited logic in games.

## Cross-references

- **Compendium:** "Vsync" — agrees.
- **Profibuch:** "Vsync (0x25)" — agrees, notes IPL change quirk.
- `docs/api/xbios/05-setscreen.md` — internal user.

## Configuration

- `USE_STOP_INSN_TO_FREE_HOST_CPU` (build option).
- `CONF_WITH_ATARI_VIDEO` (controls SR clobber).

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: NewTOS exposes a vblank IRQ from the VideoCore mailbox
or HDMI controller. `Vsync` waits on a per-CPU futex bumped by the
vblank ISR, using `wfi`/`wfe` to idle.
