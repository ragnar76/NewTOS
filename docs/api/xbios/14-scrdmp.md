# `Scrdmp` — XBIOS 0x14

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/xbios.c:490` (`scrdmp`).

## EmuTOS source

```c
static void scrdmp(void)
{
    protect_v((PFLONG)dump_vec);
    dumpflg = -1;            /* re-enable future dumps */
}
```

`dump_vec` is the OS variable at addr `0x502`, a function pointer
to the screen-dump routine (defaults to `dmpfunc` in
`bios/screen.c`). `protect_v` is a marshalling wrapper that calls
the function while preserving caller registers and aligning stack.
`dumpflg` is the OS variable at addr `0x4EE` — when set to ≠ 0,
the IKBD interrupt handler observing Alt-Help triggers a dump on
the next VBL.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x14` |
| WORD args | 0 |
| Stack frame | `+0: 0x0014` |
| Vector | `xbios_vecs[20]` |

## Inputs

None.

## Outputs

`void`. D0 == 0 from dispatcher default.

## Errors

None.

## Edge cases

### 1. `dump_vec` redirection

Programs override the dump destination (printer / file / network)
by writing a new pointer to `dump_vec` (address 0x502). The
default `dmpfunc` reads pixels and emits a printer-byte stream via
`Bconout(0)` (printer device).

### 2. `dumpflg` interaction

The Alt-Help key sets `dumpflg = 0`; the VBL handler checks
`dumpflg` and calls `Scrdmp` when zero. After a dump, `Scrdmp`
sets `dumpflg = -1` to re-arm. A program calling `Scrdmp`
explicitly bypasses Alt-Help.

### 3. No printer = silent no-op

Default `dmpfunc` writes through `Bconout(0)`. NewTOS profile
disables printer device — `Bconout(0)` returns ENODEV. The dump
still runs; output is discarded.

### 4. Synchronous

`Scrdmp` blocks until the entire dump is sent. On a 9-pin printer,
that's tens of seconds.

### 5. Stack usage

`protect_v` calls a vector through an indirect pointer. EmuTOS's
implementation in `vectors.S` saves D0..D7/A0..A6 (60 bytes). The
called function must respect this convention — Atari-style
register preservation, not C ABI.

## Dependencies

- `dump_vec` (OS var 0x502).
- `dumpflg` (OS var 0x4EE).
- `protect_v` (vectors.S).
- `Bconout(0)` (printer) — default dump target.

Used by: Alt-Help via VBL handler, programs offering a "print
screen" function.

## Cross-references

- **Compendium:** "Scrdmp" — agrees, notes `dump_vec` redirection.
- **Profibuch:** "Scrdmp (0x14)" — agrees.
- `docs/api/xbios/_state.md` (pending) — `dumpflg` / `dump_vec`.

## Configuration

None at XBIOS level; printer support gates the actual dump
behaviour.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: `dump_vec` is preserved as ABI hook. NewTOS replaces the
default function with one that writes a PNG/BMP into the
filesystem (path configurable). Programs that override the vector
work unchanged, but their printer-byte-stream output writes to
`/dev/null` since printer device returns ENODEV.
