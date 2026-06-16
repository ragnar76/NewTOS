# `Cprnout` — GEMDOS 0x05

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xprtout()`.

## EmuTOS source

- **Primary:** `bdos/console.c:338` — `xprtout()`
- **Helpers:** `Bconout` (BIOS 0x03)
- **Globals read:** `run->p_uft[3]` (stdprn standard handle)
- **Build config:** `CONF_WITH_PRINTER_PORT` (transitive).

## Source extraction

```c
long xprtout(int ch)
{
    return Bconout(HXFORM(run->p_uft[3]), ch);
}
```

Default `p_uft[3] = H_Print` → `HXFORM(-3) = 0` → BIOS Centronics handle.

Pure pass-through. **No** retry loop on busy printer at the GEMDOS
level — the BIOS `bconout0` (parport) returns 0 if the printer is busy;
the application is expected to retry.

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0005` |
| WORD args | 1 |
| Stack frame | `+2: ch (WORD)`, `+0: 0x0005` |

## Inputs

| Pos | Name | Type | Notes                                         |
|-----|------|------|-----------------------------------------------|
| 0   | `ch` | WORD | low 8 bits → byte to print                    |

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Encoding | non-zero when the byte was accepted by the printer; 0 if printer not ready (timeout). |

This deviates from the other "send a byte" functions (`Cconout`,
`Cauxout`) which are unconditional — `Cprnout` is the one place where
the GEMDOS layer surfaces hardware ready/busy.

## Errors

| Condition                                | Returned value |
|------------------------------------------|----------------|
| Printer not ready (parport BUSY timeout) | `0`            |
| Printer accepted byte                    | non-zero       |

## Edge cases

- **No buffering**: each call drives one byte directly to the parport;
  no internal queue. High-throughput printer code should poll
  `Cprnos` (0x11) before each `Cprnout` to avoid wasted retries.
- **Standard-handle redirection**: dispatcher writes the byte to the
  redirected file; the printer-busy contract changes (file write
  always "succeeds", returning 0 from the dispatcher's hardcoded
  rawout path).

## Dependencies

- `Bconout(h, c)`
- `run->p_uft[3]`

Used by: programs producing printer output, the `Cconout` redirector
when the user has Ctrl-P enabled (handled at a higher level — see
`bdos/console.c`/`conterm` flag, not in this function).

## Cross-references

- **Compendium:** "Cprnout" — agrees on busy-returns-0 semantic.
- **Profibuch:** "Cprnout (5)" — agrees.

## Configuration / version

- `CONF_WITH_PRINTER_PORT = 0` removes the underlying BIOS parport
  driver. NewTOS will not target a physical printer in early phases —
  the spec is recorded for completeness; implementation may stub
  `Bconout(0, c)` as a virtual sink.

## NewTOS implementation notes

(Empty during Phase 00.)
