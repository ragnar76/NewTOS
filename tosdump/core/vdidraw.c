/*
 * vdidraw.c -- VDIDRAW: draw every VDI output primitive, numbered and labelled.
 *
 * The reference dumper (dumpvdi.c) records the five parameter-block arrays,
 * and for the drawing primitives those arrays come back empty: `v_pline`,
 * `v_fillarea`, `v_gtext`, the GDP sub-ops and `v_contourfill` write nothing.
 * A dump that matches proves the block survived, not that a pixel landed
 * where it should. Thirteen opcodes are invisible to it for that reason
 * (tosdump/reference/FINDINGS-r3.md).
 *
 * This program closes that gap the only way a chunky framebuffer allows: it
 * draws each primitive into its own numbered cell with a caption saying what
 * the cell should contain, so a person can judge it. Report the numbers that
 * look wrong; each is one opcode or one attribute.
 *
 * It is deliberately NOT a pass/fail program. It has no expected image to
 * compare against -- it renders and labels, and the judgement is the
 * operator's. That is the honest division of labour here: the dump measures
 * what can be measured, this shows what can only be seen.
 *
 * Timing: the whole sheet draws in milliseconds, so without a pause the
 * desktop repaints over it before anything can be read.  It therefore waits
 * through `evnt_timer` (AES 24) -- once after each row, so the sheet builds up
 * where you can watch it, and a long hold at the end.  `evnt_timer` blocks on
 * a deadline, never on input: a blocking console read hangs real hardware
 * outright (QUIRK-002), which is the bug that had to be taken back out of the
 * dumper.
 *
 * Layout is absolute, computed from the workstation's own reported size, and
 * every primitive is drawn at a fixed offset inside its cell. Building the
 * same source for the m68k would therefore produce a directly comparable
 * picture on a TT; only the NewTOS target is built today.
 */
#include <stdio.h>
#include <string.h>

#include "abi.h"
#include "gem.h"
#include "log.h"

#ifndef TD_BUILD_ID
#define TD_BUILD_ID "----"
#endif

/* One shared parameter block: this program measures nothing, so there is no
 * poison to preserve and no record to write. */
/* Millisecond waits.  ROW_MS lets the sheet build up visibly; HOLD_MS is
 * long enough to read all of it, or photograph it, before the desktop
 * comes back. */
#define ROW_MS   1500L
#define HOLD_MS 60000L

static int16_t contrl[TD_VDI_CONTRL_N];
static int16_t intin[TD_VDI_INTIN_N];
static int16_t ptsin[TD_VDI_PTSIN_N];
static int16_t intout[TD_VDI_INTOUT_N];
static int16_t ptsout[TD_VDI_PTSOUT_N];

/* AES parameter block, for evnt_timer only. */
static int16_t aes_control[5];
static int16_t aes_global[15];
static int16_t aes_intin[16];
static int16_t aes_intout[7];
static void   *aes_addrin[3];
static void   *aes_addrout[1];

/* Wait `ms` milliseconds through the AES timer event (opcode 24, int_in[0] =
 * low WORD, int_in[1] = high WORD -- EmuTOS `gemsuper.c:142`,
 * `MAKE_ULONG(T_HICOUNT, T_LOCOUNT)`). */
static void wait_ms(long ms)
{
	aes_control[0] = 24;
	aes_control[1] = 2;
	aes_control[2] = 1;
	aes_control[3] = 0;
	aes_control[4] = 0;
	aes_intin[0] = (int16_t)(ms & 0xFFFFL);
	aes_intin[1] = (int16_t)((ms >> 16) & 0xFFFFL);
	td_abi_aes(aes_control, aes_global, aes_intin, aes_intout,
	           aes_addrin, aes_addrout);
}

static int16_t g_handle;
static int16_t cell_w = 200, cell_h = 170;
static int16_t cols = 9, rows = 6;
static int16_t ox, oy;              /* current cell origin */

