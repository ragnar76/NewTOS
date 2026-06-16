# `Sversion` — GEMDOS 0x30

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/bdosmain.c:298` (`xgetver`).

## EmuTOS source

```c
static long xgetver(void) { return (long)GEMDOS_VERSION; }
```

`GEMDOS_VERSION` is a build constant (`include/config.h:1590`),
default `0x2000`.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0030` |
| WORD args | 0 |
| Stack frame | `+0: 0x0030` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (packed version) |

### Encoding (Atari convention — **byte-swapped**)

Compendium specifies the return as a 16-bit value where:
- low byte = major version
- high byte = minor version

So `0x2000` ⇒ major 0x00, minor 0x20 ⇒ "GEMDOS version 0.32 / TOS
v2.06–v3.06 era". Programs typically display it as
`(D0 >> 8) & 0xFF` "." `D0 & 0xFF` — i.e. minor.major.

| Constant | Atari TOS era                 |
|----------|-------------------------------|
| 0x0F00   | TOS 1.00                      |
| 0x1300   | TOS 1.0/1.02                  |
| 0x1500   | TOS 1.04, 1.06                |
| 0x1700   | TOS 1.62                      |
| 0x1900   | TOS 2.01, 2.05, 3.01, 3.05    |
| 0x2000   | TOS 2.06, 3.06 (EmuTOS default) |
| 0x3000   | TOS 4.0x (Falcon)             |

High 16 bits of the LONG return are zero.

## Errors

None.

## Edge cases

- **Build-time choice**: NewTOS will report a version chosen for
  the simulated TOS profile. Reporting too-old a version causes
  programs to refuse to use newer GEMDOS calls (Maddalt, Mxalloc,
  etc.); reporting too new may cause programs to invoke calls that
  don't exist. Default `0x2000` matches the EmuTOS reference
  feature set (Maddalt available, GEMDOS-modern semantics).

- **Not a feature flag**: programs cannot reliably probe individual
  function presence via Sversion. The version-to-feature table is
  unwritten Atari lore. Some programs do their own `Setexc(33,
  -1)` to read the GEMDOS trap-1 vector and inspect the dispatcher
  — outside the API contract.

## Dependencies

- `GEMDOS_VERSION` build constant.

## Cross-references

- **Compendium:** "Sversion" — agrees.
- **Profibuch:** "Sversion (0x30)" — agrees.
- `include/config.h:1581-1592` (build matrix).

## Configuration

- `GEMDOS_VERSION` — set per-build in `config.h`. NewTOS will
  default to `0x2000` to match the EmuTOS feature baseline.

## NewTOS implementation notes

(Empty during Phase 00.)

Decision pending for PHASE-02: confirm `0x2000` as the reported
version. Rationale: matches feature set we're implementing
(Mxalloc, Maddalt, Pexec mode 6, etc.).
