/*
 * ball-tex.h -- the sphere's texture: where it comes from and how it is held.
 *
 * The renderer in ball.c maps an EQUIRECTANGULAR texture onto the ball: the
 * horizontal axis is 360 degrees of longitude, the vertical axis 180 degrees
 * of latitude, row 0 the north pole. That is the one layout in which the
 * texture is locally isotropic at the equator when it is twice as wide as it
 * is tall, which is why the default is 256 x 128.
 *
 * ---------------------------------------------------------------------------
 * The pixel format, and why it is not RGB
 * ---------------------------------------------------------------------------
 * A texel is not a colour, it is a HUE: one of at most four surface colours
 * the ball is made of. Lighting it is not a multiply -- a palette index times
 * 0.7 means nothing -- it is a lookup:
 *
 *      screen index = shade_lut[light level][texel]
 *
 * The screen palette holds a ramp of entries per hue, from unlit through the
 * hue's own colour to the white of the specular highlight, and ball.c builds
 * that table once at start-up. BOTH machines take this path: the demo is
 * meant to be a benchmark of a Pi against an Atari TT, and it can only be one
 * if the two are doing the same work per pixel. The only thing that differs
 * is what plat_blit() does with the finished byte.
 *
 * Nothing here converts colours at run time. The file carries indices and the
 * palette they belong to, and the loader reads both as they are -- an Atari TT
 * with a NOVA card in 640x480x256 must not have to pay for a quantisation the
 * host already did. The one thing done to the bytes is to copy each row after
 * itself (below); no byte's value is changed.
 *
 * ---------------------------------------------------------------------------
 * The palette split -- the contract with the rest of the demo
 * ---------------------------------------------------------------------------
 * There are 256 palette entries for the whole picture, so the texture cannot
 * be allowed to spend them freely. It gets a fixed slice, and it gets it as
 * HUES rather than as colours, which is what stops it from ever addressing an
 * entry outside its own slice: the texture names a hue, ball.c's shade table
 * names the entry. The split, in full, is stated once in ball.c's palette
 * section and is
 *
 *      0 ..  23   backdrop
 *     24 ..  95   floor and its shadow
 *     96 .. 119   the grid room's glowing lines, and the lettering drawn with
 *                 them: milestone 2b spent 24 of the 32 entries reserved for
 *                 it here
 *    120 .. 127   still reserved, still black, still used by nothing
 *    128 .. 255   the ball: TEX_MAT_MAX hues, unevenly split into shades
 *
 * The reserved block is why 2b did not have to re-quantise this texture, or
 * anything else pre-computed from this palette -- and why the floor
 * reflection, which needs the ball DARKENED, spends no entry at all: it reads
 * the same ramps at a lower light level (ball.c, build_refl_lut).
 *
 * ---------------------------------------------------------------------------
 * Rows are stored TWICE
 * ---------------------------------------------------------------------------
 * Rotating the ball about its polar axis is, after the spherical projection,
 * nothing but adding a constant to the texture column (see ball.c). Making
 * that addition wrap would cost a mask per pixel; storing every row twice
 * end-to-end costs 32 KB and makes it cost nothing -- a column base in
 * [0, w) plus an offset in [0, w) is always inside the doubled row. Hence
 * `stride == 2 * w` and a texel offset that fits in 16 bits as long as
 * 2 * w * h <= 65536.
 *
 * ---------------------------------------------------------------------------
 * The file format: NTX1
 * ---------------------------------------------------------------------------
 * tex_build() looks for the texture in a file first and generates one only if
 * that fails, so the real artwork can be dropped in later without the
 * renderer changing by a line. The file is written by tools/png2ntx.py.
 *
 * All multi-byte fields are BIG-endian and are read a byte at a time, so the
 * same file is valid for the AArch64 build and for a future big-endian m68k
 * one.
 *
 *   offset  size   field
 *   0       4      magic, "NTX1"
 *   4       2      width in texels   -- a power of two, <= 256
 *   6       2      height in texels  -- a power of two, <= 128
 *   8       1      material count, 1 .. 4
 *   9       1      flags, 0
 *   10      2      reserved, 0
 *   12      3*n    material colours, r, g, b per material
 *   12+3n   w*h    one byte per texel: the material index, < material count
 *
 *   constraint: 2 * width * height <= 65536 (the 16-bit texel offset above)
 *   pixels are row-major, row 0 = north pole, column 0 = longitude -180 deg
 *
 * A file that is missing, short, or fails any of those checks is not an
 * error: the demo says so in its log and draws the generated texture instead.
 */
#ifndef BALL_TEX_H
#define BALL_TEX_H

/* ------------------------------------------------------------------------ */
/* The 5x7 block font                                                        */
/*                                                                           */
/* One font for the whole demo: the lettering on the ball's panels and the    */
/* room's corner lines and status readout are the same glyphs. The table is   */
/* contiguous ASCII from space to underscore, so a character is its own       */
/* index; font_glyph() does the range check and answers '?' outside it.      */
/* ------------------------------------------------------------------------ */
#define GLYPH_W		5
#define GLYPH_H		7
#define FONT_FIRST	0x20
#define FONT_LAST	0x5F
#define FONT_GLYPHS	(FONT_LAST - FONT_FIRST + 1)

extern const unsigned char font_bits[FONT_GLYPHS][GLYPH_H];

/* The seven rows of `c`, or of '?' when the font has no such character. */
const unsigned char *font_glyph(char c);

/* Hues the screen palette can hold a shade ramp for. Four, because four ramps
 * long enough to shade a 300-pixel sphere without banding is what half a
 * 256-entry palette buys; see the header comment. */
#define TEX_MAT_MAX	4

/* The largest texture the 16-bit texel offset allows, doubled rows included.
 * The caller owns this buffer; tex_build() never allocates. */
#define TEX_MAX_BYTES	65536

typedef struct {
	int w;			/* texels across, 360 degrees of longitude   */
	int h;			/* texels down, 180 degrees of latitude	     */
	int stride;		/* 2 * w -- every row is stored twice	     */
	int nmat;		/* hues the texture really uses		     */
	unsigned char *px;	/* stride * h texels, each a hue index	     */
	unsigned char rgb[TEX_MAT_MAX][3];	/* every material's colour   */
	const char *source;	/* where it came from, for the log	     */
} ball_tex;

/*
 * Fill `t` from `path`, or -- if that file is absent or unusable -- with the
 * generated texture. `buf` must be TEX_MAX_BYTES large and outlive `t`.
 * Always succeeds; `t->source` says which of the two happened.
 */
void tex_build(ball_tex *t, unsigned char *buf, const char *path);

#endif /* BALL_TEX_H */
