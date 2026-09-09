/*
 * plat-tt.c -- the Atari TT / NOVA half of the bouncing-ball demo.
 *
 * The counterpart of plat-newtos.c: the same six functions of ball.h, for a
 * TT030 with a NOVA (ATI mach64) graphics card running at 640x480 in 256
 * colours. ball.c is compiled unchanged for both machines; this file is the
 * only one that knows what a pixel is here.
 *
 *
 * THE ROUTE, AND WHY
 * ------------------
 * The frame is composed into a bitmap in TT-RAM and put on screen with one
 * vro_cpyfm() per frame -- a memory MFDB as the source, the screen (fd_addr
 * == 0) as the destination.
 *
 * The alternative -- work out the card's framebuffer address and store into
 * it, the way plat-newtos.c does -- is not available here and would not be
 * faster if it were:
 *
 *   * It is not available. On a card-equipped TT the VDI's screen is NOT at
 *     Physbase(): Physbase() still reports the TT's own ST/TT video memory.
 *     The card's aperture lives in VME space and no standard TOS call hands
 *     an application its address. Inventing one is exactly what CLAUDE.md
 *     rule 6 forbids.
 *
 *   * It would not be faster. The mach64's blitter reads its operands out of
 *     the card's own VRAM; the card is a VME/ISA *slave* and cannot read TT
 *     memory at all. So a memory->screen raster copy is a CPU loop over the
 *     VME bus no matter who writes it -- the driver's loop and ours would
 *     move the same bytes over the same 16-bit bus. (fVDI's mach64 driver
 *     states the same division of labour: it hooks the blit only when source
 *     AND destination are on the visible screen, "everything else -- anything
 *     involving main memory -- falls through to the CPU"; see
 *     /Volumes/Daten/Development/fVDI-MACH64/fvdi/fvdi/drivers/mach64/
 *     mach64_blit.c. Read for understanding only; no code is taken from it.)
 *
 * So the VDI route costs what any route costs and needs no address the driver
 * does not publish. The user's hope that "the VDI is free because the NOVA is
 * accelerated" does not hold for memory->screen: that direction is the one
 * the hardware cannot accelerate. What IS accelerated is screen->screen, and
 * exploiting that would need an off-screen bitmap in card VRAM (v_opnbm,
 * NVDI 3 / fVDI) -- a different design, and out of scope here.
 *
 *
 * WHAT THIS FILE REFUSES TO GUESS
 * -------------------------------
 * Three things decide whether the bytes we hand the VDI mean what we think,
 * and none of them can be read out of a document with certainty for THIS
 * driver. So none of them is assumed while the machine is willing to answer:
 * plat_open() MEASURES all three, the way plat-newtos.c measures the Pi's
 * screen pitch instead of computing it.
 *
 *   1. Is a device pixel one chunky byte? A 256-colour NOVA is packed 8-bit;
 *      native TT video at 256 colours is 8 interleaved bitplanes. The two are
 *      not interchangeable and an MFDB must be in the device's format.
 *
 *   2. What is fd_wdwidth for such a form? The field is defined for the
 *      planar model and drivers differ: stride = fd_wdwidth * 2 * fd_nplanes
 *      (fVDI's 8-bit driver, 8b_blit.c) or stride = fd_wdwidth * 2. Both are
 *      in circulation; the byte layout is identical, only the number differs.
 *
 *   3. Which VDI pen owns which device pixel value? On the Atari VDI the pen
 *      number is NOT the hardware index. We write device pixel values into
 *      the MFDB but set colours with vs_color(), which takes a pen. Get the
 *      mapping wrong and every colour in the picture is somebody else's.
 *
 * The measurements, both built on v_get_pixel() (VDI 105, "all VDI versions,
 * screen drivers only"), which reports the raw device pixel value at a point:
 *
 *   probe_pen_map()  fills one pixel per pen with vr_recfl() and reads back
 *                    the pixel value each pen produced -- the pen->pixel map,
 *                    measured. Inverted, it is what plat_palette() needs.
 *   probe_format()   stages a known chunky pattern, blits it with vro_cpyfm()
 *                    and reads back six points. Agreement proves 1 and 2 at
 *                    once: a planar device or the wrong stride cannot return
 *                    the bytes we put in.
 *
 * If v_get_pixel() will not answer, assume_format() -- ONE function, the only
 * place in this file that assumes anything -- supplies the NOVA defaults, and
 * plat_close() writes down that it had to. See BALL-TT-CONFIRM.md next to
 * this file for the full list of what still has to be confirmed on the
 * machine itself.
 *
 *
 * TT-RAM, AND WHY THE STAGING COPY IS NOT WASTE
 * ---------------------------------------------
 * The staging bitmap is Mxalloc'd with MX_PREFTTRAM: it is read by the CPU
 * and never by any DMA, so it belongs in fast RAM. The extra copy it costs is
 * the cheap half of the trip -- a TT-RAM to TT-RAM move against a VME
 * transfer measured at roughly 5 MB/s -- and it buys the one thing a
 * zero-copy source out of ball.c's own buffer cannot: a form whose width,
 * stride and alignment we chose and then verified.
 *
 * The rectangle is staged at x offset (dst_x & 1) so that source and
 * destination have the same byte parity. The TT's VME window to the NOVA is
 * 16 bits wide -- one VME cycle moves either one byte or two -- so a driver
 * that can use word moves halves the bus traffic, and it can only do that
 * when the parity matches. It costs nothing to hand it that chance.
 */
