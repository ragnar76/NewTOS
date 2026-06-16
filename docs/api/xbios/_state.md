# XBIOS Subsystem State

> Phase 00 deliverable D7 (XBIOS). Documents the XBIOS-internal state
> referenced by spec entries in this group. Source: EmuTOS @ `1a7484ab`.

XBIOS owns less mutable state than GEMDOS — most of its surface is
direct hardware access plus a handful of OS variables in low memory.
This file describes those variables and the small number of tables
XBIOS maintains in higher memory.

---

## 1. `kbdvecs` (`bios/ikbd.c:64`, returned by `Kbdvbase` 0x22)

```c
struct kbdvecs_t {
    PFVOID midivec;     /* MIDI input byte */
    PFVOID vkbderr;     /* IKBD overrun / parity error */
    PFVOID vmiderr;     /* MIDI ACIA overrun */
    PFVOID statvec;     /* IKBD status packet (0xF6) */
    PFVOID mousevec;    /* IKBD mouse packet (0xF8..0xFB) */
    PFVOID clockvec;    /* IKBD time-of-day packet (0xFC) */
    PFVOID joyvec;      /* IKBD joystick packet (0xFD..0xFF) */
    PFVOID midisys;     /* MIDI system handler */
    PFVOID ikbdsys;     /* IKBD system handler */
} kbdvecs;
```

36 bytes, 9 function pointers. `Kbdvbase` returns `&kbdvecs` so
applications can install custom packet handlers (mouse, joystick,
clock).

NewTOS: preserve as ABI surface. The synthetic IKBD-packet generator
(USB-HID translator) feeds these vectors in the same byte format.

---

## 2. `bioskeys` State (`bios/bios.c:140`, used by `Bioskeys` 0x18, `Keytbl` 0x10, `Kbrate` 0x23)

```c
struct keytbl current_keytbl;       /* unshift, shift, caps */
WORD          kb_initial = 25;      /* repeat-delay (50 Hz units) */
WORD          kb_repeat  = 5;       /* repeat-rate (50 Hz units) */
```

Set by:
- `Bioskeys` resets `current_keytbl` to defaults from `country.c`.
- `Keytbl(unshift, shift, caps)` overwrites individual table pointers
  (sentinel `(UBYTE*)-1` keeps current).
- `Kbrate(initial, repeat)` updates the two WORDs.

Read by IKBD interrupt handler when translating scancodes to ASCII.

NewTOS: software shadow only; HID-translator consumes the same
tables.

---

## 3. RS-232 / `bconmap` State (`bios/serport.c`)

### `bconmap_root` (`MAPTAB`-array root, see [2c-bconmap.md](2c-bconmap.md))

```c
struct bconmap {
    WORD     mapped_device;     /* current dev, default 6 */
    WORD     maptabsize;        /* number of MAPTAB entries */
    MAPTAB  *maptab;            /* array */
} bconmap_root;
```

NewTOS profile: `maptabsize = 1`, single PL011 entry.

### `rsconfptr`, `rs232iorecptr`

Pointers to the current RS-232 device's `Rsconf` handler and IORec
buffer. Rewired by `Bconmap` from the active `MAPTAB` entry.

### IORec layout (`bios/serport.h`, used by `Iorec` 0x0E)

Per-device: 16 bytes (input ringbuf head/tail/size/pointer +
output ringbuf), tracked separately for IKBD, MIDI, RS-232.
`Iorec(dev)` returns the appropriate record pointer.

---

## 4. Display State

### Falcon / TT / ST screen pointers (`bios/screen.c`)

```c
extern UBYTE *v_bas_ad;     /* logical screen base, OS var 0x44E */
extern UWORD  sshiftmod;    /* shift-mode shadow */
extern WORD   sscreen[2];   /* physical/logical screen geometry */
```

NewTOS framebuffer driver maintains shadow of all three; writes are
mirrored to the VideoCore mailbox on `Setscreen`.

### TT shifter shadow (see [_tt-shifter.md](_tt-shifter.md))

```c
UWORD tt_shifter_shadow;            /* mirrors *0xFFFF8262 */
UWORD tt_palette_shadow[256];       /* mirrors *0xFFFF8400 */
WORD  has_tt_shifter;               /* 1 in NewTOS profile */
```

### Palette (`bios/screen.c`, OS var `palmode_flags`)

Software shadow of the ST/STe 16-entry palette, used by `Setpalette`
0x06 and `Setcolor` 0x07. NewTOS forwards changes to VideoCore.

---

## 5. Screen Dump (`bios/xbios.c`, `Scrdmp` 0x14)

```c
void (*dump_vec)(void);      /* OS var 0x502, default = dmpfunc */
WORD  dumpflg;               /* OS var 0x4EE, -1 = idle */
```

`dumpflg = 0` triggers a dump on next VBL; `Scrdmp` resets it to -1.
Programs replace `dump_vec` to redirect output (file, network,
PostScript, …).

NewTOS: preserve both vars; default `dmpfunc` writes a PNG to
configured path instead of the printer-byte stream.

---

## 6. PSG / Sound (`bios/sound.c`, `Dosound` 0x20)

```c
const UBYTE *sndtable;       /* current sound script, NULL = idle */
UBYTE        snddelay;       /* VBL delay counter */
```

NewTOS profile: no YM2149; both unused. `Dosound(NULL)` returns 0.

---

## 7. Random Seed (`bios/xbios.c`, `Random` 0x11)

```c
LONG  seed;                  /* LCG state, seeded from hz_200 at boot */
```

24-bit LCG state. Updated on every `Random` call. ABI-relevant: the
documented sequence is reproducible from a known seed.

---

## 8. `dumpflg` / VBL hooks (`bios/vectors.S`)

```c
void (*vblqueue[8])(void);   /* OS var 0x456, slot 0..7 */
WORD  nvbls;                 /* number of slots, OS var 0x454 */
```

VBL queue installed by AES, `vsync()` waits one tick. Specs that
reference the VBL (e.g. `Vsync` 0x25, `Setpalette` 0x06 timing) read
this state but do not modify it from XBIOS surface.

---

## 9. Concurrency

XBIOS is also single-threaded under EmuTOS. Hardware register
accesses (PSG, palette, MFP) are bracketed by `set_sr(0x2700)` to
lock interrupts. NewTOS replaces SR-locking with explicit per-device
spinlocks; the ABI surface still ignores SMP because the trap
dispatcher is serialised per-CPU.

---

## Cross-references

- Per-function specs in `docs/api/xbios/`.
- BIOS state — `docs/api/bios/_state.md`.
- GEMDOS state — `docs/api/gemdos/_state.md`.
- Structures — `docs/api/structures/`.
