/*
 * plat-pistorm.c -- the platform half of the bouncing-ball demo for an Atari
 * with a PiStorm and an ET4000 in a TRUECOLOUR mode, through the VDI.
 *
 * ===========================================================================
 * STATUS: WRITTEN, NEVER COMPILED, NEVER RUN.
 * ===========================================================================
 * The m68k cross toolchain was unavailable when this was written, so not one
 * line of it has been through a compiler, let alone a screen. Read it as a
 * proposal with its assumptions made explicit, not as working code. It is
 * modelled line for line on plat-tt.c, which IS proven on this machine in
 * 256 colours, so the delta between the two files is the whole of what is
 * new and is what wants reviewing.
 *
 * ---------------------------------------------------------------------------
 * The four things nobody here knows about the target
 * ---------------------------------------------------------------------------
 * None of these is guessed below. Each is either MEASURED at start-up or the
 * demo REFUSES TO RUN and says which one failed -- the same discipline
 * plat-tt.c applies to the 8-bit card, for the same reason: a wrong guess
 * about a pixel format does not crash, it draws a scrambled picture that
 * somebody then tries to explain.
 *
 *   1. Does the driver offer a truecolour mode at 640x480 at all, and at
 *      what depth? This file serves 32 bits per pixel ONLY. 16-bit is a
 *      different probe (bit fields, not bytes) and is refused rather than
 *      half-written -- if the card turns out to offer 16 and not 32, the
 *      refusal message says so and that is the answer we needed.
 *
 *   2. What does vq_extnd() report there? Only fd_nplanes == 32 is accepted.
 *
 *   3. Which byte of a device pixel is red, which green, which blue?
 *      Measured (probe_channels), because XRGB and BGRX are both in
 *      circulation and the VDI does not say which this is.
 *
 *   4. Which fd_wdwidth convention does vro_cpyfm() want at this depth?
 *      Measured (probe_stride) by trying both and keeping the one whose
 *      round trip reads back what was written.
 *
 * v_get_pixel() cannot help with any of it: it hands back a short, and a
 * 32-bit pixel does not fit in one. So the screen is read back the only way
 * that works at this depth -- vro_cpyfm() from the screen into a memory form
 * -- and, where the stride is itself what is in question, ONE PIXEL AT A
 * TIME, because a one-pixel rectangle has no stride to get wrong.
 *
 * ---------------------------------------------------------------------------
 * What it does NOT do
 * ---------------------------------------------------------------------------
 * There is no palette here and no plat_palette(): ball.c is built for this
 * file with BALL_TRUECOLOR set, composes 0x00RRGGBB, and the core's own tag
 * bits (see ball.c, "The composition pixel") live above the colour where the
 * card never sees them. Masking them off is this file's job and is one AND
 * per pixel in the staging loop -- the only per-pixel work added over the
 * 8-bit path, which memcpy'd.
 */
#include <gem.h>
#include <mint/osbind.h>
#include <mint/sysvars.h>
#include <string.h>

#include "ball.h"

/* The other half of the contract plat-tt.c states: this file is written for
 * an RGB picture and would draw nonsense if handed indices. */
#if !BALL_TRUECOLOR
#error "plat-pistorm.c serves BALL_TRUECOLOR 1 only"
#endif

#ifndef MX_PREFTTRAM
#define MX_PREFTTRAM	3	/* TT-RAM preferred, ST-RAM if there is none */
#endif

/* Same file ball.c logs to, appended at close, so one run leaves one
 * artefact. ball.c created and truncated it long before we get here. */
#define PS_REPORT_PATH	"C:\\BALL.LOG"

/* The one depth this file serves. See open question 1. */
#define PS_PLANES	32
#define PS_BYTES	4

/* probe_stride()'s test patch, in pixels. 16 wide so that it is one whole
 * unit of every candidate stride convention, 4 tall so that a wrong stride
 * shows in rows 1..3 while row 0 still matches. */
#define PROBE_W		16
#define PROBE_H		4

/* How long probe_vbl() waits for the vertical blank counter to move, in
 * 200 Hz ticks. 100 ms is five 50 Hz frames. */
#define VBL_WAIT_TICKS	20L

/* Pens borrowed for the colour probe. Any three will do; these are high
 * enough not to be the desktop's own first few. */
#define PROBE_PEN_R	13
#define PROBE_PEN_G	14
#define PROBE_PEN_B	15

/* ---- AES / VDI ---------------------------------------------------------- */
static short ap_id = -1;
static short vdi_handle;
static short work_in[11];
static short work_out[57];
static int screen_locked;

