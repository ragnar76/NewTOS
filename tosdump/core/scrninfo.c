/*
 * scrninfo.c -- SCRNINFO.TTP, the EdDI / vq_scrninfo probe.
 *
 * Records, on a real machine, what its VDI says about the screen format and
 * what the pixels it draws actually look like in memory, so the UNKNOWN
 * points of docs/api/vdi/eddi-vq_scrninfo.md (sections 6.1 and 7/E3: what a
 * real NVDI/Nova driver reports, its version, its byte order) can be filled
 * from a device instead of guessed.
 *
 * usage: SCRNINFO [OUT.TXT]        default SCRNINFO.TXT in the current folder
 *
 * What it does, in order -- and nothing else:
 *   1. copies the cookie jar and looks for 'EdDI' (spec section 2);
 *   2. if EdDI is there, calls its dispatcher with function 0 (spec
 *      section 2) and records the version;
 *   3. opens a virtual workstation (appl_init, graf_handle, v_opnvwk with the
 *      standard work_in, core/gem.c) and records v_opnvwk's work_out,
 *      vq_extnd(0) and vq_extnd(1); then, ONLY if EdDI is there,
 *      vq_scrninfo exactly per spec section 3.1 (CONTRL[5] = 1,
 *      INTIN[0] = 2) with all 273 work_out words and CONTRL[2]/[4];
 *   4. pixel truth: draws known single-colour pixels with the VDI, copies
 *      the area into a device-format memory MFDB (fd_stand = 0) with
 *      vro_cpyfm, and records the raw bytes -- never reading screen memory
 *      directly.  The screen area is saved first and copied back
 *      afterwards; on a >8-plane screen the pens it re-colours with vs_color
 *      are given their old values back;
 *   5. writes one plain-text report and prints one summary line.
 *
 * No file other than the report is written.  The only memory written is the
 * program's own (and, transiently, the 32x2-pixel screen area it restores).
 */
#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "abi.h"
#include "gem.h"
#include "scrninfo.h"

#ifndef TD_BUILD_ID
#define TD_BUILD_ID "----"
#endif

/* ---------------------------------------------------------------- report */

#define RPT_SIZE 49152
static char   rpt_buf[RPT_SIZE];
static size_t rpt_len;
static int    rpt_cut;

static void rpt(const char *fmt, ...)
{
	va_list ap;
	int n;

	if (rpt_cut)
		return;
	va_start(ap, fmt);
	n = vsnprintf(rpt_buf + rpt_len, RPT_SIZE - rpt_len, fmt, ap);
	va_end(ap);
	if (n < 0)
		return;
	if ((size_t)n >= RPT_SIZE - rpt_len) {
		rpt_len = RPT_SIZE - 1;
		rpt_cut = 1;
		return;
	}
	rpt_len += (size_t)n;
}

/* `n` words, 8 per line, decimal and hex, with the index of the first. */
static void rpt_words(const char *name, const int16_t *w, int first, int n)
{
	int i;

	for (i = 0; i < n; i++) {
		if (i % 8 == 0)
			rpt("%s[%3d]", name, first + i);
		rpt(" %6d/%04X", w[i], (uint16_t)w[i]);
		if (i % 8 == 7 || i + 1 == n)
			rpt("\n");
	}
}

static void tag_text(uint32_t tag, char *out)
{
	int i;

	for (i = 0; i < 4; i++) {
		char c = (char)((tag >> (24 - 8 * i)) & 0xFF);
		out[i] = (c >= 0x20 && c < 0x7F) ? c : '.';
	}
	out[4] = '\0';
}

/* ------------------------------------------------------------ GEM calls */

/* One VDI parameter block for everything but vq_scrninfo.  CONTRL has room
 * for two MFDB pointers at [7..10] (m68k: two WORDs each). */
static int16_t contrl[TD_VDI_CONTRL_N];
static int16_t intin[TD_VDI_INTIN_N];
static int16_t ptsin[TD_VDI_PTSIN_N];
static int16_t intout[TD_VDI_INTOUT_N];
static int16_t ptsout[TD_VDI_PTSOUT_N];

#define POISON 0x5A5A

