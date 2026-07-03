# `Midiws` (XBIOS 0x0C)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/midi.c:104` (`midiws`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, `void` return, the **count = bytes − 1** (unsigned)
> convention. Reference: `bconout3` and the MIDI ACIA transport.

## EmuTOS source

**Primary location:** `bios/midi.c:104` —
`void midiws(WORD cnt, const UBYTE *ptr)`, slot 0x0C of `xbios_vecs[]`
(`bios/xbios.c:1298`).

**Installed handler / vector:** (none) — direct table entry. Each byte goes
through `bconout3` (the MIDI BIOS output handler, handle 3).

**Helpers called:**
- `bios/midi.c` — `bconout3(3, byte)` — write one byte to the MIDI ACIA
  (`CONF_WITH_MIDI_ACIA`), blocking on TX-ready.

**Headers / type definitions used:** none specific.

**System variables / globals read/written:** none.

**Build configuration affecting this function:**
- `CONF_WITH_MIDI_ACIA` — the transport. Without it (NewTOS profile) the
  bytes go nowhere; `Midiws` is effectively a no-op.

---

## Source extraction summary

`Midiws` writes a byte string to the MIDI port. `cnt` is the number of bytes
to write **minus one**, treated as **unsigned** ("just like Atari TOS"). The
loop sends `*ptr++` via `bconout3(3, …)` and continues while `cnt--`, so it
always sends at least one byte and, for `cnt == 0xFFFF`, sends 65536 bytes.
Each byte blocks on the MIDI ACIA TX-ready flag, so the call is synchronous.
There is no return value.

On the NewTOS profile there is no MIDI ACIA, so `Midiws` transmits nothing;
it is documented here for ABI completeness.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x0C
- **Stack frame on entry (m68k):** `Midiws(cnt, ptr)` = `xbios_v_wl(12, a, b)`
  (`include/xbiosbind.h:30`):
  ```
  +4: ptr (LONG, pointer)
  +2: cnt (word)
   0: function_id (word) = 0x000C
  ```
- **Total parameter bytes:** 6 (W + L).
- **Native (AArch64) ABI:** ID `0x0C` in `x8`; args in `x0..x1`; **`void`** —
  no return value.

  | m68k stack | AArch64 reg | Name  | Type / size |
  |------------|-------------|-------|-------------|
  | +2         | `x0`        | `cnt` | `WORD` (16) — **bytes − 1, unsigned** |
  | +4         | `x1`        | `ptr` | `const UBYTE *` (ptr) |

---

## Inputs

| Position | Name  | Type   | Size | Source location   | Notes |
|----------|-------|--------|------|-------------------|-------|
| arg0     | `cnt` | `WORD` | 2    | `bios/midi.c:104` | count of bytes **minus one**, unsigned |
| arg1     | `ptr` | `const UBYTE *` | 4 | `bios/midi.c:106` | first byte to send |

---

## Outputs

### Return value

- **Register:** none (`void`).
- **Type:** `void`.
- **Source location:** `bios/midi.c:104` (declared `void`).

### Side effects on global state

None.

### Side effects on hardware

- Writes `cnt + 1` bytes to the MIDI ACIA (with `CONF_WITH_MIDI_ACIA`),
  blocking per byte.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

No validation; a bogus `cnt` sends more/fewer bytes.

---

## Verification criteria

- **Send N bytes:** `Midiws(N-1, buf)` transmits exactly `N` bytes to the MIDI
  port in order.
- **Single byte:** `Midiws(0, &b)` sends one byte.
- **Unsigned count:** `Midiws(0xFFFF, buf)` attempts 65536 bytes.
- **No-MIDI profile:** with no MIDI ACIA the call returns without transmitting
  (no error).

---

## Edge cases discovered in source

1. **`cnt` is bytes − 1, unsigned.** The loop sends at least one byte and
   wraps to 65536 for `0xFFFF` (`bios/midi.c:106-109`) — the same off-by-one
   contract as `Ikbdws`.
2. **Routes through `bconout3`, not the ACIA directly.** So it shares the MIDI
   output path; on a build without `CONF_WITH_MIDI_ACIA` the bytes are
   dropped.

---

## Dependencies

### Requires (must be initialized / functional)

- The MIDI ACIA output path (`bconout3`).

### Used by

- MIDI sequencers, synth controllers, any program driving MIDI out.

---

## Related data structures

- `docs/api/xbios/0e-iorec.md` — the MIDI input record (`Iorec(2)`).

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents `cnt` as
  bytes − 1.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `CONF_WITH_MIDI_ACIA` — off in the NewTOS profile (no MIDI hardware).

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07 (optional): a NewTOS MIDI-over-USB or -UART path could give `Midiws`
a real transport; until then it is an ABI no-op. The `bytes-1` unsigned count
is honoured either way.
