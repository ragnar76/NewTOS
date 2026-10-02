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
 * is doomgeneric's business, i_swap.h; this file never looks at the bytes of
 * a multi-byte value.)
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
 * This is the slower of the two routes PORT06 names; the EdDI route (compute
 * the device format directly when the driver publishes it) is a later step.
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
 * which makes the AES redraw the desktop. The picture is DOOM's 320x200,
 * centred, optionally zoomed by an integer factor (-scale N, default 1); it is
 * deliberately not stretched to the full screen.
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
 * standard and device forms, a few hundred KiB -- come from GEMDOS
 * Mxalloc(MX_PREFTTRAM), Malloc as fallback: CPU-only memory, no DMA reason to
 * want ST-RAM (PORT06 "ST-RAM ist auch auf dem TT kein Thema"), and outside
 * the TPA the zone has to share.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <gem.h>
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

#define PIC_W		DOOMGENERIC_RESX	/* 320 */
#define PIC_H		DOOMGENERIC_RESY	/* 200 */
#define NCOLORS		256
#define MAX_PLANES	32			/* pixval is 32 bits wide */

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
static int zoom = 1;
static int out_w, out_h;		/* PIC_W*zoom, PIC_H*zoom */
static int out_x, out_y;		/* top-left on the screen */
static long wdw;			/* fd_wdwidth of the output forms */
static long plane_words;		/* out_h * wdw */
static void *std_raw, *dev_raw;
static uint16_t *std_buf;		/* standard form, nplanes planes */
static uint16_t *dev_buf;		/* device form, same size */
static MFDB std_mfdb, dev_mfdb, screen_mfdb;	/* screen: fd_addr == 0 */
static uint8_t *zoom_row;		/* one zoomed index row (zoom > 1) */

/* ---- read-back (one row, any width up to out_w) ---------------------------- */
static void *peek_dev_raw, *peek_std_raw;
static uint16_t *peek_dev, *peek_std;
static MFDB peek_dev_mfdb, peek_std_mfdb;

/* ---- the colour table ----------------------------------------------------- */
static uint32_t pixval[NCOLORS];	/* bit p = plane p for that colour */
static int pixval_valid;
static short saved_pens[NCOLORS][3];
static int pens_saved;

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