static void vdi(int16_t opcode, int16_t sub, int16_t n_pts, int16_t n_int)
{
	memset(contrl, 0, sizeof contrl);
	contrl[0] = opcode;
	contrl[1] = n_pts;
	contrl[3] = n_int;
	contrl[5] = sub;
	contrl[6] = g_handle;
	td_abi_vdi(contrl, intin, ptsin, intout, ptsout);
}

static void seti(int16_t opcode, int16_t v)
{
	intin[0] = v;
	vdi(opcode, 0, 0, 1);
}

/* Points are given cell-relative and translated here, so a cell's drawing
 * code never has to know where the cell sits. */
static void pts(const int16_t *xy, int n_points)
{
	int i;

	for (i = 0; i < n_points * 2; i += 2) {
		ptsin[i]     = (int16_t)(ox + xy[i]);
		ptsin[i + 1] = (int16_t)(oy + xy[i + 1]);
	}
}

static void text_at(int16_t x, int16_t y, const char *s)
{
	int n = 0;

	while (s[n] && n < TD_VDI_INTIN_N - 1) {
		intin[n] = (int16_t)(unsigned char)s[n];
		n++;
	}
	ptsin[0] = (int16_t)(ox + x);
	ptsin[1] = (int16_t)(oy + y);
	vdi(8, 0, 1, (int16_t)n);       /* v_gtext */
}

/* Attributes every cell starts from, so a cell never inherits the one before
 * it.  v_clrwk is deliberately NOT part of this -- the screen is cleared once
 * at the start and then every cell draws into it. */
static void cell_defaults(void)
{
	seti(32, 1);                    /* vswr_mode  replace      */
	seti(15, 1);                    /* vsl_type   solid        */
	ptsin[0] = 1; ptsin[1] = 0;
	vdi(16, 0, 1, 0);               /* vsl_width  1            */
	seti(17, 1);                    /* vsl_color  pen 1        */
	intin[0] = 0; intin[1] = 0;
	vdi(108, 0, 0, 2);              /* vsl_ends   squared      */
	seti(18, 3);                    /* vsm_type   asterisk     */
	seti(20, 1);                    /* vsm_color  pen 1        */
	seti(23, 1);                    /* vsf_interior solid      */
	seti(24, 1);                    /* vsf_style  1            */
	seti(25, 1);                    /* vsf_color  pen 1        */
	seti(104, 1);                   /* vsf_perimeter on        */
	seti(21, 1);                    /* vst_font   1            */
	seti(22, 1);                    /* vst_color  pen 1        */
	seti(13, 0);                    /* vst_rotation 0          */
	seti(106, 0);                   /* vst_effects none        */
	intin[0] = 0; intin[1] = 0;
	vdi(39, 0, 0, 2);               /* vst_alignment left/base */
	seti(129, 0);                   /* vs_clip    off          */
}

/* Start cell `n`: set its origin, reset attributes, print its number and its
 * caption.  The caption is what makes the picture judgeable without a
 * reference machine beside it. */
static void cell(int n, const char *caption)
{
	char num[8];

	/* A new row is starting: hold the sheet so far, so it builds up at a
	 * pace a person can follow instead of appearing all at once. */
	if (n > 0 && (n % cols) == 0)
		wait_ms(ROW_MS);

	ox = (int16_t)((n % cols) * cell_w + 8);
	oy = (int16_t)((n / cols) * cell_h + 8);
	cell_defaults();

	snprintf(num, sizeof num, "%d", n + 1);
	text_at(0, 12, num);
	text_at(0, cell_h - 14, caption);
	/* the drawing area of a cell is x 4..cell_w-8, y 18..cell_h-30 */
}

/* ── the cells ───────────────────────────────────────────────────────────
 *
 * One primitive per cell, chosen so the shape itself carries the verdict:
 * a wrong arc, a wrong dash pattern or a wrong text origin is visible, where
 * a probe at three pixels would miss it.  The thirteen opcodes the dump
 * cannot see are all here.
 */
