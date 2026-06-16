# `Bconmap` — XBIOS 0x2C

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/serport.c:1082` (`bconmap`).

## EmuTOS source (essence)

```c
LONG bconmap(WORD dev)
{
#if BCONMAP_AVAILABLE
    WORD old_dev = bconmap_root.mapped_device;

    if (dev == -1)  return old_dev;             /* query current */
    if (dev == -2)  return (LONG) &bconmap_root; /* expose table */

    map_index = dev - BCONMAP_START_HANDLE;
    if (map_index < 0 || map_index >= bconmap_root.maptabsize)
        return 0L;                              /* invalid */

    /* save current low-mem vectors into the previously-mapped slot */
    maptabptr = &bconmap_root.maptab[old_dev - BCONMAP_START_HANDLE];
    maptabptr->Bconstat = bconstat_vec[1]; /* +Bconin/Bcostat/Bconout/Rsconf/Iorec */

    /* install new vectors from target slot */
    maptabptr = &bconmap_root.maptab[map_index];
    bconstat_vec[1] = maptabptr->Bconstat;
    /* ... */
    rsconfptr = maptabptr->Rsconf;
    rs232iorecptr = maptabptr->Iorec;

    bconmap_root.mapped_device = dev;
    return old_dev;
#else
    return 0x2c;       /* function number — see D-008 */
#endif
}
```

`bconmap_root` is a `MAPTAB` array indexed by device number. Each
entry holds the six vectors (Bconstat/Bconin/Bcostat/Bconout/
Rsconf/Iorec) for that serial device. Device number 1 in
`bconstat_vec[1]` etc. is the "modem 1" / "current RS-232" slot
that `Bconmap` rewires.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x2C` |
| WORD args | 1 |
| Stack frame | `+2: dev (W)`, `+0: 0x002C` |
| Vector | `xbios_vecs[44]` |

## Inputs

| Pos | Name  | Type | Range / meaning                                         |
|-----|-------|------|---------------------------------------------------------|
| 0   | `dev` | WORD | `-1` query, `-2` table pointer, otherwise device number  |

`BCONMAP_START_HANDLE` is `6` (Falcon convention: device 6 = MIDI
output, 7..N = additional serial ports). EmuTOS supports `dev` in
`6..6+maptabsize-1`.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG |

| `dev`   | Return                                          |
|---------|-------------------------------------------------|
| `-1`    | currently-mapped device number                   |
| `-2`    | pointer to `bconmap_root` (`MAPTAB`)             |
| valid   | previously-mapped device number                  |
| invalid | `0L`                                             |

When `BCONMAP_AVAILABLE` is 0, returns `0x2C` (function number).

## Errors

`0L` for invalid `dev`. The `BCONMAP_AVAILABLE=0` case returns
`0x2C` — see Discrepancy D-008.

## `MAPTAB` and `bconmap_root`

```c
typedef struct {
    PFLONG  Bconstat;
    PFLONG  Bconin;
    PFLONG  Bcostat;
    PFLONG  Bconout;
    PFLONG  Rsconf;
    void   *Iorec;
} MAPTAB;

struct bconmap {
    WORD     mapped_device;     /* current dev, default 6 */
    WORD     maptabsize;        /* number of MAPTAB entries */
    MAPTAB  *maptab;            /* array of MAPTABs */
};
```

Falcon TOS ships 4 entries (dev 6..9): "modem 1", "modem 2",
"serial 1", "serial 2". EmuTOS sizes per build.

## Edge cases

### 1. Vector save-on-switch

`Bconmap` saves the current vector contents back into the *old*
slot before installing the new ones. Programs that mutated
`bconstat_vec[1]` directly therefore preserve their changes
across `Bconmap` calls — but only if they routed via `Bconmap`
not via direct vector poke.

### 2. `dev = -2` exposes mutable table

Returning `&bconmap_root` lets programs walk the `MAPTAB` array
and pre-configure entries before activating them. AES does this
on boot to install custom RS-232 handlers.

### 3. Default device

`mapped_device = 6` ("modem 1") at boot. The legacy `Rsconf`,
`Iorec`, `Bconout(1, ...)` calls all route to this slot.

### 4. NewTOS without multiple serial ports

NewTOS profile: single PL011 UART. `bconmap_root.maptabsize = 1`
with one entry. `Bconmap(6)` is a no-op (already mapped). `Bconmap`
of any other device returns `0L` (invalid).

### 5. ABI compatibility for `dev = -2`

Falcon TOS programs walk the table to detect "how many serial
ports". NewTOS will report 1 entry; programs adapt to single-port
operation.

### 6. The `0x2C` quirk

When `BCONMAP_AVAILABLE` is 0 (compile-time), returning the
function number 0x2C deliberately signals "unimplemented" via the
*positive* number quirk (D-008). NewTOS profile sets
`BCONMAP_AVAILABLE=1` with a 1-entry table, so this branch is
unreachable.

## Dependencies

- `bconmap_root`, `MAPTAB` array.
- `bconstat_vec[1]`, `bconin_vec[1]`, `bcostat_vec[1]`,
  `bconout_vec[1]`.
- `rsconfptr`, `rs232iorecptr`.

Used by: terminal programs targeting modem 2, AES routing, UI for
"select serial port" dialogs.

## Cross-references

- **Compendium:** "Bconmap" — agrees on `-1`/`-2` queries and
  device-handle convention.
- **Profibuch:** "Bconmap (0x2C)" — agrees.
- `docs/api/xbios/0e-iorec.md`, `docs/api/xbios/0f-rsconf.md`.
- `docs/api/bios/_state.md` — vector tables.

## Configuration

- `BCONMAP_AVAILABLE` (`bios/serport.c`). NewTOS: enabled with
  single-entry `MAPTAB`.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: `MAPTAB` table contains one entry pointing at the PL011
UART driver. Future expansion via USB-CDC adapters can grow the
table — programs unaware of this will simply only see device 6.