/* ---- screen ------------------------------------------------------------- */
static int scr_w, scr_h;
static int have_vbl;

/* ---- the staging bitmap and its two MFDBs ------------------------------- */
static void *stage_raw;
static unsigned char *stage;		/* aligned start of the form	     */
static long stage_stride;		/* bytes per staged row		     */
static int stage_w;			/* form width in pixels (padded)     */
static int blit_widen;			/* may plat_blit widen? see there      */
static MFDB stage_mfdb;
static MFDB screen_mfdb;		/* all zero: fd_addr == 0 is "screen" */

/* ---- one-pixel read-back form, for the probes --------------------------- */
static void *peek_raw;
static unsigned char *peek;
static MFDB peek_mfdb;

/* ---- what the probes decided -------------------------------------------- */
static int ch_r, ch_g, ch_b;		/* byte offset of each channel	     */
static int fast_xrgb;			/* channels are 0x00RRGGBB as we hold
					 * them, so staging is a mask only   */
static int wdwidth_conv;		/* 1 = *2*nplanes, 2 = *2	     */
static int fmt_measured;

static const char *err = "";
static int opened;

const char *plat_error(void)
{
	return err;
}

static int open_fail(int code, const char *why)
{
	err = why;
	plat_close();
	return code;
}

/* Set an MFDB up for one of the two fd_wdwidth conventions in circulation.
 * The bytes in memory are the same either way; only the number differs, and
 * with it the stride the driver computes from it. */
static void set_form(MFDB *m, void *addr, int w_px, int h_px, int conv)
{
	m->fd_addr = addr;
	m->fd_w = (short)w_px;
	m->fd_h = (short)h_px;
	m->fd_stand = 0;			/* device-specific format    */
	m->fd_nplanes = PS_PLANES;
	m->fd_r1 = 0;
	m->fd_r2 = 0;
	m->fd_r3 = 0;
	/* conv 1: the driver reads stride = fd_wdwidth * 2 * fd_nplanes
	 * conv 2: the driver reads stride = fd_wdwidth * 2 */
	m->fd_wdwidth = (short)((conv == 1) ? (w_px / 16)
					    : (w_px * PS_BYTES / 2));
}

/*
 * Read ONE screen pixel into peek[] and hand back its four bytes.
 *
 * A one-pixel rectangle is the only read-back whose correctness does not
 * depend on the stride convention being right, which is exactly what the
 * probes need: the destination form is one pixel wide and one row tall, so
 * there is no second row for a wrong stride to misplace.
 */
static void peek_pixel(int x, int y, int conv)
{
	short pxy[8];

	memset(peek, 0, (size_t)(PS_BYTES * 4));
	set_form(&peek_mfdb, peek, 16, 1, conv);	/* 16 px: one unit of
							 * either convention */
	pxy[0] = (short)x;
	pxy[1] = (short)y;
	pxy[2] = (short)x;
	pxy[3] = (short)y;
	pxy[4] = 0;
	pxy[5] = 0;
	pxy[6] = 0;
	pxy[7] = 0;
	vro_cpyfm(vdi_handle, S_ONLY, pxy, &screen_mfdb, &peek_mfdb);
}

/* ------------------------------------------------------------------------ */
/* Probe 1: which byte of a device pixel is which channel                    */
/*                                                                           */
/* Paint three saturated primaries through the VDI, read each back a pixel   */
/* at a time, and see which byte came out 0xFF. That names the layout        */
/* whatever it is -- XRGB, BGRX or anything else -- and it names it from the */
/* card rather than from a table of what cards usually do.                   */
/*                                                                           */
/* It fails, and the demo then refuses to run, if the driver does not honour */
/* vs_color() at this depth: the primaries would not read back as primaries  */
/* and no layout would be identified. That is a real possibility on a card   */
/* driver written for indexed modes, and it is one of the things the first   */
/* run on hardware is there to find out.                                     */
/* ------------------------------------------------------------------------ */
static int channel_byte(int x, int y, int conv)
{
	int i, found = -1;

	peek_pixel(x, y, conv);
	for (i = 0; i < PS_BYTES; i++) {
		if (peek[i] == 0xFF) {
			if (found >= 0)
				return -1;	/* two full channels: not a
						 * primary, so not usable    */
			found = i;
		} else if (peek[i] != 0x00) {
			return -1;		/* neither on nor off	     */
		}
	}
	return found;
}