static void draw_all(void)
{
	static const int16_t horiz[4]  = { 10, 60, 110, 60 };
	static const int16_t vert[4]   = { 60, 25, 60, 110 };
	static const int16_t diag[4]   = { 10, 25, 110, 110 };
	static const int16_t zig[10]   = { 10,100, 35,30, 60,100, 85,30, 110,100 };
	static const int16_t tri[6]    = { 20,110, 110,110, 65,30 };
	static const int16_t quad[8]   = { 20,35, 110,35, 110,105, 20,105 };
	static const int16_t rect[4]   = { 20,35, 110,105 };
	static const int16_t circ[10]  = { 65,70, 0,0, 40,0, 0,0, 0,0 };
	static const int16_t elli[10]  = { 65,70, 50,30, 0,0, 50,0, 0,0 };
	int n = 0;

	cell(n++, "v_pline horizontal 100");
	pts(horiz, 2); vdi(6, 0, 2, 0);

	cell(n++, "v_pline vertical 85");
	pts(vert, 2); vdi(6, 0, 2, 0);

	cell(n++, "v_pline diagonal");
	pts(diag, 2); vdi(6, 0, 2, 0);

	cell(n++, "v_pline 5-point zigzag");
	pts(zig, 5); vdi(6, 0, 5, 0);

	cell(n++, "vsl_type 2 long dash");
	seti(15, 2); pts(horiz, 2); vdi(6, 0, 2, 0);

	cell(n++, "vsl_type 4 dash-dot");
	seti(15, 4); pts(horiz, 2); vdi(6, 0, 2, 0);

	cell(n++, "vsl_udsty 0xAAAA");
	seti(15, 7); seti(113, (int16_t)0xAAAA); pts(horiz, 2); vdi(6, 0, 2, 0);

	cell(n++, "vsl_width 7 thick line");
	ptsin[0] = 7; ptsin[1] = 0; vdi(16, 0, 1, 0);
	pts(horiz, 2); vdi(6, 0, 2, 0);

	cell(n++, "vsl_ends 1,2 arrow+round");
	ptsin[0] = 5; ptsin[1] = 0; vdi(16, 0, 1, 0);
	intin[0] = 1; intin[1] = 2; vdi(108, 0, 0, 2);
	pts(horiz, 2); vdi(6, 0, 2, 0);

	cell(n++, "v_pmarker type 3 x5");
	pts(zig, 5); vdi(7, 0, 5, 0);

	cell(n++, "v_pmarker type 6 big");
	seti(18, 6); ptsin[0] = 0; ptsin[1] = 30; vdi(19, 0, 1, 0);
	pts(zig, 5); vdi(7, 0, 5, 0);

	cell(n++, "v_gtext ABCdef 123");
	text_at(10, 70, "ABCdef 123");

	cell(n++, "vst_effects 1 thickened");
	seti(106, 1); text_at(10, 70, "ABCdef 123");

	cell(n++, "vst_effects 8 outlined");
	seti(106, 8); text_at(10, 70, "ABCdef 123");

	cell(n++, "vst_rotation 900 up");
	seti(13, 900); text_at(60, 110, "ROTATED");

	cell(n++, "v_fillarea triangle");
	pts(tri, 3); vdi(9, 0, 3, 0);

	cell(n++, "v_fillarea 4-gon");
	pts(quad, 4); vdi(9, 0, 4, 0);

	cell(n++, "vsf_interior 0 hollow");
	seti(23, 0); pts(quad, 4); vdi(9, 0, 4, 0);

	cell(n++, "vsf_interior 2 pattern 8");
	seti(23, 2); seti(24, 8); pts(quad, 4); vdi(9, 0, 4, 0);

	cell(n++, "vsf_interior 3 hatch 4");
	seti(23, 3); seti(24, 4); pts(quad, 4); vdi(9, 0, 4, 0);

	cell(n++, "vr_recfl filled rect");
	pts(rect, 2); vdi(114, 0, 2, 0);

	cell(n++, "GDP 1 v_bar");
	pts(rect, 2); vdi(11, 1, 2, 0);

	cell(n++, "GDP 2 v_arc 0-90deg");
	intin[0] = 0; intin[1] = 900; pts(elli, 5); vdi(11, 2, 5, 2);

	cell(n++, "GDP 3 v_pieslice 0-180");
	intin[0] = 0; intin[1] = 1800; pts(elli, 5); vdi(11, 3, 5, 2);

	cell(n++, "GDP 4 v_circle r=40");
	intin[0] = 0; intin[1] = 3600; pts(circ, 5); vdi(11, 4, 5, 2);

	cell(n++, "GDP 5 v_ellipse 50x30");
	intin[0] = 0; intin[1] = 3600; pts(elli, 5); vdi(11, 5, 5, 2);

	cell(n++, "GDP 6 v_ellarc 0-90");
	intin[0] = 0; intin[1] = 900; pts(elli, 5); vdi(11, 6, 5, 2);

	cell(n++, "GDP 7 v_ellpie 0-180");
	intin[0] = 0; intin[1] = 1800; pts(elli, 5); vdi(11, 7, 5, 2);

	cell(n++, "GDP 8 v_rbox rounded");
	pts(rect, 2); vdi(11, 8, 2, 0);

	cell(n++, "GDP 9 v_rfbox filled");
	pts(rect, 2); vdi(11, 9, 2, 0);

	cell(n++, "GDP 10 v_justified");
	{
		static const int16_t jp[4] = { 10, 70, 110, 0 };
		int i; const char *s = "JUSTIFY";
		intin[0] = 1; intin[1] = 1;
		for (i = 0; s[i]; i++) intin[2 + i] = (int16_t)(unsigned char)s[i];
		pts(jp, 2); vdi(11, 10, 2, (int16_t)(2 + 7));
	}

	cell(n++, "v_contourfill in circle");
	seti(23, 0);                                  /* hollow outline first */
	intin[0] = 0; intin[1] = 3600; pts(circ, 5); vdi(11, 4, 5, 2);
	seti(23, 1); seti(25, 1);
	ptsin[0] = (int16_t)(ox + 65); ptsin[1] = (int16_t)(oy + 70);
	intin[0] = 1; vdi(103, 0, 1, 1);

	cell(n++, "vs_clip: box clips X");
	{
		static const int16_t cl[4] = { 30, 45, 95, 95 };
		static const int16_t d2[4] = { 110, 25, 10, 110 };
		pts(cl, 2);
		intin[0] = 1; vdi(129, 0, 2, 1);          /* clip on            */
		pts(diag, 2); vdi(6, 0, 2, 0);
		pts(d2, 2);   vdi(6, 0, 2, 0);
		seti(129, 0);
		pts(cl, 2); seti(23, 0); vdi(11, 8, 2, 0); /* show the clip box  */
	}

	cell(n++, "vswr_mode 3 XOR overlap");
	pts(rect, 2); vdi(114, 0, 2, 0);
	seti(32, 3);
	{
		static const int16_t r2[4] = { 60, 65, 150, 135 };
		pts(r2, 2); vdi(114, 0, 2, 0);
	}
}

