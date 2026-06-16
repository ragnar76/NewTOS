# `OSHEADER` — Operating System Header

> Phase 00 spec D6. Source: EmuTOS @ `1a7484ab` —
> `include/biosdefs.h:44-63`. Located at the start of the ROM/RAM OS
> image; pointer in low-memory `_sysbase` (0x4F2).

## Purpose

Top-level kernel descriptor exposed to applications. Programs read
`OSHEADER` fields to query OS version, locale, the current basepage
pointer, and the magic-cookie chain entry point.

The structure is at a **well-known fixed location**: low-memory long
at `0x4F2` (`_sysbase`) holds `OSHEADER *`. The OSHEADER itself is
typically the very first bytes of the ROM, with `os_entry` doubling
as the m68k reset vector landing pad (`bra.s reseth`).

## Binary layout (44+ bytes)

```c
typedef struct _osheader {
/* 0x00 */ UWORD            os_entry;    /* BRA.S to reset handler  */
/* 0x02 */ UWORD            os_version;  /* TOS version (BCD)       */
/* 0x04 */ PFVOID           reseth;      /* reset handler ptr       */
                                          /* --- following 12 bytes are the default GEM_MUPB --- */
/* 0x08 */ struct _osheader *os_beg;     /* base of OS image        */
/* 0x0C */ UBYTE            *os_end;     /* end of static OS ST-RAM */
/* 0x10 */ PRG_ENTRY        *os_rsvl;    /* reserved (was UI entry) */
/* 0x14 */ GEM_MUPB         *os_magic;   /* GEM MUPB ptr            */
/* 0x18 */ ULONG            os_date;     /* TOS date BCD MMDDYYYY   */
/* 0x1C */ UWORD            os_conf;     /* PAL flag + country code */
/* 0x1E */ UWORD            os_dosdate;  /* TOS date in BDOS format */
/* 0x20 */ WORD             **os_root;   /* ptr to BDOS quick pool  */
/* 0x24 */ UBYTE            *os_kbshift; /* ptr to BIOS kbshift var */
/* 0x28 */ struct _pd       **os_run;    /* ptr to current PD ptr   */
/* 0x2C */ ULONG            os_dummy;    /* 'ETOS' signature        */
/* 0x30 — end */
} OSHEADER;
```

### `GEM_MUPB` (12 bytes, embedded at offset 0x08)

```c
typedef struct {
    ULONG     gm_magic;  /* must be 0x87654321 (GEM_MUPB_MAGIC) */
    UBYTE    *gm_end;    /* end of static OS ST-RAM */
    PRG_ENTRY *gm_init;  /* main UI entry point (AES) */
} GEM_MUPB;
```

Pre-1.04 ROMs contained a `GEM_MUPB` directly inline in the OSHEADER
at offsets 0x08..0x13; from 1.04 onwards the inline three longs were
re-purposed (`os_beg`, `os_end`, `os_rsvl`) and `os_magic` at 0x14
points to a separately-located GEM_MUPB. EmuTOS keeps the modern
layout.

## Field semantics

### `os_entry` (0x00)

Two-byte `BRA.S reseth` opcode. The CPU executes this on cold reset
(via the reset vector at 0x00000000 → 0x00000004 → OSHEADER) which
implicitly fetches `os_entry` and branches into the actual reset
handler. **m68k-specific**; on AArch64 the reset path is utterly
different — NewTOS will populate this field with a meaningful pattern
only when the m68k user emulator runs.

### `os_version` (0x02)

BCD-packed: high byte = major, low byte = minor. Examples:
- `0x0102` → TOS 1.02
- `0x0206` → TOS 2.06
- `0x0404` → TOS 4.04

EmuTOS reports a version compatible with the configured target
(typically `0x0306` to claim TOS 3.06 compatibility). NewTOS will
report `0x0400` to match the latest pre-Falcon Atari TOS.

### `reseth` (0x04)

C-language reset handler. Called via `os_entry`'s branch.

### `os_beg` (0x08)

Address of the OSHEADER itself (the OS image base). Programs use this
to locate themselves in memory; bootstraps use it to compute the OS
image size.