#include <gem.h>
#include <mint/osbind.h>
#include <mint/sysvars.h>
#include <string.h>

#include "ball.h"

/* This half of the seam is written for a PALETTE picture: it is handed
 * indices and it owns the colour table. Building it against a truecolour
 * core would compile and then draw nonsense, so say so here instead. */
#if BALL_TRUECOLOR
#error "plat-tt.c serves BALL_TRUECOLOR 0 only"
#endif

#ifndef MX_PREFTTRAM
#define MX_PREFTTRAM	3	/* TT-RAM preferred, ST-RAM if there is none */
#endif

/* Where plat_close() writes what the probes found. Deliberately the same file
 * ball.c logs its frame rate to (its LOGPATH), so one run leaves one
 * artefact; ball.c has created and truncated it long before we append. */
#define TT_REPORT_PATH	"C:\\BALL.LOG"

/* The demo needs one byte per palette index on the screen. */
#define TT_PLANES	8

/* probe_format()'s test patch, in pixels. 16 wide so that it is one whole
 * unit of every candidate stride convention, 4 tall so that a wrong stride
 * shows up in rows 1..3 while row 0 still matches. */
#define PROBE_W		16
#define PROBE_H		4

/* How long probe_vbl() waits for the vertical blank counter to move, in 200 Hz
 * ticks. 100 ms is five 50 Hz frames -- long enough that a running VBL cannot
 * be missed, short enough to be unnoticeable if there is none. */
#define VBL_WAIT_TICKS	20L

/* ---- AES / VDI ---------------------------------------------------------- */
static short ap_id = -1;
static short vdi_handle;
static short work_in[11];
static short work_out[57];
static int screen_locked;		/* wind_update/form_dial pair is open  */

/* ---- screen ------------------------------------------------------------- */
static int scr_w, scr_h;
static int have_vbl;			/* the VBL counter actually advances   */

/* ---- the staging bitmap and its two MFDBs ------------------------------- */
static void *stage_raw;			/* what Mxalloc gave, for Mfree()      */
static unsigned char *stage;		/* aligned start of the form	       */
static long stage_stride;		/* bytes per staged row		       */
static int stage_w;			/* form width in pixels (padded)       */
static MFDB stage_mfdb;
static MFDB screen_mfdb;		/* all zero: fd_addr == 0 is "screen"  */

/* ---- the colour mapping ------------------------------------------------- */
static short pen_of_pixel[BALL_PAL_N];	/* device pixel value -> VDI pen       */
static short saved_pal[BALL_PAL_N][3];	/* the CLUT as we found it	       */
static int pal_saved;

/* ---- what the probes decided, for the report ---------------------------- */
static int fmt_measured;		/* format + stride were measured       */
static int pen_measured;		/* the pen map was measured	       */
static int wdwidth_conv;		/* 1 = *2*nplanes, 2 = *2	       */

static const char *err = "";
static int opened;			/* plat_open() ran to the end	       */

const char *plat_error(void)
{
	return err;
}

/*
 * Give everything back and report why. ball.c does NOT call plat_close() when
 * plat_open() returns a failure -- it prints plat_error() and exits -- so a
 * half-finished open has to undo itself here, or the desktop is left with a
 * locked screen, a hidden mouse arrow and an AES slot that was never released.
 * plat_close() is written to be safe on a partly-built state and leaves `err`
 * alone.
 */
static int open_fail(int code, const char *why)
{
	err = why;
	plat_close();
	return code;
}

