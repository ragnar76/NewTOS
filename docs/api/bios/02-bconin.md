# `Bconin` (BIOS 0x02)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/bios.c:958` (`bconin`).

## EmuTOS source

**Primary location:** `bios/bios.c:958` — `LONG bconin(WORD handle)`

**Helpers called:**
- `util/miscasm.S:213` — `protect_v(func)` — d2/a2-preserving indirect call.
- Per-handle input routines: `bios/parport.c:123` `bconin0` (**stub**,
  returns 0), `bios/serport.c:260` `bconin1` (RS-232, blocking),
  `bios/ikbd.c:250` `bconin2` (IKBD queue dequeue, blocking),
  `bios/midi.c` `bconin3` (MIDI, blocking), `bios/chardev.c:50` `char_dummy`
  (returns 0).

**Headers / type definitions used:**
- `include/tosvars.h` — `bconin_vec[8]`
- `bios/bios.h` — `boot_status`, `CHARDEV_AVAILABLE`
- `bios/iorec.h` — `ikbdiorec` (queue read by `bconin2`)

**System variables / globals read:**
- `boot_status` (`bios/bios.c:964`)
- `bconin_vec[handle]` (TOS var `0x53e`)
- `bconmap_root.maptab[]` — when `BCONMAP_AVAILABLE`

**System variables / globals written:** none in `bconin`; `bconin2` advances
`ikbdiorec.head` (`bios/ikbd.c:269`).

**Build configuration affecting this function:**
- `BCONMAP_AVAILABLE` — handle-remap stage for handles ≥ 6.
- `CONF_SERIAL_CONSOLE_POLLING_MODE` — makes `bconin2` read RS-232 and
  synthesise an IKBD record via `ikbdiorec_from_ascii` (`bios/ikbd.c:255`).

---

## Source extraction summary

`Bconin` reads (and consumes) one character from an input device, blocking
until one is available. Structure is identical to `Bconstat`: boot guard →
optional Bconmap remap → `bconin_vec[handle]` via `protect_v`; out-of-range
returns `0L`.

The default routines (`bconin_init[]`, `bios/chardev.c:34`) are
`{bconin0, bconin1, bconin2, bconin3, char_dummy×4}`. Blocking is a property
of the underlying routine, not of `bconin`: `bconin1`/`bconin2`/`bconin3`
spin until their device delivers a byte. `bconin2` (`bios/ikbd.c:250`) busy-
waits on `bconstat2()`, disables interrupts (`set_sr(0x2700)`), advances
`ikbdiorec.head` by 4 and reads the 4-byte record — so IKBD input is a
32-bit packed record, not a raw byte.

The **return encoding** is documented in the header comment at
`bios/bios.c:948-956`: for CON (handle 2) the high word carries the GSX-2.0
scancode in its low byte and the ASCII value is in the low byte of the low
word (0 if the key is non-ASCII); for AUX (handle 1) the character is in the
low byte only.

---

## Trap interface

- **Trap number:** 13 (BIOS)
- **Function ID:** 0x02
- **Stack frame on entry (m68k):**
  ```
  +2: handle (word)
   0: function_id (word) = 0x0002
  ```
- **Total parameter bytes:** 4
- **Native (AArch64) ABI:** ID `0x02` in `x8`; `handle` in `x0`; the packed
  LONG result in `x0` (see encoding below).

  | m68k stack | AArch64 reg | Name     | Type / size |
  |------------|-------------|----------|-------------|
  | +2         | `x0`        | `handle` | `WORD` (16-bit) |

---

## Inputs

| Position | Name     | Type | Size | Source location   | Notes |
|----------|----------|------|------|-------------------|-------|
| arg0     | `handle` | `WORD` | 2  | `bios/bios.c:958` | `0..7` table-indexed; else `0L` |

Handle meanings (default `bconin_vec`, `bios/chardev.c:34`):

| Handle | Device (input)         | Default routine | Blocking? |
|-------:|------------------------|-----------------|-----------|
| 0      | PRT (parallel)         | `bconin0` (stub → 0) | no |
| 1      | AUX / RS-232           | `bconin1`       | yes |
| 2      | CON / IKBD             | `bconin2`       | yes |
| 3      | MIDI in                | `bconin3`       | yes |
| 4..7   | unused / Bconmap       | `char_dummy` → 0 | no |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding (CON, handle 2):** bits 0–7 = ASCII (0 if non-ASCII key);
  bits 8–15 = 0; bits 16–23 = GSX-2.0 IKBD scancode; bits 24–31 = 0.
  (Shift state is *not* folded into this word by `bconin`; use `Kbshift`.)
