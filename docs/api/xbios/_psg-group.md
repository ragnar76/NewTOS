# YM2149 PSG group — XBIOS 0x1C / 0x1D / 0x1E / 0x20

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/sound.c:94/115/130/145`.
>
> NewTOS profile: no YM2149 hardware. All four functions return
> 0 immediately (the EmuTOS `#else` branch — *not* `xbios_unimpl`,
> so callers see plausible defaults).

This spec covers four small, related functions:

| ID    | Name      | Args                   | NewTOS return |
|------:|-----------|------------------------|---------------|
| 0x1C  | Giaccess  | data:WORD, reg:WORD    | `0`           |
| 0x1D  | Offgibit  | mask:WORD              | `void`        |
| 0x1E  | Ongibit   | mask:WORD              | `void`        |
| 0x20  | Dosound   | table:UBYTE *          | `0`           |

The PSG (Programmable Sound Generator, Yamaha YM2149) on Atari ST
implements both audio and a parallel I/O port (Port A — drive
select, side select, RTS, DTR, parallel-port strobe).

## EmuTOS source

```c
LONG giaccess(WORD data, WORD reg)
{
#if CONF_WITH_YM2149
    WORD old_sr = set_sr(0x2700);
    PSG->control = reg & 0xF;
    if (reg & GIACCESS_WRITE)  PSG->data = data;
    LONG value = PSG->control;        /* read-back */
    set_sr(old_sr);
    return value;
#else
    return 0;
#endif
}

void ongibit(WORD data) { /* atomic OR into PSG_PORT_A */ }
void offgibit(WORD data){ /* atomic AND into PSG_PORT_A */ }

LONG dosound(const UBYTE *table)
{
#if CONF_WITH_YM2149
    const UBYTE *oldtable = sndtable;
    if ((LONG)table >= 0) { sndtable = table; snddelay = 0; }
    return (LONG)oldtable;
#else
    return 0;
#endif
}
```

## Trap interface

Standard XBIOS. Stack frames:

- 0x1C: `+4: reg (W), +2: data (W), +0: 0x001C`
- 0x1D: `+2: mask (W), +0: 0x001D`
- 0x1E: `+2: mask (W), +0: 0x001E`
- 0x20: `+2: table (L), +0: 0x0020`

## Per-function details

### `Giaccess` (0x1C)

| | |
|--|--|
| Vector | `xbios_vecs[28]` |
| `reg` low 4 bits | PSG register 0..15                    |
| `reg` bit 7 (`GIACCESS_WRITE`, `0x80`) | 1 = write, 0 = read |
| `data` | byte to write (low 8 bits)                  |
| Returns | previous value at the selected register     |

PSG registers:

| Reg | Purpose                            |
|----:|-------------------------------------|
| 0   | Channel A frequency low              |
| 1   | Channel A frequency high (4 bits)    |
| 2   | Channel B frequency low              |
| 3   | Channel B frequency high (4 bits)    |
| 4   | Channel C frequency low              |
| 5   | Channel C frequency high (4 bits)    |
| 6   | Noise frequency (5 bits)             |
| 7   | Mixer / I/O direction                |
| 8   | Channel A volume                     |
| 9   | Channel B volume                     |
| 10  | Channel C volume                     |
| 11  | Envelope frequency low               |
| 12  | Envelope frequency high              |
| 13  | Envelope shape                       |
| 14  | Port A (drive/side/RTS/DTR/strobe)   |
| 15  | Port B (parallel port data byte)     |

### `Ongibit` / `Offgibit` (0x1D, 0x1E)

| | |
|--|--|
| Vectors | `xbios_vecs[29]`, `xbios_vecs[30]` |
| `Ongibit(mask)` | sets bits where `mask` has 1 |
| `Offgibit(mask)` | clears bits where `mask` has 0 |

Atomic read-modify-write on PSG Port A (register 14). The "and"
in `Offgibit` is intentional: pass `~bit_to_clear` to clear a
specific bit (parport.c:93 calls `offgibit(~GI_STROBE)`).

### `Dosound` (0x20)

| | |
|--|--|
| Vector | `xbios_vecs[32]` |
| `table` | pointer to a sound script |
| `table == -1` | query; returns current `sndtable` |
| `table == NULL` | stop sound playback |
| Returns | previous `sndtable` pointer |

Sound script format: a sequence of opcode bytes consumed by
`sndirq` from the 50 Hz VBL. Opcodes:

- `0x00..0x0F`: load PSG register (next byte = value)
- `0x80`: set 8-bit signed counter from next byte
- `0x81`: do `next byte` decrement steps
- `0x82..0xFF`: pause N VBLs

EmuTOS's `sndirq` runs the table tick by tick; complete description
deferred to per-function spec at PHASE-07 time.

## Outputs (NewTOS profile)

All four return defaults — `Giaccess` and `Dosound` return `0`,
`Ongibit`/`Offgibit` are void. No errors.

## Errors

None.

## Edge cases

### 1. SR clobber to 0x2700

All four functions raise IPL to 7 around the PSG access (atomic
register access is critical: a VBL handler also writes Port A for
disk-drive select). NewTOS without YM2149 hardware skips the SR
manipulation.

### 2. Port A bit assignments (Atari ST)

| Bit | Function                 |
|----:|--------------------------|
| 0   | Side 0/1 select (floppy) |
| 1   | Drive 0 select           |
| 2   | Drive 1 select           |
| 3   | RTS (RS-232)             |
| 4   | DTR (RS-232)             |
| 5   | Parallel-port strobe     |
| 6   | reserved (GPO)           |
| 7   | reserved (GPO)           |

Programs hitting Port A bypass `Floprd` etc. — direct hardware
control. NewTOS without ST hardware silently ignores.

### 3. `Dosound` interrupt-driven

The actual playback runs in `sndirq` (called from VBL). NewTOS
synthesises a no-op `sndirq` so legacy programs that also call
`Dosound(NULL)` to stop see consistent behaviour (ABI: both calls
return 0; nothing audible).

### 4. Atomicity of `Ongibit`/`Offgibit`

The "atomic" claim depends on IPL=7 lockout of VBL/MFP. On
multicore aarch64 NewTOS would need a spinlock — but since there
is no hardware to access, the operations are effectively no-ops
and trivially atomic.

## Dependencies

- (Atari) PSG MMIO at `0xFF8800/0xFF8802`.
- `sndtable`, `snddelay`, `sndirq` (`bios/sound.c`).
- `set_sr` (cpu.h).

Used by: drive-select code, RS-232 handshake, parallel-port
strobe, sound effects, classic Atari games for music.

## Cross-references

- **Compendium:** all four functions documented; agrees on
  semantics.
- **Profibuch:** agrees, lists Port A bits.

## Configuration

- `CONF_WITH_YM2149`. NewTOS profile: not present.

## NewTOS implementation notes

(Empty during Phase 00.)

**ABI choice:** EmuTOS-without-YM returns `0` — *not* the
`xbios_unimpl` "function number" pattern. NewTOS follows EmuTOS
to avoid breaking programs that probe `Giaccess(0, n)` and check
for `==0`. Programs touching Port A for drive select / RS-232
handshake will not function correctly on NewTOS, but they would
not function on a Raspberry Pi anyway — this is intentional ABI
silence.

PHASE-07 may add a NewTOS-extension audio path: `Dosound(NULL)`
remains a no-op; a new XBIOS slot exposes 16-bit PCM playback via
the audio block of the VideoCore.