static int probe_channels(int conv)
{
	static const short red[3]   = { 1000, 0, 0 };
	static const short green[3] = { 0, 1000, 0 };
	static const short blue[3]  = { 0, 0, 1000 };
	short pxy[4];

	vsf_interior(vdi_handle, FIS_SOLID);
	vsf_perimeter(vdi_handle, 0);

	vs_color(vdi_handle, PROBE_PEN_R, (short *)red);
	vs_color(vdi_handle, PROBE_PEN_G, (short *)green);
	vs_color(vdi_handle, PROBE_PEN_B, (short *)blue);

	pxy[0] = 0;
	pxy[1] = 0;
	pxy[2] = 0;
	pxy[3] = 0;
	vsf_color(vdi_handle, PROBE_PEN_R);
	vr_recfl(vdi_handle, pxy);
	pxy[0] = 1;
	pxy[2] = 1;
	vsf_color(vdi_handle, PROBE_PEN_G);
	vr_recfl(vdi_handle, pxy);
	pxy[0] = 2;
	pxy[2] = 2;
	vsf_color(vdi_handle, PROBE_PEN_B);
	vr_recfl(vdi_handle, pxy);

	ch_r = channel_byte(0, 0, conv);
	ch_g = channel_byte(1, 0, conv);
	ch_b = channel_byte(2, 0, conv);

	if (ch_r < 0 || ch_g < 0 || ch_b < 0)
		return 0;
	if (ch_r == ch_g || ch_g == ch_b || ch_r == ch_b)
		return 0;

	/* The composition pixel is held as 0x00RRGGBB. On a big-endian m68k
	 * that is byte 1 red, byte 2 green, byte 3 blue in memory, so when
	 * the card agrees, staging a row is a mask and a copy and nothing
	 * else. */
	fast_xrgb = (ch_r == 1 && ch_g == 2 && ch_b == 3);
	return 1;
}

/* ------------------------------------------------------------------------ */
/* Probe 2: which fd_wdwidth convention vro_cpyfm() wants here               */
/*                                                                           */
/* Stage a patch whose value varies along both axes, blit it to the top-left */
/* corner with the very call plat_blit() will use, then read six of its      */
/* pixels back ONE AT A TIME. Reading them singly is what keeps this honest: */
/* a stride that is wrong in the same way in both directions would cancel    */
/* out in a rectangle-to-rectangle round trip and prove nothing.             */
/* ------------------------------------------------------------------------ */
static int probe_stride(int conv)
{
	static const short px[6] = { 0, 1, 15,  0,  7, 15 };
	static const short py[6] = { 0, 0,  0,  1,  2,  3 };
	short pxy[8];
	long clear;
	int r, c, i;

	set_form(&stage_mfdb, stage, stage_w, scr_h, conv);

	/* A wrong convention makes the driver fetch rows 1..3 from somewhere
	 * else in this buffer. Zero everything it could reach first, so that
	 * "somewhere else" can never hold a sentinel by accident: every
	 * sentinel below is non-zero. */
	clear = 8L * stage_stride * PROBE_H + (long)PROBE_W * PS_BYTES;
	if (clear > stage_stride * (long)scr_h)
		clear = stage_stride * (long)scr_h;
	memset(stage, 0, (size_t)clear);

	for (r = 0; r < PROBE_H; r++)
		for (c = 0; c < PROBE_W; c++) {
			unsigned char *p = stage + (long)r * stage_stride +
					   (long)c * PS_BYTES;
			unsigned char v = (unsigned char)(0x11 * (r + 1) + c);

			p[0] = 0;
			p[ch_r] = v;
			p[ch_g] = (unsigned char)(v ^ 0x5A);
			p[ch_b] = (unsigned char)(v ^ 0xA5);
		}

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
		unsigned char v =
			(unsigned char)(0x11 * (py[i] + 1) + px[i]);

		peek_pixel(px[i], py[i], conv);
		if (peek[ch_r] != v ||
		    peek[ch_g] != (unsigned char)(v ^ 0x5A) ||
		    peek[ch_b] != (unsigned char)(v ^ 0xA5))
			return 0;
	}
	return 1;
}

/* Does the vertical blank counter move? Vsync() waits for it, so if the
 * machine's own video is switched off behind the card there is nothing to
 * wait for and Vsync() would never return. Asked once; plat_present() acts
 * on the answer. Bounded by the 200 Hz clock, which is driven by the MFP
 * timer and keeps running whatever the video does. */
