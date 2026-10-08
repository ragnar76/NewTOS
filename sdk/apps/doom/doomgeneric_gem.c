/*
 * doomgeneric_gem.c -- the GEM half of DOOM: doomgeneric's platform contract
 * (DG_Init, DG_DrawFrame, DG_SleepMs, DG_GetTicksMs, DG_GetKey,
 * DG_SetWindowTitle) and main(), for any TOS machine with an AES and a VDI.
 * TASK/PORT06.
 *
 * Copyright (C) 2026 the NewTOS project. GPLv2 or later, like the doomgeneric
 * tree it is linked with.
 *
 *
 * THE RULE THIS FILE IS WRITTEN UNDER
 * -----------------------------------
 * Only the AES, the VDI, the BIOS/XBIOS and GEMDOS. No screen-base XBIOS
 * call, no framebuffer address, no assumption about what a pixel looks like
 * in memory. A graphics card replaces the VDI DRIVER, not the hardware behind
 * it: what goes through the VDI runs on NewTOS, on a TT, on a TT with a NOVA,
 * on a Falcon, under Hatari -- and inherits whatever acceleration that driver
 * has. The same source is meant to build for m68k (TT + NOVA) later; nothing
 * below depends on the CPU, the byte order or the pointer width. (Byte order
 * is doomgeneric's business, i_swap.h; the vr_trnfm route never looks at the
 * bytes of a multi-byte value, and the EdDI route stores pixels the way the
 * driver's description says, see THE EdDI ROUTE.)
 *
 *
 * THE PICTURE: STANDARD FORM -> vr_trnfm -> vro_cpyfm
 * ---------------------------------------------------
 * The engine is built with CMAP256: it draws its 320x200 picture as 8-bit
 * palette indices (DG_ScreenBuffer) and publishes the palette in colors[]
 * with palette_changed set when it moves (i_video.c). It stays 8-bit to the
 * end -- lighting, the player colours and every palette effect are index
 * lookups that cost nothing that way (PORT06 §3a).
 *
 * The one route to the screen that every VDI has is the STANDARD form: the
 * machine-independent bitmap, one plane after the other, each plane a
 * fd_h x fd_wdwidth array of 16-pixel words, bit 15 the leftmost pixel.
 * vr_trnfm turns it into whatever this driver's device form is (interleaved
 * planes on a ST/TT, chunky on a NOVA, the 16-bit Falcon word on NewTOS), and
 * vro_cpyfm copies that onto the screen. Per frame:
 *
 *     indices --(pixval[], chunky-to-planar)--> standard form (N planes)
 *             --vr_trnfm--> device form --vro_cpyfm S_ONLY--> screen
 *
 * N is the screen's plane count as the VDI reports it (vq_extnd INQ_TAB[4]).
 * This is the slower of the two routes PORT06 names, and the one that runs
 * everywhere. The faster one follows.
 *
 *
 * THE EdDI ROUTE: THE DEVICE FORM AS THE DRIVER DESCRIBES IT
 * ---------------------------------------------------------
 * A VDI that installs the cookie 'EdDI' (NVDI, the NOVA drivers, fVDI, the
 * Atari VDI Enhancer, NewTOS) answers vq_scrninfo (VDI 102, CONTRL[5] = 1,
 * INTIN[0] = 2) with a description of its device form (docs/api/vdi/
 * eddi-vq_scrninfo.md §3). Where that description covers the screen, the
 * frame is built in the device form directly and vr_trnfm is skipped:
 *
 *     indices --(devpix[])--> device form --vro_cpyfm S_ONLY--> screen
 *
 * Covered (§3.2, §3.5):
 *   work_out[0] = 2 (packed pixels), [1] = 2 (HiColor/TrueColor): the pixel
 *     is built from DOOM's RGB with the bit-number tables [16..63] -- for
 *     red/green/blue, intensity bit k sits at pixel bit [16 + 16c + k]. The
 *     channel widths are [8..10]; an n-bit intensity is the 8-bit one
 *     rounded to n bits. Alpha, genlock and unused bits stay 0 (§7 E9).
 *     (bytes per pixel = (planes + 7) / 8.) A palette change recomputes the
 *     256 pixels; no VDI call.
 *   work_out[0] = 2 or 0 (packed / interleaved planes), [1] = 1 (hardware
 *     CLUT), 8 resp. 1..8 planes: DOOM's palette goes into the pens with
 *     vs_color as on the vr_trnfm route, and the pixel of pen i is the
 *     value the driver lists in [16 + i]. Packed is one byte per pixel;
 *     interleaved is the Atari screen form, per 16 pixels one word per
 *     plane, plane p carrying bit p of the pixel value, bit 15 leftmost.
 *   Anything else -- format -1 or 1, no CLUT, an answer that contradicts
 *   vq_extnd or does not hold together -- falls back to the vr_trnfm route,
 *   with the reason on the console. So does a start with -noeddi.
 *
 * Byte order: a pixel of 2 or 4 bytes is stored the way the CPU stores a
 * value of that size, 3 bytes in the CPU's order too, and the table's bit
 * numbers count in that value. That is what the bit numbers mean (§3.2:
 * numbered as the pixel read as a word from memory, the lower address on top
 * -- which IS the CPU's reading on a 680x0; NewTOS answers the same for its
 * native little-endian word, decision E3 = 0x02). work_out[14], the bit-order
 * flag, therefore needs no action of its own: §3.3's swapped layouts (bit 7,
 * e.g. Crazy Dots 0x81) arrive already folded into the bit numbers (§3.2,
 * the Crazy Dots example), and applying the flag on top would swap twice.
 * It only exists from EdDI 1.10, and the version is only available from the
 * cookie's dispatcher, which this file never calls (NewTOS' cookie value is a
 * marker, not code, E2); it is printed, not used. HighWire and zView read the
 * tables the same way (§3.4).
 *
 * The cookie value is only tested for presence -- never called.
 *
 * Before the route is used, the description is checked against the driver
 * itself: 256 test colours are put into the pens, painted, read back with
 * vro_cpyfm into a device form (no vr_trnfm), and decoded with the tables.
 * Each channel must come back within one step of the colour that was set
 * (the VDI rounds its per-mille values on its own), resp. each pen's pixel
 * must be the listed [16 + i]. A description that fails this is not used:
 * the vr_trnfm route takes over, with the reason on the console. The first
 * frame is then read back the same way and compared pixel by pixel.
 *
 *
 * THE COLOUR TABLE: MEASURED, NOT COMPUTED
 * ----------------------------------------
 * What N-bit value a colour has is the driver's business, so it is asked,
 * not computed: DOOM's 256 colours are installed as VDI pens with vs_color,
 * one pixel per pen is painted with vr_recfl, the row is read back with
 * vro_cpyfm into a device form and vr_trnfm'd into a standard form. Plane p's
 * bit for pixel i is then bit p of pixval[i]. No RGB-to-word formula and no
 * plane-to-bit convention appears in this file, so neither can be wrong.
 *
 * When the VDI has a colour lookup table (INQ_TAB[5] = 1: a palette screen,
 * ST/TT/NOVA 8-bit) a pen's pixel value never changes, so it is measured once
 * and a DOOM palette change is 256 vs_color calls -- the effect is free, as it
 * was on the PC. Without one (INQ_TAB[5] = 0: truecolour, NewTOS reports 16
 * planes) the pixel IS the colour, so the table is measured again whenever
 * DOOM changes its palette. The measurement paints inside the picture's own
 * rectangle, which the frame blitted right after it covers.
 *
 * The whole route is verified once, on the first frame: every row of the
 * picture is read back from the screen through the same two calls and
 * compared with what was meant. A driver whose standard form is not what
 * this file builds is refused by name (I_Error) instead of drawn wrong.
 *
 *
 * THE SCREEN
 * ----------
 * Singletasking: DOOM owns the whole screen, the way a GEM application takes
 * it -- wind_update(BEG_UPDATE/BEG_MCTRL), form_dial(FMD_START) over the full
 * screen, the mouse arrow off -- and gives it back with form_dial(FMD_FINISH),
 * which makes the AES redraw the desktop. The picture is centred on a black
 * screen, at the largest whole stage that fits (SCALING); it is deliberately
 * not stretched to the full screen beyond that.
 *
 *
 * SCALING (TASK/PORT06, user decisions 2026-10-02 "Praezisiert" and
 * 2026-10-04 "non-square pixels")
 * -------------------------------------------------------------
 * DOOM's 320x200 picture was made for a 4:3 screen. The stage N is the
 * HORIZONTAL factor -- every DOOM pixel N device pixels wide -- and the rows
 * follow from the shape of a device pixel, so that the picture is 4:3 on the
 * glass. The VDI reports that shape: DEV_TAB[3] / [4], the pixel width and
 * height in microns, v_opnvwk work_out[3] / [4] (docs/api/vdi/workstation/
 * 100-v_opnvwk.md, DEV_TAB table; EmuTOS bios/screen.c get_pixel_size:
 * 278 x 278 for a 640x480 VGA mode, 556 x 278 for the TT's 320x480). With
 * pixel width pw and height ph, a picture 320 * N pixels wide is 4:3 when
 *
 *     rows = 240 * N * pw / ph            (rounded; square pixels: 240 * N)
 *
 * and the 200 source rows are spread over them by repeating rows (output row
 * y shows source row y * 200 / rows). Never fewer than 200 rows: dropping
 * DOOM's rows would lose picture, so tall device pixels get at least 1:1.
 * On square pixels that gives the stages of 2026-10-02 unchanged:
 *
 *     -scale 1    320x200   DOOM's picture as it is, no aspect correction
 *     -scale 2    640x480   } horizontally N times, vertically 1.2 * N times
 *     -scale 3    960x720   } (200 rows -> 240 * N by repeating rows), which
 *     -scale 4   1280x960   } restores the 4:3 shape a 320x200 picture had on
 *                           } a CRT
 *     -scale 0    the largest stage that fits the screen (also what an
 *                 out-of-range N means)
 *
 * Stage 1 on square pixels stays DOOM's picture as it is (2026-10-02). On
 * non-square pixels there is no "as it is": the rows have to be resampled for
 * any right shape, so stage 1 there is 4:3 too -- on the TT's 320x480
 * (556 x 278 um) that is 320 wide and 240 * 2 = 480 rows, the full screen.
 * A stage whose rows exceed the screen steps down like a too-wide one; only
 * stage 1, which has nowhere to step to, is fitted instead: its rows are cut
 * to the screen height (never below 200).
 *
 * Without the parameter the stage is DOOM_GEM_SCALE (one constant, below).
 * A stage larger than the screen steps down to the largest one that fits.
 *
 * The picture is scaled AFTER vr_trnfm (on the EdDI route: after the device
 * form is built), in the device form, by repeating whole pixels and whole
 * rows: c2p and vr_trnfm keep working on 320x200, and the copying stays out
 * of the standard form, where one pixel is spread over N planes.
 *
 * Rows: a device form is stored row after row ("interleaved (for 1-8 planes)
 * or pixel-packed (Falcon Truecolor mode)", EmuTOS vdi/vdi_raster.c:264-267),
 * every row of a w-pixel form the same number of bytes: (w + 15) / 16 words
 * per plane with 1-8 planes -- interleaved planes and 8-bit chunky hold the
 * same w * planes bits -- and (w + 15) / 16 * 16 pixels of (planes + 7) / 8
 * bytes when packed (form_row_bytes). So repeating a row is a byte copy on
 * every device, and the vertical stretch runs on every route and depth.
 *
 * Pixels: repeating a pixel needs the pixel to be a run of bytes, which only
 * the packed forms are: more than 8 planes (packed truecolour, (planes + 7) /
 * 8 bytes per pixel, EmuTOS vr_trnfm16, vdi_raster.c:190), and 8 planes where
 * EdDI says packed pixels with a CLUT (vq_scrninfo format 2, clut 1, one byte
 * per pixel: a NOVA 256-colour mode; docs/api/vdi/eddi-vq_scrninfo.md §3.2).
 * Everywhere else -- interleaved planes (TT, ST, EdDI format 0), and 8 planes
 * on the vr_trnfm route, where the driver has not said which form it uses --
 * the horizontal factor is 1. The first-frame read-back (below) checks the
 * scaled picture pixel by pixel, so a form that is not what this paragraph
 * says is refused by name, not drawn wrong. Copying a pixel copies its bytes;
 * nothing here looks at what the bytes mean, so byte order does not matter.
 *
 *
 * INPUT (TASK/KEYS02)
 * -------------------
 * TOS tells a program when a key goes down (GEMDOS, BIOS, AES), never when
 * it comes up. The one place that sees both is the IKBD keyboard vector
 * `kbdvec`, the slot right before the table XBIOS Kbdvbase() returns
 * (docs/api/xbios/22-kbdvbase.md A2, A4: every keyboard byte, bit 7 = break).
 * So DOOM hooks it:
 *
 *   * kbdvec route (the normal one). At DG_Init the hook
 *     (doomgeneric_gem_kbdvec.c) goes into the slot. It records each make
 *     and break byte in a small ring and chains to the previous vector, so
 *     the BIOS queue, Kbshift and the AES go on as before (A6.3). DG_GetKey
 *     turns the bytes into presses and releases with the real timing: a key
 *     is down from its make to its break, however many other keys come and
 *     go meanwhile, and a tap is a press and a release as far apart as the
 *     finger was. Scancode to DOOM key: the special keys by scancode (Help is
 *     F11 = gamma, Undo is Pause), every other key by the character
 *     Keytbl()'s unshifted table gives it, so the keyboard's own layout
 *     still decides which key is 'y'; '+' counts as '=' (screen size up,
 *     map zoom in -- '=' is Shift+0 on the German layout). ',' and '.' stay
 *     characters; DG_Init binds them to strafe left and right, as in
 *     vanilla DOOM. The AES keyboard
 *     events the chained vector still produces are read and dropped, so they
 *     do not pile up for whoever owns the keyboard after DOOM.
 *     The hook comes out again on every way out: atexit() (a normal quit,
 *     I_Error -- both end in exit()), and on a 680x0 also etv_term, which
 *     TOS runs on every Pterm, crashes included (doomgeneric_gem_kbdvec.h).
 *     On NewTOS the hook ends with the process anyway (22-kbdvbase.md C5).
 *
 *   * AES route (the fallback). evnt_multi(MU_KEYBD | MU_TIMER) with a zero
 *     timer, which is a poll. The modifier state comes with every event
 *     (kstate), so SHIFT (run), CONTROL (fire) and ALTERNATE (strafe) are
 *     true held keys. Ordinary keys deliver presses only, so a key counts as
 *     held while its keyboard auto-repeat keeps arriving and is released when
 *     the repeat stops; the timeouts come from the keyboard's own repeat rate
 *     (XBIOS Kbrate). Only the last key pressed repeats (EmuTOS bios/ikbd.c
 *     :399-423), so a held key "comes up" when a second one goes down
 *     (TASK/KEYS01-SEQUENCE-CHECK.MD) -- the reason the kbdvec route exists.
 *
 * Which route: the kbdvec route needs TOS 2.00 or later -- "TOS < 2.00 doesn't
 * know about this vector" -- or EmuTOS, the rule FreeMiNT applies to the OS
 * header (references/freemint/sys/keyboard.c:1938-1941 `has_kbdvec`, version
 * and 'ETOS' read in sys/init.c:386-401); NewTOS reports 4.04 there
 * (kernel/src/selftest_sysvars.rs) and has the vector since KEYS02
 * (22-kbdvbase.md C). A kbdvec slot that holds 0, or no Keytbl, also means
 * the AES route. A start with -nokbdvec takes the AES route on purpose, for
 * comparison. The choice is printed on the console. Not handled: FireTOS
 * and CT60 TOS 2.xx, whose ikbdsys never calls kbdvec (freemint sys/arch/
 * init_intr.c:104-111) -- there a hooked DOOM gets no keys from the hook;
 * start it with -nokbdvec.
 *
 *
 * TIME
 * ----
 * The 200 Hz system tick, _hz_200 (0x4ba), read with mintlib's get_sysvar():
 * on TOS that is a supervisor read (Super), under MiNT Ssystem(S_GETLVAL), on
 * NewTOS a plain load of the per-process sysvar page the loader maps
 * (sdk/libc/newtos-arch/getsysvar.c) -- the same counter everywhere, 5 ms
 * resolution. DOOM's game clock is 35 Hz, so 5 ms is fine-grained enough.
 * Sleeping is evnt_timer(): the AES call that gives the time to whoever else
 * wants it instead of spinning.
 *
 *
 * MEMORY
 * ------
 * DOOM allocates its zone with malloc() (i_system.c I_ZoneBase), 6 MiB unless
 * -mb N says otherwise. On NewTOS the program gets the largest free block as
 * its TPA and its start-up code shrinks it to program + stack, as mintlib's
 * crtinit does on TOS (sdk/libc/newtos-arch/heap_init.c, ARCH03 E9); malloc
 * then grows through GEMDOS Malloc (PF_TTRAMMEM: TT-RAM preferred), so the
 * zone is one Malloc block of its own and DOOM's 6 MiB default fits wherever
 * that much is free. DOOM_GEM_ARGS (build-doom.sh, default "-mb 6" -- DOOM's
 * own value, passed explicitly as in build-doom-m68k.sh) is appended to every
 * start; a "-mb N" given at the start wins. Until ARCH03 E9 (2026-10-05) the
 * TPA was a fixed 4 MiB and malloc ran inside it, which capped the zone at
 * 2 MiB ("-mb 2").
 * This file's own buffers -- the
 * device form of the 320x200 picture and, on the vr_trnfm route only, its
 * standard form (125 KiB each at 16 planes) and, when scaling,the scaled device form (out_w * out_h * bytes
 * per pixel: 2400 KiB for 1280x960 at 16 planes) -- come from GEMDOS
 * Mxalloc(MX_PREFTTRAM), Malloc as fallback: CPU-only memory, no DMA reason to
 * want ST-RAM (PORT06 "ST-RAM ist auch auf dem TT kein Thema").
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* ======================================================================== */
/* Chunky to planar                                                          */
/* ======================================================================== */

