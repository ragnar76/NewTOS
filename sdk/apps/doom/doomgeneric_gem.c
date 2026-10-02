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
 * screen; it is deliberately not stretched to the full screen.
 *
 *
 * SCALING (TASK/PORT06, user decision 2026-10-02 "Praezisiert")
 * -------------------------------------------------------------
 * Four stages, chosen with the start parameter -scale N:
 *
 *     -scale 1    320x200   DOOM's picture as it is, no aspect correction
 *     -scale 2    640x480   } horizontally N times, vertically 1.2 * N times
 *     -scale 3    960x720   } (200 rows -> 240 * N by repeating rows), which
 *     -scale 4   1280x960   } restores the 4:3 shape a 320x200 picture had on
 *                           } a CRT
 *     -scale 0    the largest stage that fits the screen (also what an
 *                 out-of-range N means)
 *
 * Without the parameter the stage is DOOM_GEM_SCALE (one constant, below).
 * A stage larger than the screen steps down to the largest one that fits.
 *
 * The picture is scaled AFTER vr_trnfm (on the EdDI route: after the device
 * form is built), in the device form, by repeating
 * whole pixels and whole rows: c2p and vr_trnfm keep working on 320x200, and
 * the copying stays out of the standard form, where one pixel is spread over
 * N planes. That needs the device form's layout, and only one kind of layout
 * is known without asking the driver: the pixel-packed truecolour form, one
 * pixel after the other, every pixel (planes + 7) / 8 bytes -- "interleaved
 * (for 1-8 planes) or pixel-packed (Falcon Truecolor mode)", EmuTOS
 * vdi/vdi_raster.c:264-267 and vr_trnfm16 (:190). So scaling is done only
 * with more than 8 planes. With 1-8 planes (TT, ST, a NOVA palette mode) the
 * device form is interleaved bit planes or chunky, and this file does not
 * guess which: it stays at stage 1. The first-frame read-back (below) checks
 * the scaled picture pixel by pixel, so a driver whose truecolour form is not
 * packed is refused by name, not drawn wrong. Copying a pixel copies its
 * bytes; nothing here looks at what the bytes mean, so byte order does not
 * matter.
 *
 *
 * INPUT
 * -----
 * The AES: evnt_multi(MU_KEYBD | MU_TIMER) with a zero timer, which is a
 * poll. The modifier state comes with every event (kstate), so SHIFT (run),
 * CONTROL (fire) and ALTERNATE (strafe) are true held keys. Ordinary keys
 * deliver presses only -- TOS has no key-up event outside the IKBD packet
 * vector -- so a key counts as held while its keyboard auto-repeat keeps
 * arriving and is released when the repeat stops; the timeouts come from the
 * keyboard's own repeat rate (XBIOS Kbrate). See key_poll().
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
 * -mb N says otherwise. NewTOS' libc runs malloc inside the 4 MiB TPA
 * (sdk/libc/newtos-arch/heap_init.c), so the 6 MiB default cannot be met
 * there; a start without a command tail therefore gets DOOM_GEM_ARGS
 * appended (build-doom.sh, default "-mb 2"). Measured 2026-10-02 (PORT06):
 * the image ends at ~0xcd400, which leaves ~3211 KiB of malloc arena below
 * the 64 KiB stack reserve; doomgeneric_Create() takes 250 KiB of it for
 * DG_ScreenBuffer before the zone is asked for, so 3 MiB (3072 KiB) does not
 * fit and 2 MiB does, with ~900 KiB left for the WAD directory and stdio.
 * -mb takes whole MiB, so 2 is the largest zone this TPA can give.
 * This file's own buffers -- the
 * device form of the 320x200 picture and, on the vr_trnfm route only, its
 * standard form (125 KiB each at 16 planes) and, when scaling,the scaled device form (out_w * out_h * bytes
 * per pixel: 2400 KiB for 1280x960 at 16 planes) -- come from GEMDOS
 * Mxalloc(MX_PREFTTRAM), Malloc as fallback: CPU-only memory, no DMA reason to
 * want ST-RAM (PORT06 "ST-RAM ist auch auf dem TT kein Thema"), and outside
 * the TPA the zone has to share.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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

#ifndef CMAP256
#error "doomgeneric_gem.c serves the CMAP256 build only (8-bit palette picture)"
#endif

#ifndef MX_PREFTTRAM
#define MX_PREFTTRAM	3	/* alternate RAM preferred, ST-RAM if none */
#endif

/* Extra arguments for a start without a command tail; build-doom.sh sets it. */
#ifndef DOOM_GEM_ARGS
#define DOOM_GEM_ARGS "-mb", "2"
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
#define MAX_PLANES	32			/* pixval is 32 bits wide */
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