static void update_palette(void)
{
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

/* The whole picture, zoomed, into the standard form. */
static void build_standard_form(void)
{
	const uint8_t *src = (const uint8_t *)DG_ScreenBuffer;
	long y, r;
	int x, z, p;

	for (y = 0; y < PIC_H; y++, src += PIC_W) {
		long row = y * zoom;
		const uint8_t *line = src;

		if (zoom > 1) {
			for (x = 0; x < PIC_W; x++)
				for (z = 0; z < zoom; z++)
					zoom_row[x * zoom + z] = src[x];
			line = zoom_row;
		}
		c2p_row(line, out_w, row);
		/* The other zoom-1 rows are copies, plane by plane. */
		for (r = 1; r < zoom; r++)
			for (p = 0; p < nplanes; p++)
				memcpy(std_buf + p * plane_words + (row + r) * wdw,
				       std_buf + p * plane_words + row * wdw,
				       (size_t)wdw * 2);
	}
}

/* ======================================================================== */
/* The one-time verification                                                 */
/* ======================================================================== */

static void verify_frame(void)
{
	static uint32_t got[PIC_W * 8];	/* out_w <= PIC_W * 8, see DG_Init */
	const uint8_t *src = (const uint8_t *)DG_ScreenBuffer;
	int y, x;

	for (y = 0; y < out_h; y++) {
		const uint8_t *line = src + (y / zoom) * PIC_W;

		read_row(out_x, out_y + y, out_w, got);
		for (x = 0; x < out_w; x++)
			if (got[x] != pixval[line[x / zoom]]) {
				I_Error("DOOM: the picture did not read back from the "
					"screen (row %d, column %d: 0x%lx, meant 0x%lx) -- "
					"this VDI's standard form is not the one "
					"doomgeneric_gem.c builds", y, x,
					(unsigned long)got[x],
					(unsigned long)pixval[line[x / zoom]]);
			}
	}
	printf("DOOM: picture %dx%d at (%d,%d) VERIFIED via vr_trnfm+vro_cpyfm\n",
	       out_w, out_h, out_x, out_y);
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
	if (peek_dev_raw) { Mfree(peek_dev_raw); peek_dev_raw = NULL; }
	if (peek_std_raw) { Mfree(peek_std_raw); peek_std_raw = NULL; }
	free(zoom_row);
	zoom_row = NULL;
}

/* An open that cannot finish gives back what it took, then says why. */
static void open_fail(const char *why)
{
	gem_close();
	I_Error("DOOM: %s", why);
}

static void key_init(void);

void DG_Init(void)
{
	short cw, ch, bw, bh, pxy[4];
	long form_bytes, peek_bytes;
	int i, p;

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

	/* Zoom: -scale N, else 1; never larger than the screen. */
	p = M_CheckParmWithArgs("-scale", 1);
	if (p > 0)
		zoom = atoi(myargv[p + 1]);
	if (zoom < 1)
		zoom = 1;
	if (zoom > 8)
		zoom = 8;
	while (zoom > 1 && (PIC_W * zoom > scr_w || PIC_H * zoom > scr_h))
		zoom--;
	if (PIC_W > scr_w || PIC_H > scr_h)
		open_fail("the screen is smaller than DOOM's 320x200 picture");
	out_w = PIC_W * zoom;
	out_h = PIC_H * zoom;
	out_x = (scr_w - out_w) / 2;
	out_y = (scr_h - out_h) / 2;
	wdw = (out_w + 15) / 16;
	plane_words = (long)out_h * wdw;

	/* The two full forms and the two one-row read-back forms. */
	form_bytes = plane_words * nplanes * 2;
	peek_bytes = wdw * nplanes * 2;
	std_raw = alloc_fast(form_bytes);
	dev_raw = alloc_fast(form_bytes);
	peek_dev_raw = alloc_fast(peek_bytes);
	peek_std_raw = alloc_fast(peek_bytes);
	if (!std_raw || !dev_raw || !peek_dev_raw || !peek_std_raw)
		open_fail("no memory for the picture forms");
	/* GEMDOS blocks are at least word aligned, all a form needs. */
	std_buf = std_raw;
	dev_buf = dev_raw;
	peek_dev = peek_dev_raw;
	peek_std = peek_std_raw;
	if (zoom > 1) {
		zoom_row = malloc((size_t)out_w);
		if (!zoom_row)
			open_fail("no memory for the zoom row");
	}
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

	key_init();

	printf("DOOM: GEM platform up -- %dx%d, %d planes, lookup table %s, "
	       "zoom %d\n", scr_w, scr_h, nplanes, has_lut ? "yes" : "no", zoom);
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

/* The three steps of a frame after the palette, each a separate function so
 * the cost probe below can time them one by one. */
static void stage_trnfm(void)
{
	set_form(&std_mfdb, std_buf, out_w, out_h, 1);
	set_form(&dev_mfdb, dev_buf, out_w, out_h, 0);
	vr_trnfm(vh, &std_mfdb, &dev_mfdb);
}

static void stage_cpyfm(void)
{
	short pxy[8];

	pxy[0] = 0;
	pxy[1] = 0;
	pxy[2] = (short)(out_w - 1);
	pxy[3] = (short)(out_h - 1);
	pxy[4] = (short)out_x;
	pxy[5] = (short)out_y;
	pxy[6] = (short)(out_x + out_w - 1);
	pxy[7] = (short)(out_y + out_h - 1);
	vro_cpyfm(vh, S_ONLY, pxy, &dev_mfdb, &screen_mfdb);
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
	uint32_t c2p = probe_cost(build_standard_form);
	uint32_t trn = probe_cost(stage_trnfm);
	uint32_t cpy = probe_cost(stage_cpyfm);

	printf("DOOM: per frame %dx%d: c2p %lu us, vr_trnfm %lu us, "
	       "vro_cpyfm %lu us\n", out_w, out_h, (unsigned long)c2p,
	       (unsigned long)trn, (unsigned long)cpy);
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
		 * newline on the last row would scroll the picture. */
		printf("DOOM: %lu frames/%lu ms = %lu.%02lu fps, %lu palette changes\r",
		       (unsigned long)st_frames, (unsigned long)(span * 5u),
		       (unsigned long)(st_frames * 200u / span),
		       (unsigned long)((st_frames * 20000u / span) % 100u),
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

	build_standard_form();
	stage_trnfm();
	stage_cpyfm();

	if (first_frame) {
		first_frame = 0;
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

	/* A desktop or #Z start passes no command tail; the zone size the
	 * engine would ask for by default does not fit this machine's TPA
	 * (see MEMORY), so such a start gets the built-in arguments. A start
	 * with arguments is taken as it is. */
	if (argc <= 1) {
		args[n++] = (argc > 0 && argv[0]) ? argv[0] : "DOOM";
		for (i = 0; i < (int)(sizeof(extra) / sizeof(extra[0])) && n < 63; i++)
			args[n++] = extra[i];
		args[n] = NULL;
		argc = n;
		argv = args;
	}

	doomgeneric_Create(argc, argv);
	for (;;)
		doomgeneric_Tick();
	return 0;
}
