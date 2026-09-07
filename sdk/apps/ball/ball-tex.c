/*
 * ball-tex.c -- the ball's skin: read it from a file, or draw it.
 *
 * See ball-tex.h for the NTX1 file layout and for why a texel is a material
 * index rather than a colour. This file is machine-independent -- it reads
 * bytes and writes bytes, and the only system calls it makes are GEMDOS file
 * calls, which are the same three calls on an Atari as they are here.
 *
 * ---------------------------------------------------------------------------
 * Why there is a generator at all
 * ---------------------------------------------------------------------------
 * The artwork the demo is meant to wear does not exist yet as an asset, and
 * the demo must not be undemonstrable until it does. So the texture is a
 * PIPELINE, not a picture: tools/png2ntx.py turns artwork into C:\BALL.NTX,
 * tex_build() prefers that file, and when it is not there the generator below
 * draws a texture from geometry -- a white ball with a wide red equatorial
 * band, black pinstripes, and two white panels in the band carrying marks and
 * lettering. It is meant to be looked at, not to stand in for something: what
 * it draws is what the demo shows until the artwork lands.
 *
 * The marks are deliberately NOT reproductions of anybody's logo. The
 * three-stroke mark is three splaying bars; the berry is a cluster of circles
 * under two leaves. Real trademarks are an asset decision, and they arrive
 * through the PNG, not through this file.
 */
#include <mint/osbind.h>
#include <string.h>

#include "ball-tex.h"

/* What the generator draws, and the size the file loader prefers. */
#define GEN_W		256
#define GEN_H		128

/*
 * The four materials. A texture file brings its own colours; these are the
 * ones the generator uses and the ones any material a file leaves undefined
 * falls back to, so the screen palette always has four complete ramps.
 */
#define MAT_WHITE	0
#define MAT_RED		1
#define MAT_BLACK	2
#define MAT_GREEN	3

static const unsigned char def_rgb[TEX_MAT_MAX][3] = {
	{ 236, 238, 240 },	/* white, a shade cool so the highlight reads */
	{ 206,  38,  32 },	/* red					      */
	{  22,  22,  26 },	/* black				      */
	{  58, 150,  58 },	/* green				      */
};

/* ------------------------------------------------------------------------ */
/* A 5x7 block font, ASCII 0x20 .. 0x5F                                      */
/*                                                                           */
/* It began as the nine letters the ball's two panels spell, and it is now    */
/* the ONE font in the demo: milestone 2b's room draws its corner lines and   */
/* its status readout from this same table (ball.c has its own renderer, be-  */
/* cause it writes into the screen's index buffer rather than into a texture, */
/* but not its own letterforms). The nine original glyphs are byte for byte   */
/* what they were, so the ball's texture is unchanged.                        */
/*                                                                           */
/* The range is contiguous from space to underscore, so a character IS its    */
/* index -- no search, and every unrepresentable character is caught by one   */
/* range test rather than by falling off the end of a string.                 */
/*                                                                           */
/* Each row is five bits, bit 4 leftmost.                                     */
/* ------------------------------------------------------------------------ */
const unsigned char font_bits[FONT_GLYPHS][GLYPH_H] = {
	{ 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 },	/* space */
	{ 0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04 },	/* ! */
	{ 0x0A, 0x0A, 0x0A, 0x00, 0x00, 0x00, 0x00 },	/* " */
	{ 0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A },	/* # */
	{ 0x04, 0x0F, 0x14, 0x0E, 0x05, 0x1E, 0x04 },	/* $ */
	{ 0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03 },	/* % */
	{ 0x08, 0x14, 0x14, 0x08, 0x15, 0x12, 0x0D },	/* & */
	{ 0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00 },	/* ' */
	{ 0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02 },	/* ( */
	{ 0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08 },	/* ) */
	{ 0x00, 0x04, 0x15, 0x0E, 0x15, 0x04, 0x00 },	/* * */
	{ 0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00 },	/* + */
	{ 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x08 },	/* , */
	{ 0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00 },	/* - */
	{ 0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C },	/* . */
	{ 0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x00 },	/* / */
	{ 0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E },	/* 0 */
	{ 0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E },	/* 1 */
	{ 0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F },	/* 2 */
	{ 0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E },	/* 3 */
	{ 0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02 },	/* 4 */
	{ 0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E },	/* 5 */
	{ 0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E },	/* 6 */
	{ 0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08 },	/* 7 */
	{ 0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E },	/* 8 */
	{ 0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C },	/* 9 */
	{ 0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00 },	/* : */
	{ 0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x04, 0x08 },	/* ; */
	{ 0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02 },	/* < */
	{ 0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00 },	/* = */
	{ 0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08 },	/* > */
	{ 0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04 },	/* ? */
	{ 0x0E, 0x11, 0x01, 0x0D, 0x15, 0x15, 0x0E },	/* @ */
	{ 0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11 },	/* A */
	{ 0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E },	/* B */
	{ 0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E },	/* C */
	{ 0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C },	/* D */
	{ 0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F },	/* E */
	{ 0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10 },	/* F */
	{ 0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F },	/* G */
	{ 0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11 },	/* H */
	{ 0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x1F },	/* I */
	{ 0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C },	/* J */
	{ 0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11 },	/* K */
	{ 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F },	/* L */
	{ 0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11 },	/* M */
	{ 0x11, 0x19, 0x19, 0x15, 0x13, 0x13, 0x11 },	/* N */
	{ 0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E },	/* O */
	{ 0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10 },	/* P */
	{ 0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D },	/* Q */
	{ 0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11 },	/* R */
	{ 0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E },	/* S */
	{ 0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04 },	/* T */
	{ 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E },	/* U */
	{ 0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04 },	/* V */
	{ 0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11 },	/* W */
	{ 0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11 },	/* X */
	{ 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04 },	/* Y */
	{ 0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F },	/* Z */
	{ 0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E },	/* [ */
	{ 0x00, 0x10, 0x08, 0x04, 0x02, 0x01, 0x00 },	/* backslash */
	{ 0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E },	/* ] */
	{ 0x04, 0x0A, 0x11, 0x00, 0x00, 0x00, 0x00 },	/* ^ */
	{ 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F },	/* _ */
};