static void vdi(int16_t op, int16_t n_pts, int16_t n_int, int16_t sub,
                int16_t handle)
{
	memset(contrl, 0, sizeof contrl);
	contrl[0] = op;
	contrl[1] = n_pts;
	contrl[2] = POISON;
	contrl[3] = n_int;
	contrl[4] = POISON;
	contrl[5] = sub;
	contrl[6] = handle;
	td_abi_vdi(contrl, intin, ptsin, intout, ptsout);
}

static int16_t aes_control[5];
static int16_t aes_intin[16];
static int16_t aes_intout[7];
static void   *aes_addrin[3];
static void   *aes_addrout[1];

static void aes(int16_t op, int16_t n_int, int16_t n_out, int16_t n_addr)
{
	aes_control[0] = op;
	aes_control[1] = n_int;
	aes_control[2] = n_out;
	aes_control[3] = n_addr;
	aes_control[4] = 0;
	td_abi_aes(aes_control, td_gem_global(), aes_intin, aes_intout,
	           aes_addrin, aes_addrout);
}

/* graf_mouse(M_OFF/M_ON): AES 78, 1 intin, 1 intout, 1 addrin. */
static void mouse(int on)
{
	aes_intin[0] = on ? 257 : 256;
	aes_addrin[0] = 0;
	aes(78, 1, 1, 1);
}

/* wind_update(BEG_UPDATE/END_UPDATE): AES 107, 1 intin, 1 intout. */
static void screen_lock(int lock)
{
	aes_intin[0] = lock ? 1 : 0;
	aes(107, 1, 1, 0);
}

/* The MFDB in the target's own layout (see runner_rast.c, td_mfdb). */
struct mfdb {
	void   *fd_addr;
	int16_t fd_w;
	int16_t fd_h;
	int16_t fd_wdwidth;
	int16_t fd_stand;
	int16_t fd_nplanes;
	int16_t fd_r1;
	int16_t fd_r2;
	int16_t fd_r3;
};

static void set_ptr(int n, void *p)
{
	const int nptrints = (int)(sizeof(void *) / sizeof(int16_t));

	memcpy(&contrl[7 + n * nptrints], &p, sizeof p);
}

/* vro_cpyfm, mode 3 (S_ONLY): gemlib vro_cpyfm.c:53-62, opcode 109, 4 ptsin
 * points, 1 intin, MFDB pointers at contrl[7] and contrl[7 + N_PTRINTS]. */
static void cpyfm(int16_t handle, struct mfdb *src, int16_t sx, int16_t sy,
                  struct mfdb *dst, int16_t dx, int16_t dy, int16_t w,
                  int16_t h)
{
	memset(contrl, 0, sizeof contrl);
	contrl[0] = 109;
	contrl[1] = 4;
	contrl[3] = 1;
	contrl[6] = handle;
	set_ptr(0, src);
	set_ptr(1, dst);
	intin[0] = 3;
	ptsin[0] = sx;
	ptsin[1] = sy;
	ptsin[2] = (int16_t)(sx + w - 1);
	ptsin[3] = (int16_t)(sy + h - 1);
	ptsin[4] = dx;
	ptsin[5] = dy;
	ptsin[6] = (int16_t)(dx + w - 1);
	ptsin[7] = (int16_t)(dy + h - 1);
	td_abi_vdi(contrl, intin, ptsin, intout, ptsout);
}

/* ------------------------------------------------------------- the data */

static struct td_jar   jar;
static struct td_clock clk;
static struct td_wk    wk;

static int16_t ext0_out[45], ext0_pts[12], ext0_c2, ext0_c4;
static int16_t ext1_out[45], ext1_pts[12], ext1_c2, ext1_c4;

/* vq_scrninfo: its own arrays, sized as gemlib sizes them -- intout for
 * well over 273 words, ptsout a 1024-word dummy (spec section 3.1, gemlib
 * n_vq_scrninfo.c / vdi_dummy.c:4) -- and pre-filled with POISON so a word
 * the driver did not write stays recognisable. */
#define SI_OUT_N   512
#define SI_WORK_N  273
static int16_t si_contrl[TD_VDI_CONTRL_N];
static int16_t si_intin[16];
static int16_t si_ptsin[16];
static int16_t si_intout[SI_OUT_N];
static int16_t si_ptsout[1024];

/* The pixel test area: 32 x 2 pixels.  The largest device form is 32 planes:
 * 2 words/plane-row x 2 rows x 32 planes = 128 words = 256 bytes. */