/* ------------------------------------------------------------------------ */
/* THE ONE ASSUMPTION                                                        */
/*                                                                           */
/* Reached only when v_get_pixel() does not give usable answers, i.e. when    */
/* the machine has refused to be measured. Everything it sets is what a       */
/* 256-colour NOVA is expected to be; none of it is verified here, and        */
/* plat_close() writes into the log that it was assumed rather than found.    */
/*                                                                           */
/*   packed pixels    one byte per pixel, chunky -- what an ATI mach64 in an  */
/*                    8-bit mode has. (A native TT screen at 256 colours is   */
/*                    8 interleaved planes instead, and on such a machine     */
/*                    this assumption is WRONG and the picture will be        */
/*                    scrambled. That is the visible symptom to look for.)    */
/*   fd_wdwidth       pixels / 16, i.e. stride = fd_wdwidth * 2 * fd_nplanes. */
/*                    The convention fVDI's 8-bit and 16-bit drivers use      */
/*                    uniformly (drivers/8_bit/8b_blit.c:290).                */
/*   pen map          identity: VDI pen p paints device pixel value p.        */
/* ------------------------------------------------------------------------ */
static void assume_format(void)
{
	int i;

	wdwidth_conv = 1;
	fmt_measured = 0;
	for (i = 0; i < BALL_PAL_N; i++)
		pen_of_pixel[i] = (short)i;
	pen_measured = 0;
}

/* Set stage_mfdb up for one of the two fd_wdwidth conventions in circulation.
 * The bytes in memory are the same either way; only the number differs, and
 * with it the stride the driver computes from it. */
static void set_wdwidth(int conv, int w_px, int h_px)
{
	stage_mfdb.fd_addr = stage;
	stage_mfdb.fd_w = (short)w_px;
	stage_mfdb.fd_h = (short)h_px;
	stage_mfdb.fd_stand = 0;		/* device-specific format      */
	stage_mfdb.fd_nplanes = TT_PLANES;
	stage_mfdb.fd_r1 = 0;
	stage_mfdb.fd_r2 = 0;
	stage_mfdb.fd_r3 = 0;
	/* conv 1: the driver reads stride = fd_wdwidth * 2 * fd_nplanes
	 * conv 2: the driver reads stride = fd_wdwidth * 2 */
	stage_mfdb.fd_wdwidth = (short)((conv == 1) ? (w_px / 16) : (w_px / 2));
}

/* ------------------------------------------------------------------------ */
/* Probe 1: which VDI pen owns which device pixel value                      */
/*                                                                           */
/* One solid 1x1 fill per pen, then v_get_pixel() at the same point: the      */
/* pixel value that came out IS what that pen paints. Inverting the result    */
/* gives what plat_palette() wants -- given a device pixel value the demo     */
/* will write, the pen whose colour has to be set.                           */
/*                                                                           */
/* Returns 1 when the 256 pens produced 256 different pixel values, i.e. when */
/* the map is a bijection and can be inverted. Anything less is not a usable  */
/* answer and is refused rather than patched up.                             */
/* ------------------------------------------------------------------------ */
static int probe_pen_map(void)
{
	short pixel_of_pen[BALL_PAL_N];
	short pxy[4], pel, cidx;
	int p, n = 0;

	vsf_interior(vdi_handle, FIS_SOLID);
	vsf_perimeter(vdi_handle, 0);

	for (p = 0; p < BALL_PAL_N; p++) {
		short x = (short)(p % scr_w);
		short y = (short)(p / scr_w);

		vsf_color(vdi_handle, (short)p);
		pxy[0] = x;
		pxy[1] = y;
		pxy[2] = x;
		pxy[3] = y;
		vr_recfl(vdi_handle, pxy);
	}
	for (p = 0; p < BALL_PAL_N; p++) {
		short x = (short)(p % scr_w);
		short y = (short)(p / scr_w);

		pel = -1;
		cidx = -1;
		v_get_pixel(vdi_handle, x, y, &pel, &cidx);
		pixel_of_pen[p] = pel;
	}

	for (p = 0; p < BALL_PAL_N; p++)
		pen_of_pixel[p] = -1;
	for (p = 0; p < BALL_PAL_N; p++) {
		short v = pixel_of_pen[p];

		if (v < 0 || v >= BALL_PAL_N)
			return 0;		/* not an 8-bit pixel value    */
		if (pen_of_pixel[v] >= 0)
			return 0;		/* two pens, one pixel value   */
		pen_of_pixel[v] = (short)p;
		n++;
	}
	return n == BALL_PAL_N;
}