/* ---- the picture ---------------------------------------------------------- */
static int stage = 1;			/* 1..MAX_STAGE, see SCALING */
static int out_w, out_h;		/* stage_w(stage), stage_h(stage) */
static int out_x, out_y;		/* top-left on the screen */
static const long wdw = (PIC_W + 15) / 16;	/* fd_wdwidth, 320x200 forms */
static const long plane_words = (long)PIC_H * ((PIC_W + 15) / 16);
static int dev_bpp;			/* bytes per pixel, packed form (> 8 planes) */
static long dev_stride;			/* bytes per row, 320x200 device form */
static long big_stride;			/* bytes per row, scaled device form */
static void *std_raw, *dev_raw, *big_raw;
static uint16_t *std_buf;		/* standard form, 320x200, nplanes planes */
static uint16_t *dev_buf;		/* device form, 320x200 */
static void *big_buf;			/* device form, out_w x out_h (stage > 1) */
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
 * Bytes a device form of w x h pixels needs. With 1-8 planes that is the
 * planar size the VDI documents (h * fd_wdwidth words per plane). With more
 * planes the form is pixel-packed (SCALING), dev_bpp bytes per pixel and
 * fd_wdwidth * 16 pixels per row; for 16/24/32 planes the two sizes are the
 * same, for a 15-plane form the packed one is larger. The larger of the two
 * is taken, so neither reading can run past the end.
 */
static long form_bytes_for(int w, int h)
{
	long wd = (w + 15) / 16;
	long planar = (long)h * wd * 2 * nplanes;
	long packed = (long)h * wd * 16 * dev_bpp;

	return (nplanes > 8 && packed > planar) ? packed : planar;
}

/* The stage geometry (SCALING). */
static int stage_w(int s)
{
	return PIC_W * s;
}