/*
 * WHAT IT BUILDS. c2p_row converts `n` 8-bit indices (any n >= 0) into planar
 * words: per 16-pixel group, plane p's word collects bit p of the 16 pixels'
 * values, the leftmost pixel in bit 15; in a last, partial group the pixels
 * past n are 0 in every plane. Where the words go is two strides: plane p of
 * group g is dst[g * gs + p * ps]. The standard form is ps = plane_words,
 * gs = 1 (one plane after the other); the interleaved device form of the
 * EdDI route is ps = 1, gs = planes (per group one word per plane). The
 * values come from the 256-entry table given to c2p_prepare (pixval[] on the
 * vr_trnfm route, devpix[] on the EdDI one), up to C2P_MAX_PLANES planes.
 *
 * HOW (user decision 2026-10-04: the format is known once, the conversion
 * data is computed once, a frame only looks up and writes). The value is
 * cut into slices of 8 planes. For a slice, 4 consecutive pixels are
 * 4 x 8 = 32 bits, one long, and c2p_prepare's table (256 longs per slice)
 * puts a pixel's 8 plane bits where the long needs them -- the palette
 * lookup and the bit spreading in one load. Bit b4..b0 of the long of
 * pixels k = 0..3 holds:
 *
 *     b4 = NOT bit 0 of the plane number, b3 = bit 2, b2 = bit 1,
 *     b1 b0 = 3 - k
 *
 * The table has the bits at b1 b0 = 0; the long is gathered as
 * ((t[p0] << 1 | t[p1]) << 1 | t[p2]) << 1 | t[p3], which moves each pixel
 * to its 3 - k (never out of its 4-bit field). The 16 pixels of a group give
 * 4 longs; the one of pixels 4j..4j+3 goes into slot 3 - j. Two exchanges of
 * the classic merge c2p then move the slot number into b3 b2 and plane bits
 * 2, 1 into the slot number (slot bit 1 <-> b3, slot bit 0 <-> b2, one
 * masked swap per slot pair each), after which slot r is
 * (word of plane 2r) << 16 | (word of plane 2r + 1): b4 says which half,
 * b3..b0 = 15 - pixel. The words are stored as 16-bit values, so the byte
 * order of the CPU never enters.
 *
 * Why this shape (measured on Hatari's TT/68030 model, measurements/
 * measurement-2026-10-04-doom-c2p/, against four per-position tables, one
 * 16 x 4-long table per pixel and the full five-pass merge c2p): ONE 1 KiB
 * table per slice instead of four keeps most loads in the 68030's 256-byte
 * data cache (DOOM's neighbouring pixels repeat colours), and a row is done
 * in chunks of 4 groups -- first the longs of 64 pixels into a 64-byte
 * buffer, then the exchanges and stores -- so that each loop fits the
 * 256-byte instruction cache. One loop over a group did not, and missed it
 * ~16 times per group.
 *
 * c2p_prepare runs when its source table changes -- once on a CLUT screen,
 * on every DOOM palette change where the pixel IS the colour (no CLUT) --
 * and costs 256 * 8 bit tests per slice. The bit-by-bit routine this
 * replaced is kept as the oracle of sdk/apps/doom/tests/c2p-host.c, which
 * compiles only this section (DOOM_GEM_C2P_HOST_TEST).
 */
#define C2P_MAX_PLANES	32
#define C2P_SLICES	(C2P_MAX_PLANES / 8)
#define C2P_CHUNK	4		/* groups per gather pass: 64 pixels */

#if defined(__GNUC__)
#define C2P_INLINE	static inline __attribute__((always_inline))
#else
#define C2P_INLINE	static inline
#endif

static uint32_t c2p_tab[C2P_SLICES][256];	/* [slice][index] */
static int c2p_planes;				/* the tables' plane count */

/* The tables for `planes` planes from val[256] (bit p = plane p). */
static void c2p_prepare(const uint32_t *val, int planes)
{
	int s, v, p;

	for (s = 0; s < (planes + 7) / 8; s++)
		for (v = 0; v < 256; v++) {
			uint32_t b = (val[v] >> (8 * s)) & 0xffu, t = 0;

			for (p = 0; p < 8; p++)
				if (b & (1u << p))
					t |= (uint32_t)1 << ((((p & 1) ^ 1) << 4) |
							     (((p >> 2) & 1) << 3) |
							     (((p >> 1) & 1) << 2));
			c2p_tab[s][v] = t;
		}
	c2p_planes = planes;
}

