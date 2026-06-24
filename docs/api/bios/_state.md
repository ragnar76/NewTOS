# BIOS Subsystem State

> Phase 00 deliverable D7 (BIOS, partial). Documents the BIOS-internal
> state required by spec entries in this group. Source: EmuTOS @
> `1a7484ab`.

> **Observable vs. internal** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> **Contract** (programs observe — keep value/semantics exact): the
> character-device vector tables a program can intercept, the device-handle
> mapping (`bconmap_root`), the VT52 console *behaviour*. **Free to
> reimplement** (unobservable): `boot_status` flags, how the vector tables
> are stored internally, `kprintf` routing.

## `boot_status` flags (`bios/bios.h:80-95`)

```c
#define DEVICES_INITIALIZED 0x01
#define MACHINE_INITIALIZED 0x02
#define DOS_AVAILABLE       0x04
#define LINEA_AVAILABLE     0x08
#define CHARDEV_AVAILABLE   0x10
#define RAM_VALIDATED       0x20
#define TPA_INITIALIZED     0x40
#define DRIVER_LOADED       0x80
```

`CHARDEV_AVAILABLE` is set by `bios/bios.c:430` after `chardev_init()`
returns. Until then, all four character-device BIOS calls
(`Bconstat`/`Bconin`/`Bconout`/`Bcostat`) return `0L`.

NewTOS will need an equivalent guard so that early panic messages
emitted via `kprintf` before `chardev_init` route to the dedicated
debug-UART path, not the GEMDOS/BIOS console layer.

## Character-device vector tables (`bios/chardev.c`, `bios/tosvars.S`)

Four 8-entry function-pointer tables, all declared in
`include/tosvars.h` and located at the canonical TOS low-RAM addresses
in `bios/tosvars.S`:

```c
extern LONG  (*bconstat_vec[8])(void);
extern LONG  (*bconin_vec [8])(void);
extern LONG  (*bcostat_vec[8])(void);
extern LONG  (*bconout_vec[8])(WORD dev, WORD ch);
```

Initial wiring (`bios/chardev.c:30-44`):

| Handle | bconstat_vec  | bconin_vec  | bcostat_vec       | bconout_vec     |
|-------:|---------------|-------------|-------------------|-----------------|
| 0      | `char_dummy`  | `bconin0`   | `bcostat0`        | `bconout0`      |
| 1      | `bconstat1`   | `bconin1`   | `bcostat1`        | `bconout1`      |
| 2      | `bconstat2`   | `bconin2`   | `bcostat2`        | `bconout2`      |
| 3      | `bconstat3`   | `bconin3`   | **`bcostat4`** *  | `bconout3`      |
| 4      | `char_dummy`  | `char_dummy`| **`bcostat3`** *  | `bconout4`      |
| 5      | `char_dummy`  | `char_dummy`| `char_dummy`      | `bconout5`      |
| 6      | `char_dummy`  | `char_dummy`| `char_dummy`      | `charout_dummy` |
| 7      | `char_dummy`  | `char_dummy`| `char_dummy`      | `charout_dummy` |

\* MIDI (handle 3) and IKBD (handle 4) `bcostat` slots are swapped on
purpose. Source comment: `/* note that IKBD and MIDI bcostat() are
swapped! */` (`bios/chardev.c:42`). This swap is **historical Atari
behaviour** and applications depend on it.

Tables are arrays of pointers, not jump tables — TOS programs may
overwrite individual entries to install custom drivers. NewTOS must
preserve the table-at-fixed-address contract for binary
compatibility once the m68k user emulator is in place (Phase 11).

## Auxiliary hooks (`bios/chardev.c:75-79`)

```c
aux_stat = just_rts;
aux_vec  = just_rts;
prt_stat = just_rts;
prt_vec  = just_rts;
dump_vec = just_rts;
```

These are five additional indirect-call slots used by old-style
(pre-vec) printer/serial filters, and by `Scrdmp` (XBIOS 0x14):

| Symbol     | Purpose                                                     |
|------------|-------------------------------------------------------------|
| `aux_stat` | optional pre-Bconstat hook for AUX                          |
| `aux_vec`  | optional pre-Bconout hook for AUX                           |
| `prt_stat` | optional pre-Bcostat hook for PRN                           |
| `prt_vec`  | optional pre-Bconout hook for PRN                           |
| `dump_vec` | screen-dump hook called by `Scrdmp`                         |

Default value `just_rts` is a no-op `rts` opcode at a known low-RAM
location. Replacing these with custom code is the documented
extension mechanism for printer drivers.

## `bconmap_root` (`bios/bconmap.c`, gated by `BCONMAP_AVAILABLE`)

Provides per-handle remap for handles ≥ `BCONMAP_START_HANDLE`
(canonically 6). Used by `Bconmap` (XBIOS 0x2C) to switch which
hardware port responds to handles 6/7. Each `MAPTAB` entry holds the
four function pointers (`Bconstat`/`Bconin`/`Bcostat`/`Bconout`) for
one mapped device, plus IOREC pointers for the input record returned
by `Iorec` (XBIOS 0x0E).

NewTOS scope: deferred. The TT-baseline configuration that NewTOS
targets has `BCONMAP_AVAILABLE` enabled but the user-visible API is
limited to handles 0–5 in the early phases.

## VT52 console state (`bios/conout.c`, `lineavars.h`)

The screen handler (handle 2) keeps:
- `v_cur_xy.x`, `v_cur_xy.y` — cursor position
- `v_cur_of` — offset of cursor cell in screen RAM
- `escape_state` — current state of the VT52 escape parser
- `escape_y` / `escape_x` — staging cells for `<ESC>Y r c`
- foreground/background colour (Line-A vars)

The full layout is part of the Line-A variable block — applications
read these directly. NewTOS must preserve the layout for binary
compatibility (deferred to Line-A spec, D10).

## `kprintf()` early routing (`bios/kprint.c:159`)

```c
if (boot_status & CHARDEV_AVAILABLE) {
    /* route to bconout(2, ...) */
} else {
    /* route to native debug print (NatFeats / framebuffer / serial) */
}
```

NewTOS analogue: route early panic + boot logs through the dedicated
PL011 debug UART, switching to GEMDOS/BIOS console only after
`chardev_init` (NewTOS-equivalent).

## Cross-references

- `docs/api/bios/_dispatcher.md` — trap-13 trampoline.
- `docs/api/bios/01-bconstat.md` … `08-bcostat.md` — uses these tables.
- `docs/api/_dependency-graph.md` — implementation order.
