# `v_clsvwk` — VDI opcode 101 (close virtual workstation)

> Phase 00b spec. Source: EmuTOS @ `1a7484ab` —
> `vdi/vdi_control.c:462` (`vdi_v_clsvwk`). See also `v_clswk`
> (opcode 2, close *physical* workstation).

Closes the virtual workstation named by the call's handle: frees its
`Vwk`, rebuilds the workstation chain, and repoints `CUR_WORK` at the
physical workstation.

## Source extraction

```c
void vdi_v_clsvwk(Vwk *vwk) {
    handle = vwk->handle;
    if (handle == VDI_PHYS_HANDLE) return;  /* can't close physical here */
    if (!vwk_ptr[handle]) return;           /* already closed */
    vwk_ptr[handle] = NULL;                  /* close */
    build_vwk_chain();                       /* relink */
    CUR_WORK = &phys_work;                    /* keep CUR_WORK valid */
    Mfree(vwk);
}
```

## Trap interface

| | |
|--|--|
| Opcode | `contrl[0] = 101` |
| Handle | `contrl[6]` — the workstation to close |
| INTIN / PTSIN / INTOUT / PTSOUT | 0 / 0 / 0 / 0 |

## Outputs / Errors

No outputs, no return value, no error. Closing the physical handle
(`VDI_PHYS_HANDLE`) via this call, or an already-closed handle, is a
silent no-op.

## Notes

- `v_clswk` (opcode 2) closes the **physical** workstation (the screen
  device itself); on EmuTOS it tears down the screen driver. NewTOS's
  bring-up keeps the physical workstation open for the lifetime of the
  system, so `v_clswk` is effectively a no-op there.
- Dependency: `Mfree` (GEMDOS 0x49). Globals `vwk_ptr[]`, `CUR_WORK`,
  `phys_work`. Mirror image of [`100-v_opnvwk`](100-v_opnvwk.md).