#define PX_W     32
#define PX_H     2
#define PX_WDW   (PX_W / 16)
#define PX_X0    32
#define PX_Y0    32
#define PX_WORDS 512
#define FILL     0xA5A5
static int16_t px_save[PX_WORDS];
static int16_t px_probe[PX_WORDS];
static int16_t px_check[PX_WORDS];
static struct mfdb mf_screen, mf_save, mf_probe, mf_check;
static int16_t px_pen[PX_H][PX_W];
static int16_t px_get0[PX_H][PX_W], px_get1[PX_H][PX_W];

#define MAX_PENS 24
static int16_t pens[MAX_PENS];
static int     npens;
static int16_t pen_old[MAX_PENS][3];     /* vq_color(i,0) before          */
static int16_t pen_set[MAX_PENS][3];     /* what vs_color was given        */
static int16_t pen_real[MAX_PENS][4];    /* vq_color(i,1): [0] = index rc  */
static int16_t pen_req[MAX_PENS][4];     /* vq_color(i,0): [0] = index rc  */

/* RGB in VDI per-mille for a >8-plane screen: primaries, mixes, and
 * half/low intensities so single channel bits show in the pixel word. */
static const int16_t tc_rgb[16][3] = {
	{1000, 1000, 1000}, {   0,    0,    0}, {1000,    0,    0},
	{   0, 1000,    0}, {   0,    0, 1000}, {1000, 1000,    0},
	{   0, 1000, 1000}, {1000,    0, 1000}, { 500,    0,    0},
	{   0,  500,    0}, {   0,    0,  500}, { 100,    0,    0},
	{   0,  100,    0}, {   0,    0,  100}, {1000,  500,  250},
	{ 250,  500, 1000},
};

static void vq_color(int16_t h, int16_t pen, int16_t flag, int16_t *out4)
{
	intin[0] = pen;
	intin[1] = flag;
	vdi(26, 0, 2, 0, h);
	out4[0] = intout[0];
	out4[1] = intout[1];
	out4[2] = intout[2];
	out4[3] = intout[3];
}

static void vs_color(int16_t h, int16_t pen, const int16_t *rgb)
{
	intin[0] = pen;
	intin[1] = rgb[0];
	intin[2] = rgb[1];
	intin[3] = rgb[2];
	vdi(14, 0, 4, 0, h);
}

static void pick_pens(int16_t ncolors, int truecolor)
{
	static const int16_t extra[] = { 16, 32, 64, 128, 254, 255 };
	int i;

	npens = 0;
	if (truecolor) {
		for (i = 0; i < 16; i++)
			pens[npens++] = (int16_t)i;
		return;
	}
	for (i = 0; i < 16 && i < ncolors; i++)
		pens[npens++] = (int16_t)i;
	for (i = 0; i < (int)(sizeof extra / sizeof extra[0]); i++)
		if (extra[i] < ncolors && npens < MAX_PENS)
			pens[npens++] = extra[i];
}

/* Row 0: pens in list order, row 1: reversed; pixels past the list repeat
 * the first pen.  Two different rows show the row stride in the bytes. */
static void plan_pixels(void)
{
	int c;

	for (c = 0; c < PX_W; c++) {
		px_pen[0][c] = (c < npens) ? pens[c] : pens[0];
		px_pen[1][c] = (c < npens) ? pens[npens - 1 - c] : pens[0];
	}
}

static void draw_pixels(int16_t h)
{
	int r, c;

	intin[0] = 1;  vdi(32, 0, 1, 0, h);      /* vswr_mode(MD_REPLACE)    */
	intin[0] = 1;  vdi(23, 0, 1, 0, h);      /* vsf_interior(FIS_SOLID)  */
	intin[0] = 0;                            /* vs_clip(off)             */
	ptsin[0] = ptsin[1] = ptsin[2] = ptsin[3] = 0;
	vdi(129, 2, 1, 0, h);

	for (r = 0; r < PX_H; r++) {
		for (c = 0; c < PX_W; c++) {
			intin[0] = px_pen[r][c];
			vdi(25, 0, 1, 0, h);     /* vsf_color                */
			ptsin[0] = (int16_t)(PX_X0 + c);
			ptsin[1] = (int16_t)(PX_Y0 + r);
			ptsin[2] = ptsin[0];
			ptsin[3] = ptsin[1];
			vdi(114, 2, 0, 0, h);    /* vr_recfl, 1x1            */
		}
	}
}