static int stage_h(int s)
{
	return s == 1 ? PIC_H : PIC_H * 6 * s / 5;	/* x 1.2 * s */
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
/* Chunky to planar                                                          */
/* ======================================================================== */

/*
 * Convert `n` (a multiple of 16) indices into the standard form's row `row`:
 * for each 16-pixel group, plane p's word collects bit p of the 16 pixel
 * values, leftmost pixel in bit 15.
 */
static void c2p_row(const uint8_t *src, int n, long row)
{
	uint16_t *dst = std_buf + row * wdw;
	uint32_t m[16];
	int g, k, p;

	for (g = 0; g < n / 16; g++) {
		for (k = 0; k < 16; k++)
			m[k] = pixval[src[k]];
		src += 16;
		for (p = 0; p < nplanes; p++) {
			uint32_t w = 0;

			for (k = 0; k < 16; k++)
				w = (w << 1) | ((m[k] >> p) & 1u);
			dst[p * plane_words + g] = (uint16_t)w;
		}
	}
}

/* The whole 320x200 picture into the standard form. */
static void build_standard_form(void)
{
	const uint8_t *src = (const uint8_t *)DG_ScreenBuffer;
	long y;

	for (y = 0; y < PIC_H; y++, src += PIC_W)
		c2p_row(src, PIC_W, y);
}

/* ======================================================================== */
/* Scaling in the packed device form (stage > 1, more than 8 planes)         */
/* ======================================================================== */

/* One 320-pixel device row, every pixel `stage` times. A pixel is dev_bpp
 * bytes and is copied as bytes (two 16-bit halves for 2 and 4); the buffers
 * are GEMDOS blocks and every row starts at an even offset, so 16-bit access
 * is aligned on any CPU. */
static void scale_row(const void *srow, void *drow)
{
	int x, k;

	if (dev_bpp == 2) {
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
 * source row y * 200 / out_h, so each source row appears 1.2 * stage times
 * (2 or 3 times at stage 2); a repeated row is a copy of the one above. */
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
	printf("DOOM: EdDI description not used (%s) -- vr_trnfm route\n", why);
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
		printf("DOOM: -noeddi -- vr_trnfm route\n");
		return EDDI_NONE;
	}
	if (Getcookie(C_EdDI, &cookie) != C_FOUND) {
		printf("DOOM: no EdDI cookie -- vr_trnfm route\n");
		return EDDI_NONE;
	}
	/* Present is all that counts; the value is never called. */
	memset(si, 0, sizeof(si));
	vq_scrninfo(vh, si);
	printf("DOOM: EdDI: format %d, clut %d, %d planes, bits r/g/b %d/%d/%d, "
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
				printf("DOOM: EdDI check: pen %d reads 0x%lx, the "
				       "description lists 0x%lx\n", i,
				       (unsigned long)got[i], (unsigned long)devpix[i]);
				return 0;
			}
		printf("DOOM: EdDI check: 256 pens read back as listed\n");
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
	printf("DOOM: EdDI check: %d of 256 test colours exact, largest step "
	       "off r/g/b %d/%d/%d\n", exact, worst[0], worst[1], worst[2]);
	if (worst[0] > 1 || worst[1] > 1 || worst[2] > 1) {
		for (i = 0; i < NCOLORS; i++) {
			test_rgb(i, c8);
			if (encode_rgb(c8[0], c8[1], c8[2]) != (got[i] & cmp_mask)) {
				printf("DOOM: EdDI check: first difference, pen %d "
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
	uint32_t m[16];
	int y, x, g, k, p;

	for (y = 0; y < PIC_H; y++, src += PIC_W, row += dev_stride) {
		if (eddi == EDDI_PLANES) {
			uint16_t *dst = (uint16_t *)row;
			const uint8_t *s = src;

			for (g = 0; g < PIC_W / 16; g++, s += 16) {
				for (k = 0; k < 16; k++)
					m[k] = devpix[s[k]];
				for (p = 0; p < nplanes; p++) {
					uint32_t w = 0;

					for (k = 0; k < 16; k++)
						w = (w << 1) | ((m[k] >> p) & 1u);
					*dst++ = (uint16_t)w;
				}
			}
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
	printf("DOOM: picture %dx%d at (%d,%d) VERIFIED via EdDI%s+vro_cpyfm\n",
	       out_w, out_h, out_x, out_y, stage > 1 ? "+scale" : "");
	need_clear = 1;
}

static void verify_frame(void)
{
	static uint32_t got[PIC_W * MAX_STAGE];	/* out_w <= PIC_W * MAX_STAGE */
	const uint8_t *src = (const uint8_t *)DG_ScreenBuffer;
	int y, x;

	for (y = 0; y < out_h; y++) {
		/* The same row mapping as stage_scale (identity at stage 1). */
		const uint8_t *line = src + ((long)y * PIC_H / out_h) * PIC_W;

		read_row(out_x, out_y + y, out_w, got);
		for (x = 0; x < out_w; x++)
			if (got[x] != pixval[line[x / stage]]) {
				I_Error("DOOM: the picture did not read back from the "
					"screen (stage %d, row %d, column %d: 0x%lx, "
					"meant 0x%lx) -- this VDI's standard form is not "
					"the one doomgeneric_gem.c builds%s", stage, y, x,
					(unsigned long)got[x],
					(unsigned long)pixval[line[x / stage]],
					stage > 1 ? ", or its truecolour device form is "
					"not pixel-packed (start with -scale 1)" : "");
			}
	}
	printf("DOOM: picture %dx%d at (%d,%d) VERIFIED via vr_trnfm%s+vro_cpyfm\n",
	       out_w, out_h, out_x, out_y, stage > 1 ? "+scale" : "");
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
	long dev_bytes, peek_bytes;
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
	if (!stage_fits(1))
		open_fail("the screen is smaller than DOOM's 320x200 picture");

	/* The stage (SCALING): -scale N, else DOOM_GEM_SCALE; 0 or a value
	 * out of range = the largest that fits. */
	want = DOOM_GEM_SCALE;
	p = M_CheckParmWithArgs("-scale", 1);
	if (p > 0)
		want = atoi(myargv[p + 1]);
	if (want < 0 || want > MAX_STAGE) {
		printf("DOOM: -scale %d is not a stage (1..%d), taking the largest "
		       "that fits\n", want, MAX_STAGE);
		want = 0;
	}
	if (nplanes <= 8) {
		/* Not a packed truecolour device form: no scaling. */
		if (want != 1)
			printf("DOOM: %d planes -- the device form is not packed "
			       "truecolour, no scaling (stage 1)\n", nplanes);
		stage = 1;
	} else {
		stage = want == 0 ? MAX_STAGE : want;
		while (stage > 1 && !stage_fits(stage))
			stage--;
		if (want != 0 && stage != want)
			printf("DOOM: stage %d (%dx%d) does not fit the %dx%d screen, "
			       "stage %d\n", want, stage_w(want), stage_h(want),
			       scr_w, scr_h, stage);
	}
	out_w = stage_w(stage);
	out_h = stage_h(stage);
	out_x = (scr_w - out_w) / 2;
	out_y = (scr_h - out_h) / 2;

	/* The 320x200 device form, the scaled device form and the one-row
	 * read-back device form (as wide as the output); the standard forms of
	 * the vr_trnfm route only when that route runs (alloc_trnfm_forms). */
	dev_bytes = form_bytes_for(PIC_W, PIC_H);
	peek_bytes = form_bytes_for(out_w, 1);
	dev_raw = alloc_fast(dev_bytes);
	peek_dev_raw = alloc_fast(peek_bytes);
	if (!dev_raw || !peek_dev_raw)
		open_fail("no memory for the picture forms");
	if (stage > 1) {
		big_raw = alloc_fast(form_bytes_for(out_w, out_h));
		if (!big_raw)
			open_fail("no memory for the scaled picture (try -scale 1)");
	}
	/* GEMDOS blocks are at least word aligned, all a form needs. */
	dev_buf = dev_raw;
	big_buf = big_raw;
	peek_dev = peek_dev_raw;
	/* Packed rows are fd_wdwidth * 16 pixels; PIC_W and every out_w are
	 * multiples of 16, so that is the picture's own width. Interleaved
	 * rows (EDDI_PLANES) are fd_wdwidth groups of nplanes words. */
	if (eddi == EDDI_PLANES)
		dev_stride = (long)wdw * nplanes * 2;
	else
		dev_stride = (long)wdw * 16 * dev_bpp;
	big_stride = (long)((out_w + 15) / 16) * 16 * dev_bpp;
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

	/* The EdDI description against the driver, before it is used. */
	if (eddi != EDDI_NONE && !eddi_check()) {
		printf("DOOM: EdDI description does not match the screen -- "
		       "vr_trnfm route\n");
		eddi = EDDI_NONE;
		alloc_trnfm_forms();
	}

	key_init();

	printf("DOOM: GEM platform up -- %dx%d, %d planes, lookup table %s, "
	       "stage %d = %dx%d, route %s\n", scr_w, scr_h, nplanes,
	       has_lut ? "yes" : "no", stage, out_w, out_h,
	       eddi == EDDI_NONE ? "vr_trnfm" : "EdDI");
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
	if (stage > 1) {
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
	uint32_t scl = stage > 1 ? probe_cost(stage_scale) : 0;
	uint32_t cpy = probe_cost(stage_cpyfm);

	printf("DOOM: per frame, %s, stage %d %dx%d: %s %lu us, vr_trnfm %lu us, "
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
		printf("DOOM x%d%s: %lu frames/%lu ms = %lu.%02lu fps, %lu.%02lu ms/frame, "
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
	if (stage > 1)
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

static struct {
	unsigned char key;
	uint32_t last;			/* ms of the last press/repeat */
	int repeated;
} held[MAX_HELD];
static int nheld;

static short mods;			/* last kstate seen */
static uint32_t hold_first_ms, hold_repeat_ms;
static int polled;			/* one AES poll per I_GetEvent pass */

static void key_init(void)
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

static void keyq_put(int pressed, unsigned char key)
{
	if (((keyq_w + 1) % KEYQ_SIZE) == keyq_r)
		return;				/* full: drop, as a hardware buffer does */
	keyq[keyq_w] = (uint16_t)((pressed ? 0x100 : 0) | key);
	keyq_w = (keyq_w + 1) % KEYQ_SIZE;
}

/* AES key word (scancode << 8 | ASCII) to a DOOM key; 0 = not used. */
static unsigned char map_key(short kc)
{
	int scan = (kc >> 8) & 0xff;
	int ascii = kc & 0xff;

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
	default: break;
	}
	/* Everything else by its character, so the keyboard's own layout
	 * decides which key is 'y'. DOOM wants lower case. */
	if (ascii >= 'A' && ascii <= 'Z')
		ascii += 'a' - 'A';
	if (ascii >= 0x20 && ascii < 0x7f)
		return (unsigned char)ascii;
	return 0;
}

static void mod_edge(short now, short mask, unsigned char key)
{
	if ((now & mask) && !(mods & mask))
		keyq_put(1, key);
	else if (!(now & mask) && (mods & mask))
		keyq_put(0, key);
}

static void key_poll(void)
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
			unsigned char k = map_key(kc);

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
	printf("DOOM: %s\n", title);
}

/* ======================================================================== */
/* main                                                                      */
/* ======================================================================== */

int main(int argc, char **argv)
{
	static char *extra[] = { DOOM_GEM_ARGS };
	static char *args[64];
	int n = 0, i;

	/* The zone size the engine would ask for by default does not fit this
	 * machine's TPA (see MEMORY), so the built-in arguments are appended to
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

	doomgeneric_Create(argc, argv);
	for (;;)
		doomgeneric_Tick();
	return 0;
}
