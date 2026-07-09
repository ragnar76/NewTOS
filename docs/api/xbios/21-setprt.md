# `Setprt` (XBIOS 0x21)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/parport.c:49-57` (`setprt`). Re-validation pass,
> code-audit follow-up (2026-07-09): promoted out of the printer/NVRAM
> `xbios_unimpl` grouping in `_dispatcher.md`/`_triage.md` — see
> `docs/api/_discrepancies.md` D-060.

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the exact `config == -1` query sentinel, the
> unconditional old-value return, the default `0`. Reference: the historic
> "bit 4 selects parallel vs. serial printer route" convention, which EmuTOS
> itself never reads back (see Edge cases).

## EmuTOS source

**Primary location:** `bios/parport.c:49-57` — `WORD setprt(WORD config)`,
slot 0x21 of `xbios_vecs[]` (`VEC(xbios_21, setprt)`, `bios/xbios.c:1333`,
inside `#if CONF_WITH_PRINTER_PORT` / `#endif`, `bios/xbios.c:1332-1336`).

**Installed handler / vector:** (none) — direct table entry (`DBG_XBIOS=0`
default takes the `direct` branch of `VEC()`, `bios/xbios.c:1267-1270`; the
`DBG_XBIOS` wrapper `xbios_21()` at `bios/xbios.c:699-704` just adds a
`kprintf` and forwards to `setprt()`).

**Helpers called:** none. `setprt()` reads/writes only its own file-local
static; it does **not** call `prnout()` or touch any I/O port.

**Headers / type definitions used:**
- `bios/parport.h:19-21` — `#if CONF_WITH_PRINTER_PORT` / `WORD
  setprt(WORD config);` / `#endif`.
- `include/xbiosbind.h:51` — `#define Setprt(a) xbios_w_w(33,a)`.

**System variables / globals read:**
- `printer_config` (`bios/parport.c:36`, `static WORD`, file-local to
  `parport.c` — read at `:51`).

**System variables / globals written:**
- `printer_config` (`bios/parport.c:54`, when `config != -1`); initialised
  to `0` in `parport_init()` (`bios/parport.c:118`, called from
  `bios_init()` at `bios/bios.c:509-510`).

**Build configuration affecting this function:**
- `CONF_WITH_PRINTER_PORT` — in stock EmuTOS this gates the *entire*
  `setprt()`/`prnout()` pair (`bios/parport.c:45-106`) and the `xbios_vecs[]`
  slot (`bios/xbios.c:1332-1336`); when off, XBIOS 0x21 falls to
  `xbios_unimpl` (returns the function number, D-008). Default is `1` for
  Atari hardware (`include/config.h:987-988`) and `0` for non-Atari hardware
  (`include/config.h:673,707-708`) — i.e. a literal EmuTOS build for the
  NewTOS (`!CONF_ATARI_HARDWARE`) profile would **not** compile `setprt()`
  at all. **NewTOS decision (D-060):** implement `Setprt`'s software
  contract regardless of this gate — see "NewTOS implementation notes"
  below.

---

## Source extraction summary

`Setprt` is a **pure software** get/set of a single `WORD` — EmuTOS's entire
implementation is:

```c
WORD setprt(WORD config)
{
    WORD old_config = printer_config;
    if (config != -1)
        printer_config = config;
    return old_config;
}
```