/* The two exchanges (slot bit 1 <-> b3, slot bit 0 <-> b2). */
#define C2P_MERGE(x0, x1, x2, x3, t) do {				\
	t = ((x0 >> 8) ^ x2) & 0x00ff00ffu; x2 ^= t; x0 ^= t << 8;	\
	t = ((x1 >> 8) ^ x3) & 0x00ff00ffu; x3 ^= t; x1 ^= t << 8;	\
	t = ((x0 >> 4) ^ x1) & 0x0f0f0f0fu; x1 ^= t; x0 ^= t << 4;	\
	t = ((x2 >> 4) ^ x3) & 0x0f0f0f0fu; x3 ^= t; x2 ^= t << 4;	\
} while (0)

/* The first np (< 8, or a partial group) plane words of a merged group. */
static void c2p_store(uint16_t *d, long ps, int np, uint32_t x0, uint32_t x1,
		      uint32_t x2, uint32_t x3)
{
	uint32_t x[4];
	int p;

	x[0] = x0;
	x[1] = x1;
	x[2] = x2;
	x[3] = x3;
	for (p = 0; p < np; p++)
		d[p * ps] = (uint16_t)((p & 1) ? x[p >> 1] : x[p >> 1] >> 16);
}

/* One slice (np planes, its table tab) of a row. Inlined per call site so
 * that ps = 1 (interleaved) gets constant store offsets. */
C2P_INLINE void c2p_slice(const uint8_t *s, int n, uint16_t *d, long ps,
			  long gs, const uint32_t *tab, int np)
{
	uint32_t buf[4 * C2P_CHUNK];
	uint32_t x0, x1, x2, x3, t, x[4];
	int groups = n / 16, c, ng, g, i;

	for (c = 0; c < groups; c += ng) {
		ng = groups - c < C2P_CHUNK ? groups - c : C2P_CHUNK;
		for (i = 0; i < 4 * ng; i++, s += 4)
			buf[i] = (((((tab[s[0]] << 1) | tab[s[1]]) << 1) |
				   tab[s[2]]) << 1) | tab[s[3]];
		for (g = 0; g < ng; g++, d += gs) {
			x3 = buf[4 * g];
			x2 = buf[4 * g + 1];
			x1 = buf[4 * g + 2];
			x0 = buf[4 * g + 3];
			C2P_MERGE(x0, x1, x2, x3, t);
			if (np == 8) {
				d[0] = (uint16_t)(x0 >> 16);
				d[ps] = (uint16_t)x0;
				d[2 * ps] = (uint16_t)(x1 >> 16);
				d[3 * ps] = (uint16_t)x1;
				d[4 * ps] = (uint16_t)(x2 >> 16);
				d[5 * ps] = (uint16_t)x2;
				d[6 * ps] = (uint16_t)(x3 >> 16);
				d[7 * ps] = (uint16_t)x3;
			} else {
				c2p_store(d, ps, np, x0, x1, x2, x3);
			}
		}
	}
	if (n & 15) {
		x[0] = x[1] = x[2] = x[3] = 0;
		for (i = 0; i < (n & 15); i++)
			x[3 - (i >> 2)] |= tab[s[i]] << (3 - (i & 3));
		x0 = x[0];
		x1 = x[1];
		x2 = x[2];
		x3 = x[3];
		C2P_MERGE(x0, x1, x2, x3, t);
		c2p_store(d, ps, np, x0, x1, x2, x3);
	}
}

/* `n` indices from src into planar words at dst (WHAT IT BUILDS). */
static void c2p_row(const uint8_t *src, int n, uint16_t *dst, long ps, long gs)
{
	int s, np;

	for (s = 0; s < (c2p_planes + 7) / 8; s++) {
		uint16_t *d = dst + 8L * s * ps;

		np = c2p_planes - 8 * s < 8 ? c2p_planes - 8 * s : 8;
		if (np == 8 && ps == 1)
			c2p_slice(src, n, d, 1, gs, c2p_tab[s], 8);
		else if (np == 8)
			c2p_slice(src, n, d, ps, gs, c2p_tab[s], 8);
		else
			c2p_slice(src, n, d, ps, gs, c2p_tab[s], np);
	}
}

#ifndef DOOM_GEM_C2P_HOST_TEST	/* the rest of the platform layer */

#include <gem.h>
#include <gemx.h>		/* vq_scrninfo (EdDI) */
#include <mint/cookie.h>
#include <mint/osbind.h>
#include <mint/sysvars.h>

#include "doomgeneric.h"
#include "doomkeys.h"
#include "m_argv.h"
#include "i_video.h"		/* colors[], palette_changed (CMAP256) */
#include "i_system.h"		/* I_Error */
#include "m_controls.h"		/* key_strafeleft / key_straferight */
#include "doomgeneric_gem_kbdvec.h"

#ifndef CMAP256
#error "doomgeneric_gem.c serves the CMAP256 build only (8-bit palette picture)"
#endif

#ifndef MX_PREFTTRAM
#define MX_PREFTTRAM	3	/* alternate RAM preferred, ST-RAM if none */
#endif

/* Extra arguments for a start without a command tail; build-doom.sh sets it. */
#ifndef DOOM_GEM_ARGS
#define DOOM_GEM_ARGS "-mb", "6"
#endif

/* Seconds between frame-time reports on the console; 0 = none. */
#ifndef DOOM_GEM_STATS
#define DOOM_GEM_STATS 0
#endif

/* The stage a start without -scale gets (see SCALING): 1..4, or 0 for the
 * largest stage that fits the screen. To be fixed after the first run on the
 * real Pi (PORT06, 2026-10-02). */
#ifndef DOOM_GEM_SCALE
#define DOOM_GEM_SCALE 0
#endif

#define PIC_W		DOOMGENERIC_RESX	/* 320 */
#define PIC_H		DOOMGENERIC_RESY	/* 200 */
#define NCOLORS		256
#define MAX_PLANES	C2P_MAX_PLANES		/* pixval is 32 bits wide */
#define MAX_STAGE	4			/* 1280x960 */

/* ---- AES / VDI ------------------------------------------------------------ */
static short ap_id = -1;
static short vh;			/* virtual workstation handle */
static short work_in[11];
static short work_out[57];
static int screen_taken;		/* wind_update + form_dial pair open */
static int scr_w, scr_h;
static int nplanes;			/* INQ_TAB[4] */
static int has_lut;			/* INQ_TAB[5]: colour lookup table */
static int pix_w, pix_h;		/* DEV_TAB[3] / [4]: pixel size, microns */

/* ---- the picture ---------------------------------------------------------- */
static int want_stage;			/* -scale N or DOOM_GEM_SCALE, 0..MAX_STAGE */
static int stage = 1;			/* horizontal factor 1..MAX_STAGE, SCALING */
static int out_w, out_h;		/* stage_w(stage), stage_h(stage) fitted */
static int scaled;			/* out_w x out_h is not 320x200 */
static int out_x, out_y;		/* top-left on the screen */
static const long wdw = (PIC_W + 15) / 16;	/* fd_wdwidth, 320x200 forms */
static const long plane_words = (long)PIC_H * ((PIC_W + 15) / 16);
static int dev_bpp;			/* bytes per pixel, packed form (> 8 planes) */
static long dev_stride;			/* bytes per row, 320x200 device form */
static long big_stride;			/* bytes per row, scaled device form */
static void *std_raw, *dev_raw, *big_raw;
static uint16_t *std_buf;		/* standard form, 320x200, nplanes planes */
static uint16_t *dev_buf;		/* device form, 320x200 */
static void *big_buf;			/* device form, out_w x out_h (scaled) */
static MFDB std_mfdb, dev_mfdb, big_mfdb, screen_mfdb;	/* screen: fd_addr == 0 */

/* ---- read-back (one row, any width up to out_w) ---------------------------- */
static void *peek_dev_raw, *peek_std_raw;
static uint16_t *peek_dev, *peek_std;
static MFDB peek_dev_mfdb, peek_std_mfdb;

/* ---- the colour table ----------------------------------------------------- */
static uint32_t pixval[NCOLORS];	/* bit p = plane p for that colour */
static int pixval_valid;
static short saved_pens[NCOLORS][3];
static int pens_saved;

/* ---- the EdDI route (THE EdDI ROUTE) --------------------------------------- */
#define SI_WORDS	280		/* vq_scrninfo writes 273 (spec §3.1) */
#define EDDI_NONE	0		/* vr_trnfm route */
#define EDDI_TRUE	1		/* packed, bit-number tables */
#define EDDI_PACKED8	2		/* packed, 8 planes, CLUT pixel values */
#define EDDI_PLANES	3		/* interleaved planes, CLUT pixel values */

static int eddi;			/* EDDI_*: the route in use */
static short si[SI_WORDS];		/* the vq_scrninfo answer */
static int ch_n[3];			/* red/green/blue intensity bits */
static int ch_bit[3][16];		/* pixel bit of intensity bit k */
static uint32_t cmp_mask;		/* the pixel bits this file defines */
static uint32_t devpix[NCOLORS];	/* device pixel per DOOM index */
static int host_big_endian;		/* for the 3-byte pixel only */

static int first_frame = 1;
static int need_clear = 1;		/* paint the screen around the picture */

/* ---- stats ---------------------------------------------------------------- */
#if DOOM_GEM_STATS > 0
static uint32_t st_t0, st_frames, st_pal;
#endif

/*
 * diag(): every line this file prints for diagnosis ("DOOM: ..."). In the
 * shipped build (DOOM_GEM_STATS == 0) it is printf and nothing else. In a
 * stats build the line still goes to the console unchanged and is also
 * appended to DOOMSTAT.TXT in the current directory, CRLF-terminated (Atari
 * text viewers), so the startup log, the cost table and the periodic frame
 * lines can be read after the run -- on a 256-colour screen the console text
 * is drawn in pens DOOM's palette has taken over.
 *
 * Plain GEMDOS (Fcreate/Fopen/Fseek/Fwrite/Fclose), as on TOS 3.06, MagiC
 * and NewTOS alike. GEMDOS has no flush: written sectors may sit in the
 * buffer cache and the directory entry (the file length) is updated only by
 * Fclose. So every line is appended with open - seek to end - write - close;
 * a crash or a power-off loses at most the line being written, and no handle
 * stays open that exit would have to close. The first line of a run creates
 * (truncates) the file; a failing open ends the file for the run, said once
 * on the console.
 */
#if DOOM_GEM_STATS > 0
#include <stdarg.h>

#define STAT_FILE	"DOOMSTAT.TXT"

static int stat_file_state;		/* 0 not yet created, 1 open-able, -1 off */

static void stat_append(const char *s, long n)
{
	long fh, r;

	if (stat_file_state < 0)
		return;
	if (stat_file_state == 0) {
		fh = Fcreate(STAT_FILE, 0);
	} else {
		fh = Fopen(STAT_FILE, 1);		/* write only */
		if (fh >= 0 && Fseek(0L, (short)fh, 2) < 0) {
			Fclose((short)fh);
			fh = -1;
		}
	}
	if (fh < 0) {
		printf("DOOM: %s cannot be %s (%ld) -- no more lines go to the "
		       "file\n", STAT_FILE, stat_file_state ? "opened" : "created",
		       fh);
		stat_file_state = -1;
		return;
	}
	stat_file_state = 1;
	r = Fwrite((short)fh, n, s);
	Fclose((short)fh);
	if (r != n) {
		stat_file_state = -1;
		printf("DOOM: %s: Fwrite wrote %ld of %ld bytes -- no more lines "
		       "go to the file\n", STAT_FILE, r, n);
	}
}