static int probe_vbl(void)
{
	unsigned long t0 = (unsigned long)get_sysvar(_hz_200);
	unsigned long v0 = (unsigned long)get_sysvar(_vbclock);

	while ((unsigned long)get_sysvar(_hz_200) - t0 <
	       (unsigned long)VBL_WAIT_TICKS)
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

	for (i = 0; i < 10; i++)
		work_in[i] = 1;
	work_in[10] = 2;			/* raster coordinates	     */
	vdi_handle = ph;
	v_opnvwk(work_in, &vdi_handle, work_out);
	if (vdi_handle <= 0)
		return open_fail(-3, "v_opnvwk returned no handle");

	scr_w = work_out[0] + 1;
	scr_h = work_out[1] + 1;

	/* Take the screen the way a GEM application is supposed to. */
	wind_update(BEG_UPDATE);
	wind_update(BEG_MCTRL);
	screen_locked = 1;
	graf_mouse(M_OFF, 0L);

	pxy[0] = 0;
	pxy[1] = 0;
	pxy[2] = (short)(scr_w - 1);
	pxy[3] = (short)(scr_h - 1);
	vs_clip(vdi_handle, 0, pxy);

	/* Open question 1 and 2, answered or refused. */
	vq_extnd(vdi_handle, 1, work_out);
	if (work_out[4] != PS_PLANES)
		return open_fail(-4, "screen is not 32 bits per pixel -- this "
				     "build serves a truecolour ET4000 only; "
				     "use BALL.PRG for a 256-colour mode");

	/* The staging bitmap. One pixel wider than the screen before padding,
	 * then rounded up to 16 pixels, which is the unit both fd_wdwidth
	 * conventions agree on. */
	stage_w = ((scr_w + 1 + 15) / 16) * 16;
	stage_stride = (long)stage_w * PS_BYTES;
	need = stage_stride * (long)scr_h + 16L;
	stage_raw = (void *)Mxalloc(need, MX_PREFTTRAM);
	if (stage_raw == 0)
		stage_raw = (void *)Malloc(need);
	if (stage_raw == 0)
		return open_fail(-5, "no memory for the staging bitmap");
	a = (unsigned long)stage_raw;
	stage = (unsigned char *)((a + 15UL) & ~15UL);

	/* The one-pixel read-back form the probes use. 16 pixels wide so it
	 * is a whole unit of either convention, one row tall. */
	peek_raw = (void *)Malloc((long)(16 * PS_BYTES) + 16L);
	if (peek_raw == 0)
		return open_fail(-5, "no memory for the read-back form");
	a = (unsigned long)peek_raw;
	peek = (unsigned char *)((a + 15UL) & ~15UL);

	/* Open questions 3 and 4. The channel probe reads single pixels, so
	 * it works under either convention; the stride probe then settles
	 * which convention the rectangle calls want. */
	if (!probe_channels(1) && !probe_channels(2))
		return open_fail(-6,
			"the screen would not say which byte is which colour: "
			"three primaries painted through vs_color did not read "
			"back as primaries. The driver may not honour a pen "
			"colour at this depth");

	if (probe_stride(1))
		wdwidth_conv = 1;
	else if (probe_stride(2))
		wdwidth_conv = 2;
	else
		return open_fail(-7,
			"a staged patch did not read back from the screen "
			"under either fd_wdwidth convention -- the memory "
			"form this driver wants at 32 bits is not one of the "
			"two in circulation");
	fmt_measured = 1;

	/* From here on the form is the full staging bitmap. */
	set_form(&stage_mfdb, stage, stage_w, scr_h, wdwidth_conv);

	have_vbl = probe_vbl();

	scr->w = scr_w;
	scr->h = scr_h;
	scr->pages = 1;			/* as plat-tt.c: one page, and
					 * plat_present() is the only place a
					 * second one would change anything  */

	/* ... and plat_blit(), which widens its rectangle out of the
	 * composition buffer -- sound only while that buffer is a complete,
	 * current picture, which is only true on a single-page display. The
	 * full argument is in plat-tt.c. Derived rather than written as 1 so
	 * that adding a page disables the widening instead of corrupting it. */
	blit_widen = (scr->pages == 1);
	opened = 1;
	err = "";
	return 0;
}

/*
 * Stage a rectangle and hand it to the driver.
 *
 * The core's pixel is 0x00RRGGBB with its own tag bits above the colour, so
 * the mask is not optional: the card must never be shown them. Where the
 * card's byte order is the one the core already holds -- which is what
 * fast_xrgb records -- that mask is the whole of the conversion and the row
 * is a straight copy of masked longs. Where it is not, the row is built a
 * channel at a time, which is three stores instead of one and is why the
 * order was measured rather than assumed.
 */