const unsigned char *font_glyph(char c)
{
	unsigned u = (unsigned char)c;

	if (u < FONT_FIRST || u > FONT_LAST)
		u = (unsigned)'?';
	return font_bits[u - FONT_FIRST];
}

/* ------------------------------------------------------------------------ */
/* Drawing primitives, all writing material indices into the first w columns  */
/* of each row. tex_finish() below turns that into what the renderer reads.   */
/* ------------------------------------------------------------------------ */

static unsigned char *tex_row(ball_tex *t, int y)
{
	return t->px + (long)y * (long)t->stride;
}

static void put_px(ball_tex *t, int x, int y, int m)
{
	if (x < 0 || y < 0 || x >= t->w || y >= t->h)
		return;
	tex_row(t, y)[x] = (unsigned char)m;
}

static void fill_rect(ball_tex *t, int x0, int y0, int x1, int y1, int m)
{
	int x, y;

	if (x0 < 0)
		x0 = 0;
	if (y0 < 0)
		y0 = 0;
	if (x1 > t->w)
		x1 = t->w;
	if (y1 > t->h)
		y1 = t->h;
	for (y = y0; y < y1; y++) {
		unsigned char *r = tex_row(t, y);
		for (x = x0; x < x1; x++)
			r[x] = (unsigned char)m;
	}
}

static void fill_disc(ball_tex *t, int cx, int cy, int r, int m)
{
	int x, y, rr = r * r;

	for (y = cy - r; y <= cy + r; y++)
		for (x = cx - r; x <= cx + r; x++) {
			int dx = x - cx, dy = y - cy;

			if (dx * dx + dy * dy <= rr)
				put_px(t, x, y, m);
		}
}

/*
 * An ellipse with semi-axes (a, b) whose long axis points along the unit
 * vector (ux, uy) -- a leaf. The test is the ellipse equation in the leaf's
 * own frame, so no trigonometry is needed anywhere in this file.
 */
static void fill_leaf(ball_tex *t, int cx, int cy, float a, float b,
		      float ux, float uy, int m)
{
	int span = (int)(a > b ? a : b) + 1;
	int x, y;

	for (y = cy - span; y <= cy + span; y++)
		for (x = cx - span; x <= cx + span; x++) {
			float dx = (float)(x - cx);
			float dy = (float)(y - cy);
			float du = (dx * ux + dy * uy) / a;
			float dv = (-dx * uy + dy * ux) / b;

			if (du * du + dv * dv <= 1.0f)
				put_px(t, x, y, m);
		}
}

static void put_text(ball_tex *t, const char *s, int x, int y, int scale,
		     int m)
{
	int gx, gy, sx, sy;

	for (; *s; s++) {
		const unsigned char *g = font_glyph(*s);

		for (gy = 0; gy < GLYPH_H; gy++) {
			unsigned bits = g[gy];

			for (gx = 0; gx < GLYPH_W; gx++) {
				if (!(bits & (1u << (GLYPH_W - 1 - gx))))
					continue;
				for (sy = 0; sy < scale; sy++)
					for (sx = 0; sx < scale; sx++)
						put_px(t,
						       x + gx * scale + sx,
						       y + gy * scale + sy,
						       m);
			}
		}
		x += (GLYPH_W + 1) * scale;
	}
}