It performs **no hardware access whatsoever**. `config` is stored verbatim
(no masking, unlike `Kbrate`'s `& 0xFF`) whenever it is not exactly `-1`
(`0xFFFF`); the previous value is always returned. `printer_config` defaults
to `0` at boot (`parport_init()`, `bios/parport.c:118`, comment: "Setprt()
default: output via parallel port").

The historic meaning of the word (per the `xbios_21` wrapper comment,
`bios/xbios.c:694-695`: *"Note that only bit 4 (parallel or serial) is used
by EmuTOS"*) is a GEM-Desktop-level convention — bit 4 clear selects
parallel (Centronics) output, set selects serial/other — but **EmuTOS's own
kernel never reads `printer_config` back** for any purpose (verified: no
other reference to the symbol in `references/emutos/`); it exists purely so
user-level software (Desktop, print-spooler utilities) can stash and later
retrieve that one flag across calls.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x21 (33 decimal)
- **Stack frame on entry (m68k):** `Setprt(config)` = `xbios_w_w(33, a)`
  (`include/xbiosbind.h:51`, expansion `include/xbiosbind.h:240-254`):
  ```
  +2: config       (word)
   0: function_id  (word) = 0x0021
  ```
- **Total parameter bytes:** 2 (W).
- **Native (AArch64) ABI:** ID `0x21` in `x8`; arg in `x0`; the WORD result
  **sign-extended** into `x0`.

  | m68k stack | AArch64 reg | Name     | Type / size |
  |------------|-------------|----------|-------------|
  | +2         | `x0`        | `config` | `WORD` (16) — `== -1` (exactly) = query |

---

## Inputs

| Position | Name     | Type   | Size | Source location      | Notes |
|----------|----------|--------|------|-----------------------|-------|
| arg0     | `config` | `WORD` | 2    | `bios/parport.c:53`  | new config word; stored **verbatim** (no masking) unless `config == -1` |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** WORD
- **Encoding:** the **previous** `printer_config` value, unconditionally
  (even when a new value is also stored in the same call).
- **Source location:** `bios/parport.c:51,56`.

### Side effects on global state

- `printer_config = config` when `config != -1` (`bios/parport.c:53-54`).

### Side effects on hardware

**None.** `setprt()` never touches the parallel port, the PSG, or any
MMIO/GPIO. (Contrast with the separate `prnout()` helper,
`bios/parport.c:62-105`, which does the Centronics handshake via PSG Port B
— `Setprt` never calls it.)

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

---

## Verification criteria

- **Default at boot:** `Setprt(-1)` (query) returns `0` before any program
  has called `Setprt` with a real value (`parport_init()` default,
  `bios/parport.c:118`).
- **Set, get old:** `Setprt(0x10)` returns the previous config word and
  makes the next `Setprt(-1)` return `0x0010`.
- **Query is exact `-1`, not "any negative":** `Setprt(-2)` **stores**
  `0xFFFE` (it is *not* treated as a query) and returns the old value —
  unlike `Kbrate`/`Rsconf`, which treat any negative parameter as "keep
  unchanged". Only the literal sentinel `-1` (`0xFFFF`) is the query
  (`bios/parport.c:53`, `config != -1`).
- **No masking:** `Setprt(0x1234)` stores the full 16-bit value verbatim;
  a subsequent `Setprt(-1)` returns exactly `0x1234`.
- **Save/restore idiom:** `WORD old = Setprt(new_cfg); … ; Setprt(old);`
  round-trips exactly, because every call (query or set) returns the true
  prior value.

---

## Edge cases discovered in source

1. **Query sentinel is exactly `-1`, not `< 0`.** Unlike `Kbrate` (any
   negative parameter = "keep") or `Rsconf` (`-1` = keep, `-2` = query),
   `Setprt` only special-cases the literal value `-1`; any other value,
   including other negative numbers, is stored as-is (`bios/parport.c:53`).
2. **`printer_config` is never read anywhere else in EmuTOS.** The "bit 4 =
   parallel/serial" meaning documented in the `xbios_21` wrapper comment
   (`bios/xbios.c:694-695`) is a convention for **user-level** software
   (Desktop, print spoolers) to interpret; the kernel itself treats the word
   as opaque storage (verified: no other reference to `printer_config` in
   `references/emutos/`).
3. **No masking of the stored value.** The full 16-bit `config` is stored
   verbatim — no `& 0xFF` or similar truncation (contrast `Kbrate`).
4. **Old value is always returned, even when also setting.** There is no
   way to "set blind" — every call returns the pre-call state
   (`bios/parport.c:51,56`), which is what makes the save/restore idiom
   safe.
5. **In EmuTOS, `setprt()` does not exist at all when
   `CONF_WITH_PRINTER_PORT=0`** (`bios/parport.c:45/106` wraps both
   `setprt()` and `prnout()` in the same `#if`); the XBIOS 0x21 slot then
   falls back to `xbios_unimpl` (function-number return, D-008). This is a
   **build-time coupling of two independent concerns** in EmuTOS (the
   config-word storage and the Centronics output path) — see "NewTOS
   implementation notes" for why NewTOS decouples them. `CONF_WITH_PRINTER_PORT`
   defaults to `0` for non-Atari hardware (`include/config.h:673,707-708`),
   which is the profile NewTOS's target most resembles.

---

## Dependencies

### Requires (must be initialized / functional)

- `printer_config` initialised to `0` by `parport_init()`
  (`bios/parport.c:118`), called from `bios_init()` (`bios/bios.c:509-510`).

### Used by

- GEM Desktop / print-spooler-style utilities that remember "route printer
  output via parallel vs. serial" across program runs; general
  save/restore idioms (`old = Setprt(cfg); …; Setprt(old);`).

---

## Related data structures

None — a single scalar `WORD`, no struct.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Typically documents
  bit 4 as "0 = printer, 1 = serial device" or similar; EmuTOS's kernel does
  not enforce or read this encoding — see Edge case 2.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `CONF_WITH_PRINTER_PORT` gates EmuTOS's own `setprt()`/`prnout()`
  compilation (`bios/parport.c:45-106`) and the `xbios_vecs[]` slot
  (`bios/xbios.c:1332-1336`); default `1` on Atari hardware
  (`include/config.h:987-988`), `0` on non-Atari hardware
  (`include/config.h:673,707-708`). **NewTOS ignores this gate for `Setprt`
  specifically** (D-060) — the config-word contract is honoured
  unconditionally, while `prnout`'s Centronics/PSG path remains genuinely
  absent (no parallel-port hardware on the Pi 4).

---

## NewTOS implementation notes

(Empty during Phase 00.)

**NewTOS decision (D-060): honour the software contract, not the hardware
absence.** `Setprt`'s entire EmuTOS implementation is `store-if-not-(-1),
return-old` on a single in-memory `WORD` — it has no dependency on the
Centronics port at all. Treating it like the genuinely hardware-dependent
XBIOS calls (floppy `EUNDEV`, PSG `0`/no-op, per `_floppy-stubs.md` /
`_psg-group.md`) would be wrong in the other direction: those calls fail
because the *operation itself* requires absent silicon (spinning a floppy
motor, clocking a YM2149). `Setprt` requires **no silicon** — it is
observable-state, not hardware access. Per `docs/COMPATIBILITY.md` ("if a
program can read it … through a documented interface, it is contract"), the
save/restore idiom `old = Setprt(cfg); …; Setprt(old);` is part of the
observable contract; falling back to EmuTOS's `xbios_unimpl` function-number
return (D-008) — i.e. `Setprt(-1)` returning `33` — would silently corrupt
that idiom for zero benefit (unlike the floppy/PSG stubs, where returning a
"real" value would be a lie about hardware that does not exist). PHASE-07/08:
implement as a single `WORD` field in the XBIOS/BIOS state (default `0`,
matching `parport_init()`), independent of any printer-hardware feature
flag. This is the same "config variable vs. real HW" split already applied
to `Rsconf`'s baud-index shadow (`docs/api/xbios/0f-rsconf.md`) — parallel
to, but distinct from, the PSG/floppy **absence-contract** functions, which
`Setprt` does **not** belong to.

---

## Pi / BCM2711 binding

> Re-points the *hardware reference* (the Centronics parallel port /
> `prnout()` path) at the NewTOS profile, which has **no printer port** —
> same hardware absence as `Scrdmp`'s dump target
> (`docs/api/xbios/14-scrdmp.md` — `HARDWARE-PROFILE.md` §3.9, "Centronics
> printer → absent"). The **contract above is unchanged**: `Setprt`'s
> config-word get/set is pure software and needs no peripheral at all.

- **No printer hardware.** The Pi-4 profile has no Centronics/parallel port
  (`HARDWARE-PROFILE.md` §3.9); this affects only a hypothetical `prnout`-style
  output path, which `Setprt` does not call.
- **Datasheet.** None applicable — `Setprt` is pure software state; there is
  no BCM2711 peripheral involved in its contract.
