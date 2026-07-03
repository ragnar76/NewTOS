# `Mediach` (BIOS 0x09)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/bios.c:1184` (`mediach`) and the installed handler
> `bios/blkdev.c:783` (`blkdev_mediach`).

## EmuTOS source

**Primary location:** `bios/bios.c:1184` — `LONG mediach(WORD drv)`
(body: `return protect_w(hdv_mediach, drv);`).

**Handler behind the vector:** `bios/blkdev.c:783` — `blkdev_mediach()`,
installed as `hdv_mediach` by `blkdev_init()` (`bios/blkdev.c:103`).

**Helpers called:**
- `util/miscasm.S:225` — `protect_w(func, drv)` — d2/a2-preserving thunk.
- `bios/floppy.c` — `flop_mediach(dev)` (units < `NUMFLOPPIES`).
- `bios/disk.c` — `disk_mediach(unit)` (hard-disk units).
- `bios/blkdev.c` — `call_etv_critic()` (critical-error retry).

**Headers / type definitions used:**
- `bios/disk.h:69-71` — `MEDIANOCHANGE` (0), `MEDIAMAYCHANGE` (1),
  `MEDIACHANGE` (2)
- `include/gemerror.h` — `EUNDEV` (-15)
- `include/tosvars.h` — `hdv_mediach`

**System variables / globals read/written:**
- `hdv_mediach` (TOS var **`0x47e`**) — the media-change vector
  (`bios/blkdev.c:103`)
- `blkdev[dev].mediachange` (read; latched), `blkdev[dev].forcechange` (read)
- `units[unit].status` (`UNIT_CHANGED` bit set on change, `bios/blkdev.c:815`)

**Build configuration affecting this function:** the underlying
`flop_mediach`/`disk_mediach` behaviour depends on the floppy/ACSI/IDE/SCSI
driver `CONF_*` flags (behaviour reference).

---

## Source extraction summary

`Mediach` reports whether the media in a drive may have changed since the last
access, as a 3-state value. The trap-level `mediach` forwards to
`hdv_mediach`; on EmuTOS that is `blkdev_mediach`.

`blkdev_mediach` validates the drive (`0..BLKDEVNUM-1`, `DEVICE_VALID`) and
returns `EUNDEV` for an invalid one. If the drive is already latched as
`MEDIACHANGE`, it returns that immediately. If a `forcechange` is armed (e.g.
by a prior write to logical sector 0, see `04-rwabs.md`) it returns
`MEDIACHANGE`. Otherwise it calls `flop_mediach`/`disk_mediach` (with
`call_etv_critic` retry). If that returns a **negative** error, `Mediach`
**returns that error unchanged**. If it reports a change (`!= MEDIANOCHANGE`),
it marks `units[unit].status |= UNIT_CHANGED` and latches
`blkdev[dev].mediachange`. Finally it returns the latched `mediachange`.

The 3-state distinction (`MAYCHANGE` vs `CHANGE`) exists because some hardware
cannot tell "new disk" from "same disk reinserted" without re-reading the boot
sector; `MEDIAMAYCHANGE` forces GEMDOS to re-validate the BPB/serial.

---

## Trap interface

- **Trap number:** 13 (BIOS)
- **Function ID:** 0x09
- **Stack frame on entry (m68k):**
  ```
  +2: drv (word)
   0: function_id (word) = 0x0009
  ```
- **Total parameter bytes:** 4
- **Native (AArch64) ABI:** ID `0x09` in `x8`; `drv` in `x0`; result
  sign-extended into `x0` (0/1/2, or a negative error code).

  | m68k stack | AArch64 reg | Name  | Type / size |
  |------------|-------------|-------|-------------|
  | +2         | `x0`        | `drv` | `WORD` (16) |

---

## Inputs

| Position | Name  | Type   | Size | Source location     | Notes |
|----------|-------|--------|------|---------------------|-------|
| arg0     | `drv` | `WORD` | 2    | `bios/blkdev.c:783` | logical drive 0=A:..15=P:; validated against `BLKDEVNUM` |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding (success):** 3-state — `0` no change, `1` may have changed, `2`
  definitely changed (`bios/disk.h:69-71`).
- **Encoding (error):** a negative BIOS error code (see below), passed through
  from the driver.
- **Source location:** `bios/blkdev.c:819` (success) / `:790,811` (errors).