/* Width in texels the string above will occupy. */
static int text_w(const char *s, int scale)
{
	int n = 0;

	while (*s++)
		n++;
	return n > 0 ? n * (GLYPH_W + 1) * scale - scale : 0;
}

/* ------------------------------------------------------------------------ */
/* The two panel marks                                                       */
/* ------------------------------------------------------------------------ */

/*
 * Three bars rising from a common baseline: the middle one straight, the
 * outer two splaying away from it and thickening as they fall. Pure geometry
 * -- the shape is described by two ratios, not by a bitmap.
 */
static void draw_bars(ball_tex *t, int cx, int y0, int y1)
{
	int y, h = y1 - y0;

	if (h < 4)
		return;
	fill_rect(t, cx - 2, y0, cx + 3, y1, MAT_BLACK);
	for (y = y0 + h / 8; y < y1; y++) {
		float f = (float)(y - y0) / (float)h;
		int inner = 5 + (int)(5.0f * f);
		int thick = 2 + (int)(2.0f * f);

		fill_rect(t, cx - inner - thick, y, cx - inner, y + 1,
			  MAT_BLACK);
		fill_rect(t, cx + inner + 1, y, cx + inner + 1 + thick, y + 1,
			  MAT_BLACK);
	}
}

/* A cluster of berries under two leaves. */
static void draw_berry(ball_tex *t, int cx, int cy)
{
	/* Six drupes in three rows, widest at the top and coming to a point at
	 * the bottom, spaced a little more than two radii apart so a hairline
	 * of the panel shows between neighbours -- that separation is what
	 * makes a cluster of circles read as a berry rather than as a blob. */
	static const signed char drupe[6][2] = {
		{ -7, -5 }, {  0, -5 }, {  7, -5 },
		{ -4,  1 }, {  4,  1 },
		{  0,  7 },
	};
	int i;

	fill_leaf(t, cx - 5, cy - 11, 6.0f, 2.5f, -0.82f, -0.57f, MAT_GREEN);
	fill_leaf(t, cx + 5, cy - 11, 6.0f, 2.5f, 0.82f, -0.57f, MAT_GREEN);
	for (i = 0; i < 6; i++)
		fill_disc(t, cx + drupe[i][0], cy + drupe[i][1], 3, MAT_RED);
}

/* ------------------------------------------------------------------------ */
/* The generated texture                                                     */
/* ------------------------------------------------------------------------ */
static void tex_generate(ball_tex *t)
{
	int i;
	int bt, bb, pin, pw, pa, pb;

	t->w = GEN_W;
	t->h = GEN_H;
	t->stride = 2 * GEN_W;
	t->nmat = TEX_MAT_MAX;
	for (i = 0; i < TEX_MAT_MAX; i++) {
		t->rgb[i][0] = def_rgb[i][0];
		t->rgb[i][1] = def_rgb[i][1];
		t->rgb[i][2] = def_rgb[i][2];
	}

	/* Everything as a fraction of the texture, so the shape survives a
	 * change of size: a band across the equator between 5/16 and 11/16 of
	 * the latitude -- which covers the middle 56 % of the ball's height --
	 * and two panels a quarter of the circumference wide, half a turn
	 * apart, so half the equator is red and half is panel. The sizes come
	 * off the reference render, converted through the fact that one texel
	 * is about 3.7 screen pixels at the middle of a 306-pixel ball: a word
	 * that should span a third of the ball is 29 texels, so the lettering
	 * is drawn at one texel per font pixel, not two. */
	bt = t->h * 5 / 16;
	bb = t->h * 11 / 16;
	pin = t->h / 64;
	if (pin < 1)
		pin = 1;
	pw = t->w / 4;
	pa = 0;
	pb = t->w / 2;

	fill_rect(t, 0, 0, t->w, t->h, MAT_WHITE);
	fill_rect(t, 0, bt, t->w, bb, MAT_RED);
	fill_rect(t, 0, bt - pin, t->w, bt, MAT_BLACK);
	fill_rect(t, 0, bb, t->w, bb + pin, MAT_BLACK);

	for (i = 0; i < 2; i++) {
		int x0 = i ? pb : pa;

		fill_rect(t, x0, bt, x0 + pw, bb, MAT_WHITE);
		fill_rect(t, x0, bt, x0 + 2, bb, MAT_BLACK);
		fill_rect(t, x0 + pw - 2, bt, x0 + pw, bb, MAT_BLACK);
	}

	/* Panel one: the three bars over a word. */
	draw_bars(t, pa + pw / 2, bt + 8, bt + 30);
	put_text(t, "ATARI", pa + (pw - text_w("ATARI", 1)) / 2, bt + 33, 1,
		 MAT_BLACK);

	/* Panel two: the berry over a word, the same size as panel one's. */
	draw_berry(t, pb + pw / 2, bt + 21);
	put_text(t, "NEWTOS", pb + (pw - text_w("NEWTOS", 1)) / 2, bt + 33, 1,
		 MAT_BLACK);

	t->source = "generated";
}

