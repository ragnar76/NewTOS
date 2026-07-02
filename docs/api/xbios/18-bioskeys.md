# `Bioskeys` — XBIOS 0x18

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/ikbd.c:136` (`bioskeys`).

## NewTOS status: **out-of-scope**

`Bioskeys` resets the Atari keyboard-translation tables to their ROM defaults —
tables NewTOS does not use (it translates scancodes natively in `crate::ikbd`
off the IKBD path). It is therefore **not exposed** as a trap; the XBIOS
dispatcher's `EINVFN` catch-all is the intended, documented behaviour (not a
stub). See `docs/STATUS.md` (2026-07-02) and the "Keyboard via Atari IKBD"
decision.

## EmuTOS source

```c
void bioskeys(void)
{
    current_keytbl = *get_keytbl();
}
```

`get_keytbl()` (defined in `bios/country.c`) returns a pointer to
the keyboard table for the **default country** as built into the
ROM. `current_keytbl` is the active KEYTAB struct — three pointers
to 128-byte scancode-to-ASCII tables (norm / shft / caps).

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x18` |
| WORD args | 0 |
| Stack frame | `+0: 0x0018` |
| Vector | `xbios_vecs[24]` |

## Inputs

None.

## Outputs

`void` — no return.

## Errors

None.

## Edge cases

### 1. Reset to ROM defaults

After a program has used `Keytbl(...)` (XBIOS 0x10) to install
custom translation tables, `Bioskeys` reverts to the ROM-default
country table. Used by exit handlers.

### 2. Country selection

The "default country" is set at boot by reading a NVRAM byte
(Falcon) or compile-time constant (older TOS, EmuTOS). NewTOS
profile: build-time constant (`CONF_LANG`). Runtime country
switching requires NVRAM modification + reboot, mirroring Atari's
behaviour.

### 3. Atomic with respect to IKBD

The IKBD interrupt handler dereferences `current_keytbl.norm[]`
etc. on every keypress. A `Bioskeys` mid-keypress can race — but
since the call is just `current_keytbl = *new_table`, three
pointer writes, a partially-updated state shows mixed tables for
~6 m68k cycles. EmuTOS does not block IKBD interrupts. NewTOS on
AArch64 must ensure the three-pointer copy is atomic or
interrupt-disabled.

### 4. CONF_WITH_FORCED_LOCALE

Some EmuTOS builds force a specific country regardless of NVRAM.
NewTOS will follow the build-config approach.

## Dependencies

- `current_keytbl` (BIOS BSS, exposed via `Keytbl(-1, -1, -1)`).
- `get_keytbl()` (`bios/country.c`).

Used by: program exit cleanup, AES locale change.

## Cross-references

- **Compendium:** "Bioskeys" — agrees.
- **Profibuch:** "Bioskeys (0x18)" — agrees.
- `docs/api/bios/0b-kbshift.md` — shift-state companion.
- `docs/api/xbios/_state.md` (pending) — `current_keytbl`
  structure.

## Configuration

- `CONF_LANG` — chooses default country.

## NewTOS implementation notes

(Empty during Phase 00.)
