# 13 — zView (LDG version): the image viewer and its ten codecs

Stand 2026-10-07, TASK/ZVIEW-M4. Design input:
`docs/analysis/zview-ldg-2026-10-04.md` (P-ZVIEW, P-CODEC),
`docs/analysis/ldg-after-arch03-2026-10-06.md` (G7). Evidence for every
"verified" below: `measurements/measurement-2026-10-06-zview-m4/RUNINFO.TXT`.

zView 1.0.1 (Zorro, Miro Kropacek; GPL, codecs LGPL 2.1) is the z-tools image
viewer in its **LDG version**: a WinDom program that loads its image codecs at
run time as LDG modules. On NewTOS it is `ZVIEW.NAP` plus ten codec modules
`CODECS\*.NLD`, built from the unmodified sources with toolchain overlays —
**without the PDF module**.

> **Open blocker K1 (kernel, not fixed here).** On NewTOS the VDI handle that
> `graf_handle` returns (1, EmuTOS `gl_handle`) is never opened, so
> `vq_extnd(1, …)` answers nothing. WinDom's `ApplInit` reads the screen depth
> exactly that way (`windom-1.21.3/src/appl_init.c:42-46`) and gets
> `app.nplanes = 0`, `app.color = 0`; zView then builds every picture MFDB with
> 0 planes and its window stays grey (§6). The codecs themselves are verified
> pixel-exact with a diagnostic build that only replaces those two values
> (§5). Until K1 is fixed in the kernel, the shipped `ZVIEW.NAP` opens, loads
> its codecs and decodes, but does not show pictures, and a picture wider than
> 128 pixels overruns its heap (Bus Error).

## 1. The pieces