static void read_pixels(int16_t h)
{
	int r, c;

	for (r = 0; r < PX_H; r++) {
		for (c = 0; c < PX_W; c++) {
			intout[0] = intout[1] = POISON;
			ptsin[0] = (int16_t)(PX_X0 + c);
			ptsin[1] = (int16_t)(PX_Y0 + r);
			vdi(105, 1, 0, 0, h);    /* v_get_pixel              */
			px_get0[r][c] = intout[0];
			px_get1[r][c] = intout[1];
		}
	}
}

static void init_mfdb(struct mfdb *m, int16_t *buf, int16_t planes)
{
	memset(m, 0, sizeof *m);
	m->fd_addr    = buf;
	m->fd_w       = PX_W;
	m->fd_h       = PX_H;
	m->fd_wdwidth = PX_WDW;
	m->fd_stand   = 0;                       /* device-specific format    */
	m->fd_nplanes = planes;
}

/* ------------------------------------------------------------------ main */

int main(int argc, char **argv)
{
	const char *out_path = (argc > 1 && argv[1][0]) ? argv[1] : "SCRNINFO.TXT";
	const struct td_cookie *eddi = 0;
	uint32_t eddi_ret = 0;
	int eddi_called = 0, si_ran = 0;
	int16_t planes = 0, ncolors, w, h;
	int truecolor, pixel_ok = 0;
	int rc, i, r, c;
	size_t probe_bytes = 0;
	FILE *f;
	char tag[5];

	td_clock_read(&clk);
	td_jar_read(&jar);
	for (i = 0; i < jar.count; i++)
		if (jar.e[i].tag == TD_COOKIE_EDDI)
			eddi = &jar.e[i];

	rpt("SCRNINFO build %s -- EdDI / vq_scrninfo probe\n", TD_BUILD_ID);
	rpt("spec docs/api/vdi/eddi-vq_scrninfo.md\n");
	rpt("date %04u-%02u-%02u time %02u:%02u:%02u (Tgetdate %04X, "
	    "Tgettime %04X)\n",
	    1980 + (clk.date >> 9), (clk.date >> 5) & 15, clk.date & 31,
	    clk.time >> 11, (clk.time >> 5) & 63, (clk.time & 31) * 2,
	    clk.date, clk.time);
	rpt("Getrez %d\n", clk.rez);

	/* ---- 1. cookie jar */
	rpt("\n== 1. COOKIE JAR ==\n");
	rpt("_p_cookies = %08lX, %u entries%s, end entry value %lu\n",
	    (unsigned long)jar.jar_addr, jar.count,
	    jar.cut ? " (CUT: more entries than recorded)" : "",
	    (unsigned long)jar.end_value);
	for (i = 0; i < jar.count; i++) {
		tag_text(jar.e[i].tag, tag);
		rpt("  %2d  %08lX '%s'  value %08lX (%lu)\n", i,
		    (unsigned long)jar.e[i].tag, tag,
		    (unsigned long)jar.e[i].value,
		    (unsigned long)jar.e[i].value);
	}
	if (eddi)
		rpt("EdDI: PRESENT, value %08lX\n", (unsigned long)eddi->value);
	else
		rpt("EdDI: ABSENT\n");

	/* ---- 2. dispatcher, function 0 */
	rpt("\n== 2. EdDI DISPATCHER, FUNCTION 0 ==\n");
	if (!eddi) {
		rpt("skipped: no EdDI cookie\n");
	} else if (eddi->value == 0 || (eddi->value & 1)) {
		rpt("skipped: cookie value %08lX is not a callable address "
		    "(zero or odd)\n", (unsigned long)eddi->value);
	} else {
		eddi_ret = td_eddi_call0(eddi->value);
		eddi_called = 1;
		rpt("d0 after call = %08lX; low word (Pure C int) = %04X = %d\n",
		    (unsigned long)eddi_ret, (unsigned)(eddi_ret & 0xFFFF),
		    (int16_t)(eddi_ret & 0xFFFF));
	}

	/* ---- 3. workstation */
	rpt("\n== 3. WORKSTATION ==\n");
	rc = td_gem_open(&wk);
	if (rc != 0) {
		rpt("%s -- nothing further measured\n", td_gem_error_text(rc));
		goto write_out;
	}
	rpt("appl_init id %d, AES version global[0] %04X, graf_handle %d, "
	    "v_opnvwk handle %d\n", wk.ap_id, (uint16_t)td_gem_global()[0],
	    wk.phys_handle, wk.handle);
	rpt("work_in");
	for (i = 0; i < TD_WORK_IN_N; i++)
		rpt(" %d", wk.work_in[i]);
	rpt("\n");
	rpt_words("opnvwk_out", wk.work_out, 0, TD_WORK_OUT_N);
	rpt_words("opnvwk_pts", wk.work_pts, 0, TD_WORK_PTS_N);

	for (i = 0; i < 2; i++) {
		memset(intout, 0, sizeof intout);
		memset(ptsout, 0, sizeof ptsout);
		intin[0] = (int16_t)i;
		vdi(102, 0, 1, 0, wk.handle);            /* vq_extnd */
		memcpy(i ? ext1_out : ext0_out, intout, 45 * sizeof(int16_t));
		memcpy(i ? ext1_pts : ext0_pts, ptsout, 12 * sizeof(int16_t));
		if (i) { ext1_c2 = contrl[2]; ext1_c4 = contrl[4]; }
		else   { ext0_c2 = contrl[2]; ext0_c4 = contrl[4]; }
	}
	rpt("vq_extnd(0): CONTRL[2]=%d CONTRL[4]=%d\n", ext0_c2, ext0_c4);
	rpt_words("extnd0_out", ext0_out, 0, 45);
	rpt_words("extnd0_pts", ext0_pts, 0, 12);
	rpt("vq_extnd(1): CONTRL[2]=%d CONTRL[4]=%d\n", ext1_c2, ext1_c4);
	rpt_words("extnd1_out", ext1_out, 0, 45);
	rpt_words("extnd1_pts", ext1_pts, 0, 12);

	w       = (int16_t)(wk.work_out[0] + 1);
	h       = (int16_t)(wk.work_out[1] + 1);
	ncolors = wk.work_out[13];
	planes  = ext1_out[4];
	rpt("screen %dx%d, %d planes (vq_extnd(1)[4]), %d pens (DEV_TAB[13]), "
	    "lookup table %d (vq_extnd(1)[5])\n", w, h, planes, ncolors,
	    ext1_out[5]);

	/* vq_scrninfo, spec section 3.1: CONTRL 102/0/-/1/-/1/handle,
	 * INTIN[0] = 2.  Only with the cookie (spec section 0). */
	rpt("\n== 3b. vq_scrninfo (102/1, INTIN[0]=2) ==\n");
	if (!eddi) {
		rpt("skipped: no EdDI cookie (spec section 0: not available)\n");
	} else {
		for (i = 0; i < SI_OUT_N; i++)
			si_intout[i] = POISON;
		for (i = 0; i < 1024; i++)
			si_ptsout[i] = POISON;
		memset(si_contrl, 0, sizeof si_contrl);
		si_contrl[0] = 102;
		si_contrl[1] = 0;
		si_contrl[2] = POISON;
		si_contrl[3] = 1;
		si_contrl[4] = POISON;
		si_contrl[5] = 1;
		si_contrl[6] = wk.handle;
		si_intin[0]  = 2;
		td_abi_vdi(si_contrl, si_intin, si_ptsin, si_intout, si_ptsout);
		si_ran = 1;
		rpt("CONTRL after: [2]=%d/%04X [4]=%d/%04X (both were %04X going "
		    "in)\n", si_contrl[2], (uint16_t)si_contrl[2], si_contrl[4],
		    (uint16_t)si_contrl[4], POISON);
		rpt_words("work_out", si_intout, 0, SI_WORK_N);
		{
			int last = -1;

			for (i = SI_WORK_N; i < SI_OUT_N; i++)
				if (si_intout[i] != POISON)
					last = i;
			if (last < 0)
				rpt("INTOUT[273..%d] untouched (still %04X)\n",
				    SI_OUT_N - 1, POISON);
			else
				rpt("INTOUT written beyond 272: last changed "
				    "word %d\n", last);
			last = -1;
			for (i = 0; i < 1024; i++)
				if (si_ptsout[i] != POISON)
					last = i;
			if (last < 0)
				rpt("PTSOUT untouched\n");
			else
				rpt_words("ptsout", si_ptsout, 0, last + 1);
		}
		rpt("summary: format %d, clut %d, planes %d, colours %08lX, "
		    "line bytes %d, address %04X%04X, bits r/g/b %d/%d/%d, "
		    "alpha %d, genlock %d, unused %d, organisation %d/%04X\n",
		    si_intout[0], si_intout[1], si_intout[2],
		    ((unsigned long)(uint16_t)si_intout[3] << 16) |
		    (uint16_t)si_intout[4], si_intout[5],
		    (uint16_t)si_intout[6], (uint16_t)si_intout[7],
		    si_intout[8], si_intout[9], si_intout[10], si_intout[11],
		    si_intout[12], si_intout[13], si_intout[14],
		    (uint16_t)si_intout[14]);
	}

	/* ---- 4. pixel truth */
	rpt("\n== 4. PIXELS: VDI draw -> vro_cpyfm -> device-format MFDB ==\n");
	truecolor = (planes > 8);
	if (planes < 1 || planes > 32) {
		rpt("skipped: plane count %d out of range 1..32\n", planes);
		goto close_wk;
	}
	if (w < PX_X0 + PX_W || h < PX_Y0 + PX_H) {
		rpt("skipped: screen too small for the %dx%d area at %d,%d\n",
		    PX_W, PX_H, PX_X0, PX_Y0);
		goto close_wk;
	}
	pick_pens(ncolors, truecolor);
	if (npens == 0) {
		rpt("skipped: no pens (DEV_TAB[13] = %d)\n", ncolors);
		goto close_wk;
	}
	plan_pixels();
	rpt("area x %d..%d, y %d..%d; MFDB fd_w %d fd_h %d fd_wdwidth %d "
	    "fd_stand 0 fd_nplanes %d\n", PX_X0, PX_X0 + PX_W - 1, PX_Y0,
	    PX_Y0 + PX_H - 1, PX_W, PX_H, PX_WDW, planes);
	rpt("pens re-coloured with vs_color: %s\n",
	    truecolor ? "YES (>8 planes), restored afterwards"
	              : "NO (palette screen, the existing palette is used)");

	for (i = 0; i < PX_WORDS; i++) {
		px_save[i]  = (int16_t)FILL;
		px_probe[i] = (int16_t)FILL;
		px_check[i] = (int16_t)FILL;
	}
	memset(&mf_screen, 0, sizeof mf_screen);   /* fd_addr 0 = screen */
	init_mfdb(&mf_save, px_save, planes);
	init_mfdb(&mf_probe, px_probe, planes);
	init_mfdb(&mf_check, px_check, planes);

	screen_lock(1);
	mouse(0);

	/* save the area */
	cpyfm(wk.handle, &mf_screen, PX_X0, PX_Y0, &mf_save, 0, 0, PX_W, PX_H);

	for (i = 0; i < npens; i++) {
		vq_color(wk.handle, pens[i], 0, pen_req[i]);
		pen_old[i][0] = pen_req[i][1];
		pen_old[i][1] = pen_req[i][2];
		pen_old[i][2] = pen_req[i][3];
		if (truecolor) {
			pen_set[i][0] = tc_rgb[i][0];
			pen_set[i][1] = tc_rgb[i][1];
			pen_set[i][2] = tc_rgb[i][2];
			vs_color(wk.handle, pens[i], pen_set[i]);
			vq_color(wk.handle, pens[i], 0, pen_req[i]);
		}
		vq_color(wk.handle, pens[i], 1, pen_real[i]);
	}

	draw_pixels(wk.handle);
	cpyfm(wk.handle, &mf_screen, PX_X0, PX_Y0, &mf_probe, 0, 0, PX_W, PX_H);
	read_pixels(wk.handle);

	/* put everything back: the screen area, then the pen colours */
	cpyfm(wk.handle, &mf_save, 0, 0, &mf_screen, PX_X0, PX_Y0, PX_W, PX_H);
	/* and read the area once more, to show the restore took */
	cpyfm(wk.handle, &mf_screen, PX_X0, PX_Y0, &mf_check, 0, 0, PX_W, PX_H);
	if (truecolor)
		for (i = 0; i < npens; i++)
			vs_color(wk.handle, pens[i], pen_old[i]);

	mouse(1);
	screen_lock(0);
	pixel_ok = 1;

	rpt("pens (vq_color flag 0 = requested, flag 1 = realized; "
	    "[rc] = INTOUT[0]):\n");
	for (i = 0; i < npens; i++) {
		rpt("  pen %3d", pens[i]);
		if (truecolor)
			rpt("  old %4d,%4d,%4d  set %4d,%4d,%4d", pen_old[i][0],
			    pen_old[i][1], pen_old[i][2], pen_set[i][0],
			    pen_set[i][1], pen_set[i][2]);
		rpt("  req [%d] %4d,%4d,%4d  real [%d] %4d,%4d,%4d\n",
		    pen_req[i][0], pen_req[i][1], pen_req[i][2], pen_req[i][3],
		    pen_real[i][0], pen_real[i][1], pen_real[i][2],
		    pen_real[i][3]);
	}

	rpt("pixel map (row,col: pen drawn | v_get_pixel INTOUT[0] INTOUT[1]):\n");
	for (r = 0; r < PX_H; r++)
		for (c = 0; c < PX_W; c++)
			rpt("  %d,%2d: pen %3d | %04X %04X\n", r, c, px_pen[r][c],
			    (uint16_t)px_get0[r][c], (uint16_t)px_get1[r][c]);

	/* device form size: fd_wdwidth words x 2 bytes x planes x rows */
	probe_bytes = (size_t)PX_WDW * 2 * (size_t)planes * PX_H;
	rpt("device-format bytes after vro_cpyfm (%lu bytes = fd_wdwidth %d "
	    "* 2 * planes %d * rows %d; buffer pre-filled %04X):\n",
	    (unsigned long)probe_bytes, PX_WDW, planes, PX_H, FILL);
	{
		const uint8_t *b = (const uint8_t *)px_probe;
		size_t k;

		for (k = 0; k < probe_bytes; k++) {
			if (k % 16 == 0)
				rpt("  bytes +%04lX:", (unsigned long)k);
			rpt(" %02X", b[k]);
			if (k % 16 == 15 || k + 1 == probe_bytes)
				rpt("\n");
		}
		for (k = 0; k + 1 < probe_bytes; k += 2) {
			if (k % 16 == 0)
				rpt("  BEwords +%04lX:", (unsigned long)k);
			rpt(" %04X", (unsigned)((b[k] << 8) | b[k + 1]));
			if (k % 16 == 14 || k + 2 >= probe_bytes)
				rpt("\n");
		}
		/* anything written past the computed form size? */
		for (k = probe_bytes; k < sizeof px_probe; k++)
			if (b[k] != 0xA5)
				break;
		if (k < sizeof px_probe)
			rpt("NOTE: buffer changed beyond %lu bytes (first at "
			    "+%04lX)\n", (unsigned long)probe_bytes,
			    (unsigned long)k);
		else
			rpt("buffer beyond %lu bytes untouched\n",
			    (unsigned long)probe_bytes);
		rpt("screen area restore: %s\n",
		    memcmp(px_save, px_check, probe_bytes) == 0
		    ? "VERIFIED (re-read equals the saved copy)"
		    : "MISMATCH (re-read differs from the saved copy)");
	}

close_wk:
	td_gem_close(&wk);

write_out:
	rpt("\nend of report%s\n", rpt_cut ? " (REPORT CUT)" : "");
	f = fopen(out_path, "wb");
	if (!f || fwrite(rpt_buf, 1, rpt_len, f) != rpt_len) {
		if (f)
			fclose(f);
		printf("SCRNINFO: cannot write %s\n", out_path);
		return 1;
	}
	if (fclose(f) != 0) {
		printf("SCRNINFO: cannot write %s\n", out_path);
		return 1;
	}

	if (eddi && eddi_called && si_ran)
		printf("SCRNINFO: EdDI %08lX version %04X, %d planes, "
		       "scrninfo fmt %d clut %d org %04X, pixels %s -> %s\n",
		       (unsigned long)eddi->value,
		       (unsigned)(eddi_ret & 0xFFFF), planes, si_intout[0],
		       si_intout[1], (uint16_t)si_intout[14],
		       pixel_ok ? "ok" : "skipped", out_path);
	else
		printf("SCRNINFO: %s, %d planes, pixels %s -> %s\n",
		       !eddi ? "no EdDI"
		       : eddi_called ? "EdDI present, no workstation"
		       : "EdDI present (dispatcher not called)", planes,
		       pixel_ok ? "ok" : "skipped", out_path);
	return 0;
}
