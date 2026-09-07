/*
 * plat-newtos.c -- the NewTOS / Raspberry Pi 4 half of the bouncing-ball demo.
 *
 * This is the ONLY file of the demo that knows what a pixel looks like. The
 * counterpart for an Atari TT with a NOVA card would be plat-tt.c: the same
 * five functions, a 256-entry hardware palette instead of a lookup table, a
 * memcpy instead of the index->XRGB conversion, and an empty plat_present()
 * (a NOVA driver has one page, so there is nothing to flip). Nothing else in
 * the demo changes.
 *
 * Three questions have to be answered here, and NewTOS answers all three
 * through documented TOS calls -- no invented interfaces:
 *
 *  1. How big is the screen?  XBIOS cannot say: Getrez is a colour code, not
 *     a geometry (docs/api/xbios/04-getrez.md). The answer comes from a VDI
 *     workstation, opened the way every GEM application opens one --
 *     appl_init, graf_handle, v_opnvwk -- whose DEV_TAB reports xmax/ymax in
 *     work_out[0]/[1]. That also makes this program a `.NAP`: it is an AES
 *     client, so it is launched from the desktop, never from \AUTO\.
 *
 *  2. How many bytes is a screen row?  Nothing reports the hardware pitch to
 *     an application, and it is not width*4: a display controller may pad the
 *     line (docs/api/xbios/02-physbase.md, ScreenInfo::stride_bytes). So it is
 *     *measured*: zero the start of the screen through the pointer Logbase()
 *     hands out, ask the VDI to fill screen row 1 with a solid pen, and find
 *     the first byte that changed. Its offset is the pitch, exactly, whatever
 *     the firmware chose. The measurement costs two writes' worth of screen
 *     and runs once.
 *
 *  3. Is there a second screen page?  Setscreen's contract says a base the
 *     display cannot use leaves the base untouched and Logbase() keeps
 *     reporting the truth (docs/api/xbios/05-setscreen.md), so asking for
 *     page 0 + one page and reading Logbase() back answers it without
 *     assuming an answer. sdk/examples/scrflip.c proves the same contract on
 *     real VideoCore firmware.
 *
 * Cache maintenance: none, deliberately. The pages Physbase/Logbase publish
 * to a process are mapped Normal Non-Cacheable (ADR-023 amendment), so a
 * plain store lands in DRAM and the display engine sees it.
 */
#include <gem.h>
#include <mint/osbind.h>

#include "ball.h"

/* This half of the seam is written for a PALETTE picture: it is handed
 * indices and it owns the colour table. Building it against a truecolour
 * core would compile and then draw nonsense, so say so here instead. */
#if BALL_TRUECOLOR
#error "plat-newtos.c serves BALL_TRUECOLOR 0 only"
#endif

/* Largest line padding, in pixels, the pitch measurement looks for. A display
 * controller pads a line to its own alignment; a whole extra 1024 pixels is
 * far more than any of them use, and bounding the search means a failure is
 * reported instead of guessed. */
#define PITCH_SLACK_PX 1024

/* AES / VDI state. */
static short ap_id = -1;
static short vdi_handle;
static short work_in[11];
static short work_out[57];

/* Screen memory. */
static unsigned char *page0;
static long pitch;			/* bytes per screen row		     */
static long page_bytes;			/* bytes per screen page	     */
static int npages;
static int draw_page;			/* page plat_blit currently writes   */
static unsigned char *draw_base;

/* index -> XRGB8888. The TT/NOVA build has no such table: there the palette
 * IS the hardware's, and the blit is a copy. */
static unsigned int pal32[BALL_PAL_N];

static const char *err = "";

/* 8-byte stores into the framebuffer; may_alias keeps -O2 honest about the
 * unsigned int * / unsigned long long * pun below. */
typedef unsigned long long u64_alias __attribute__((__may_alias__));

const char *plat_error(void)
{
	return err;
}

/*
 * Measure the hardware pitch (question 2 above). Returns the byte stride of
 * one screen row, or 0 when the answer is not inside the searched range --
 * in which case the demo stops rather than draw a skewed picture.
 */
static long measure_pitch(int w)
{
	volatile unsigned int *p = (volatile unsigned int *)(void *)page0;
	long span = (long)w + PITCH_SLACK_PX;
	short pxy[4];
	long i;

	/* Row 0 plus every offset row 1 could start at, cleared to a value the
	 * VDI cannot write: every VDI pen is opaque (alpha 0xFF), so any pixel
	 * it draws is non-zero. */
	for (i = 0; i < span; i++)
		p[i] = 0u;

	vsf_interior(vdi_handle, FIS_SOLID);
	vsf_color(vdi_handle, 1);
	pxy[0] = 0;
	pxy[1] = 1;
	pxy[2] = (short)(w - 1);
	pxy[3] = 1;
	vr_recfl(vdi_handle, pxy);

	/* Row 1 cannot start before width pixels; the first changed word is
	 * its first pixel. */
	for (i = w; i < span; i++)
		if (p[i] != 0u)
			return i * 4L;

	return 0L;
}