/* One formatted line into the file only; the line ending becomes CRLF. */
static void stat_vfile(const char *fmt, va_list ap)
{
	char line[512];
	int n;

	n = vsnprintf(line, sizeof(line) - 2, fmt, ap);
	if (n < 0)
		return;
	if (n > (int)sizeof(line) - 3)
		n = (int)sizeof(line) - 3;		/* cut, still one line */
	while (n > 0 && (line[n - 1] == '\n' || line[n - 1] == '\r'))
		n--;
	line[n++] = '\r';
	line[n++] = '\n';
	stat_append(line, n);
}

static void stat_file(const char *fmt, ...)
	__attribute__((format(printf, 1, 2)));
static void stat_file(const char *fmt, ...)
{
	va_list ap;

	va_start(ap, fmt);
	stat_vfile(fmt, ap);
	va_end(ap);
}

static int diag(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
static int diag(const char *fmt, ...)
{
	va_list ap;
	int r;

	va_start(ap, fmt);
	r = vprintf(fmt, ap);			/* the console, as before */
	va_end(ap);
	va_start(ap, fmt);
	stat_vfile(fmt, ap);
	va_end(ap);
	return r;
}
#else
#define diag printf
#endif

static uint32_t ticks200(void)
{
	return (uint32_t)get_sysvar(_hz_200);
}

uint32_t DG_GetTicksMs(void)
{
	return ticks200() * 5u;
}

void DG_SleepMs(uint32_t ms)
{
	evnt_timer((unsigned long)ms);
}

/* ======================================================================== */
/* Forms                                                                     */
/* ======================================================================== */

static void set_form(MFDB *m, void *addr, int w, int h, int stand)
{
	m->fd_addr = addr;
	m->fd_w = (short)w;
	m->fd_h = (short)h;
	m->fd_wdwidth = (short)((w + 15) / 16);
	m->fd_stand = (short)stand;
	m->fd_nplanes = (short)nplanes;
	m->fd_r1 = 0;
	m->fd_r2 = 0;
	m->fd_r3 = 0;
}

/*
 * Bytes one row of a w-pixel device form takes (SCALING, "Rows"). With 1-8
 * planes that is the planar size the VDI documents, fd_wdwidth words per
 * plane -- the same for 8-bit chunky. With more planes the form is
 * pixel-packed, dev_bpp bytes per pixel and fd_wdwidth * 16 pixels per row;
 * for 16/24/32 planes the two readings agree, for a 15-plane form the packed
 * one is larger and is the one taken, so neither can run past the end.
 */
static long form_row_bytes(int w)
{
	long wd = (w + 15) / 16;

	return nplanes > 8 ? wd * 16 * dev_bpp : wd * 2 * nplanes;
}

/* Bytes a device form of w x h pixels needs. */
static long form_bytes_for(int w, int h)
{
	return (long)h * form_row_bytes(w);
}

/* The stage geometry (SCALING). Horizontal: N times. */
static int stage_w(int s)
{
	return PIC_W * s;
}

/* Vertical: the rows that make 320 * s pixels of pix_w x pix_h microns a 4:3
 * picture, at least DOOM's 200; stage 1 on square pixels is DOOM's picture
 * as it is. Not yet fitted to the screen (see choose_stage). */
static int stage_h(int s)
{
	long rows;

	if (s == 1 && pix_w == pix_h)
		return PIC_H;
	rows = (240L * s * pix_w + pix_h / 2) / pix_h;
	return rows < PIC_H ? PIC_H : (int)rows;
}

static int stage_fits(int s)
{
	return stage_w(s) <= scr_w && stage_h(s) <= scr_h;
}

/* A buffer for CPU-only data: alternate RAM preferred, any RAM accepted. */
static void *alloc_fast(long bytes)
{
	void *p = (void *)Mxalloc(bytes, MX_PREFTTRAM);

	/* Mxalloc returns -32 (EINVFN) on a TOS that does not know it. */
	if ((long)p == -32L || p == NULL)
		p = (void *)Malloc(bytes);
	return p;
}

/*
 * Read `n` screen pixels of row `y` from column `x` and return them as pixel
 * values in vals[] (bit p = plane p). The forms are one row tall, so they
 * have no row stride to get wrong.
 */
static void read_row(int x, int y, int n, uint32_t *vals)
{
	short pxy[8];
	long wd = (n + 15) / 16;
	int i, p;

	set_form(&peek_dev_mfdb, peek_dev, n, 1, 0);
	set_form(&peek_std_mfdb, peek_std, n, 1, 1);
	memset(peek_dev, 0, (size_t)(wd * nplanes * 2));
	pxy[0] = (short)x;
	pxy[1] = (short)y;
	pxy[2] = (short)(x + n - 1);
	pxy[3] = (short)y;
	pxy[4] = 0;
	pxy[5] = 0;
	pxy[6] = (short)(n - 1);
	pxy[7] = 0;
	vro_cpyfm(vh, S_ONLY, pxy, &screen_mfdb, &peek_dev_mfdb);
	vr_trnfm(vh, &peek_dev_mfdb, &peek_std_mfdb);

	for (i = 0; i < n; i++) {
		uint32_t v = 0;
		uint16_t bit = (uint16_t)(0x8000u >> (i & 15));

		for (p = 0; p < nplanes; p++)
			if (peek_std[p * wd + (i >> 4)] & bit)
				v |= (uint32_t)1 << p;
		vals[i] = v;
	}
}

/* ======================================================================== */
/* The colour table                                                          */
/* ======================================================================== */

/* DOOM's palette into the 256 pens. The VDI takes per-mille intensities. */
static void load_pens(void)
{
	short rgb[3];
	int i;

	for (i = 0; i < NCOLORS; i++) {
		rgb[0] = (short)(((long)colors[i].r * 1000L + 127L) / 255L);
		rgb[1] = (short)(((long)colors[i].g * 1000L + 127L) / 255L);
		rgb[2] = (short)(((long)colors[i].b * 1000L + 127L) / 255L);
		vs_color(vh, (short)i, rgb);
	}
}

/* Paint one pixel per pen inside the picture rectangle, read the row back. */
static void measure_pixvals(void)
{
	short pxy[4];
	int i;

	for (i = 0; i < NCOLORS; i++) {
		vsf_color(vh, (short)i);
		pxy[0] = (short)(out_x + i);
		pxy[1] = (short)out_y;
		pxy[2] = pxy[0];
		pxy[3] = pxy[1];
		vr_recfl(vh, pxy);
	}
	read_row(out_x, out_y, NCOLORS, pixval);
	pixval_valid = 1;
	c2p_prepare(pixval, nplanes);
}

static void eddi_palette(void);

static void update_palette(void)
{
	if (eddi != EDDI_NONE) {
		eddi_palette();
		return;
	}
	load_pens();
	if (!has_lut || !pixval_valid)
		measure_pixvals();
}

/* ======================================================================== */
/* The standard form (chunky to planar: top of the file)                     */
/* ======================================================================== */

/* The whole 320x200 picture into the standard form: one plane after the
 * other, plane_words apart (c2p_prepare(pixval) ran in measure_pixvals). */
static void build_standard_form(void)
{
	const uint8_t *src = (const uint8_t *)DG_ScreenBuffer;
	long y;

	for (y = 0; y < PIC_H; y++, src += PIC_W)
		c2p_row(src, PIC_W, std_buf + y * wdw, plane_words, 1);
}

/* ======================================================================== */
/* Scaling in the device form (SCALING)                                      */
/* ======================================================================== */

/* One 320-pixel device row, every pixel `stage` times. At stage 1 the row is
 * copied whole, whatever its form. Above 1 the form is packed (choose_stage
 * allows no other): a pixel is dev_bpp bytes and is copied as bytes (two
 * 16-bit halves for 2 and 4); the buffers are GEMDOS blocks and every row
 * starts at an even offset, so 16-bit access is aligned on any CPU. */
static void scale_row(const void *srow, void *drow)
{
	int x, k;

	if (stage == 1) {
		memcpy(drow, srow, (size_t)dev_stride);
	} else if (dev_bpp == 1) {
		const uint8_t *s = srow;
		uint8_t *d = drow;

		for (x = 0; x < PIC_W; x++) {
			uint8_t v = s[x];

			for (k = 0; k < stage; k++)
				*d++ = v;
		}
	} else if (dev_bpp == 2) {
		const uint16_t *s = srow;
		uint16_t *d = drow;

		for (x = 0; x < PIC_W; x++) {
			uint16_t v = s[x];

			for (k = 0; k < stage; k++)
				*d++ = v;
		}
	} else if (dev_bpp == 4) {
		const uint16_t *s = srow;
		uint16_t *d = drow;

		for (x = 0; x < PIC_W; x++, s += 2) {
			uint16_t a = s[0], b = s[1];

			for (k = 0; k < stage; k++) {
				*d++ = a;
				*d++ = b;
			}
		}
	} else {
		const uint8_t *s = srow;
		uint8_t *d = drow;

		for (x = 0; x < PIC_W; x++, s += dev_bpp)
			for (k = 0; k < stage; k++, d += dev_bpp)
				memcpy(d, s, (size_t)dev_bpp);
	}
}

/* The 320x200 device form into the out_w x out_h one: output row y shows
 * source row y * 200 / out_h, so each source row appears out_h / 200 times
 * (2 or 3 times at 480 rows); a repeated row is a copy of the one above. */
static void stage_scale(void)
{
	const uint8_t *src = (const uint8_t *)dev_buf;
	uint8_t *dst = big_buf;
	long prev = -1, sy;
	int y;

	for (y = 0; y < out_h; y++, dst += big_stride) {
		sy = (long)y * PIC_H / out_h;
		if (sy == prev) {
			memcpy(dst, dst - big_stride, (size_t)big_stride);
		} else {
			scale_row(src + sy * dev_stride, dst);
			prev = sy;
		}
	}
}

/* ======================================================================== */
/* The EdDI route                                                            */
/* ======================================================================== */

/* One pixel into / out of a packed device row, in the CPU's byte order (THE
 * EdDI ROUTE). Rows start at even offsets of GEMDOS blocks; a 4-byte pixel
 * goes through memcpy, so nothing needs more than word alignment. */
static void put_packed(uint8_t *row, int x, uint32_t v)
{
	uint8_t *p;

	switch (dev_bpp) {
	case 1:
		row[x] = (uint8_t)v;
		break;
	case 2:
		((uint16_t *)row)[x] = (uint16_t)v;
		break;
	case 3:
		p = row + (long)x * 3;
		if (host_big_endian) {
			p[0] = (uint8_t)(v >> 16);
			p[1] = (uint8_t)(v >> 8);
			p[2] = (uint8_t)v;
		} else {
			p[0] = (uint8_t)v;
			p[1] = (uint8_t)(v >> 8);
			p[2] = (uint8_t)(v >> 16);
		}
		break;
	default:
		memcpy(row + (long)x * 4, &v, 4);
		break;
	}
}

static uint32_t get_packed(const uint8_t *row, int x)
{
	const uint8_t *p;
	uint32_t v;

	switch (dev_bpp) {
	case 1:
		return row[x];
	case 2:
		return ((const uint16_t *)row)[x];
	case 3:
		p = row + (long)x * 3;
		if (host_big_endian)
			return ((uint32_t)p[0] << 16) | ((uint32_t)p[1] << 8) | p[2];
		return ((uint32_t)p[2] << 16) | ((uint32_t)p[1] << 8) | p[0];
	default:
		memcpy(&v, row + (long)x * 4, 4);
		return v;
	}
}

/* Pixel x of an interleaved-planes row: per 16 pixels one word per plane,
 * plane p = bit p of the pixel value, bit 15 the leftmost pixel. */
static uint32_t get_planes(const uint16_t *row, int x)
{
	const uint16_t *grp = row + (long)(x >> 4) * nplanes;
	uint16_t bit = (uint16_t)(0x8000u >> (x & 15));
	uint32_t v = 0;
	int p;

	for (p = 0; p < nplanes; p++)
		if (grp[p] & bit)
			v |= (uint32_t)1 << p;
	return v;
}

/* An 8-bit intensity rounded to n bits. */
static uint32_t quant(int v8, int n)
{
	uint32_t max = ((uint32_t)1 << n) - 1u;

	return ((uint32_t)v8 * max + 127u) / 255u;
}

/* RGB to the device pixel with the bit-number tables (EDDI_TRUE). */
static uint32_t encode_rgb(int r, int g, int b)
{
	int c8[3];
	uint32_t v = 0, q;
	int c, k;

	c8[0] = r;
	c8[1] = g;
	c8[2] = b;
	for (c = 0; c < 3; c++) {
		q = quant(c8[c], ch_n[c]);
		for (k = 0; k < ch_n[c]; k++)
			if ((q >> k) & 1u)
				v |= (uint32_t)1 << ch_bit[c][k];
	}
	return v;
}

/* Channel c's intensity in a device pixel (EDDI_TRUE). */
static uint32_t decode_ch(uint32_t v, int c)
{
	uint32_t q = 0;
	int k;

	for (k = 0; k < ch_n[c]; k++)
		if ((v >> ch_bit[c][k]) & 1u)
			q |= (uint32_t)1 << k;
	return q;
}

/* Read `n` screen pixels of row `y` from column `x` as device pixels: one
 * vro_cpyfm into a one-row device form, decoded as the description says. */
static void read_row_dev(int x, int y, int n, uint32_t *vals)
{
	short pxy[8];
	int i;

	set_form(&peek_dev_mfdb, peek_dev, n, 1, 0);
	memset(peek_dev, 0, (size_t)form_bytes_for(n, 1));
	pxy[0] = (short)x;
	pxy[1] = (short)y;
	pxy[2] = (short)(x + n - 1);
	pxy[3] = (short)y;
	pxy[4] = 0;
	pxy[5] = 0;
	pxy[6] = (short)(n - 1);
	pxy[7] = 0;
	vro_cpyfm(vh, S_ONLY, pxy, &screen_mfdb, &peek_dev_mfdb);
	for (i = 0; i < n; i++)
		vals[i] = eddi == EDDI_PLANES ? get_planes(peek_dev, i)
					      : get_packed((const uint8_t *)peek_dev, i);
}

static int eddi_refuse(const char *why)
{
	diag("DOOM: EdDI description not used (%s) -- vr_trnfm route\n", why);
	return EDDI_NONE;
}

/*
 * Ask for the device form (THE EdDI ROUTE) and decide whether this file can
 * build it. Returns the EDDI_* route; on EDDI_TRUE ch_n/ch_bit hold the
 * tables, on the CLUT routes devpix[] holds the pens' pixel values.
 */
static int eddi_query(void)
{
	char why[96];
	long cookie = 0;
	uint32_t seen = 0;
	int c, k, b, i;

	if (M_CheckParm("-noeddi") > 0) {
		diag("DOOM: -noeddi -- vr_trnfm route\n");
		return EDDI_NONE;
	}
	if (Getcookie(C_EdDI, &cookie) != C_FOUND) {
		diag("DOOM: no EdDI cookie -- vr_trnfm route\n");
		return EDDI_NONE;
	}
	/* Present is all that counts; the value is never called. */
	memset(si, 0, sizeof(si));
	vq_scrninfo(vh, si);
	diag("DOOM: EdDI: format %d, clut %d, %d planes, bits r/g/b %d/%d/%d, "
	       "bit order 0x%x\n", si[0], si[1], si[2], si[8], si[9], si[10],
	       (unsigned)(uint16_t)si[14]);

	if (si[0] == -1)
		return eddi_refuse("format -1: not directly writable");
	if (si[2] != nplanes) {
		snprintf(why, sizeof(why), "%d planes, vq_extnd says %d",
			 si[2], nplanes);
		return eddi_refuse(why);
	}

	if (si[0] == 2 && si[1] == 2) {
		if (nplanes < 9)
			return eddi_refuse("truecolour with 8 planes or fewer");
		for (c = 0; c < 3; c++) {
			ch_n[c] = si[8 + c];
			if (ch_n[c] < 1 || ch_n[c] > 16) {
				snprintf(why, sizeof(why), "channel %d has %d bits",
					 c, ch_n[c]);
				return eddi_refuse(why);
			}
			for (k = 0; k < ch_n[c]; k++) {
				b = si[16 + 16 * c + k];
				if (b < 0 || b >= dev_bpp * 8 ||
				    (seen & ((uint32_t)1 << b))) {
					snprintf(why, sizeof(why), "bit table "
						 "entry [%d] = %d", 16 + 16 * c + k, b);
					return eddi_refuse(why);
				}
				seen |= (uint32_t)1 << b;
				ch_bit[c][k] = b;
			}
		}
		cmp_mask = seen;
		return EDDI_TRUE;
	}

	if (si[1] == 1 && (si[0] == 2 || si[0] == 0)) {
		/* 256 pens are DOOM's 256 colours; fewer planes cannot show
		 * them, on either route. */
		if (nplanes != 8) {
			snprintf(why, sizeof(why), "CLUT with %d planes", nplanes);
			return eddi_refuse(why);
		}
		for (i = 0; i < NCOLORS; i++) {
			devpix[i] = (uint16_t)si[16 + i];
			if (devpix[i] > 0xffu) {
				snprintf(why, sizeof(why), "pen %d has pixel value 0x%lx",
					 i, (unsigned long)devpix[i]);
				return eddi_refuse(why);
			}
		}
		cmp_mask = 0xffu;
		return si[0] == 2 ? EDDI_PACKED8 : EDDI_PLANES;
	}

	snprintf(why, sizeof(why), "format %d with clut %d is not one this file "
		 "builds", si[0], si[1]);
	return eddi_refuse(why);
}

/* DOOM-independent test colours for the check: every channel runs through
 * all 256 levels, each in a different order, so a swapped channel or a
 * misplaced bit cannot pass by accident. */
static void test_rgb(int i, int c8[3])
{
	c8[0] = i;
	c8[1] = (i * 3) & 0xff;
	c8[2] = (i * 11 + 128) & 0xff;
}

/* One pen to an 8-bit RGB colour; the VDI takes per-mille intensities. */
static void set_pen(int i, int r, int g, int b)
{
	short rgb[3];

	rgb[0] = (short)(((long)r * 1000L + 127L) / 255L);
	rgb[1] = (short)(((long)g * 1000L + 127L) / 255L);
	rgb[2] = (short)(((long)b * 1000L + 127L) / 255L);
	vs_color(vh, (short)i, rgb);
}

/*
 * The description against the driver (THE EdDI ROUTE): 256 pens painted one
 * pixel each inside the picture rectangle, read back as device pixels. With
 * tables, every channel must decode to the test colour within one step; with
 * a CLUT, every pen's pixel must be the listed one. Returns 1 if it holds.
 */
static int eddi_check(void)
{
	static uint32_t got[NCOLORS];
	short pxy[4];
	int c8[3], worst[3] = { 0, 0, 0 };
	int i, c, d, exact = 0;

	for (i = 0; i < NCOLORS; i++) {
		test_rgb(i, c8);
		set_pen(i, c8[0], c8[1], c8[2]);
		vsf_color(vh, (short)i);
		pxy[0] = (short)(out_x + i);
		pxy[1] = (short)out_y;
		pxy[2] = pxy[0];
		pxy[3] = pxy[1];
		vr_recfl(vh, pxy);
	}
	read_row_dev(out_x, out_y, NCOLORS, got);

	if (eddi != EDDI_TRUE) {
		for (i = 0; i < NCOLORS; i++)
			if ((got[i] & cmp_mask) != devpix[i]) {
				diag("DOOM: EdDI check: pen %d reads 0x%lx, the "
				       "description lists 0x%lx\n", i,
				       (unsigned long)got[i], (unsigned long)devpix[i]);
				return 0;
			}
		diag("DOOM: EdDI check: 256 pens read back as listed\n");
		return 1;
	}

	for (i = 0; i < NCOLORS; i++) {
		int same = 1;

		test_rgb(i, c8);
		for (c = 0; c < 3; c++) {
			d = (int)decode_ch(got[i], c) - (int)quant(c8[c], ch_n[c]);
			if (d < 0)
				d = -d;
			if (d > worst[c])
				worst[c] = d;
			if (d)
				same = 0;
		}
		exact += same;
	}
	diag("DOOM: EdDI check: %d of 256 test colours exact, largest step "
	       "off r/g/b %d/%d/%d\n", exact, worst[0], worst[1], worst[2]);
	if (worst[0] > 1 || worst[1] > 1 || worst[2] > 1) {
		for (i = 0; i < NCOLORS; i++) {
			test_rgb(i, c8);
			if (encode_rgb(c8[0], c8[1], c8[2]) != (got[i] & cmp_mask)) {
				diag("DOOM: EdDI check: first difference, pen %d "
				       "rgb %d/%d/%d: device 0x%lx, built 0x%lx\n",
				       i, c8[0], c8[1], c8[2], (unsigned long)got[i],
				       (unsigned long)encode_rgb(c8[0], c8[1], c8[2]));
				break;
			}
		}
		return 0;
	}
	return 1;
}

/* DOOM's palette for the EdDI route: with tables the pixels are computed,
 * with a CLUT the pens are loaded and the listed pixel values stay. */
static void eddi_palette(void)
{
	int i;

	if (eddi == EDDI_TRUE) {
		for (i = 0; i < NCOLORS; i++)
			devpix[i] = encode_rgb(colors[i].r, colors[i].g, colors[i].b);
	} else {
		load_pens();
	}
}

/* The whole 320x200 picture into the device form (EdDI route). */
static void build_device_form(void)
{
	const uint8_t *src = (const uint8_t *)DG_ScreenBuffer;
	uint8_t *row = (uint8_t *)dev_buf;
	int y, x;

	for (y = 0; y < PIC_H; y++, src += PIC_W, row += dev_stride) {
		if (eddi == EDDI_PLANES) {
			/* Interleaved: per group one word per plane
			 * (c2p_prepare(devpix) ran in DG_Init). */
			c2p_row(src, PIC_W, (uint16_t *)row, 1, nplanes);
		} else if (dev_bpp == 2) {
			uint16_t *dst = (uint16_t *)row;

			for (x = 0; x < PIC_W; x++)
				dst[x] = (uint16_t)devpix[src[x]];
		} else if (dev_bpp == 1) {
			for (x = 0; x < PIC_W; x++)
				row[x] = (uint8_t)devpix[src[x]];
		} else {
			for (x = 0; x < PIC_W; x++)
				put_packed(row, x, devpix[src[x]]);
		}
	}
}

/* ======================================================================== */
/* The one-time verification                                                 */
/* ======================================================================== */

/* The EdDI route's frame, read back as device pixels: every pixel of every
 * output row equals the pixel built for it (in the bits this file defines). */
static void verify_frame_eddi(void)
{
	static uint32_t got[PIC_W * MAX_STAGE];
	const uint8_t *src = (const uint8_t *)DG_ScreenBuffer;
	int y, x;

	for (y = 0; y < out_h; y++) {
		const uint8_t *line = src + ((long)y * PIC_H / out_h) * PIC_W;

		read_row_dev(out_x, out_y + y, out_w, got);
		for (x = 0; x < out_w; x++)
			if ((got[x] & cmp_mask) != (devpix[line[x / stage]] & cmp_mask))
				I_Error("DOOM: the EdDI picture did not read back "
					"from the screen (stage %d, row %d, column %d: "
					"0x%lx, meant 0x%lx) -- start with -noeddi",
					stage, y, x, (unsigned long)got[x],
					(unsigned long)devpix[line[x / stage]]);
	}
	diag("DOOM: picture %dx%d at (%d,%d) VERIFIED via EdDI%s+vro_cpyfm\n",
	       out_w, out_h, out_x, out_y, scaled ? "+scale" : "");
	need_clear = 1;
}

static void verify_frame(void)
{
	static uint32_t got[PIC_W * MAX_STAGE];	/* out_w <= PIC_W * MAX_STAGE */
	const uint8_t *src = (const uint8_t *)DG_ScreenBuffer;
	int y, x;

	for (y = 0; y < out_h; y++) {
		/* The same row mapping as stage_scale (identity if unscaled). */
		const uint8_t *line = src + ((long)y * PIC_H / out_h) * PIC_W;

		read_row(out_x, out_y + y, out_w, got);
		for (x = 0; x < out_w; x++)
			if (got[x] != pixval[line[x / stage]]) {
				I_Error("DOOM: the picture did not read back from the "
					"screen (%dx%d, row %d, column %d: 0x%lx, "
					"meant 0x%lx) -- this VDI's standard form is not "
					"the one doomgeneric_gem.c builds%s", out_w, out_h,
					y, x, (unsigned long)got[x],
					(unsigned long)pixval[line[x / stage]],
					scaled ? ", or its device form is not stored row "
					"by row / pixel-packed as SCALING assumes" : "");
			}
	}
	diag("DOOM: picture %dx%d at (%d,%d) VERIFIED via vr_trnfm%s+vro_cpyfm\n",
	       out_w, out_h, out_x, out_y, scaled ? "+scale" : "");
	/* The console line may have scrolled the screen, picture included. */
	need_clear = 1;
}

/* ======================================================================== */
/* Opening and closing                                                       */
/* ======================================================================== */

static void gem_close(void)
{
	int i;

	if (vh > 0 && pens_saved) {
		for (i = 0; i < NCOLORS; i++)
			vs_color(vh, (short)i, saved_pens[i]);
		pens_saved = 0;
	}
	if (ap_id >= 0) {
		if (screen_taken) {
			graf_mouse(M_ON, NULL);
			form_dial(FMD_FINISH, 0, 0, 0, 0, 0, 0,
				  (short)scr_w, (short)scr_h);
			wind_update(END_MCTRL);
			wind_update(END_UPDATE);
			screen_taken = 0;
		}
		if (vh > 0) {
			v_clsvwk(vh);
			vh = 0;
		}
		appl_exit();
		ap_id = -1;
	}
	if (std_raw) { Mfree(std_raw); std_raw = NULL; }
	if (dev_raw) { Mfree(dev_raw); dev_raw = NULL; }
	if (big_raw) { Mfree(big_raw); big_raw = NULL; }
	if (peek_dev_raw) { Mfree(peek_dev_raw); peek_dev_raw = NULL; }
	if (peek_std_raw) { Mfree(peek_std_raw); peek_std_raw = NULL; }
}

/* An open that cannot finish gives back what it took, then says why. */
static void open_fail(const char *why)
{
	gem_close();
	I_Error("DOOM: %s", why);
}

static void key_init(void);

/*
 * The stage and the picture's place on the screen (SCALING), from want_stage
 * (0 = the largest that fits), the route and the screen. Horizontal stages
 * above 1 only in a packed form: more than 8 planes, or EdDI packed 8-bit.
 */
static void choose_stage(void)
{
	int hmax = (nplanes > 8 || eddi == EDDI_PACKED8) ? MAX_STAGE : 1;
	int want = want_stage;

	if (want > hmax) {
		diag("DOOM: -scale %d: %d planes, not a packed form -- each "
		       "DOOM pixel one device pixel wide (stage 1)\n", want,
		       nplanes);
		want = hmax;
	}
	stage = want == 0 ? hmax : want;
	while (stage > 1 && !stage_fits(stage))
		stage--;
	if (want != 0 && stage != want)
		diag("DOOM: stage %d (%dx%d) does not fit the %dx%d screen, "
		       "stage %d\n", want, stage_w(want), stage_h(want),
		       scr_w, scr_h, stage);
	out_w = stage_w(stage);
	out_h = stage_h(stage);
	if (out_h > scr_h) {
		/* Stage 1 only: nowhere to step down to, so fit the rows. */
		diag("DOOM: %d rows for 4:3 do not fit the %d-row screen, %d "
		       "rows\n", out_h, scr_h, scr_h);
		out_h = scr_h;
	}
	scaled = out_w != PIC_W || out_h != PIC_H;
	out_x = (scr_w - out_w) / 2;
	out_y = (scr_h - out_h) / 2;
}

/* The 320x200 device form, the scaled device form and the one-row read-back
 * device form (as wide as the output), for the stage just chosen; the
 * standard forms of the vr_trnfm route only when that route runs
 * (alloc_trnfm_forms). Called again when the stage changes. */
static void alloc_picture_forms(void)
{
	if (dev_raw) { Mfree(dev_raw); dev_raw = NULL; }
	if (big_raw) { Mfree(big_raw); big_raw = NULL; }
	if (peek_dev_raw) { Mfree(peek_dev_raw); peek_dev_raw = NULL; }

	dev_raw = alloc_fast(form_bytes_for(PIC_W, PIC_H));
	peek_dev_raw = alloc_fast(form_bytes_for(out_w, 1));
	if (!dev_raw || !peek_dev_raw)
		open_fail("no memory for the picture forms");
	if (scaled) {
		big_raw = alloc_fast(form_bytes_for(out_w, out_h));
		if (!big_raw)
			open_fail("no memory for the scaled picture");
	}
	/* GEMDOS blocks are at least word aligned, all a form needs. */
	dev_buf = dev_raw;
	big_buf = big_raw;
	peek_dev = peek_dev_raw;
	/* PIC_W and every out_w are multiples of 16, so a row is the
	 * picture's own width (form_row_bytes). */
	dev_stride = form_row_bytes(PIC_W);
	big_stride = form_row_bytes(out_w);
}

/* The vr_trnfm route's standard forms: the 320x200 picture and the one-row
 * read-back form. */
static void alloc_trnfm_forms(void)
{
	std_raw = alloc_fast(plane_words * nplanes * 2);
	peek_std_raw = alloc_fast(form_bytes_for(out_w, 1));
	if (!std_raw || !peek_std_raw)
		open_fail("no memory for the picture forms");
	std_buf = std_raw;
	peek_std = peek_std_raw;
}

void DG_Init(void)
{
	short cw, ch, bw, bh, pxy[4];
	int i, p, want;

	ap_id = appl_init();
	if (ap_id < 0)
		open_fail("appl_init failed");
	atexit(gem_close);

	vh = graf_handle(&cw, &ch, &bw, &bh);
	for (i = 0; i < 10; i++)
		work_in[i] = 1;
	work_in[10] = 2;			/* raster coordinates */
	v_opnvwk(work_in, &vh, work_out);
	if (vh <= 0)
		open_fail("v_opnvwk returned no handle");
	scr_w = work_out[0] + 1;
	scr_h = work_out[1] + 1;
	/* DEV_TAB[3] / [4], the pixel size in microns (SCALING); vq_extnd
	 * below overwrites work_out. A driver that reports none is taken to
	 * have square pixels. */
	pix_w = work_out[3];
	pix_h = work_out[4];
	if (pix_w <= 0 || pix_h <= 0) {
		diag("DOOM: the VDI reports pixel size %dx%d -- taking square "
		       "pixels\n", pix_w, pix_h);
		pix_w = pix_h = 1;
	}

	vq_extnd(vh, 1, work_out);
	nplanes = work_out[4];
	has_lut = work_out[5];
	if (nplanes < 1 || nplanes > MAX_PLANES)
		open_fail("the VDI reports a plane count this file cannot hold");

	dev_bpp = (nplanes + 7) / 8;
	{
		const uint16_t one = 1;

		host_big_endian = *(const uint8_t *)&one == 0;
	}
	eddi = eddi_query();
	if (scr_w < PIC_W || scr_h < PIC_H)
		open_fail("the screen is smaller than DOOM's 320x200 picture");

	/* The stage (SCALING): -scale N, else DOOM_GEM_SCALE; 0 or a value
	 * out of range = the largest that fits. */
	want = DOOM_GEM_SCALE;
	p = M_CheckParmWithArgs("-scale", 1);
	if (p > 0)
		want = atoi(myargv[p + 1]);
	if (want < 0 || want > MAX_STAGE) {
		diag("DOOM: -scale %d is not a stage (1..%d), taking the largest "
		       "that fits\n", want, MAX_STAGE);
		want = 0;
	}
	want_stage = want;
	choose_stage();
	alloc_picture_forms();
	if (eddi == EDDI_NONE)
		alloc_trnfm_forms();
	memset(&screen_mfdb, 0, sizeof(screen_mfdb));

	/* Keep the pens; the palette is about to overwrite all of them. */
	for (i = 0; i < NCOLORS; i++)
		(void)vq_color(vh, (short)i, 0, saved_pens[i]);
	pens_saved = 1;

	/* Take the screen. */
	wind_update(BEG_UPDATE);
	wind_update(BEG_MCTRL);
	form_dial(FMD_START, 0, 0, 0, 0, 0, 0, (short)scr_w, (short)scr_h);
	screen_taken = 1;
	graf_mouse(M_OFF, NULL);

	pxy[0] = 0;
	pxy[1] = 0;
	pxy[2] = (short)(scr_w - 1);
	pxy[3] = (short)(scr_h - 1);
	vs_clip(vh, 1, pxy);
	vswr_mode(vh, MD_REPLACE);
	vsf_interior(vh, FIS_SOLID);
	vsf_perimeter(vh, 0);

	/* The EdDI description against the driver, before it is used. Without
	 * it an 8-plane screen loses the packed form's horizontal stages, so
	 * the stage is chosen again for the vr_trnfm route. */
	if (eddi != EDDI_NONE && !eddi_check()) {
		diag("DOOM: EdDI description does not match the screen -- "
		       "vr_trnfm route\n");
		eddi = EDDI_NONE;
		choose_stage();
		alloc_picture_forms();
		alloc_trnfm_forms();
	}

	/* The interleaved route's conversion tables: its pixel values are the
	 * listed ones and never change. (The vr_trnfm route builds them from
	 * pixval[] whenever that is measured, measure_pixvals.) */
	if (eddi == EDDI_PLANES)
		c2p_prepare(devpix, nplanes);

	key_init();

	/* Strafe left/right on ',' and '.', vanilla DOOM's own defaults.
	 * doomgeneric's are KEY_STRAFE_L/R, codes no key produces; binding the
	 * characters instead (rather than translating them in map_key) keeps
	 * ',' and '.' typeable in a savegame name. DG_Init runs before
	 * D_DoomMain; its D_BindVariables / M_LoadDefaults only register the
	 * variable, and reading a config file is compiled out in this tree
	 * (config.h: ORIGCODE undefined, m_config.c LoadDefaultCollection), so
	 * nothing overwrites this. Were config loading ever on, a saved
	 * binding would win -- as a user's config should. */
	key_strafeleft = ',';
	key_straferight = '.';

	diag("DOOM: GEM platform up -- %dx%d, %d planes, lookup table %s, "
	       "pixel %dx%d um, stage %d = %dx%d, route %s\n", scr_w, scr_h,
	       nplanes, has_lut ? "yes" : "no", pix_w, pix_h, stage, out_w,
	       out_h, eddi == EDDI_NONE ? "vr_trnfm" : "EdDI");
}

/* The screen around the picture: black. Done before the first frame -- which
 * also wipes the startup log DOOM printed on the console -- and again after
 * any console line of ours, since the console scrolls the whole screen.
 * Pen 0 is borrowed for it; the caller reloads DOOM's palette right after. */
static void clear_screen(void)
{
	static short black[3] = { 0, 0, 0 };
	short pxy[4];

	vs_color(vh, 0, black);
	vsf_color(vh, 0);
	pxy[0] = 0;
	pxy[1] = 0;
	pxy[2] = (short)(scr_w - 1);
	pxy[3] = (short)(scr_h - 1);
	vr_recfl(vh, pxy);
}

/* The steps of a frame after the palette (c2p = build_standard_form, then
 * these), each a separate function so the cost probe below can time them one
 * by one. */
static void stage_trnfm(void)
{
	set_form(&std_mfdb, std_buf, PIC_W, PIC_H, 1);
	set_form(&dev_mfdb, dev_buf, PIC_W, PIC_H, 0);
	vr_trnfm(vh, &std_mfdb, &dev_mfdb);
}

static void stage_cpyfm(void)
{
	MFDB *from = &dev_mfdb;
	short pxy[8];

	/* The EdDI route has no vr_trnfm step that would describe it. */
	set_form(&dev_mfdb, dev_buf, PIC_W, PIC_H, 0);
	if (scaled) {
		set_form(&big_mfdb, big_buf, out_w, out_h, 0);
		from = &big_mfdb;
	}
	pxy[0] = 0;
	pxy[1] = 0;
	pxy[2] = (short)(out_w - 1);
	pxy[3] = (short)(out_h - 1);
	pxy[4] = (short)out_x;
	pxy[5] = (short)out_y;
	pxy[6] = (short)(out_x + out_w - 1);
	pxy[7] = (short)(out_y + out_h - 1);
	vro_cpyfm(vh, S_ONLY, pxy, from, &screen_mfdb);
}

#if DOOM_GEM_STATS > 0
/*
 * What each step of a frame costs, measured once (stats builds only): every
 * step is repeated on the frame just shown until it has run for at least a
 * second of _hz_200 time, so the 5 ms clock resolution is below 0.5 %. The
 * per-frame breakdown cannot be taken inside the game loop: a 5 ms clock
 * cannot split a 15 ms frame into its parts.
 */
static uint32_t probe_cost(void (*step)(void))
{
	uint32_t n = 0, t0 = ticks200(), t;

	do {
		step();
		n++;
		t = ticks200() - t0;
	} while (t < 200u);
	return (uint32_t)(((unsigned long)t * 5000UL) / n);	/* us per step */
}

static void cost_report(void)
{
	/* EdDI route: "build" is the device form, there is no vr_trnfm. */
	uint32_t c2p = probe_cost(eddi != EDDI_NONE ? build_device_form
						     : build_standard_form);
	uint32_t trn = eddi != EDDI_NONE ? 0 : probe_cost(stage_trnfm);
	uint32_t scl = scaled ? probe_cost(stage_scale) : 0;
	uint32_t cpy = probe_cost(stage_cpyfm);

	diag("DOOM: per frame, %s, stage %d %dx%d: %s %lu us, vr_trnfm %lu us, "
	       "scale %lu us, vro_cpyfm %lu us, sum %lu us\n",
	       eddi != EDDI_NONE ? "EdDI" : "vr_trnfm", stage, out_w, out_h,
	       eddi != EDDI_NONE ? "build" : "c2p",
	       (unsigned long)c2p, (unsigned long)trn, (unsigned long)scl,
	       (unsigned long)cpy, (unsigned long)(c2p + trn + scl + cpy));
	need_clear = 1;
}

static void frame_report(void)
{
	uint32_t now = ticks200();

	st_frames++;
	if (st_t0 == 0) {
		st_t0 = now;
		st_frames = 0;
		st_pal = 0;
	} else if (now - st_t0 >= (uint32_t)DOOM_GEM_STATS * 200u) {
		uint32_t span = now - st_t0;

		/* One console row, rewritten in place ("\r", no newline): a
		 * newline on the last row would scroll the picture. Kept under
		 * 80 columns, so it cannot wrap (and scroll) on a 640-pixel
		 * screen either. The frame time is the mean over the window. */
		diag("DOOM x%d%s: %lu frames/%lu ms = %lu.%02lu fps, %lu.%02lu ms/frame, "
		       "%lu pal\r", stage, eddi != EDDI_NONE ? " EdDI" : "",
		       (unsigned long)st_frames, (unsigned long)(span * 5u),
		       (unsigned long)(st_frames * 200u / span),
		       (unsigned long)((st_frames * 20000u / span) % 100u),
		       (unsigned long)(st_frames ? span * 5u / st_frames : 0u),
		       (unsigned long)(st_frames ? (span * 500u / st_frames) % 100u : 0u),
		       (unsigned long)st_pal);
		fflush(stdout);
		st_t0 = now;
		st_frames = 0;
		st_pal = 0;
	}
}
#endif

void DG_DrawFrame(void)
{
	if (need_clear) {
		clear_screen();
		need_clear = 0;
		palette_changed = true;
	}
	if (palette_changed) {
		update_palette();
		palette_changed = false;
#if DOOM_GEM_STATS > 0
		st_pal++;
#endif
	}

	if (eddi != EDDI_NONE) {
		build_device_form();
	} else {
		build_standard_form();
		stage_trnfm();
	}
	if (scaled)
		stage_scale();
	stage_cpyfm();

	if (first_frame) {
		first_frame = 0;
		if (eddi != EDDI_NONE)
			verify_frame_eddi();
		else
			verify_frame();
#if DOOM_GEM_STATS > 0
		cost_report();
#endif
	}
#if DOOM_GEM_STATS > 0
	frame_report();
#endif
}

/* ======================================================================== */
/* Keyboard                                                                  */
/* ======================================================================== */

#define KEYQ_SIZE	64
#define MAX_HELD	16

static uint16_t keyq[KEYQ_SIZE];	/* (pressed << 8) | doom key */
static unsigned keyq_r, keyq_w;
static int polled;			/* one poll per I_GetEvent pass */

#define KBD_AES		0		/* the fallback: AES + repeat heuristic */
#define KBD_VEC		1		/* the kbdvec hook */
static int kbd_route;

static void keyq_put(int pressed, unsigned char key)
{
	if (((keyq_w + 1) % KEYQ_SIZE) == keyq_r)
		return;				/* full: drop, as a hardware buffer does */
	keyq[keyq_w] = (uint16_t)((pressed ? 0x100 : 0) | key);
	keyq_w = (keyq_w + 1) % KEYQ_SIZE;
}

/* IKBD scancode (+ the character it stands for) to a DOOM key; 0 = not
 * used. The same table on both routes. */
static unsigned char map_key(int scan, int ascii)
{
	switch (scan) {
	case 0x01: return KEY_ESCAPE;
	case 0x0e: return KEY_BACKSPACE;
	case 0x0f: return KEY_TAB;
	case 0x1c: case 0x72: return KEY_ENTER;	/* Return, keypad Enter */
	case 0x39: return KEY_USE;		/* space */
	case 0x48: return KEY_UPARROW;
	case 0x50: return KEY_DOWNARROW;
	case 0x4b: return KEY_LEFTARROW;
	case 0x4d: return KEY_RIGHTARROW;
	case 0x3b: return KEY_F1;
	case 0x3c: return KEY_F2;
	case 0x3d: return KEY_F3;
	case 0x3e: return KEY_F4;
	case 0x3f: return KEY_F5;
	case 0x40: return KEY_F6;
	case 0x41: return KEY_F7;
	case 0x42: return KEY_F8;
	case 0x43: return KEY_F9;
	case 0x44: return KEY_F10;
	case 0x62: return KEY_F11;		/* Help: gamma (USB F11 = Help) */
	case 0x61: return KEY_PAUSE;		/* Undo: the Atari has no Pause */
	default: break;
	}
	/* Everything else by its character, so the keyboard's own layout
	 * decides which key is 'y'. DOOM wants lower case. */
	if (ascii >= 'A' && ascii <= 'Z')
		ascii += 'a' - 'A';
	/* DOOM wants '=' (screen size up, automap zoom in); on the German
	 * layout that is Shift+0, so '+' stands in for it on any layout. */
	if (ascii == '+')
		return KEY_EQUALS;
	if (ascii >= 0x20 && ascii < 0x7f)
		return (unsigned char)ascii;
	return 0;
}

/* ---- the kbdvec route ----------------------------------------------------- */

#define SC_CTRL		0x1d
#define SC_LSHIFT	0x2a
#define SC_RSHIFT	0x36
#define SC_ALT		0x38

static const unsigned char *kbd_unshift;	/* Keytbl: scancode -> char */
static unsigned char sc_down[128];		/* make seen, break not yet */
static kbdslot_t *kbd_slot_want;		/* Supexec arguments ... */
static unsigned kbd_tos_version;		/* ... and results */
static int kbd_emutos;
static int kbd_installed;

/* A 32-bit OS-header cell at any (even) address, in the CPU's own order:
 * the header is the CPU's own data on both machines, and 0x4f2 is not
 * 4-byte aligned. */
static uint32_t rd32(uintptr_t a)
{
	uint32_t v;

	memcpy(&v, (const void *)a, sizeof(v));
	return v;
}

/*
 * Supexec: the OS version, the way FreeMiNT reads it (sys/init.c:386-401):
 * _sysbase (0x4f2) -> OSHEADER, its os_beg (+0x08) -> the ROM's header,
 * os_version (+0x02), p_rsv2 (+0x2c) == 'ETOS' on EmuTOS. On a 680x0 the
 * low-memory read needs the supervisor mode; on NewTOS the sysvar page is
 * the program's own and Supexec is a plain call (mintlib osbind.h Supexec).
 * Offsets, not mintlib's OSHEADER: that struct has pointer members, which
 * are 8 bytes wide on LP64.
 */
static long kbd_probe(void)
{
	uintptr_t h = (uintptr_t)rd32(0x4f2);

	h = (uintptr_t)rd32(h + 0x08);
	kbd_tos_version = *(const volatile uint16_t *)(h + 0x02);
	kbd_emutos = rd32(h + 0x2c) == 0x45544f53UL;	/* 'ETOS' */
	return 0;
}

/* Supexec: put the hook in. Nothing else writes the slot, and each access
 * is one store, so no interrupt can see half a pointer. On a 680x0 the
 * etv_term handler goes in first, so the hook is never in without it. */
static long kbd_install(void)
{
	kbdslot_t old = *kbd_slot_want;

	if (old == 0)
		return -1;			/* no vector to chain to: unusable */
	kbdvec_old = old;
	kbdvec_r = kbdvec_w = 0;
	kbdvec_slot = kbd_slot_want;
#ifdef __m68k__
	kbdvec_etv_old = *(volatile kbdslot_t *)0x408;
	*(volatile kbdslot_t *)0x408 = (kbdslot_t)kbdvec_etv_term;
#endif
	*kbd_slot_want = (kbdslot_t)kbdvec_hook;
	return 0;
}

/* Supexec: take the hook out -- only if the slot still holds it; a program
 * that hooked in after DOOM chains to DOOM and would be cut off. Then
 * etv_term likewise. */
static long kbd_remove(void)
{
	kbdslot_t *slot = kbdvec_slot;

	if (slot && *slot == (kbdslot_t)kbdvec_hook)
		*slot = kbdvec_old;
	kbdvec_slot = 0;
#ifdef __m68k__
	if (*(volatile kbdslot_t *)0x408 == (kbdslot_t)kbdvec_etv_term)
		*(volatile kbdslot_t *)0x408 = kbdvec_etv_old;
#endif
	return 0;
}

static void kbd_close(void)
{
	if (kbd_installed) {
		(void)Supexec(kbd_remove);
		kbd_installed = 0;
	}
}

/* The kbdvec route if this TOS has the vector (see INPUT), else 0. */
static int kbd_open(void)
{
	kbdslot_t *table;
	_KEYTAB *kt;

	if (M_CheckParm("-nokbdvec") > 0) {
		diag("DOOM: -nokbdvec -- keys via the AES (repeat heuristic)\n");
		return 0;
	}
	(void)Supexec(kbd_probe);
	if (kbd_tos_version < 0x200 && !kbd_emutos) {
		diag("DOOM: TOS %x.%02x has no kbdvec (needs 2.00) -- keys via "
		       "the AES (repeat heuristic)\n", kbd_tos_version >> 8,
		       kbd_tos_version & 0xff);
		return 0;
	}
	kt = (_KEYTAB *)Keytbl(-1L, -1L, -1L);
	table = (kbdslot_t *)Kbdvbase();
	if (kt == NULL || kt->unshift == NULL || table == NULL) {
		diag("DOOM: no Keytbl/Kbdvbase -- keys via the AES (repeat "
		       "heuristic)\n");
		return 0;
	}
	kbd_unshift = (const unsigned char *)kt->unshift;
	kbd_slot_want = table - 1;		/* kbdvec: the slot before */
	if (Supexec(kbd_install) != 0) {
		diag("DOOM: Kbdvbase %08lx: kbdvec slot is 0 -- keys via the "
		       "AES (repeat heuristic)\n", (unsigned long)table);
		return 0;
	}
	kbd_installed = 1;
	atexit(kbd_close);
	diag("DOOM: keys via kbdvec -- TOS %x.%02x%s, Kbdvbase %08lx, "
	       "kbdvec was %08lx\n", kbd_tos_version >> 8,
	       kbd_tos_version & 0xff, kbd_emutos ? " (EmuTOS)" : "",
	       (unsigned long)table, (unsigned long)kbdvec_old);
	return 1;
}

/* SHIFT, CONTROL, ALTERNATE: DOOM's run, fire and strafe. Either shift key
 * is "shift"; the DOOM key is down while either is. */
static int mod_key(int scan, unsigned char *key, int *down)
{
	switch (scan) {
	case SC_LSHIFT:
	case SC_RSHIFT:
		*key = KEY_RSHIFT;
		*down = sc_down[SC_LSHIFT] || sc_down[SC_RSHIFT];
		return 1;
	case SC_CTRL:
		*key = KEY_FIRE;
		*down = sc_down[SC_CTRL];
		return 1;
	case SC_ALT:
		*key = KEY_LALT;
		*down = sc_down[SC_ALT];
		return 1;
	default:
		return 0;
	}
}

static void vec_byte(unsigned char b)
{
	int scan = b & 0x7f, make = !(b & 0x80), was, now;
	unsigned char k;

	/* A make of a key that is down, or a break of one that is up (held
	 * before DOOM started), changes nothing. */
	if ((sc_down[scan] != 0) == make)
		return;
	if (mod_key(scan, &k, &was)) {
		sc_down[scan] = (unsigned char)make;
		(void)mod_key(scan, &k, &now);
		if (now != was)
			keyq_put(now, k);
		return;
	}
	sc_down[scan] = (unsigned char)make;
	k = map_key(scan, kbd_unshift[scan]);
	if (k != 0)
		keyq_put(make, k);
}

static void vec_poll(void)
{
	short msg[8], mx, my, mb, ks, kc, br;
	int ev, guard = 32;

	/* Read the AES keyboard events the chained vector produced, and drop
	 * them. On NewTOS this call is also where the hook runs: a pending
	 * kbdvec upcall is delivered when a syscall returns (22-kbdvbase.md
	 * C3). */
	do {
		ev = evnt_multi(MU_KEYBD | MU_TIMER, 0, 0, 0,
				0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
				msg, 0UL, &mx, &my, &mb, &ks, &kc, &br);
	} while ((ev & MU_KEYBD) && --guard > 0);

	while (kbdvec_r != kbdvec_w) {
		unsigned char r = kbdvec_r;
		unsigned char b = kbdvec_ring[r];		/* the byte first */

		kbdvec_r = (unsigned char)((r + 1) & (KBDVEC_RING - 1));	/* then free it */
		vec_byte(b);
	}
}

/* ---- the AES route (fallback) --------------------------------------------- */

static struct {
	unsigned char key;
	uint32_t last;			/* ms of the last press/repeat */
	int repeated;
} held[MAX_HELD];
static int nheld;

static short mods;			/* last kstate seen */
static uint32_t hold_first_ms, hold_repeat_ms;

static void aes_key_init(void)
{
	long rate = (long)Kbrate(-1, -1);	/* query: (delay << 8) | repeat */
	int delay = (int)((rate >> 8) & 0xff);
	int repeat = (int)(rate & 0xff);

	/* Kbrate counts in 50 Hz ticks (20 ms). A rate of zero would mean no
	 * repeat at all; take the TOS defaults then. The margin covers the
	 * time between two of our polls (one game tic, 29 ms) twice over. */
	if (delay <= 0)
		delay = 15;
	if (repeat <= 0)
		repeat = 2;
	hold_first_ms = (uint32_t)delay * 20u + 60u;
	hold_repeat_ms = (uint32_t)repeat * 20u + 60u;
}

static void mod_edge(short now, short mask, unsigned char key)
{
	if ((now & mask) && !(mods & mask))
		keyq_put(1, key);
	else if (!(now & mask) && (mods & mask))
		keyq_put(0, key);
}

static void aes_poll(void)
{
	short msg[8], mx, my, mb, ks = 0, kc, br;
	uint32_t now;
	int ev, i, guard = 32;

	do {
		ev = evnt_multi(MU_KEYBD | MU_TIMER, 0, 0, 0,
				0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
				msg, 0UL, &mx, &my, &mb, &ks, &kc, &br);
		now = DG_GetTicksMs();
		if (ev & MU_KEYBD) {
			unsigned char k = map_key((kc >> 8) & 0xff, kc & 0xff);

			if (k == 0)
				continue;
			for (i = 0; i < nheld; i++)
				if (held[i].key == k)
					break;
			if (i < nheld) {		/* auto-repeat */
				held[i].last = now;
				held[i].repeated = 1;
			} else {
				keyq_put(1, k);
				if (nheld < MAX_HELD) {
					held[nheld].key = k;
					held[nheld].last = now;
					held[nheld].repeated = 0;
					nheld++;
				} else {
					keyq_put(0, k);
				}
			}
		}
	} while ((ev & MU_KEYBD) && --guard > 0);

	/* Modifiers are true held state. */
	mod_edge(ks, K_RSHIFT | K_LSHIFT, KEY_RSHIFT);
	mod_edge(ks, K_CTRL, KEY_FIRE);
	mod_edge(ks, K_ALT, KEY_LALT);
	mods = ks;

	/* An ordinary key is released when its repeat stops arriving. */
	now = DG_GetTicksMs();
	for (i = 0; i < nheld; ) {
		uint32_t limit = held[i].repeated ? hold_repeat_ms : hold_first_ms;

		if (now - held[i].last > limit) {
			keyq_put(0, held[i].key);
			held[i] = held[--nheld];
		} else {
			i++;
		}
	}
}

/* ---- both ------------------------------------------------------------------ */

static void key_init(void)
{
	kbd_route = kbd_open() ? KBD_VEC : KBD_AES;
	if (kbd_route == KBD_AES)
		aes_key_init();
}

static void key_poll(void)
{
	if (kbd_route == KBD_VEC)
		vec_poll();
	else
		aes_poll();
}

int DG_GetKey(int *pressed, unsigned char *key)
{
	if (keyq_r == keyq_w) {
		if (polled) {
			polled = 0;		/* next I_GetEvent polls again */
			return 0;
		}
		polled = 1;
		key_poll();
		if (keyq_r == keyq_w) {
			polled = 0;
			return 0;
		}
	}
	*pressed = (keyq[keyq_r] >> 8) & 1;
	*key = (unsigned char)(keyq[keyq_r] & 0xff);
	keyq_r = (keyq_r + 1) % KEYQ_SIZE;
	return 1;
}

/* There is no window to put a title on -- DOOM owns the whole screen -- so
 * the title goes where a singletasking TOS program's messages go: the
 * console, which DOOM's startup log is printed on as well. */
void DG_SetWindowTitle(const char *title)
{
	diag("DOOM: %s\n", title);
}

/* ======================================================================== */
/* main                                                                      */
/* ======================================================================== */

int main(int argc, char **argv)
{
	static char *extra[] = { DOOM_GEM_ARGS };
	static char *args[64];
	int n = 0, i;

	/* The built-in arguments (the zone size, see MEMORY) are appended to
	 * every start -- a desktop or #Z start, which passes no command tail,
	 * and a start with one (e.g. "-scale 2" typed into the parameter box).
	 * They go AFTER the given ones: DOOM takes the first occurrence of an
	 * option (m_argv.c M_CheckParmWithArgs), so a "-mb" typed at the start
	 * wins over the built-in one. */
	args[n++] = (argc > 0 && argv[0]) ? argv[0] : "DOOM";
	for (i = 1; i < argc && n < 63; i++)
		args[n++] = argv[i];
	for (i = 0; i < (int)(sizeof(extra) / sizeof(extra[0])) && n < 63; i++)
		args[n++] = extra[i];
	args[n] = NULL;
	argc = n;
	argv = args;
#if DOOM_GEM_STATS > 0
	/* The file only: the arguments the engine gets, the zone size (-mb)
	 * among them -- the engine's own "zone memory" line goes to the
	 * console alone (i_system.c, not this file). */
	{
		char line[256];
		size_t len = 0;

		line[0] = '\0';
		for (i = 0; i < argc && len + 2 < sizeof(line); i++)
			len += (size_t)snprintf(line + len, sizeof(line) - len,
						"%s%s", i ? " " : "", argv[i]);
		stat_file("DOOM: arguments: %s\n", line);
	}
#endif

	doomgeneric_Create(argc, argv);
	for (;;)
		doomgeneric_Tick();
	return 0;
}

#endif /* DOOM_GEM_C2P_HOST_TEST */