/* ------------------------------------------------------------------------ */
/* Turning material indices into what the frame loop reads                   */
/* ------------------------------------------------------------------------ */

/*
 * Copy each row after itself so a rotated column never has to wrap. No texel
 * is converted, scaled or re-quantised -- the byte the file holds is the byte
 * the renderer reads. The one exception is a safety clamp: a texel naming a
 * hue the file did not declare is forced to hue 0, so a truncated or corrupt
 * file cannot make the renderer address outside the ball's palette slice. For
 * a valid file the clamp never fires and nothing changes.
 */
static void tex_finish(ball_tex *t)
{
	int x, y;

	for (y = 0; y < t->h; y++) {
		unsigned char *r = tex_row(t, y);

		for (x = 0; x < t->w; x++)
			if (r[x] >= t->nmat)
				r[x] = 0;
		memcpy(r + t->w, r, (size_t)t->w);
	}
}

/* ------------------------------------------------------------------------ */
/* The file                                                                  */
/* ------------------------------------------------------------------------ */
static int be16(const unsigned char *p)
{
	return ((int)p[0] << 8) | (int)p[1];
}

static int is_pow2(int v)
{
	return v > 0 && (v & (v - 1)) == 0;
}

/*
 * Read `path` into `t`. Returns 0 on success and a negative value when the
 * file is absent or does not describe a texture this build can map -- in
 * which case `t` has been left in a state the caller must overwrite.
 */
static int tex_read(ball_tex *t, unsigned char *buf, const char *path)
{
	unsigned char hdr[12 + 3 * TEX_MAT_MAX];
	long fd, n;
	int w, h, nmat, i, y;

	fd = Fopen(path, 0);
	if (fd < 0)
		return -1;

	n = Fread((short)fd, (long)sizeof hdr, hdr);
	if (n < 12) {
		(void)Fclose((short)fd);
		return -2;
	}
	if (hdr[0] != 'N' || hdr[1] != 'T' || hdr[2] != 'X' || hdr[3] != '1') {
		(void)Fclose((short)fd);
		return -3;
	}

	w = be16(hdr + 4);
	h = be16(hdr + 6);
	nmat = hdr[8];
	if (!is_pow2(w) || !is_pow2(h) || w > 256 || h > 128 ||
	    nmat < 1 || nmat > TEX_MAT_MAX ||
	    2L * (long)w * (long)h > (long)TEX_MAX_BYTES ||
	    n < (long)(12 + 3 * nmat)) {
		(void)Fclose((short)fd);
		return -4;
	}

	t->w = w;
	t->h = h;
	t->stride = 2 * w;
	t->nmat = nmat;
	for (i = 0; i < TEX_MAT_MAX; i++) {
		if (i < nmat) {
			t->rgb[i][0] = hdr[12 + 3 * i];
			t->rgb[i][1] = hdr[13 + 3 * i];
			t->rgb[i][2] = hdr[14 + 3 * i];
		} else {
			t->rgb[i][0] = def_rgb[i][0];
			t->rgb[i][1] = def_rgb[i][1];
			t->rgb[i][2] = def_rgb[i][2];
		}
	}
	t->px = buf;

	/* The header may be followed by fewer material triples than the four
	 * read above, so rewind to exactly where the pixels start. */
	if (Fseek((long)(12 + 3 * nmat), (short)fd, 0) < 0) {
		(void)Fclose((short)fd);
		return -5;
	}

	/* Straight into the first half of each doubled row. */
	for (y = 0; y < h; y++) {
		if (Fread((short)fd, (long)w, tex_row(t, y)) != (long)w) {
			(void)Fclose((short)fd);
			return -6;
		}
	}
	(void)Fclose((short)fd);
	return 0;
}

void tex_build(ball_tex *t, unsigned char *buf, const char *path)
{
	t->px = buf;
	if (tex_read(t, buf, path) == 0) {
		t->source = path;
	} else {
		t->px = buf;
		tex_generate(t);
	}
	tex_finish(t);
}
