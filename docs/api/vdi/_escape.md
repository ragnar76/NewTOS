# VDI `v_escape` (5) — alpha/cursor sub-functions (Phase 00b D6)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_esc.c` (`vdi_v_escape`,
> `esctbl[]`). Calling convention: [`_dispatcher.md`](_dispatcher.md).
> Opcode 5; the **sub-function is in `CONTRL[5]`**. `escfun > 19`
> (`ldri_escape`) ⇒ no-op (except Bezier-quality 99 when `HAVE_BEZIER`).
> Dispatch: `(*esctbl[escfun])(vwk)`.

These are the VT52-style **alpha (text-mode) cursor** controls plus a
few inquiries. They operate on the text/alpha layer, not the graphics
attributes.

| `CONTRL[5]` | Fn | Action |
|-------------|----|--------|
| 0 | (`escfn0`) | no-op padding entry |
| 1 | `vq_chcells`     | `INTOUT[0..1]` = current rows, columns |
| 2 | `v_exit_cur`     | exit alpha mode → enter graphics mode |
| 3 | `v_enter_cur`    | enter alpha mode → exit graphics mode (clears) |
| 4 | `v_curup`        | cursor up one line |
| 5 | `v_curdown`      | cursor down one line |
| 6 | `v_curright`     | cursor right one column |
| 7 | `v_curleft`      | cursor left one column |
| 8 | `v_curhome`      | cursor home |
| 9 | `v_eeos`         | erase to end of screen |
| 10 | `v_eeol`        | erase to end of line |
| 11 | `vs_curaddress` | set cursor row/col (`INTIN[0]`=row, `INTIN[1]`=col) |
| 12 | `v_curtext`     | output cursor-addressable text (`INTIN[]`, `CONTRL[3]` chars) |
| 13 | `v_rvon`        | reverse video on |
| 14 | `v_rvoff`       | reverse video off |
| 15 | `vq_curaddress` | `INTOUT[0..1]` = current row, column |
| 16 | `vq_tabstatus`  | `INTOUT[0]` = tablet-device availability (0) |
| 17 | `v_hardcopy`    | screen dump to printer |
| 18 | `v_dspcur`      | display graphics cursor at `PTSIN[0..1]` |
| 19 | `v_rmcur`       | remove graphics cursor |

`esc_init` (sub 2) is called from `v_opnwk` to enter graphics mode;
`esc_exit` (sub 3) from `v_clswk`.

## NewTOS implementation notes

- The alpha cursor layer is the **VT52 text console** the framebuffer
  already drives (Phase 06 console output sits here). `v_enter_cur`/
  `v_exit_cur` toggle between the graphics workstation and that text
  layer; map them onto the existing console cursor/erase routines.
- `vq_chcells`/`vq_curaddress` report rows/cols from the framebuffer
  font cell geometry (screen `yres`/cell_h × `xres`/cell_w).
- `v_hardcopy` (17) needs a printer device — **out of bring-up scope**;
  it must be a documented no-op (return cleanly), not a faked success.
  Record under `docs/STATUS.md` "Blocked" if a program needs it.
- `v_dspcur`/`v_rmcur` share the mouse-cursor sprite path
  ([`input/_mouse.md`](input/_mouse.md)).