int plat_open(ball_screen *scr)
{
	short cw, ch, bw, bh;
	short ph;
	int i;

	ap_id = appl_init();
	if (ap_id < 0) {
		err = "appl_init failed";
		return -1;
	}

	ph = graf_handle(&cw, &ch, &bw, &bh);
	if (ph <= 0) {
		err = "graf_handle returned no physical workstation";
		return -2;
	}

	/* The classic GEM application open: every attribute index 1, raster
	 * coordinates. */
	for (i = 0; i < 10; i++)
		work_in[i] = 1;
	work_in[10] = 2;
	vdi_handle = ph;
	v_opnvwk(work_in, &vdi_handle, work_out);
	if (vdi_handle <= 0) {
		err = "v_opnvwk returned no handle";
		return -3;
	}

	scr->w = work_out[0] + 1;
	scr->h = work_out[1] + 1;

	/* The demo owns the screen; the AES arrow has no business on it. */
	graf_mouse(M_OFF, 0L);

	page0 = (unsigned char *)Logbase();
	if (page0 == 0) {
		err = "no display -- Logbase() is 0";
		return -4;
	}

	pitch = measure_pitch(scr->w);
	if (pitch <= 0) {
		err = "could not measure the screen pitch";
		return -5;
	}
	page_bytes = pitch * (long)scr->h;

	/* Question 3: does asking for the page above page 0 move the logical
	 * base? Only a base the display can really use does. */
	Setscreen((long)(page0 + page_bytes), -1L, -1);
	npages = ((unsigned char *)Logbase() != page0) ? 2 : 1;
	Setscreen((long)page0, -1L, -1);

	scr->pages = npages;
	draw_page = 0;
	draw_base = page0;
	err = "";
	return 0;
}

void plat_palette(const ball_rgb *pal)
{
	int i;

	/* The framebuffer's own format: opaque XRGB8888, 0xFF_RR_GG_BB
	 * (kernel/src/hal/framebuffer.rs `rgb`). */
	for (i = 0; i < BALL_PAL_N; i++)
		pal32[i] = 0xFF000000u
			 | ((unsigned int)pal[i].r << 16)
			 | ((unsigned int)pal[i].g << 8)
			 | (unsigned int)pal[i].b;
}

void plat_blit(const ball_px *src, long src_pitch,
	       int x, int y, int w, int h)
{
	unsigned char *dst = draw_base + (long)y * pitch + (long)x * 4L;
	int row;

	for (row = 0; row < h; row++) {
		const unsigned char *s = src;
		unsigned int *d = (unsigned int *)(void *)dst;
		int n = w;

		/* Two pixels per store wherever the destination allows it: the
		 * screen is uncacheable, so every store goes to DRAM and half
		 * as many of them is the difference that shows. */
		if (n > 0 && (((unsigned long)(void *)d) & 7UL) != 0UL) {
			*d++ = pal32[*s++];
			n--;
		}
		while (n >= 2) {
			unsigned long long v =
				(unsigned long long)pal32[s[0]] |
				((unsigned long long)pal32[s[1]] << 32);
			*(u64_alias *)(void *)d = v;
			d += 2;
			s += 2;
			n -= 2;
		}
		while (n-- > 0)
			*d++ = pal32[*s++];

		src += src_pitch;
		dst += pitch;
	}
}

void plat_present(void)
{
	if (npages < 2) {
		/* One page: the picture is already where it is seen, so all
		 * that is left is not to tear it. */
		Vsync();
		return;
	}

	/* Flip on a frame boundary: wait for the retrace, show the page just
	 * drawn, then point both the scan-out's partner and the system's
	 * drawing base at the other one. */
	Vsync();
	Setscreen(-1L, (long)draw_base, -1);
	draw_page ^= 1;
	draw_base = page0 + (long)draw_page * page_bytes;
	Setscreen((long)draw_base, -1L, -1);
}

void plat_close(void)
{
	if (page0 != 0) {
		Vsync();
		Setscreen((long)page0, (long)page0, -1);
	}
	if (ap_id >= 0) {
		graf_mouse(M_ON, 0L);
		if (vdi_handle > 0) {
			v_clsvwk(vdi_handle);
			vdi_handle = 0;
		}
		appl_exit();
		ap_id = -1;
	}
}