| Value | Constant        | Meaning |
|------:|-----------------|---------|
| 0     | `MEDIANOCHANGE` | media definitely has not changed |
| 1     | `MEDIAMAYCHANGE`| media may have changed (re-validate BPB) |
| 2     | `MEDIACHANGE`   | media definitely has changed |
| < 0   | (driver error)  | e.g. `EUNDEV`, or `flop_/disk_mediach` failure |

### Side effects on global state

- `units[unit].status |= UNIT_CHANGED` and `blkdev[dev].mediachange` latched
  when a change is detected (`bios/blkdev.c:814-816`).
- The latched `mediachange` is **not** cleared here (it is cleared by
  `Getbpb`, `bios/blkdev.c:589`). A repeated `Mediach` keeps returning the
  latched value.

### Side effects on hardware

- May read the floppy WD1772 status register (which itself clears the
  hardware change latch) or issue a Test-Unit-Ready to a removable SCSI/USB
  unit, per the driver.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `drv` invalid / not `DEVICE_VALID` | `bios/blkdev.c:789` | `EUNDEV` (-15) |
| Underlying `flop_mediach`/`disk_mediach` returns negative (after `etv_critic` retry) | `bios/blkdev.c:810-811` | that negative code |

**Corrections vs. prior spec:** an out-of-range drive returns **`EUNDEV`, not
`0`**; and driver I/O errors are **propagated as negative codes, not collapsed
to `1` (may have changed)**. Both prior claims were wrong.

**Error paths not in Compendium:** the `EUNDEV` and driver-error pass-through.

---

## Verification criteria

- **Nominal (no change):** `Mediach(2)` on a settled fixed disk returns `0`.
- **Nominal (changed):** after a floppy swap (or a logical write to sector 0
  arming `forcechange`), `Mediach(0)` returns `2`.
- **Latched:** two successive `Mediach(d)` calls without an intervening
  `Getbpb(d)` return the same latched value; `Getbpb(d)` then resets it to `0`.
- **Invalid drive:** `Mediach(15)` with no P: returns `EUNDEV` (-15), **not**
  `0`.

---

## Edge cases discovered in source

1. **Out-of-range returns `EUNDEV`, not 0.** `bios/blkdev.c:789-790`. (Prior
   spec incorrectly said "returns 0 to avoid panic".)

2. **Negative driver errors are passed through.** If `flop_mediach`/
   `disk_mediach` returns `< 0` after the critical-error retry, that exact code
   is returned (`bios/blkdev.c:810-811`) — it is **not** normalised to
   `MEDIAMAYCHANGE`.

3. **`forcechange` short-circuits to `MEDIACHANGE`.** If a prior write to
   logical sector 0 armed `forcechange`, `Mediach` returns `2` without probing
   hardware, until `Getbpb` clears it (`bios/blkdev.c:802-803`).

4. **The change is latched, not one-shot.** Once `mediachange == MEDIACHANGE`
   is set, `Mediach` keeps returning `2` (`bios/blkdev.c:795-796`); only
   `Getbpb` clears it. (Contrast the hardware WD1772 latch, which the *first*
   read clears — but EmuTOS re-latches in `blkdev[]`.)

5. **Media-change protocol is caller responsibility.** `Rwabs` only auto-checks
   when `RW_NOMEDIACH` is *not* set; direct `Rwabs` callers that set it and
   skip `Mediach` can corrupt a swapped disk.

---

## Dependencies

### Requires (must be initialized / functional)

- `hdv_mediach` populated (`blkdev_init`).
- Per-drive `blkdev[]` state; a live driver with a `mediach` probe.

### Used by

- GEMDOS FAT-write paths (mandatory pre-flight check); `Getbpb` (resets the
  flag); `Rwabs` (internal check in logical mode).

---

## Related data structures

- `docs/api/bios/_state.md` — `hdv_mediach` vector + address, media constants
- `docs/api/structures/bpb.md` — re-validated on `MEDIAMAYCHANGE`

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically documents
  the 3-state return; does not mention `EUNDEV`/negative pass-through.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

None specific to `Mediach`; behaviour of the probes varies by driver `CONF_*`.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-04: SD-card removal is detected via the card-detect line; USB-MSC via
hot-plug events. Both feed a per-drive `mediachange` flag with the same
latch/reset contract (`Mediach` reads the latch; `Getbpb` clears it). The
`EUNDEV`-on-invalid-drive and negative-error pass-through must be preserved.