/* ------------------------------------------------------------------------ */
/* Probe 2: is a device pixel a chunky byte, and what is fd_wdwidth          */
/*                                                                           */
/* Stage a patch whose value varies along BOTH axes, blit it to the top-left  */
/* corner with the very call plat_blit() will use, and read six points back.  */
/* Only a chunky device with the stride we claimed returns what we wrote: a   */
/* planar device scrambles the bits across eight words, and a wrong stride    */
/* fetches rows 1..3 from the wrong place while row 0 still matches -- which  */
/* is why the patch is four rows tall and the points are spread over them.    */
/* ------------------------------------------------------------------------ */
static int probe_format(int conv)
{
	static const short px[6] = { 0, 1, 15,  0,  7, 15 };
	static const short py[6] = { 0, 0,  0,  1,  2,  3 };
	short pxy[8], pel, cidx;
	long clear;
	int r, c, i;

	/* The form under test is the one plat_blit() will hand over, not a
	 * small stand-in: same width, same stride, same fd_wdwidth. Only the
	 * source rectangle is small. */
	set_wdwidth(conv, stage_w, scr_h);

	/* A wrong convention makes the driver fetch rows 1..3 from somewhere
	 * else in this buffer. Zero everything it could reach first, so that
	 * "somewhere else" can never accidentally hold a sentinel: every
	 * sentinel below is non-zero. The widest wrong stride a candidate can
	 * produce is 8x the right one. */
	clear = 8L * stage_stride * PROBE_H + PROBE_W;
	if (clear > stage_stride * (long)scr_h)
		clear = stage_stride * (long)scr_h;
	memset(stage, 0, (size_t)clear);

	for (r = 0; r < PROBE_H; r++)
		for (c = 0; c < PROBE_W; c++)
			stage[(long)r * stage_stride + c] =
				(unsigned char)(0x11 * (r + 1) + c);

	pxy[0] = 0;
	pxy[1] = 0;
	pxy[2] = PROBE_W - 1;
	pxy[3] = PROBE_H - 1;
	pxy[4] = 0;
	pxy[5] = 0;
	pxy[6] = PROBE_W - 1;
	pxy[7] = PROBE_H - 1;
	vro_cpyfm(vdi_handle, S_ONLY, pxy, &stage_mfdb, &screen_mfdb);

	for (i = 0; i < 6; i++) {
		short want = (short)(unsigned char)(0x11 * (py[i] + 1) + px[i]);

		pel = -1;
		cidx = -1;
		v_get_pixel(vdi_handle, px[i], py[i], &pel, &cidx);
		if (pel != want)
			return 0;
	}
	return 1;
}

/* Does the vertical blank counter move? Vsync() waits for it, so if the TT's
 * own video is switched off behind the card there is nothing to wait for and
 * Vsync() would never return. The demo must not hang, so this is asked once
 * and plat_present() acts on the answer. Bounded by the 200 Hz clock, which
 * is driven by the MFP timer and keeps running whatever the video does. */
static int probe_vbl(void)
{
	unsigned long t0 = (unsigned long)get_sysvar(_hz_200);
	unsigned long v0 = (unsigned long)get_sysvar(_vbclock);

	while ((unsigned long)get_sysvar(_hz_200) - t0 < (unsigned long)VBL_WAIT_TICKS)
		if ((unsigned long)get_sysvar(_vbclock) != v0)
			return 1;
	return 0;
}