/*
 * The rectangle is widened in x to a 16-pixel boundary, and the columns that
 * adds are staged from the composition buffer rather than left stale. The
 * reasoning is the same as plat-tt.c's and is written out in full there: the
 * form outside the rectangle just staged holds an older picture, so a driver
 * that rounds the span out to its own blit unit would put that older picture
 * on the screen. Staging at form column 0 with a destination on a 16-pixel
 * boundary gives the two sides one alignment; staging the added columns from
 * ball.c's composition buffer -- a complete, current picture everywhere --
 * means even a rounded-out span moves nothing but the truth.
 *
 * This half had no alignment at all before: the destination x was taken as it
 * came. At four bytes a pixel a long-word blitter was safe by accident and
 * anything wider was not.
 */
#define BLIT_ALIGN	16

void plat_blit(const ball_px *src, long src_pitch,
	       int x, int y, int w, int h)
{
	int xd = x, lead = 0, wd = w;
	unsigned char *dst = stage;
	short pxy[8];
	int row, i;

	if (blit_widen) {
		xd = x & ~(BLIT_ALIGN - 1);	/* left edge, down to a block */
		lead = x - xd;			/* real columns added left    */
		wd = (lead + w + BLIT_ALIGN - 1) & ~(BLIT_ALIGN - 1);
		if (xd + wd > scr_w)
			wd = scr_w - xd;
	}

	src -= lead;
	for (row = 0; row < h; row++) {
		if (fast_xrgb) {
			unsigned int *d = (unsigned int *)(void *)dst;

			for (i = 0; i < wd; i++)
				d[i] = (unsigned int)src[i] & 0x00FFFFFFu;
		} else {
			unsigned char *d = dst;

			for (i = 0; i < wd; i++) {
				unsigned int p = (unsigned int)src[i];

				d[0] = 0;
				d[1] = 0;
				d[2] = 0;
				d[3] = 0;
				d[ch_r] = (unsigned char)((p >> 16) & 0xFFu);
				d[ch_g] = (unsigned char)((p >> 8) & 0xFFu);
				d[ch_b] = (unsigned char)(p & 0xFFu);
				d += PS_BYTES;
			}
		}
		src += src_pitch;
		dst += stage_stride;
	}

	pxy[0] = 0;
	pxy[1] = 0;
	pxy[2] = (short)(wd - 1);
	pxy[3] = (short)(h - 1);
	pxy[4] = (short)xd;
	pxy[5] = (short)y;
	pxy[6] = (short)(xd + wd - 1);
	pxy[7] = (short)(y + h - 1);
	vro_cpyfm(vdi_handle, S_ONLY, pxy, &stage_mfdb, &screen_mfdb);
}

void plat_present(void)
{
	/* One page: the picture is already where it is seen, so all that is
	 * left is not to tear it -- and not to hang if there is no retrace to
	 * wait for (probe_vbl()). */
	if (have_vbl)
		Vsync();
}

/* ------------------------------------------------------------------------ */
/* The closing report                                                        */
/*                                                                           */
/* One line into ball.c's own log saying what the probes decided. On this    */
/* file it matters more than on plat-tt.c, because everything here is new:   */
/* the first run on hardware is read off this line.                          */
/* ------------------------------------------------------------------------ */
static char rbuf[200];
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
	r_str("BALL: PiStorm/ET4000 screen ");
	r_dec((long)scr_w);
	r_str("x");
	r_dec((long)scr_h);
	r_str("x32, format ");
	r_str(fmt_measured ? "MEASURED" : "NOT ESTABLISHED");
	r_str(", channel bytes R");
	r_dec((long)ch_r);
	r_str(" G");
	r_dec((long)ch_g);
	r_str(" B");
	r_dec((long)ch_b);
	r_str(fast_xrgb ? " (XRGB: staging is a mask)"
			: " (staging shuffles channels)");
	r_str(", fd_wdwidth = pixels/");
	r_dec(wdwidth_conv == 1 ? 16L : 2L);
	r_str(", VBL ");
	r_str(have_vbl ? "running (Vsync used)" : "STOPPED (no Vsync)");
	r_str("\r\n");

	fd = Fopen(PS_REPORT_PATH, 2);		/* read/write		     */
	if (fd < 0) {
		fd = Fcreate(PS_REPORT_PATH, 0);
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
	if (vdi_handle > 0 && opened)
		write_report();

	if (ap_id >= 0) {
		graf_mouse(M_ON, 0L);
		if (screen_locked) {
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

	if (peek_raw) {
		Mfree(peek_raw);
		peek_raw = 0;
		peek = 0;
	}
	if (stage_raw) {
		Mfree(stage_raw);
		stage_raw = 0;
		stage = 0;
	}
	opened = 0;
}
