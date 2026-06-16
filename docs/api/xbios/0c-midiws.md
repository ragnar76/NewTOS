# `Midiws` — XBIOS 0x0C

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/midi.c:104` (`midiws`).

## EmuTOS source

```c
void midiws(WORD cnt, const UBYTE *ptr)
{
    do { bconout3(3, *ptr++); } while (cnt--);
}
```

`bconout3` is the BIOS `Bconout` direct entry for device 3 (MIDI).

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x0C` |
| WORD args | 4 (WORD + LONG) |
| Stack frame | `+4: ptr (LONG)`, `+2: cnt (W)`, `+0: 0x000C` |
| Vector | `xbios_vecs[12]` |

## Inputs

| Pos | Name  | Type            | Range / meaning                                  |
|-----|-------|-----------------|--------------------------------------------------|
| 0   | `cnt` | WORD            | byte count **less one** (treated unsigned)        |
| 1   | `ptr` | const UBYTE *   | source buffer                                    |

## Outputs

`void`. D0 == 0 from dispatcher default.

## Errors

None — `bconout3` blocks until each byte is sent.

## Edge cases

### 1. `cnt` is "count minus one"

The do/while sends `cnt + 1` bytes (and `cnt` is then decremented
to wrap-around at 0xFFFF). Calling `Midiws(0, ptr)` sends **one**
byte, not zero.

### 2. Unsigned semantics

`cnt = -1` (0xFFFF) sends 65 536 bytes. EmuTOS preserves the Atari
TOS quirk verbatim — programs relying on this work unchanged.

### 3. Blocks indefinitely

`bconout3` polls the MIDI ACIA until TX is ready. If the MIDI
device is wedged, this hangs forever. No timeout.

### 4. Synchronous

The function returns only after the last byte has been handed to
the ACIA — but the byte may still be in the shift register. To
guarantee transmission completion before powering down, a caller
must additionally drain via `Bcostat(3)`.

## Dependencies

- `bconout3` (BIOS char-device 3 = MIDI).
- MIDI ACIA hardware.

Used by: MIDI sequencers, sampler patch upload, MIDI-driven
demos.

## Cross-references

- **Compendium:** "Midiws" — agrees on cnt-1 quirk.
- **Profibuch:** "Midiws (0x0C)" — agrees.
- `docs/api/bios/03-bconout.md` — underlying primitive.

## Configuration

- `CONF_WITH_MIDI_ACIA`. NewTOS profile: not present.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07 plan: stub — no MIDI hardware. Returns immediately
without sending. A future NewTOS extension may route MIDI to a USB
class-compliant MIDI interface.