- **Encoding (AUX, handle 1 / MIDI, handle 3):** byte in bits 0–7, upper
  bits 0.
- **Source location:** header comment `bios/bios.c:948-956`; value produced by
  the handler (`bconin2` packs the 4-byte `ikbdiorec` record).

### Side effects on global state

- `bconin2` advances `ikbdiorec.head` (consumes one 4-byte queue record,
  `bios/ikbd.c:269-273`), wrapping at `ikbdiorec.size`.

### Side effects on hardware

Reads UART/ACIA data registers (which clears the receiver-ready flag). No
device configuration change.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `CHARDEV_AVAILABLE` clear | `bios/bios.c:964` | `0L` |
| `handle` outside `0..7` and not in Bconmap range | `bios/bios.c:975-977` | `0L` |
| Handle wired to `char_dummy` (4..7, or 0 via stub) | `bios/chardev.c:50` | `0L` |

**No distinct error code.** `0L` is ambiguous: it means "no device" *and* is a
legitimate result — a genuine NUL byte, or (handle 2) a non-ASCII key whose
ASCII byte is 0 with the scancode in the high word. Callers use `Bconstat`
first when the distinction matters.

**Error paths not in Compendium:** the boot-guard `0L`.

---

## Verification criteria

- **Nominal (AUX):** with a byte `0x41` ('A') available on RS-232,
  `Bconin(1) == 0x00000041`.
- **Nominal (CON, ASCII):** pressing 'A' → low byte `0x41`, high word = its
  scancode; `Bconin(2) & 0xFF == 0x41`.
- **Nominal (CON, non-ASCII):** pressing an arrow/function key → low byte 0,
  bits 16–23 = scancode; `Bconin(2) & 0xFF == 0` while `(Bconin(2) >> 16) &
  0xFF != 0`.
- **Consumption:** two successive `Bconstat(2)` reads without an intervening
  `Bconin(2)` still report ready for the same byte; `Bconin(2)` then removes
  it and `Bconstat(2) == 0`.
- **Invalid handle:** `Bconin(9)` → `0`.

---

## Edge cases discovered in source

1. **`bconin0` (PRT input) is a stub.** `bios/parport.c:123` returns `0`
   unconditionally with a `/* not implemented */` comment. The old spec's
   claim that it "reads the parport status/data lines" is incorrect.

2. **IKBD input is a 4-byte packed record.** `bconin2` reads a full `ULONG`
   from `ikbdiorec.buf` (`bios/ikbd.c:273`), not a byte — the packing is what
   carries the scancode in the high word. The queue advances in units of 4.

3. **`bconin2` disables interrupts during dequeue.** `set_sr(0x2700)` around
   the head advance (`bios/ikbd.c:267`) makes the dequeue atomic against the
   IKBD ISR. This is the concurrency contract the queue relies on.

4. **Blocking, but the system keeps running.** The busy-wait in `bconin2`
   optionally issues `stop_until_interrupt()` (`USE_STOP_INSN_TO_FREE_HOST_CPU`)
   so timer/VBL/keyboard interrupts continue; the machine does not freeze.

5. **No timeout on AUX.** `bconin1` blocks indefinitely on a stuck UART; only
   the IKBD path has a reset-driven recovery.

---

## Dependencies

### Requires (must be initialized / functional)

- `chardev_init()` — `CHARDEV_AVAILABLE` + `bconin_vec`.
- Handle 2: `kbd_init()`, `init_acia_vecs()`, live `ikbdiorec`.
- Handle 1: `init_serport()`.

### Used by

- GEMDOS `Cconin` (0x01), `Cnecin` (0x08), `Crawcin` (0x07), `Cauxin` (0x03),
  `Crawio` (0x06, read mode) — the console-input paths.

---

## Related data structures

- `docs/api/structures/iorec.md` — `ikbdiorec` (the 4-byte-record queue)
- `docs/api/bios/_state.md` — `bconin_vec[8]`

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically: agrees on
  the scancode/ASCII packing and on blocking.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `BCONMAP_AVAILABLE`.
- `CONF_SERIAL_CONSOLE_POLLING_MODE`: `bconin2` reads RS-232 and synthesises
  the IKBD record.

---

## NewTOS implementation notes

(Empty during Phase 00.)