| What | Where |
|---|---|
| Source pin | `sdk/zview/z-tools-04805b7.tar.xz` — a `git archive` subset of z-tools commit `04805b7` (`zview/`, `common/`, `ztask/gmem.{c,h}`; without the PDF fonts, the m68k goodies, HypView); command, origin and licence in `sdk/zview/zview-sources.sha256` |
| Build | `sdk/zview/build-zview.sh` (extract, shadow tree + overlays, compile, link, map check, stack patch, RSC, dist, samples) |
| Overlays | `sdk/zview/overlay/zview/…` — 12 files, §3 |
| Prelude | `sdk/zview/include/zview-ints.h` (`-DHAVE_INTS_DEFINED -include …`) |
| Samples | `sdk/zview/samples/gen-samples.py` → `sdk/zview/build/samples/{IMAGES,expected}` (15 synthesised files, no third-party pictures) |
| Output | `sdk/zview/build/dist/ZVIEW/` = `ZVIEW.NAP`, `ZVIEW.RSC`, `ICONS\*.GOD`, `CODECS\*.NLD` |
| Card | `tools/make-sd-image.sh` installs `C:\ZVIEW\` and `C:\ZVIEW\SAMPLES\` (QEMU image only) |

Libraries: WinDom (`libwindom1.a`, [12](12-windom.md)), `libldg.a`
([10](10-ldg-modules.md)), gemlib, libpng + zlib (zView links them,
`zview/Makefile:15`), libm, libc for the program; libshare + libldg + the
codec's image library ([11](11-codec-libraries.md)) + gemlib, libm, libc for
each codec.

## 2. Build rules (flags instead of source edits)

| Rule | Why |
|---|---|
| `-DHAVE_INTS_DEFINED -include sdk/zview/include/zview-ints.h` | `common/types2b.h:4-9` makes `int32`/`uint32` `long` — 64 bits on LP64. The names mean 32 bits (the Eureka palette is `int32[768]` = 3072 bytes, `zvdi/raster.c` writes `uint32` pixels, `chrono.c` reads `*(int32 *)0x4ba`), and `types2b.h` offers exactly this hook. `img_info` (`plugins/common/zview.h`) is shared by zView and every codec and gets the same layout on both sides |
| `-fsigned-char` | built upstream with m68k-atari-mint-gcc, plain `char` signed (same as WinDom) |
| `-fgnu89-inline` | `zvdi/*.c` define `inline` functions other files call; GNU89 semantics emit them (the gcc that built zView defaulted to gnu89) |
| `-I<tree>/ztask` | zView includes `"gmem.h"` and calls `gmalloc/grealloc/gfree` from xpdf's goo library (`Makefile:12-15`) — the PDF module NewTOS leaves out; z-tools' own `ztask/gmem.{c,h}` (same API, same repository) is compiled unchanged instead |
| `_stksize` = 512 KiB in `ZVIEW.NAP` | `Makefile:9,40`: `m68k-atari-mint-stack -S 512k`; `zv_set_stack` writes mintlib's initialised `_stksize` in the ELF the same way. Codecs: `64k` = mintlib's default, nothing to patch |
| upstream warning flags | `-Wall -Wshadow -Wno-pointer-sign` (zView), `-Wall -Wno-pointer-sign` (codecs); 11 files warn, all upstream (list: `build/warnings.txt`) |
| PDF: `pdf_load.c`, `winpdf.c` not compiled, `pdflib.cc` not built | §4 |

The source lists are zView's own: `Makefile` `OBJ` (minus the two PDF files,
plus `ztask/gmem.c`) and each codec's `Makefile` `OBJ`/`DSP_OBJ`, read with
`make -f Makefile -f print.mk`.

Codecs are linked like their makefiles (`-lshare -lldg` + image library) with
`libshare.a` **first** in the group; every link map is checked by
`ls_check_module_map` (`sdk/libshare/libshare-link.sh`): libshare's
allocator only. Two builds are byte-identical.

## 3. Overlays (12 files)

| File | Kind | Change |
|---|---|---|
| `zview/plugins.c` | ADR-018 | codec suffix `"ldg"` → `"nld"` (`:96`) |
| `zview/main.c` | PDF omission | `if (pdf_initialised) pdf_exit();` removed (`:48-49`) |
| `zview/winimg.c` | PDF omission | `if PDF return WindPdf(…)` removed (`:969-970`) |
| `zview/av_prot.c` | message pointer (endianness + width) | `VA_START` command pointer read as the hi/lo WORD pair (`:91`), the convention of the WinDom overlays |
| `plugins/degas/degasldg.c` | endianness + bit-field order | header word (compression bit 15, resolution bits 1-0) and the 16 ST palette words decoded big-endian into the struct |
| `plugins/neochrome/neoldg.c` | endianness | the 128-byte header's words swapped in place |
| `plugins/godpaint/godldg.c` | endianness | file words swapped once after reading; encoder writes width/height/pixels big-endian |
| `plugins/eureka/rawldg.c` | endianness | width/height from bytes; 768 palette LONGs from big-endian |
| `plugins/img/imgldg.c` | endianness | IMGHDR words and the XIMG RGB1000 palette swapped in place |
| `plugins/img/xp32.c` | **m68k assembly** → AArch64 | `plane2packed24` (xp32.S, 166 lines) in C; the m68k palette stride 4 is `sizeof(COLOR_MAP)` there (measured with m68k-atari-mint-gcc 7.5.0), `&palette[i]` here |
| `plugins/jpg/jpgldg.c` | LP64 | `FILE *` in the `int32` `_priv_var` via explicit casts (user addresses < 2 GiB, [05](05-abi-pitfalls.md) A2); `Getcookie` targets `long` |
| `plugins/jpg/jpgdsp.c` | **m68k assembly** → AArch64 | the five DSP-driver register-call macros as C calls through the driver table (unreachable: no `_JPD` cookie) |

Each file's header names the upstream lines and the reason. Endian-neutral
and left alone: bmp, tga (byte-wise parsers), png, gif, jpg (library-based).

**LP64 sweep of the rest (read, not changed).** No other pointer/integer
casts (clang reported none besides `chrono.c`'s `Super` token, harmless: the
SDK's `Super` is a no-op, [05](05-abi-pitfalls.md) B4). `winimg.c:1006`,
`infobox.c:294` print `uint32` colours with `%ld` — formally a mismatch,
value-correct (register-passed, AArch64 W-writes zero-extend). `zvdi/color.c`
`remap_color`/`color_lookup` (`:66-113`) view an `int32` and the `SRGB`
struct big-endian — reached only on ≤ 8-plane screens; NewTOS is fixed at
16 planes (ADR-024): latent. `gifldg.c` keeps pointers to `reader_init()`'s
stack locals in `img_info` (`:359-360`) and reads them in `reader_read()` —
undefined upstream, works here as on the 68k (verified: GIF pixel-exact).
`bmpldg.c:256` writes `buff[2]` of a 2-byte array (4-bit BMP path) —
upstream bug, not exercised.

## 4. PDF

Not built. zView's PDF module is xpdf 3.04 (C++, ~40 objects) + freetype
(`Makefile:15,35,64-72`); neither is ported. Instead of stubbing the eleven
`pdflib.h` functions, the two call sites into PDF code are removed (overlays
`main.c`, `winimg.c`) and `pdf_load.c`/`winpdf.c` are not compiled. A `.PDF`
file then takes the path of every format no codec reads: `pic_load` fails and
no window opens (silently, as upstream does for unknown types). The browser
still shows its PDF icon; the PDF tab of the preferences dialog still stores
its two settings.

## 5. Codecs and what is verified

`CODECS\` (all linked with libshare, map-checked): `BMP.NLD`, `DEGAS.NLD`
(PI1-3, PC1-3), `EUREKA.NLD` (RAW), `GIF.NLD`, `GODPAINT.NLD` (GOD; also
zView's own icons), `IMG.NLD` (GEM IMG/XIMG), `JPG.NLD` (+ EXIF, encoder),
`NEO.NLD`, `PNG.NLD`, `TGA.NLD`. **Not built: `tiff`** — it needs libtiff and
liblzma, which the SDK does not have.

**Codec directory.** zView looks first for `codecs` beside itself
(`<start dir>\codecs`, `plugins.c:56-60`), then in the `LDGM` cookie's path,
`LDGPATH`, and `C:\gemsys\ldg\codecs` (`:62-82`). NewTOS has no LDG TSR and
no `LDGPATH`, so the distribution layout is the one used: `C:\ZVIEW\CODECS\`.

Verified in QEMU raspi4b (`serial=on`; RUNINFO has the runs):

* `ZVIEW.NAP` starts through the real AES path (`#Z` → a launcher →
  `shel_write(SHW_EXEC, …, tail)` → `sh_ldapp`; the start directory is the
  caller's, as in EmuTOS' `sh_write`/`sh_curdrvdir`), loads its resource and
  menu bar, finds and loads all ten codecs (`plugins_init`: `chdir`,
  `opendir`/`readdir`, `ldg_open`, `plugin_init`), opens the file named on its
  command line (`main.c:164-165`), decodes it and opens the window with the
  right info line. The window then stays grey (K1, §6).
* With the K1 **diagnostic** build (product code plus three lines after
  `ApplInit` that take `app.nplanes`/`app.color` from WinDom's virtual
  workstation — what `ApplInit` would have read) every sample is
  **pixel-exact** against its reference (screen colour = 5 bits per gun,
  expanded like the VDI backend; JPEG against the pinned IJG decoding with
  the codec's IFAST/no-fancy-upsampling settings): 15/15 with memprot on —
  BMP 24/8-bit, TGA uncompressed/RLE, PNG RGB/palette, GIF, JPEG, DEGAS PI1
  and PC1, NEOchrome, GodPaint, Eureka RAW, GEM XIMG 4 planes, GEM IMG mono —
  4/4 under memprot strict (JPEG, PI1, XIMG, GIF). zView's toolbar icons
  (GodPaint files through `GODPAINT.NLD`) render.
* A real photograph in ten format variants (270x360, plus 320x200 16-colour
  DEGAS/NEO): 9/10 pixel-exact, including JPEG, XIMG with 8 bit planes (the
  `pp_8_to_24` path of the xp32 retarget) and the STe palette. The RLE TGA
  variant fails from row 84 on — upstream defect U1, §6.

## 6. Limits and known defects

| # | What | Effect |
|---|---|---|
| **K1** | kernel (AES/VDI): the workstation `graf_handle` returns is never opened, `vq_extnd` on it answers nothing ([12](12-windom.md) §6) | product `ZVIEW.NAP` shows grey windows: MFDBs with 0 planes (`pic_load.c:34`, `mfdb.c:30,35`), `LnSize` 0 — every row is written over row 0 of a 256-byte block, and **pictures wider than 128 pixels overrun the heap** (measured: Bus Error in WinDom `DataSearch` with a pixel word as pointer, 320x200 DEGAS). Fix belongs in the kernel, not here |
| U1 | upstream, `plugins/tga/tgaldg.c` `fill_img_buf` (`:304-327`): the 2-line buffer is refilled only once the read offset has passed its half, without checking that the next line's packets are in it | an RLE TGA whose line takes more than the bytes left (a line just under half the buffer followed by one over `line_size`) decodes stale bytes; a model of the upstream logic predicts exactly the faulty rows seen. Same logic on the 68k; not changed (Atari source unmodified). The synthetic `TREFILL.TGA` of the run triggers it hard enough to corrupt the heap (libshare `free` assertion) |
| — | no PDF | §4 |
| — | no TIFF | §5 |
| — | AV / `VA_START` | hi/lo pointer convention like WinDom; not exercised (no AV server sends it in single-tasking) |
| — | browser, save dialog, encoders (GodPaint, JPEG), slideshow | not exercised; only the window the command line opens |
| — | start directory | zView finds `zview.rsc`, `icons\`, `codecs\` relative to the directory it starts in (`main.c:123-125`); the desktop and `shel_write` give it the right one |
| — | real Pi | not run |
