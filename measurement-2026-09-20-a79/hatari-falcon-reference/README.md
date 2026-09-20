# EmuTOS on an emulated Falcon030, 640x480 truecolor — the window-frame reference (A79, 2026-09-20)

Hatari 2.6.1 (`~/Downloads/hatari-2.6.1_macos/`), `--machine falcon --monitor vga`,
ROM = the pinned source's own build `references/emutos/etos512us.img`
(EmuTOS 1.4.0+ 20260829, sha256 in `videl-shiftmode.txt`). The GEMDOS drive C: is a
host directory carrying `method/EMUDESK.INF`, whose `#E` line sets video mode
`0x001C` = `VIDEL_VGA | VIDEL_80COL | VIDEL_TRUECOLOR` (`include/biosdefs.h:151-158`,
parsed by `process_inf1`, `aes/geminit.c:361-380`). Hatari's `info videl` confirms
`$FF8266 = 0100` = `SPS_HICOLOR` (16-bit truecolor, `bios/videl.h:24`).

`method/run-hatari-falcon.sh <hd-dir> <shots-dir> <tag>` boots headless
(`SDL_VIDEODRIVER=dummy`), opens the C: window through the desktop's Alt+letter
shortcut (`check_alt_letter_key`, `desk/deskmain.c:1136-1160`; scancodes 56 = Alt,
46 = C) sent over `--cmd-fifo`, and grabs PNGs with `hatari-shortcut screenshot`.

## What the reference shows (`shots/falcon-tc-window-c.png`, zoom `-zoom3x.png`)

Default colour words (`gemwmlib.c:1349-1357`) under `CONF_WITH_3D_OBJECTS`:
gadget faces **white** (hollow fill in replace mode = background), black 1-px
borders, the 3D bevel (white top/left, grey `(140,138,140)` bottom/right) on the
FL3DACT gadgets (closer, fuller, arrows, sizer, elevators), the topped name band
in `IP_2PATT`, the slider tracks in `IP_1PATT`, the info band white.
`shots/compare-ref-lit-gray-5x.png`: top = this reference, middle = NewTOS
`FrameLook::Literal`, bottom = NewTOS `FrameLook::Tos4Gray` (today's default).

## Untopped window, and the EmuTOS 1.4 release ROM (added the same evening)

`method/EMUDESK-two-windows.INF` opens `C:\` and `C:\SUB\` at boot through two
`#W` lines with a path (`method/run-hatari-falcon-inf-windows.sh`);
`shots/falcon-tc-two-windows.png` and `falcon-tc-untopped-title-zoom5x.png`: the
untopped window keeps **white** faces and bands, its name drawn `DISABLED`
(dither, `w_bldactive`, `gemwmlib.c:537`), closer with bevel. The 1.4 release ROM
(Hatari's `etos512de.img`) renders the same (`falcon-tc-window-c-emutos14-release.png`);
the colour defaults are unchanged from 1.3 through HEAD (`git show VERSION_1_3:aes/gemwmlib.c`
`:1349-1357`).

`shots/emutos-showcase-001-website.png` (emutos.sourceforge.io showcase, EmuTOS 1.3)
shows grey window chrome with white text — that screen has XCONTROL's *Colour VDI*
CPX open and runs a changed VDI palette / 3D colours; it is not the default
rendering and not a reference for A79.

## 256 colours (`#E … 00 1B` = `VIDEL_VGA | VIDEL_80COL | VIDEL_8BPP`, user request)

`shots/falcon-256c-two-windows.png`: identical rendering model — white faces and
bands, black borders, bevel shadow `(138,138,138)`, topped name `IP_2PATT`, untopped
name `DISABLED`. The default window look does not depend on the colour depth once
`ws_ncolors > 8` (`init_3d`, `gemoblib.c:1336-1343`, only picks `LWHITE` for the
three 3D colours, which no default window word ever reaches).