### `os_end` (0x0C)

End of statically allocated OS ST-RAM. The TPA begins here (rounded
up). On Atari hardware: end of OS RAM workspace, before any video
RAM. On NewTOS: end of kernel BSS, start of available RAM for user
processes.

### `os_rsvl` (0x10)

Reserved. On classic TOS, was the AES entry point address.
EmuTOS sets `os_rsvl = 0` (AES entry is reached via `os_magic →
gm_init`).

### `os_magic` (0x14)

Pointer to the active `GEM_MUPB`. Bootstraps walk this to find the
AES entry point at boot. NewTOS will only populate this once
GEM/AES is reimplemented (PHASE-12 and beyond).

### `os_date` (0x18)

TOS build date, BCD `MMDDYYYY`. EmuTOS sets it from the build
timestamp. Programs sometimes use this to detect ROM version when
`os_version` collides between vendors.

### `os_conf` (0x1C)

```
bit 0       : 0 = NTSC, 1 = PAL
bits 1..7   : country code (0=USA, 1=DE, 2=FR, ...)
bits 8..15  : reserved
```

Set at build time from `CONF_LANG` / `CONF_PAL`. NewTOS-relevant:
country code drives keyboard layout selection in
`bios/country.c`. PAL/NTSC bit drives the default video mode in
classic Atari hardware; on NewTOS this bit will be ignored once
PHASE-08 sets video mode from the BCM2711 framebuffer config.

### `os_dosdate` (0x1E)

Same date as `os_date` but in FAT-packed format
(see `docs/api/structures/ofd.md` `DOSTIME`).

### `os_root` (0x20)

Pointer to the BDOS "quick pool" head. Internal to GEMDOS memory
allocator; not generally consumed by applications. EmuTOS exposes it
for compatibility with debuggers and disk caches that walk the pool.

### `os_kbshift` (0x24)

Pointer to the single byte that holds the current keyboard shift
state (the same value `Kbshift` returns). Programs that poll
keyboard modifiers without trapping read this byte directly. The
byte itself lives at `0xE1B` in classic TOS; EmuTOS places it
wherever the BIOS chose, and exposes the address here.

### `os_run` (0x28)

Pointer to the **pointer** to the currently-running PD. So:
```c
PD *current = *(osheader->os_run);
```

The double-indirection is so that the PD-pointer storage location
itself stays fixed while the value (current PD) updates on every
process switch / `Pexec`. Programs read this to obtain their own
basepage when they were not given it via the m68k entry-stack
convention.

### `os_dummy` (0x2C)

Four-byte signature. EmuTOS sets it to `'ETOS'` (`0x45544F53`) so
applications can distinguish EmuTOS from Atari TOS. NewTOS will set
it to `'NTOS'` (`0x4E544F53`).

## Position in low memory

```
0x000000  reset vector  → 0x000004
0x000004  initial PC    → OSHEADER + 0x00 (os_entry, BRA.S reseth)
...
0x0004F2  _sysbase      → &OSHEADER  (UBYTE*)
0x0005A0  ...
```

`_sysbase` is the documented mechanism for finding the OSHEADER
without knowing its address. NewTOS must populate `_sysbase` for
m68k-emulator-running programs.

## Cross-references

- **Compendium:** "OSHEADER" / "_sysbase" — agrees on layout.
- **Profibuch:** "OS-Header" — agrees, including 1.04 transition.
- **`docs/api/structures/basepage.md`** — `os_run` reaches PD.
- **`docs/api/xbios/_dispatcher.md`** (TBD ref) — XBIOS doesn't read
  OSHEADER but lives in the same image.
- **EmuTOS source:** `bios/startup.S` (low-memory layout),
  `bios/bios.c:os_header` (OSHEADER definition).

## Configuration

- `CONF_LANG` — drives `os_conf` country code.
- `CONF_PAL` — drives `os_conf` bit 0.
- Build date drives `os_date`/`os_dosdate`.

## NewTOS implementation notes

(Empty during Phase 00. PHASE-11 / PHASE-12 will populate the
m68k-visible OSHEADER once the user emulator can run programs that
read `_sysbase`.)