int plat_open(ball_screen *scr)
{
	short cw, ch, bw, bh, ph;
	short pxy[4];
	long need;
	unsigned long a;
	int i;

	ap_id = appl_init();
	if (ap_id < 0)
		return open_fail(-1, "appl_init failed");

	ph = graf_handle(&cw, &ch, &bw, &bh);
	if (ph <= 0)
		return open_fail(-2,
			"graf_handle returned no physical workstation");

	/* The classic GEM application open: every attribute index 1, raster
	 * coordinates. */
	for (i = 0; i < 10; i++)
		work_in[i] = 1;
	work_in[10] = 2;
	vdi_handle = ph;
	v_opnvwk(work_in, &vdi_handle, work_out);
	if (vdi_handle <= 0)
		return open_fail(-3, "v_opnvwk returned no handle");

	scr_w = work_out[0] + 1;
	scr_h = work_out[1] + 1;

	/* Take the screen the way a GEM application is supposed to: lock it
	 * against the AES's own redraws, take the mouse, hide the arrow, and
	 * declare the whole screen ours so that form_dial() can hand it back
	 * with a redraw when we are done. */
	wind_update(BEG_UPDATE);
	wind_update(BEG_MCTRL);
	form_dial(FMD_START, 0, 0, 0, 0, 0, 0, (short)scr_w, (short)scr_h);
	screen_locked = 1;
	graf_mouse(M_OFF, 0L);

	/* The destination of every vro_cpyfm() is the screen and the VDI clips
	 * to the clipping rectangle whenever it is; make sure that is not a
	 * leftover from the AES. */
	pxy[0] = 0;
	pxy[1] = 0;
	pxy[2] = (short)(scr_w - 1);
	pxy[3] = (short)(scr_h - 1);
	vs_clip(vdi_handle, 0, pxy);

	/* One byte per palette index is the demo's whole model. */
	vq_extnd(vdi_handle, 1, work_out);
	if (work_out[4] != TT_PLANES)
		return open_fail(-4, "screen is not 256 colours (8 planes) -- "
				     "set the driver to 640x480 in 256 colours");

	/* The staging bitmap. One pixel wider than the screen before padding,
	 * so that a rectangle destined for an odd x can be staged at an odd
	 * offset and keep the card-side words aligned; then rounded up to 16
	 * pixels, which is the unit both fd_wdwidth conventions agree on. */
	stage_w = ((scr_w + 1 + 15) / 16) * 16;
	stage_stride = (long)stage_w;		/* 8 bpp packed: 1 byte/pixel  */
	need = stage_stride * (long)scr_h + 16L;
	stage_raw = (void *)Mxalloc(need, MX_PREFTTRAM);
	if (stage_raw == 0)
		stage_raw = (void *)Malloc(need);
	if (stage_raw == 0)
		return open_fail(-5, "no memory for the staging bitmap");
	a = (unsigned long)stage_raw;
	stage = (unsigned char *)((a + 15UL) & ~15UL);

	/* Ask the machine the three questions this file must not answer from
	 * memory. probe_pen_map() first: it also settles whether v_get_pixel()
	 * is worth believing at all, which probe_format() depends on. */
	if (probe_pen_map()) {
		pen_measured = 1;
		if (probe_format(1)) {
			wdwidth_conv = 1;
			fmt_measured = 1;
		} else if (probe_format(2)) {
			wdwidth_conv = 2;
			fmt_measured = 1;
		} else {
			return open_fail(-6,
				"the screen is not 8-bit packed pixels: a "
				"staged chunky patch did not read back -- "
				"see BALL-TT-CONFIRM.md");
		}
	} else {
		/* The machine will not be measured. One assumption, in one
		 * place, written into the log at the end of the run. */
		assume_format();
	}

	/* From here on the form is the full staging bitmap. */
	set_wdwidth(wdwidth_conv, stage_w, scr_h);

	/* Keep the CLUT so plat_close() can give the desktop its colours
	 * back; the demo is about to overwrite all 256 of them. */
	for (i = 0; i < BALL_PAL_N; i++)
		(void)vq_color(vdi_handle, (short)i, 0, saved_pal[i]);
	pal_saved = 1;

	have_vbl = probe_vbl();

	scr->w = scr_w;
	scr->h = scr_h;
	/* One page. Standard VDI publishes no way to give a card driver a
	 * different visible base, and this driver is believed to have no
	 * second page anyway; plat_present() is written so that adding one
	 * changes that function and nothing else. */
	scr->pages = 1;
	opened = 1;
	err = "";
	return 0;
}

void plat_palette(const ball_rgb *pal)
{
	int i;

	for (i = 0; i < BALL_PAL_N; i++) {
		short rgb[3];
		short pen = pen_of_pixel[i];

		if (pen < 0)
			continue;
		/* The VDI takes intensities in per mille, 0..1000. */
		rgb[0] = (short)(((long)pal[i].r * 1000L + 127L) / 255L);
		rgb[1] = (short)(((long)pal[i].g * 1000L + 127L) / 255L);
		rgb[2] = (short)(((long)pal[i].b * 1000L + 127L) / 255L);
		vs_color(vdi_handle, pen, rgb);
	}
}

