# `v_opnwk` — VDI opcode 1 (open physical workstation)

> Phase 00b spec. Source: EmuTOS @ `1a7484ab` —
> `vdi/vdi_control.c:496` (`vdi_v_opnwk`). Shares `init_wk` and the
> `work_in`/`work_out` layout with [`100-v_opnvwk`](100-v_opnvwk.md).

Opens the one **physical** workstation — the screen device itself.
GEM calls this once at startup (via the AES); applications then open
**virtual** workstations on top. It (re)sets the screen resolution,
initialises the device capability tables, and initialises the physical
workstation's attribute area.

## Source extraction

```c
void vdi_v_opnwk(Vwk *vwk) {
    newrez = INTIN[0] - 2;          /* work_in[0] = rez + 2 */
    /* ... set screen resolution to newrez (Setscreen-like) ... */
    for (i=0;i<45;i++) DEV_TAB[i] = DEV_TAB_rom[i];   /* reset capability tables */
    for (i=0;i<12;i++) SIZ_TAB[i] = SIZ_TAB_rom[i];   /* (from ROM templates) */
    /* patch DEV_TAB[0,1,13,35,39] with real geometry/colour info */
    text_init();                    /* init SIZ_TAB font metrics */
    /* CONTRL[6] = VDI_PHYS_HANDLE (1) */
    init_wk(vwk);                   /* same attribute init as v_opnvwk */
}
```

`work_in[0]` carries the desired screen resolution **+ 2** (the
classic Atari convention); `newrez = work_in[0] - 2`. The capability
tables (`DEV_TAB`/`SIZ_TAB`) are reloaded from ROM and patched with the
live geometry — identical to what `v_opnvwk` returns.

## Trap interface

| | |
|--|--|
| Opcode | `contrl[0] = 1` |
| Handle | `contrl[6]` → `VDI_PHYS_HANDLE` (1) on output |
| `# INTIN` | 11 (`work_in`, same layout as `v_opnvwk` — see there) |
| `# INTOUT` / `# PTSOUT` | 45 / 6 (`DEV_TAB` / `SIZ_TAB`) |

## Inputs / Outputs

`work_in` and `work_out` (`INTOUT`=`DEV_TAB`, `PTSOUT`=`SIZ_TAB`) are
identical to [`v_opnvwk`](100-v_opnvwk.md); only `work_in[0]` differs
(resolution + 2 instead of a driver id).

## Errors

None (no VDI error return).

## NewTOS implementation notes (Phase 12)

- NewTOS has a single fixed-geometry framebuffer (Phase 10); the
  `work_in[0]` resolution change is honoured only insofar as the
  framebuffer/board supports it — bring-up uses one fixed mode, so
  `newrez` is effectively ignored and `DEV_TAB[0,1]` reflect the live
  framebuffer size.
- The physical workstation is opened once at graphics init and stays
  open (`v_clswk` is a no-op — see [`101-v_clsvwk`](101-v_clsvwk.md)).