int main(void)
{
	struct td_wk wk;
	int rc;

	td_log_init();
	td_log("VDIDRAW build %s\n", TD_BUILD_ID);

	rc = td_gem_open(&wk);
	if (rc != 0) {
		td_log("%s\n", td_gem_error_text(rc));
		td_log_write("VDIDRAW.LOG");
		return 1;
	}
	g_handle = wk.handle;
	td_log("workstation %d, %dx%d\n", wk.handle,
	       wk.work_out[0] + 1, wk.work_out[1] + 1);

	/* Fit the grid to whatever this machine reports. */
	cols = (int16_t)((wk.work_out[0] + 1) / cell_w);
	rows = (int16_t)((wk.work_out[1] + 1) / cell_h);
	if (cols < 1) cols = 1;
	if (rows < 1) rows = 1;
	td_log("grid %dx%d cells of %dx%d\n", cols, rows, cell_w, cell_h);

	vdi(3, 0, 0, 0);                 /* v_clrwk, once */
	draw_all();

	/* Hold the finished sheet.  The desktop repaints the moment this
	 * returns, so everything readable has to be readable now. */
	td_log("holding for %ld ms\n", HOLD_MS);
	wait_ms(HOLD_MS);

	td_gem_close(&wk);
	td_log("drawn\n");
	td_log_write("VDIDRAW.LOG");
	return 0;
}