void plat_blit(const ball_px *src, long src_pitch,
	       int x, int y, int w, int h)
{
	int par = x & 1;		/* keep the two sides the same parity  */
	unsigned char *dst = stage + par;
	short pxy[8];
	int row;

	for (row = 0; row < h; row++) {
		memcpy(dst, src, (size_t)w);
		src += src_pitch;
		dst += stage_stride;
	}

	pxy[0] = (short)par;
	pxy[1] = 0;
	pxy[2] = (short)(par + w - 1);
	pxy[3] = (short)(h - 1);
	pxy[4] = (short)x;
	pxy[5] = (short)y;
	pxy[6] = (short)(x + w - 1);
	pxy[7] = (short)(y + h - 1);
	vro_cpyfm(vdi_handle, S_ONLY, pxy, &stage_mfdb, &screen_mfdb);
}

void plat_present(void)
{
	/* One page: the picture is already where it is seen, so all that is
	 * left is not to tear it -- and not to hang if there is no retrace to
	 * wait for (probe_vbl()).
	 *
	 * A second page would be added here and nowhere else: show the page
	 * just drawn, point the staging blit's destination at the other one,
	 * and report pages = 2 from plat_open(). ball.c already keeps one
	 * dirty rectangle per page for exactly that case. */
	if (have_vbl)
		Vsync();
}

/* ------------------------------------------------------------------------ */
/* The closing report                                                        */
/*                                                                           */
/* One line into ball.c's own log saying what the probes decided, because     */
/* "the format was measured" and "the format was assumed" are the difference  */
/* between a result worth trusting and one worth checking, and nobody can     */
/* tell them apart by looking at the screen.                                  */
/* ------------------------------------------------------------------------ */
static char rbuf[160];
static int rlen;

static void r_str(const char *s)
{
	while (*s && rlen < (int)sizeof(rbuf) - 1)
		rbuf[rlen++] = *s++;
	rbuf[rlen] = '\0';
}

static void r_dec(long v)
{
	char t[12];
	int n = 0;

	if (v < 0) {
		r_str("-");
		v = -v;
	}
	do {
		t[n++] = (char)('0' + (int)(v % 10L));
		v /= 10L;
	} while (v > 0 && n < (int)sizeof(t));
	while (n > 0 && rlen < (int)sizeof(rbuf) - 1)
		rbuf[rlen++] = t[--n];
	rbuf[rlen] = '\0';
}

static void write_report(void)
{
	long fd;

	rlen = 0;
	rbuf[0] = '\0';
	r_str("BALL: TT/NOVA screen ");
	r_dec((long)scr_w);
	r_str("x");
	r_dec((long)scr_h);
	r_str("x8, pixel format ");
	r_str(fmt_measured ? "MEASURED packed-8, fd_wdwidth = pixels/"
			   : "ASSUMED packed-8, fd_wdwidth = pixels/");
	r_dec(wdwidth_conv == 1 ? 16L : 2L);
	r_str(", pen map ");
	r_str(pen_measured ? "MEASURED" : "ASSUMED identity");
	r_str(", VBL ");
	r_str(have_vbl ? "running (Vsync used)" : "STOPPED (no Vsync)");
	r_str("\r\n");

	fd = Fopen(TT_REPORT_PATH, 2);		/* read/write		     */
	if (fd < 0) {
		fd = Fcreate(TT_REPORT_PATH, 0);
		if (fd < 0)
			return;
	} else if (Fseek(0L, (short)fd, 2) < 0) {	/* SEEK_END	     */
		(void)Fclose((short)fd);
		return;
	}
	(void)Fwrite((short)fd, (long)rlen, rbuf);
	(void)Fclose((short)fd);
}

void plat_close(void)
{
	int i;

	if (vdi_handle > 0) {
		if (pal_saved) {
			for (i = 0; i < BALL_PAL_N; i++)
				vs_color(vdi_handle, (short)i, saved_pal[i]);
			pal_saved = 0;
		}
		if (opened)
			write_report();
	}
	if (ap_id >= 0) {
		graf_mouse(M_ON, 0L);
		if (screen_locked) {
			/* form_dial(FMD_FINISH) is how an application that
			 * took the screen asks the AES to draw the desktop
			 * back over it. */
			form_dial(FMD_FINISH, 0, 0, 0, 0,
				  0, 0, (short)scr_w, (short)scr_h);
			wind_update(END_MCTRL);
			wind_update(END_UPDATE);
			screen_locked = 0;
		}
		if (vdi_handle > 0) {
			v_clsvwk(vdi_handle);
			vdi_handle = 0;
		}
		appl_exit();
		ap_id = -1;
	}
	if (stage_raw != 0) {
		Mfree(stage_raw);
		stage_raw = 0;
		stage = 0;
	}
}
