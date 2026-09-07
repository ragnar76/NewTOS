/*
 * ball.c -- BALL.NAP, the NewTOS bouncing-ball demo, milestone 2a.
 *
 * A ball falls, bounces, rolls along the floor and off the side walls, and
 * casts a shadow that shrinks and fades as it rises. Milestone 2a replaces
 * milestone 1's flat shaded sphere with a TEXTURED one: an equirectangular
 * texture is wrapped onto the sphere, the sphere spins about its polar axis
 * at the rate the ball is rolling, and one light -- a diffuse term and a
 * specular highlight -- shades the result. Where the texture comes from is
 * ball-tex.c's business; this file only maps it.
 *
 * Still exactly ONE ball, and still no motion trail: the illustration's row
 * of half-transparent spheres is the illustrator's way of drawing movement,
 * not something the program does. The wireframe room, the floor reflection
 * and the status readout are milestone 2b and are not here.
 *
 * ---------------------------------------------------------------------------
 * How it draws, and why it draws that way
 * ---------------------------------------------------------------------------
 * The demo is meant to run on two machines from this one source: NewTOS on a
 * Raspberry Pi 4 (1920x1080, 32 bits per pixel) and TOS on an Atari TT with a
 * NOVA card (640x480, 8 bits per pixel). Both punish the same mistake, for
 * different reasons: on the TT every byte written to the card crosses the VME
 * bus at about 5 MB/s, and on the Pi the framebuffer is mapped Normal
 * Non-Cacheable into the process (ADR-023), so a single-pixel store goes
 * straight to DRAM. Two rules follow, and they are built in from the first
 * line rather than optimised in later:
 *
 *   1. Each frame is composed in ordinary memory and written to the screen
 *      exactly once. The rectangle composed is big enough to hold the ball's
 *      old and new position, so the background never has to be restored in a
 *      separate pass -- it is simply drawn again underneath.
 *   2. The screen is never cleared. A full screen costs 300 KB on the TT
 *      (16 frames per second would be the ceiling) and 8 MB on the Pi. Only
 *      the dirty rectangle is touched, from the first frame on -- the one
 *      exception being the very first paint, which has to put the background
 *      on the screen once (twice with two screen pages).
 *
 * The buffer holds palette INDICES, one byte per pixel, not colours: that is
 * the format the TT can blit unchanged, and on the Pi it halves the bytes
 * read out of cache while the platform layer converts. Everything about the
 * pixel format lives in plat-newtos.c; see ball.h for the seam.
 *
 * The ball is not projected per frame either. Its size, its shading and the
 * texel every one of its pixels samples at rest are all fixed, so they are
 * worked out once at start-up into a SPHERE MAP (build_sphere_map below) with
 * a per-row coverage run. What is left for the frame loop is the spin, and
 * the spin turns out to be one addition per pixel -- see the sphere map's own
 * commentary for why. That keeps the square root and the arc sine -- there is
 * no libm here; both are in this file -- out of the frame loop entirely.
 *
 * Every size is derived from the screen the VDI reports: the ball's diameter
 * is 16 % of the screen width, the floor and horizon are fractions of the
 * height, gravity is expressed in screen heights per second squared. The
 * picture is therefore the same picture at 640x480 and at 1920x1080.
 *
 * Physics runs on a fixed time step of exactly one 200 Hz system tick, driven
 * by the tick counter rather than by the frame rate, so the bounce is the
 * same bounce whatever the machine manages to draw.
 */
#include <mint/osbind.h>
#include <mint/sysvars.h>
#include <string.h>

#include "ball.h"
#include "ball-tex.h"

/* ------------------------------------------------------------------------ */
/* Palette layout -- the split, in one place, for the whole demo             */
/*                                                                           */
/*    0 ..  23   backdrop: 24 bands from the top of the screen to the horizon */
/*   24 ..  95   floor: 12 bands x 6 shadow levels (level 0 = no shadow), so  */
/*               a shadowed floor pixel is simply "floor band + level"       */
/*   96 .. 127   RESERVED. Milestone 2b's grid room, floor reflection and HUD */
/*               get these 32 entries. They are set to black here and used by */
/*               nothing, on purpose: a palette that has to be re-cut in 2b   */
/*               invalidates the texture, the sphere map and every pre-shaded */
/*               thing built from it, so the room is left now.               */
/*  128 .. 255   the ball: TEX_MAT_MAX hues, split UNEVENLY into shade ramps  */
/*               (see hue_len). A big white sphere shows every step of its    */
/*               gradient and gets the most; the two hues that only appear as */
/*               small marks on the texture get the fewest.                  */
/*                                                                           */
/* Milestone 1 spent 64 entries on one ball colour. A textured ball needs a   */
/* ramp per hue, and the four cost 128; the backdrop and the floor pay for it */
/* with coarser gradients and get the difference back by being ORDERED        */
/* DITHERED (build_dither below) -- 24 dithered steps read smoother than 64   */
/* undithered ones. This is the layout an 8-bit palette machine forces, so it */
/* is the layout both targets use.                                           */
/* ------------------------------------------------------------------------ */
#define SKY_N		24
#define SKY_BASE	0
#define FLOOR_N		12
#define SHADOW_N	6
#define FLOOR_BASE	24
#define RSVD_BASE	96	/* milestone 2b's 32 entries		     */
#define RSVD_N		32
#define BALL_BASE	128
#define BALL_N		128	/* 128 .. 255				     */

/*
 * Shading is a LOOKUP, never a multiply. Scaling a palette index by a light
 * factor is meaningless, and doing the arithmetic in RGB would make the Pi and
 * the Atari do different work per pixel -- which would ruin the one thing this
 * demo is also meant to be, a like-for-like measurement of the two machines.
 * So the renderer works in BALL_LEVELS light levels, the texture names a hue,
 * and the two meet in a table:
 *
 *      screen index = shade_lut[level * TEX_MAT_MAX + hue]
 *
 * The table is 192 bytes and lives in L1 on anything. It also decouples the
 * palette layout from the renderer completely: hue_len below can be re-cut,
 * or milestone 2b can move the whole ball slice, without draw_ball changing
 * by a line.
 *
 * BALL_LEVELS is the LONGEST ramp, not the average, so the hue that needs the
 * resolution actually gets it; a shorter ramp is sampled out of the same
 * levels and simply repeats entries.
 */
#define BALL_LEVELS	48
static const unsigned char hue_len[TEX_MAT_MAX] = { 48, 40, 20, 20 };
/* The four blocks above must tile 0..255 exactly, and the ramps must fill the
 * ball's block exactly. Getting either wrong produces a picture rather than a
 * crash, which is the worst kind of bug to find, so the first half is a
 * compile error and the second is checked where the ramps are laid out. */
typedef char ball_palette_tiles_256[
	(SKY_BASE + SKY_N == FLOOR_BASE &&
	 FLOOR_BASE + FLOOR_N * SHADOW_N == RSVD_BASE &&
	 RSVD_BASE + RSVD_N == BALL_BASE &&
	 BALL_BASE + BALL_N == BALL_PAL_N) ? 1 : -1];
static unsigned char hue_base[TEX_MAT_MAX];
static unsigned char shade_lut[BALL_LEVELS * TEX_MAT_MAX];

/*
 * THE light. There is one, it is fixed in the VIEWER's frame, and everything
 * it does comes from this vector and nothing else: the diffuse term, the
 * specular highlight, which way the shadow is thrown and how far, and how the
 * shadow is stretched. Two lights that disagree is the first thing an eye
 * notices in a picture, so there is only one to disagree with.
 *
 * Axes as everywhere in this file: x to the right, y DOWNWARDS, z towards the
 * viewer. So this light is up, to the left, and a little in front.
 *
 * Being fixed in the viewer's frame is also what makes the sphere map
 * possible: the ball spins about its own polar axis, so turning it moves the
 * texture across the shading and never the shading across the ball.
 */
#define LIGHT_X		-0.45f
#define LIGHT_Y		-0.60f
#define LIGHT_Z		 0.66f

/*
 * A shade index runs from unlit to blown-out. The bottom LIT_TOP of the ramp
 * scales the hue's own colour, which is the diffuse term; the rest blends to
 * white, which is the specular. A hue therefore keeps its colour right up to
 * the highlight and loses it only inside it, the way a glossy painted surface
 * does.
 */
#define LIT_AMB		0.12f	/* how lit the terminator still is	     */
#define LIT_TOP		0.85f	/* share of the ramp the diffuse term owns   */
#define LIT_SPEC	0.15f	/* share the specular owns		     */

/*
 * Which way the ball's surface travels across its face while the ball is
 * moving to the RIGHT. (1, -1) is from the lower left to the upper right; y
 * points down, so a negative y is up. (1, 0) is the plain horizontal spin of
 * a globe. Negating the pair mirrors the diagonal.
 *
 * This is not a per-frame cost: tilting the spin tilts the sphere's polar
 * axis, and turning the whole projection by a fixed angle about the VIEWING
 * axis is a rotation of the (x, y) the sphere map is built from -- it leaves
 * z, and therefore the front hemisphere and the arc-sine shortcut that
 * depends on it, exactly as they were. The frame loop is still one addition
 * per pixel.
 */
#define SPIN_DIR_X	 1.0f
#define SPIN_DIR_Y	-1.0f

/*
 * The shadow, in the flat little world the floor implies. The floor is drawn
 * as a gradient rather than projected, so the one thing that says how deep it
 * is is the shadow itself: milestone 1 drew the ball's shadow 1.20 radii wide
 * and 0.30 radii tall, and the ratio of those two IS this floor's
 * foreshortening. It is written down here as such and everything else is
 * derived from it, so the shadow's shape and its travel cannot disagree.
 *
 * SHADOW_THROW scales the distance the light throws the shadow. 1.0 is the
 * true throw for this light -- at the top of its arc the ball would cast its
 * shadow three radii to the side, which is correct and reads as two separate
 * objects. It is the one number here that is a judgement rather than a
 * derivation, which is why it is alone.
 */
#define SHADOW_R	1.20f	/* shadow radius on the floor, in ball radii */
#define FLOOR_SQUASH	0.25f	/* a step into the picture, as a step up it  */
#define SHADOW_THROW	0.45f

#define B_PI		3.14159265358979323846f

/* ------------------------------------------------------------------------ */
/* Shape and motion, all as fractions of the screen the VDI reported          */
/* ------------------------------------------------------------------------ */
#define DIA_FRAC	0.16f	/* ball diameter / screen width		     */
#define DIA_MAX_FRAC	0.18f	/* hard ceiling on the same		     */
#define HORIZON_FRAC	0.62f	/* where the floor begins		     */
#define FLOOR_FRAC	0.90f	/* the line the ball's underside touches     */
#define GRAVITY_H	1.90f	/* screen heights per second squared	     */
#define APEX_HEADROOM	0.92f	/* of the room above the floor, so the ball  */
				/* stays fully on screen at its highest       */

/*
 * THE BALL NEVER LOSES ENERGY. That is a decision about how the demo reads,
 * taken knowing it is not physics: a damped bounce decays, and anything that
 * re-launches a decayed ball looks like magic, because it is. With nothing
 * lost there is nothing to put back, so the re-launch is gone rather than
 * tuned -- the artefact disappears because its cause does.
 *
 * Elastic is NOT "restitution 1.0", and this is measured rather than feared.
 * Reflecting the arriving speed conserves nothing here: the contact is found
 * a step late and the ball is lifted back to the floor, and that pair costs
 * about 1.3 % of its energy every single bounce -- the apex falls from 609
 * pixels to 77 in 200 bounces, about five minutes, monotonically. Undoing the
 * last gravity increment before reflecting makes it worse; leaving the ball
 * where it penetrated makes it worse still. No cheap correction was found and
 * none is needed.
 *
 * So the leaving speed is SET, not reflected (physics_step). Every flight
 * then begins from the same two numbers and is bit-identical to the one
 * before it: over 270 seconds of demo time and 174 bounces, every apex landed
 * on the same pixel row and every flight took exactly 309 ticks.
 */

/*
 * How fast it rolls, and why that is computed rather than written down.
 *
 * Both motions are periodic in whole 200 Hz ticks -- the floor and the walls
 * both reset position and speed exactly -- so their periods are integers and
 * their ratio is a rational number. If that ratio is a simple one the ball
 * retraces one path and the demo looks stamped out. Measured with the
 * constants above at 1920x1080: the bounce is 309 ticks, a plain 0.300 screen
 * widths per second makes the traverse 1120 ticks, and 8 traverses later the
 * two are back in step to within ONE tick out of 309 -- the path repeats
 * itself every 45 seconds.
 *
 * So plan_cycle() picks the traverse instead: it measures the bounce, then
 * searches a window around the nominal speed for the one whose path goes
 * longest before coming within PATTERN_EPS per cent of retracing. At
 * 1920x1080 that is 0.2963 W/s -- a tenth of a per cent slower than the round
 * number, and 193 seconds instead of 45. At 640x480 it is 0.3088 W/s and 174
 * seconds. Nothing is random; the demo is exactly as deterministic as it was,
 * which is what a benchmark needs.
 *
 * If you are tempted to replace this with a round constant: that IS the round
 * constant, and it is the one that repeats every 45 seconds.
 */
#define VX0_W		0.30f	/* nominal roll speed, screen widths/second   */
#define VX_WINDOW	0.10f	/* how far the search may move it	     */
#define PATTERN_EPS	3	/* per cent of a bounce that counts as a      */
				/* retrace				     */
#define PATTERN_CAP	200	/* traverses to look ahead		     */

/* Physics step: exactly one tick of the 200 Hz system clock. */
#define PHYS_HZ		200
#define PHYS_DT		(1.0f / (float)PHYS_HZ)
/* Never try to catch up more than a fifth of a second at once: after a stall
 * the backlog is dropped instead of chased, which is what keeps a slow frame
 * from turning into a spiral of slower ones. */
#define MAX_STEPS	40
/* Physics steps a frame may consume before it counts as dropped: at 60 Hz on
 * a 200 Hz clock a frame is 3 or 4 ticks, so 5 means a retrace went by
 * unused. Derived, not guessed -- a 50 Hz machine would want 5. */
#define DROP_STEPS	((PHYS_HZ / 60) + 1)

typedef struct {
	int x0, y0, x1, y1;	/* x1 / y1 exclusive */
} rect;

/* ---- screen ------------------------------------------------------------- */
static ball_screen scr;
static int W, H;
static int horizon;			/* first floor row		     */
static float floor_y;			/* the ball's contact line	     */

/* ---- buffers (all from Mxalloc, TT-RAM preferred) ----------------------- */
static unsigned char *idx;		/* W*H palette indices		     */
static unsigned char *bg_row;		/* H, the background index per row   */
static unsigned short *map_off;		/* D*D, texel offset before the spin */
static unsigned char *map_shade;	/* D*D, BALL_BASE + shade	     */
static short *run0, *run1;		/* D each, the disc's row coverage   */
static unsigned char *tex_buf;		/* TEX_MAX_BYTES, the texture	     */
static ball_tex tex;

/* ---- the ball ----------------------------------------------------------- */
static int R, D;			/* radius / diameter in pixels	     */
static float Rf;
static float bx, by;			/* centre, pixels		     */
static float vx, vy;			/* pixels per second		     */
static float gravity;
static float v_launch;			/* the speed it always leaves at     */
static float vx0;			/* the speed it always rolls at	     */
static long cyc_v, cyc_h;		/* the two periods, in ticks	     */
static long cyc_hold;			/* traverses before a near-retrace   */
static float spin_u;			/* spin, in texture columns	     */

/* ---- derived per frame -------------------------------------------------- */
static int b_left, b_top;		/* the disc's origin on screen	     */
static int tex_du;			/* texture columns the spin adds     */
static float sh_cx, sh_cy, sh_al;	/* the shadow: centre and darkness   */
static float sh_ia, sh_ib;		/* 1 / its two floor semi-axes	     */
static float sh_a_bb, sh_b_bb;		/* half its extent on the screen     */
static int sh_x0, sh_y0, sh_x1, sh_y1;

/* The one light, normalised, and what it does to the shadow. All of it is
 * derived once by light_init() from LIGHT_X/Y/Z above. */
static float lgt_x, lgt_y, lgt_z;
static float sh_ex, sh_ez;		/* the light's heading across the floor */
static float sh_stretch;		/* how much it stretches the ellipse */
static float sh_throw_x, sh_throw_z;	/* floor units per pixel of height   */

static ball_rgb pal[BALL_PAL_N];
static rect prev_rect[2];		/* what each screen page still shows */

/* ------------------------------------------------------------------------ */
/* Small helpers: no libm, no printf                                         */
/* ------------------------------------------------------------------------ */

/*
 * Square root. There is no libm here and none is wanted: this is called while
 * the sphere map is built and to work out the two serve speeds, never inside
 * the frame loop. The seed halves the IEEE-754 exponent (binary32 on both
 * targets), which is accurate to a few percent, and four Newton steps take it
 * to full single precision.
 */
static float b_sqrt(float x)
{
	union {
		float f;
		unsigned int u;
	} v;
	float r;
	int i;

	if (x <= 0.0f)
		return 0.0f;
	v.f = x;
	v.u = 0x1FC00000u + (v.u >> 1);
	r = v.f;
	for (i = 0; i < 4; i++)
		r = 0.5f * (r + x / r);
	return r;
}

static float b_abs(float x)
{
	return x < 0.0f ? -x : x;
}

/*
 * Arc sine, single precision, no libm.
 *
 *      asin(x) = SUM c_n x^(2n+1) / (2n+1),   c_0 = 1, c_n = c_(n-1)(2n-1)/2n
 *
 * The series is only useful well inside the unit circle, so anything above a
 * half is folded down first with asin(x) = pi/2 - 2 asin(sqrt((1-x)/2)), which
 * maps (0.5, 1] onto (0, 0.5]. Fifteen terms then leave the error below single
 * precision -- the last one is around 1e-11 at x = 0.5.
 *
 * The coefficients are NOT a table of numbers copied from somewhere: asin_init
 * derives them from the recurrence above, once, at start-up. asin itself is
 * called about once per pixel of the ball while the sphere map is built, and
 * never inside the frame loop.
 */
#define ASIN_TERMS	15
static float asin_c[ASIN_TERMS];

static void asin_init(void)
{
	float c = 1.0f;
	int n;

	asin_c[0] = 1.0f;
	for (n = 1; n < ASIN_TERMS; n++) {
		c *= (float)(2 * n - 1) / (float)(2 * n);
		asin_c[n] = c / (float)(2 * n + 1);
	}
}

static float b_asin(float x)
{
	float a = b_abs(x), r, a2, p, sum;
	int n, folded = 0;

	if (a >= 1.0f)
		return x < 0.0f ? -0.5f * B_PI : 0.5f * B_PI;
	if (a > 0.5f) {
		a = b_sqrt(0.5f * (1.0f - a));
		folded = 1;
	}
	a2 = a * a;
	p = a;
	sum = a;
	for (n = 1; n < ASIN_TERMS; n++) {
		p *= a2;
		sum += asin_c[n] * p;
	}
	r = folded ? 0.5f * B_PI - 2.0f * sum : sum;
	return x < 0.0f ? -r : r;
}

/* ------------------------------------------------------------------------ */
/* Ordered dithering                                                         */
/*                                                                           */
/* Every gradient here is quantised harder than milestone 1's was, because    */
/* the textured ball took half the palette. Dithering buys the difference     */
/* back: instead of rounding a value to the nearest palette step, round it up */
/* or down against a threshold that varies with position, so the error        */
/* averages out across a few pixels instead of collecting into a visible      */
/* band. It costs nothing where it is used -- the ball's thresholds are baked */
/* into the sphere map at start-up, and the background's are applied once per */
/* row.                                                                      */
/*                                                                           */
/* The matrix is not typed in from a book either: it is built from the        */
/* recurrence that defines it, M(n+1) = [[4M, 4M+2], [4M+3, 4M+1]] from       */
/* M(0) = [[0]]. The 4x4 matrix is then the 8x8's top-left quarter over four, */
/* which is the same recurrence read backwards.                              */
/* ------------------------------------------------------------------------ */
static float dith8[8][8];		/* the ball's shading		     */
static float dith4[4][4];		/* the shadow			     */
static float dith1[8];			/* gradients that only vary in y     */

static void build_dither(void)
{
	unsigned char m8[8][8];
	int n, x, y;

	m8[0][0] = 0;
	for (n = 1; n <= 3; n++) {
		int m = 1 << (n - 1);

		for (y = 0; y < m; y++)
			for (x = 0; x < m; x++) {
				int v = 4 * (int)m8[y][x];

				m8[y][x] = (unsigned char)v;
				m8[y][x + m] = (unsigned char)(v + 2);
				m8[y + m][x] = (unsigned char)(v + 3);
				m8[y + m][x + m] = (unsigned char)(v + 1);
			}
	}
	for (y = 0; y < 8; y++)
		for (x = 0; x < 8; x++)
			dith8[y][x] = ((float)m8[y][x] + 0.5f) / 64.0f;
	for (y = 0; y < 4; y++)
		for (x = 0; x < 4; x++)
			dith4[y][x] = ((float)(m8[y][x] >> 2) + 0.5f) / 16.0f;
	/* One dimension, eight deep: the three-bit reversal of the row number,
	 * {0,4,2,6,1,5,3,7}, which spreads a band edge over eight rows. */
	for (y = 0; y < 8; y++) {
		int b = ((y & 1) << 2) | (y & 2) | ((y >> 2) & 1);

		dith1[y] = ((float)b + 0.5f) / 8.0f;
	}
}

static int mixc(int a, int b, float t)
{
	float v = (float)a + ((float)b - (float)a) * t + 0.5f;
	if (v < 0.0f)
		v = 0.0f;
	if (v > 255.0f)
		v = 255.0f;
	return (int)v;
}

static char obuf[160];
static int opos;

static void o_reset(void)
{
	opos = 0;
	obuf[0] = 0;
}

static void o_ch(char c)
{
	if (opos < (int)sizeof(obuf) - 1) {
		obuf[opos++] = c;
		obuf[opos] = 0;
	}
}

static void o_str(const char *s)
{
	while (*s)
		o_ch(*s++);
}

static void o_dec(long v)
{
	char t[24];
	int n = 0;
	unsigned long u;

	if (v < 0) {
		o_ch('-');
		u = (unsigned long)(-v);
	} else {
		u = (unsigned long)v;
	}
	if (u == 0)
		t[n++] = '0';
	while (u) {
		t[n++] = (char)('0' + (int)(u % 10UL));
		u /= 10UL;
	}
	while (n)
		o_ch(t[--n]);
}

/* Two decimals, computed in floating point so no intermediate overflows. */
static void o_fix2(double v)
{
	long q;

	if (v < 0.0)
		v = 0.0;
	q = (long)(v * 100.0 + 0.5);
	o_dec(q / 100);
	o_ch('.');
	o_ch((char)('0' + (int)((q / 10) % 10)));
	o_ch((char)('0' + (int)(q % 10)));
}

/* ------------------------------------------------------------------------ */
/* C:\BALL.LOG -- the performance record                                     */
/*                                                                           */
/* The demo cannot say anything on screen while it runs (the screen is the    */
/* picture) and it has nowhere else to talk: on this profile the AUX device   */
/* is absent, so Bconout(1) discards. So the frame rate goes into a file,     */
/* one line every LOG_SECS seconds plus a closing summary, each line          */
/* committed on its own (Fopen read/write -> Fseek end -> Fwrite -> Fclose)   */
/* so an interrupted run still leaves a readable, valid log. Absolute path,   */
/* because nothing sets the current directory before a program is launched.   */
/* ------------------------------------------------------------------------ */
#define LOGPATH		"C:\\BALL.LOG"
/* The texture, if there is one. An absolute path for the same reason the log
 * has one: nothing sets a current directory before a program is launched, and
 * the demo is meant to be one double-click away in the root of C:. When the
 * file is not there the texture is generated instead -- see ball-tex.c. */
#define TEXPATH		"C:\\BALL.NTX"
#define LOG_SECS	5
#define LOG_EVERY	(LOG_SECS * PHYS_HZ)

static int log_ok;

static void log_open(void)
{
	long fd = Fcreate(LOGPATH, 0);

	if (fd >= 0) {
		Fclose((short)fd);
		log_ok = 1;
	}
}

static void log_line(const char *s)
{
	long fd, n = 0;

	if (!log_ok)
		return;
	fd = Fopen(LOGPATH, 2);			/* read/write */
	if (fd < 0)
		return;
	if (Fseek(0L, (short)fd, 2) < 0) {	/* SEEK_END: GEMDOS has no append */
		(void)Fclose((short)fd);
		return;
	}
	while (s[n])
		n++;
	(void)Fwrite((short)fd, n, s);
	(void)Fclose((short)fd);		/* commits the directory entry */
}

/* Emit the built line to the log, and to the console as well when the screen
 * is the demo's no longer. */
static void o_out(int console)
{
	o_str("\r\n");
	if (console)
		Cconws(obuf);
	log_line(obuf);
	o_reset();
}

static void o_flush(void)
{
	o_out(1);
}

/* ------------------------------------------------------------------------ */
/* The light, and everything that follows from it                            */
/*                                                                           */
/* The shading needs it as a direction. The shadow needs three more things,   */
/* and all three are read straight off the same vector:                      */
/*                                                                           */
/*   where it falls   -- a ball h above the floor casts along the ray -L, so  */
/*                       the shadow lands h * (Lx / Ly) to the side and       */
/*                       h * (-Lz / Ly) further into the picture. Straight    */
/*                       down when the light is straight up, which is what    */
/*                       milestone 1 drew.                                    */
/*   which way it is  -- along the light's heading across the floor, which is */
/*      stretched        the horizontal part of -L.                          */
/*   how much          -- 1 / cos(angle from vertical). A sphere lit from     */
/*                       overhead casts a circle; lit obliquely it casts an   */
/*                       ellipse that long.                                   */
/*                                                                           */
/* A light too close to the horizon would throw the shadow to infinity, so    */
/* the stretch is capped -- not to fake anything, but because a shadow the    */
/* length of the screen is not a shadow.                                     */
/* ------------------------------------------------------------------------ */
#define SH_STRETCH_MAX	2.5f

static void light_init(void)
{
	float n, hx, hz;

	n = b_sqrt(LIGHT_X * LIGHT_X + LIGHT_Y * LIGHT_Y + LIGHT_Z * LIGHT_Z);
	if (n <= 0.0f)
		n = 1.0f;
	lgt_x = LIGHT_X / n;
	lgt_y = LIGHT_Y / n;
	lgt_z = LIGHT_Z / n;

	/* Its heading across the floor: the horizontal part of -L, with the
	 * floor's second axis running INTO the picture. */
	hx = -lgt_x;
	hz = lgt_z;
	n = b_sqrt(hx * hx + hz * hz);
	if (n > 0.0001f) {
		sh_ex = hx / n;
		sh_ez = hz / n;
	} else {
		sh_ex = 1.0f;		/* light straight overhead */
		sh_ez = 0.0f;
	}

	/* -lgt_y is the cosine of the light's angle from vertical, because the
	 * vector is a unit one and y is the vertical axis. */
	sh_stretch = (-lgt_y > 0.0001f) ? 1.0f / -lgt_y : SH_STRETCH_MAX;
	if (sh_stretch > SH_STRETCH_MAX)
		sh_stretch = SH_STRETCH_MAX;

	if (lgt_y < -0.0001f) {
		sh_throw_x = (lgt_x / lgt_y) * SHADOW_THROW;
		sh_throw_z = (-lgt_z / lgt_y) * SHADOW_THROW;
	} else {
		sh_throw_x = 0.0f;	/* a light at or below the horizon */
		sh_throw_z = 0.0f;
	}
}

/* ------------------------------------------------------------------------ */
/* Palette                                                                   */
/* ------------------------------------------------------------------------ */
static void build_palette(void)
{
	int i, m, band, lv, r, g, b, base;
	float t;

	/* Backdrop: near-black at the top, lifting slightly into the horizon,
	 * so the room reads as deep rather than flat. */
	for (i = 0; i < SKY_N; i++) {
		t = (float)i / (float)(SKY_N - 1);
		pal[SKY_BASE + i].r = (unsigned char)mixc(2, 26, t);
		pal[SKY_BASE + i].g = (unsigned char)mixc(5, 34, t);
		pal[SKY_BASE + i].b = (unsigned char)mixc(13, 60, t);
	}

	/* Floor: brightest at the horizon and falling away toward the viewer.
	 * It has to be bright enough for a shadow to be visible on it -- a
	 * shadow on black is not a shadow. Each band is repeated in SHADOW_N
	 * darkening steps, so shading a floor pixel is an addition. */
	for (band = 0; band < FLOOR_N; band++) {
		t = (float)band / (float)(FLOOR_N - 1);
		r = mixc(54, 13, t);
		g = mixc(72, 18, t);
		b = mixc(112, 34, t);
		for (lv = 0; lv < SHADOW_N; lv++) {
			float d = 1.0f - 0.88f * (float)lv /
					 (float)(SHADOW_N - 1);
			int k = FLOOR_BASE + band * SHADOW_N + lv;
			pal[k].r = (unsigned char)((float)r * d + 0.5f);
			pal[k].g = (unsigned char)((float)g * d + 0.5f);
			pal[k].b = (unsigned char)((float)b * d + 0.5f);
		}
	}

	/* Milestone 2b's block. Black, and touched by nothing until it has an
	 * owner -- an entry left at a plausible-looking colour is an entry
	 * somebody eventually draws with by accident. */
	for (i = 0; i < RSVD_N; i++) {
		pal[RSVD_BASE + i].r = 0;
		pal[RSVD_BASE + i].g = 0;
		pal[RSVD_BASE + i].b = 0;
	}

	/* The ball: one ramp per hue the texture is made of, each black at the
	 * silhouette, the hue's own colour where the light is full on, and
	 * blowing out to white inside the specular highlight. The texture
	 * decides the colours; the palette only shades them. The ramps are
	 * different LENGTHS -- see hue_len -- so lay them out end to end and
	 * remember where each one starts. */
	base = BALL_BASE;
	for (m = 0; m < TEX_MAT_MAX; m++) {
		int mr = tex.rgb[m][0];
		int mg = tex.rgb[m][1];
		int mb = tex.rgb[m][2];
		int n = hue_len[m];

		if (base + n > BALL_PAL_N)	/* hue_len edited wrong */
			n = BALL_PAL_N - base;
		if (n < 2)
			break;
		hue_base[m] = (unsigned char)base;
		for (i = 0; i < n; i++) {
			int k = base + i;

			t = (float)i / (float)(n - 1);
			if (t <= LIT_TOP) {
				float u = t / LIT_TOP;
				r = mixc(0, mr, u);
				g = mixc(0, mg, u);
				b = mixc(0, mb, u);
			} else {
				float u = (t - LIT_TOP) / (1.0f - LIT_TOP);
				r = mixc(mr, 255, u);
				g = mixc(mg, 255, u);
				b = mixc(mb, 255, u);
			}
			pal[k].r = (unsigned char)r;
			pal[k].g = (unsigned char)g;
			pal[k].b = (unsigned char)b;
		}
		base += n;
	}

	/* And the table that turns (light level, hue) into one of those
	 * entries. A ramp shorter than BALL_LEVELS is sampled out of the same
	 * levels, so every hue answers to the same light level and the
	 * renderer never needs to know how long any ramp is. */
	for (i = 0; i < BALL_LEVELS; i++)
		for (m = 0; m < TEX_MAT_MAX; m++) {
			int n = hue_len[m] - 1;
			int e = (i * n + (BALL_LEVELS - 1) / 2) /
				(BALL_LEVELS - 1);

			shade_lut[i * TEX_MAT_MAX + m] =
				(unsigned char)(hue_base[m] + e);
		}
}

/*
 * One palette index per screen row: the backdrop above the horizon, the floor
 * below it. Both gradients are dithered DOWN the screen -- a row's band is
 * rounded up or down against a threshold taken from the row number -- so a
 * band edge becomes four alternating rows instead of one hard line. A row is
 * a single index, so this costs nothing per pixel and leaves the background
 * fill a memset.
 */
static void build_bg_rows(void)
{
	int y;

	for (y = 0; y < H; y++) {
		float d = dith1[y & 7];
		int band;

		if (y < horizon) {
			band = (int)((float)y * (float)(SKY_N - 1) /
				     (float)horizon + d);
			if (band > SKY_N - 1)
				band = SKY_N - 1;
			bg_row[y] = (unsigned char)(SKY_BASE + band);
		} else {
			band = (int)((float)(y - horizon) *
				     (float)(FLOOR_N - 1) /
				     (float)(H - horizon) + d);
			if (band > FLOOR_N - 1)
				band = FLOOR_N - 1;
			bg_row[y] = (unsigned char)(FLOOR_BASE +
						    band * SHADOW_N);
		}
	}
}

/* ------------------------------------------------------------------------ */
/* The sphere map: everything about the ball that never changes              */
/*                                                                           */
/* For every pixel of the ball's bounding square that falls inside the        */
/* circle, take the surface normal (nx, ny, nz) of the sphere it belongs to   */
/* and work out the two things that are the same in every frame the demo will */
/* ever draw:                                                                */
/*                                                                           */
/*   - the texel it samples when the ball is not turned. The normal is first  */
/*     expressed in the SPHERE's frame -- the viewer's, turned about the      */
/*     viewing axis until the pole lies across the direction the surface is   */
/*     to travel (SPIN_DIR_*). That turn leaves z alone, so the front         */
/*     hemisphere stays the front hemisphere. Then latitude comes from the    */
/*     turned y alone, and longitude is atan2(gx, gz) -- which, because gz is */
/*     never negative on the half of the sphere one can see and gx*gx + gz*gz */
/*     is 1 - gy*gy, is exactly asin(gx / sqrt(1 - gy*gy)). One inverse       */
/*     function instead of two, and the row and the column are packed into    */
/*     ONE offset into the texture.                                          */
/*   - its shade, because the light is fixed in the viewer's frame while the  */
/*     spin is about the sphere's polar axis, so turning the ball moves the   */
/*     texture across the shading and never the shading across the ball. The  */
/*     shading is computed from the UNTURNED normal, which is why tilting the */
/*     spin costs nothing and does not move the highlight.                   */
/*                                                                           */
/* What is left for the frame loop is the spin, and the spin is ONE ADDITION. */
/* Turning the surface vector by an angle about the polar axis and then       */
/* taking atan2 of it gives atan2 plus that angle, exactly -- so however far  */
/* the ball has rolled, the texel every pixel samples is its resting texel    */
/* plus the same column offset. Drawing the ball is therefore a table lookup, */
/* an add, a second lookup and a store per pixel; no trigonometry, no square  */
/* root, no divide.                                                          */
/*                                                                           */
/* The shade is dithered here, not when it is used: the pattern depends on    */
/* the pixel's place in the ball's own square, which never changes, so it is  */
/* free.                                                                     */
/*                                                                           */
/* Cost, for the record: a ball 16 % of a 1920-pixel screen wide covers about */
/* 73 000 pixels, and each costs three square roots, two arc sines and two    */
/* dot products. It is measured (map_ticks) and reported in the log rather    */
/* than estimated.                                                           */
/* ------------------------------------------------------------------------ */
static long map_ticks;			/* what building it cost, in ticks   */
static long map_px;			/* pixels it covers		     */

static void build_sphere_map(void)
{
	float lx = lgt_x, ly = lgt_y, lz = lgt_z;
	float hx, hy, hz, n, tlx, tly;
	long t0 = get_sysvar(_hz_200);
	int sx, sy;

	map_px = 0;

	/* Blinn-Phong halfway vector, the viewer looking straight down +z. */
	hx = lx;
	hy = ly;
	hz = lz + 1.0f;
	n = b_sqrt(hx * hx + hy * hy + hz * hz);
	hx /= n;
	hy /= n;
	hz /= n;

	/* The tilt of the sphere's polar axis, as the direction the surface is
	 * to travel in. Turning the projection by this much about the viewing
	 * axis puts the pole where it belongs; z is untouched, so the front
	 * hemisphere -- and the arc-sine shortcut below, which needs it -- is
	 * exactly as it was. */
	tlx = SPIN_DIR_X;
	tly = SPIN_DIR_Y;
	n = b_sqrt(tlx * tlx + tly * tly);
	if (n <= 0.0001f) {
		tlx = 1.0f;
		tly = 0.0f;
	} else {
		tlx /= n;
		tly /= n;
	}

	for (sy = 0; sy < D; sy++) {
		float fy = ((float)sy + 0.5f - Rf) / Rf;
		int first = -1, last = -1;

		run0[sy] = 0;
		run1[sy] = 0;

		for (sx = 0; sx < D; sx++) {
			float fx = ((float)sx + 0.5f - Rf) / Rf;
			float q = fx * fx + fy * fy;
			float nz, gx, gy, cos_lat, p, u, v, dif, sp, lit;
			int u0, ty, sh;
			long texrow;

			if (q > 1.0f)
				continue;

			/* The sphere's surface normal at this pixel. */
			nz = b_sqrt(1.0f - q);

			/* The same normal in the SPHERE's frame, which is the
			 * viewer's turned about z until the pole lies across
			 * the direction the surface is to travel. */
			gx = fx * tlx + fy * tly;
			gy = fy * tlx - fx * tly;

			/* Latitude -> texture row. gy = -1 is the sphere's
			 * north pole, which is row 0 of the texture. */
			v = 0.5f + b_asin(gy) / B_PI;
			ty = (int)(v * (float)tex.h);
			if (ty < 0)
				ty = 0;
			if (ty > tex.h - 1)
				ty = tex.h - 1;
			texrow = (long)ty * (long)tex.stride;

			/* Longitude -> texture column. atan2(gx, gz) with a
			 * front-facing gz is asin(gx / sqrt(1 - gy*gy)). */
			cos_lat = b_sqrt(1.0f - gy * gy);
			p = (cos_lat > 0.0001f) ? gx / cos_lat : 0.0f;
			if (p < -1.0f)
				p = -1.0f;
			if (p > 1.0f)
				p = 1.0f;
			u = 0.5f + b_asin(p) / (2.0f * B_PI);
			u0 = (int)(u * (float)tex.w);
			if (u0 < 0)
				u0 = 0;
			if (u0 > tex.w - 1)
				u0 = tex.w - 1;
			map_off[(long)sy * D + sx] =
				(unsigned short)(texrow + u0);

			dif = fx * lx + fy * ly + nz * lz;
			if (dif < 0.0f)
				dif = 0.0f;

			sp = fx * hx + fy * hy + nz * hz;
			if (sp < 0.0f)
				sp = 0.0f;
			sp *= sp;	/* ^2  */
			sp *= sp;	/* ^4  */
			sp *= sp;	/* ^8  */
			sp *= sp;	/* ^16 */
			sp *= sp;	/* ^32 */
			sp *= sp;	/* ^64 -- a tight, glossy highlight */

			lit = LIT_TOP * (LIT_AMB + (1.0f - LIT_AMB) * dif)
			      + LIT_SPEC * sp;
			if (lit > 1.0f)
				lit = 1.0f;

			sh = (int)(lit * (float)(BALL_LEVELS - 1) +
				   dith8[sy & 7][sx & 7]);
			if (sh < 0)
				sh = 0;
			if (sh > BALL_LEVELS - 1)
				sh = BALL_LEVELS - 1;
			/* Stored as the ROW of the shade table, not the level:
			 * the frame loop then adds the texel and reads the
			 * palette index straight out. 47 * 4 + 3 = 191, so it
			 * still fits in the byte. */
			map_shade[(long)sy * D + sx] =
				(unsigned char)(sh * TEX_MAT_MAX);

			if (first < 0)
				first = sx;
			last = sx;
			map_px++;
		}

		/* A circle covers one contiguous run per row, which is what
		 * lets the frame loop walk rows instead of testing a mask. */
		run0[sy] = (short)(first < 0 ? 0 : first);
		run1[sy] = (short)(first < 0 ? 0 : last + 1);
	}

	map_ticks = get_sysvar(_hz_200) - t0;
}

/* ------------------------------------------------------------------------ */
/* Physics: one fixed step of 1/200 s                                        */
/* ------------------------------------------------------------------------ */
static void physics_step(void)
{
	vy += gravity * PHYS_DT;
	by += vy * PHYS_DT;
	bx += vx * PHYS_DT;

	if (by + Rf >= floor_y) {
		/* Both numbers are SET, neither is derived from what arrived
		 * (see the note by APEX_HEADROOM). The flight that follows is
		 * therefore the same flight as the last one, bit for bit, and
		 * the ball reaches exactly the same height for as long as the
		 * demo runs -- no decay, and so nothing to re-launch. */
		by = floor_y - Rf;
		vy = -v_launch;
	}

	/* The walls only turn it round. Nothing scales vx anywhere, so its
	 * magnitude stays exactly vx0 -- which is what keeps the roll, and
	 * therefore the spin below, steady instead of winding down. */
	if (bx - Rf < 0.0f) {
		bx = Rf;
		vx = -vx;
	} else if (bx + Rf > (float)W) {
		bx = (float)W - Rf;
		vx = -vx;
	}

	/* The ball rolls, so it turns: one radian of spin per radius of
	 * travel, straight out of the horizontal speed. It is accumulated in
	 * TEXTURE COLUMNS rather than radians because that is the only unit
	 * the renderer ever wants it in, and kept inside one turn so it stays
	 * exact however long the demo runs. */
	spin_u += vx * (PHYS_DT / Rf) * (float)tex.w / (2.0f * B_PI);
	while (spin_u >= (float)tex.w)
		spin_u -= (float)tex.w;
	while (spin_u < 0.0f)
		spin_u += (float)tex.w;
}

/* ------------------------------------------------------------------------ */
/* Planning the two cycles so they do not fall into step                     */
/*                                                                           */
/* Measures the bounce rather than solving for it: the integrator the frame  */
/* loop runs is right here, so run it once from the exact state every bounce  */
/* starts in and count the ticks. That number is then true whatever the       */
/* screen size, the gravity or the rounding are.                             */
/*                                                                           */
/* Then choose the traverse. Both periods are whole ticks, so after b         */
/* traverses the bounce is out of phase by (2*m*b) mod cyc_v ticks, and the   */
/* path nearly retraces when that is near 0 -- or near cyc_v, which is the    */
/* same thing from the other side. Take the m in the window whose first such  */
/* near miss is furthest away. It is integer arithmetic throughout: no        */
/* rounding to argue about, and the pathological cases (a ratio that divides  */
/* exactly, so the ball lands on two spots forever) score 1 and lose.         */
/*                                                                           */
/* Dirichlet says no ratio can hold out for ever -- some b <= B always brings */
/* it within 1/B of a repeat -- so this buys time, not perfection. Measured   */
/* at 1920x1080 it buys 193 s instead of 45.                                  */
/* ------------------------------------------------------------------------ */
#define PLAN_TICK_MAX	100000L		/* a bounce cannot be longer	     */

static void plan_cycle(void)
{
	float by_p = floor_y - Rf, vy_p = -v_launch;
	long span, m0, mlo, mhi, m, bm, best, tv;
	long eps;

	tv = 0;
	for (;;) {
		vy_p += gravity * PHYS_DT;
		by_p += vy_p * PHYS_DT;
		tv++;
		if (by_p + Rf >= floor_y || tv >= PLAN_TICK_MAX)
			break;
	}
	cyc_v = tv;

	span = (long)W - 2L * (long)R;
	m0 = (long)((float)span / (VX0_W * (float)W * PHYS_DT) + 0.5f);
	if (m0 < 2)
		m0 = 2;
	mlo = (long)((float)m0 * (1.0f - VX_WINDOW));
	mhi = (long)((float)m0 * (1.0f + VX_WINDOW));
	if (mlo < 2)
		mlo = 2;
	if (mhi < mlo)
		mhi = mlo;

	eps = tv * PATTERN_EPS / 100;
	if (eps < 1)
		eps = 1;

	bm = m0;
	best = -1;
	for (m = mlo; m <= mhi; m++) {
		long b, first = PATTERN_CAP + 1;

		for (b = 1; b <= PATTERN_CAP; b++) {
			long r = (2L * m * b) % tv;
			long d = (r < tv - r) ? r : tv - r;

			if (d < eps) {
				first = b;
				break;
			}
		}
		/* Furthest-off wins; between equals, the one nearest the
		 * nominal speed, so the look never drifts for a tie. */
		if (first > best ||
		    (first == best &&
		     ((m > m0 ? m - m0 : m0 - m) <
		      (bm > m0 ? bm - m0 : m0 - bm)))) {
			best = first;
			bm = m;
		}
	}

	cyc_h = 2L * bm;
	cyc_hold = best;
	/* Cross the wall in the MIDDLE of a step, not on its edge: that is
	 * what makes the traverse exactly bm ticks and keeps it there however
	 * the additions round. */
	vx0 = (float)span / (((float)bm - 0.5f) * PHYS_DT);
}

/* ------------------------------------------------------------------------ */
/* Geometry of the current frame                                             */
/* ------------------------------------------------------------------------ */
static void update_shape(void)
{
	float hgt, k, sz;

	b_left = (int)(bx - Rf);
	b_top = (int)(by - Rf);

	/* Sampling a column further ALONG the texture puts that column's
	 * content further to the LEFT on the ball, so the offset runs against
	 * the spin: a ball travelling right shows its face travelling right,
	 * the way a rolling ball reads. */
	tex_du = (int)((float)tex.w - spin_u);
	if (tex_du >= tex.w)
		tex_du -= tex.w;

	/* How high the ball is flying, normalised. The shadow shrinks and
	 * fades with it -- the cheapest depth cue there is. */
	hgt = floor_y - (by + Rf);
	if (hgt < 0.0f)
		hgt = 0.0f;
	k = hgt / (0.55f * (float)H);
	if (k > 1.0f)
		k = 1.0f;

	sz = 1.0f / (1.0f + 1.15f * k);
	sh_al = 1.0f / (1.0f + 2.60f * k);

	/* The shadow on the floor: a circle the ball's size, stretched along
	 * the light's heading because the light is not overhead, and thrown
	 * away from the light by as much as the ball is high. Both come from
	 * the same vector the sphere is shaded with; see light_init(). */
	{
		float r0 = Rf * SHADOW_R * sz;
		float a = r0 * sh_stretch;	/* along the light's heading */
		float b = r0;			/* across it		     */
		float ax, az, bx2, bz;

		sh_ia = 1.0f / a;
		sh_ib = 1.0f / b;

		sh_cx = bx + hgt * sh_throw_x;
		sh_cy = floor_y + b * FLOOR_SQUASH * 0.20f
			- hgt * sh_throw_z * FLOOR_SQUASH;

		/* The ellipse lies on the floor and the floor is seen
		 * edge-on, so what reaches the screen is the ellipse with its
		 * depth squashed. Its bounding box is the two semi-axes
		 * projected and added in quadrature. */
		ax = a * sh_ex;
		az = a * sh_ez;
		bx2 = -b * sh_ez;
		bz = b * sh_ex;
		sh_a_bb = b_sqrt(ax * ax + bx2 * bx2);
		sh_b_bb = FLOOR_SQUASH * b_sqrt(az * az + bz * bz);
	}

	sh_x0 = (int)(sh_cx - sh_a_bb);
	sh_x1 = (int)(sh_cx + sh_a_bb) + 1;
	sh_y0 = (int)(sh_cy - sh_b_bb);
	sh_y1 = (int)(sh_cy + sh_b_bb) + 1;
	if (sh_x0 < 0)
		sh_x0 = 0;
	if (sh_y0 < horizon)
		sh_y0 = horizon;	/* the shadow lies on the floor */
	if (sh_x1 > W)
		sh_x1 = W;
	if (sh_y1 > H)
		sh_y1 = H;
}

static rect shape_rect(void)
{
	rect r;

	r.x0 = b_left;
	r.y0 = b_top;
	r.x1 = b_left + D;
	r.y1 = b_top + D;

	if (sh_x1 > sh_x0 && sh_y1 > sh_y0) {
		if (sh_x0 < r.x0)
			r.x0 = sh_x0;
		if (sh_y0 < r.y0)
			r.y0 = sh_y0;
		if (sh_x1 > r.x1)
			r.x1 = sh_x1;
		if (sh_y1 > r.y1)
			r.y1 = sh_y1;
	}

	if (r.x0 < 0)
		r.x0 = 0;
	if (r.y0 < 0)
		r.y0 = 0;
	if (r.x1 > W)
		r.x1 = W;
	if (r.y1 > H)
		r.y1 = H;
	if (r.x1 < r.x0)
		r.x1 = r.x0;
	if (r.y1 < r.y0)
		r.y1 = r.y0;
	return r;
}

static rect rect_union(rect a, rect b)
{
	if (a.x1 <= a.x0 || a.y1 <= a.y0)
		return b;
	if (b.x1 <= b.x0 || b.y1 <= b.y0)
		return a;
	if (b.x0 < a.x0)
		a.x0 = b.x0;
	if (b.y0 < a.y0)
		a.y0 = b.y0;
	if (b.x1 > a.x1)
		a.x1 = b.x1;
	if (b.y1 > a.y1)
		a.y1 = b.y1;
	return a;
}

/* ------------------------------------------------------------------------ */
/* Composing one frame into the index buffer                                 */
/* ------------------------------------------------------------------------ */
static void draw_shadow(rect r)
{
	int x0 = sh_x0, x1 = sh_x1, y0 = sh_y0, y1 = sh_y1;
	int x, y;

	if (x0 < r.x0)
		x0 = r.x0;
	if (y0 < r.y0)
		y0 = r.y0;
	if (x1 > r.x1)
		x1 = r.x1;
	if (y1 > r.y1)
		y1 = r.y1;

	for (y = y0; y < y1; y++) {
		unsigned char *row = idx + (long)y * W;
		unsigned char base = bg_row[y];
		const float *dr = dith4[y & 3];
		/* Back onto the floor: undo the foreshortening, then the row's
		 * fixed contribution to the two ellipse axes. */
		float fz = ((float)y + 0.5f - sh_cy) * (1.0f / FLOOR_SQUASH);
		float ra = fz * sh_ez;
		float rb = fz * sh_ex;

		for (x = x0; x < x1; x++) {
			float fx = (float)x + 0.5f - sh_cx;
			float da = (fx * sh_ex + ra) * sh_ia;
			float db = (rb - fx * sh_ez) * sh_ib;
			float q = da * da + db * db;
			int lv;

			if (q >= 1.0f)
				continue;
			/* 1 - q^2 keeps the middle of the shadow flat and lets
			 * the rim fall off softly, which is what a real
			 * penumbra looks like at this size. Six levels would
			 * contour that falloff into rings, so the level is
			 * dithered rather than rounded -- the shadow already
			 * costs a per-pixel float here, and this is one add
			 * more. */
			q = 1.0f - q * q;
			lv = (int)(sh_al * (float)(SHADOW_N - 1) * q +
				   dr[x & 3]);
			if (lv <= 0)
				continue;
			if (lv > SHADOW_N - 1)
				lv = SHADOW_N - 1;
			row[x] = (unsigned char)(base + lv);
		}
	}
}

/*
 * The whole of the sphere renderer, per pixel: fetch the resting texel offset,
 * add the spin, fetch the texel -- a hue -- add the pre-computed, pre-dithered
 * shade table row, and read the palette index out of the shade table. Four
 * loads, two adds, a store; no multiply, no divide, nothing about angles, and
 * nothing that can address outside the ball's palette slice.
 *
 * The rows of the texture are stored twice end to end (ball-tex.h), so adding
 * the spin never has to wrap.
 */
static void draw_ball(rect r)
{
	const unsigned char *tp = tex.px;
	const unsigned char *lut = shade_lut;
	unsigned du = (unsigned)tex_du;
	int sy;

	for (sy = 0; sy < D; sy++) {
		int y = b_top + sy;
		int a, b, n;
		const unsigned short *op;
		const unsigned char *shp;
		unsigned char *dp;

		if (y < r.y0 || y >= r.y1)
			continue;
		a = b_left + run0[sy];
		b = b_left + run1[sy];
		if (a < r.x0)
			a = r.x0;
		if (b > r.x1)
			b = r.x1;
		if (b <= a)
			continue;

		n = a - b_left;
		op = map_off + (long)sy * D + n;
		shp = map_shade + (long)sy * D + n;
		dp = idx + (long)y * W + a;
		n = b - a;
		while (n-- > 0)
			*dp++ = lut[*shp++ + tp[(unsigned)*op++ + du]];
	}
}

/*
 * Compose rectangle `r`: background, then shadow, then ball. The background
 * is not painted underneath the ball's own run -- the ball covers it whole --
 * which is why one pass over the rectangle is enough and no separate
 * background restore is needed anywhere in this program.
 */
static void compose(rect r)
{
	int y;

	for (y = r.y0; y < r.y1; y++) {
		unsigned char *row = idx + (long)y * W;
		unsigned char c = bg_row[y];
		int sy = y - b_top;
		int a = r.x0, b = r.x0;

		if (sy >= 0 && sy < D && run1[sy] > run0[sy]) {
			a = b_left + run0[sy];
			b = b_left + run1[sy];
			if (a < r.x0)
				a = r.x0;
			if (b > r.x1)
				b = r.x1;
		}
		if (b > a) {
			if (a > r.x0)
				memset(row + r.x0, c, (size_t)(a - r.x0));
			if (r.x1 > b)
				memset(row + b, c, (size_t)(r.x1 - b));
		} else {
			memset(row + r.x0, c, (size_t)(r.x1 - r.x0));
		}
	}

	draw_shadow(r);
	draw_ball(r);
}

static void blit(rect r)
{
	if (r.x1 <= r.x0 || r.y1 <= r.y0)
		return;
	plat_blit(idx + (long)r.y0 * W + r.x0, (long)W,
		  r.x0, r.y0, r.x1 - r.x0, r.y1 - r.y0);
}

/* ------------------------------------------------------------------------ */
/* Self-benchmark: how fast could this machine draw?                         */
/*                                                                           */
/* The demo waits for the retrace, so its frame rate is the display's, not    */
/* the machine's -- and the 200 Hz system clock is far too coarse to time one */
/* frame (a frame here costs a fraction of a tick, and worse, a Vsync-locked  */
/* frame starts at a fixed phase of the tick, so a per-frame accumulator can  */
/* read a flat zero however much work it is doing). The headroom is therefore */
/* measured the only way that clock can resolve: compose and blit frame after */
/* frame with NO wait, until enough ticks have passed to divide by.           */
/*                                                                           */
/* It runs into the screen page that is not being shown, so on a machine with */
/* two pages nothing of it is visible. On a single-page display it is a brief */
/* flurry before the demo settles.                                           */
/* ------------------------------------------------------------------------ */
#define BENCH_TICKS	40		/* 0.2 s -- +-2.5 % on a 5 ms clock  */
#define BENCH_MAX	8000		/* and never longer than that	     */

static long bench_frames;
static long bench_ticks;

static void bench(void)
{
	rect prev, cur, dirty;
	long t0;

	update_shape();
	prev = shape_rect();
	bench_frames = 0;
	t0 = get_sysvar(_hz_200);
	do {
		physics_step();
		update_shape();
		cur = shape_rect();
		dirty = rect_union(cur, prev);
		compose(dirty);
		blit(dirty);
		prev = cur;
		bench_frames++;
		bench_ticks = get_sysvar(_hz_200) - t0;
	} while (bench_ticks < BENCH_TICKS && bench_frames < BENCH_MAX);
}

/* ------------------------------------------------------------------------ */
/* Memory                                                                    */
/* ------------------------------------------------------------------------ */

/*
 * MX_PREFTTRAM: on an Atari, screen memory belongs in ST-RAM and everything
 * purely processor-side belongs in TT-RAM. These buffers are read and written
 * by the CPU alone, so they ask for TT-RAM and fall back to ST-RAM.
 */
#define MX_PREFTTRAM 3

static void *b_alloc(long n)
{
	return (void *)Mxalloc(n, MX_PREFTTRAM);
}

static void b_free(void *p)
{
	if (p)
		Mfree(p);
}

/* ------------------------------------------------------------------------ */
/* main                                                                      */
/* ------------------------------------------------------------------------ */
int main(void)
{
	long apex = 0;
	float apex_px;
	int page, i, rc = 0;
	long t_last, t_start, t_end, frames = 0, dropped = 0;
	long t_mark = 0, f_mark = 0, d_mark = 0;
	double secs;

	if (plat_open(&scr) < 0) {
		Cconws("BALL: ");
		Cconws(plat_error());
		Cconws("\r\n");
		return 1;
	}

	W = scr.w;
	H = scr.h;
	horizon = (int)((float)H * HORIZON_FRAC);
	floor_y = (float)H * FLOOR_FRAC;

	/* 16 % of the screen width, capped at 18 %, and never taller than the
	 * room it has to bounce in. */
	R = (int)((float)W * DIA_FRAC * 0.5f);
	if (R > (int)((float)W * DIA_MAX_FRAC * 0.5f))
		R = (int)((float)W * DIA_MAX_FRAC * 0.5f);
	if (R > (int)(floor_y * 0.30f))
		R = (int)(floor_y * 0.30f);
	if (R < 4)
		R = 4;
	D = 2 * R;
	Rf = (float)R;

	idx = (unsigned char *)b_alloc((long)W * H);
	bg_row = (unsigned char *)b_alloc((long)H);
	map_off = (unsigned short *)b_alloc((long)D * D *
					    (long)sizeof(unsigned short));
	map_shade = (unsigned char *)b_alloc((long)D * D);
	run0 = (short *)b_alloc((long)D * (long)sizeof(short));
	run1 = (short *)b_alloc((long)D * (long)sizeof(short));
	tex_buf = (unsigned char *)b_alloc((long)TEX_MAX_BYTES);
	if (!idx || !bg_row || !map_off || !map_shade || !run0 || !run1 ||
	    !tex_buf) {
		plat_close();
		Cconws("BALL: out of memory\r\n");
		rc = 1;
		goto done;
	}

	/* Order matters here: the palette is one shade ramp per material of
	 * the texture, so the texture has to exist before the palette does,
	 * and the sphere map samples both. */
	build_dither();
	asin_init();
	light_init();
	tex_build(&tex, tex_buf, TEXPATH);
	build_palette();
	plat_palette(pal);
	build_bg_rows();
	build_sphere_map();

	/* Serve speeds. The apex is as high as the ball can go and still be
	 * fully on screen, with a little headroom. */
	gravity = GRAVITY_H * (float)H;
	apex_px = (floor_y - (float)D) * APEX_HEADROOM;
	if (apex_px < (float)D)
		apex_px = (float)D;
	v_launch = b_sqrt(2.0f * gravity * apex_px);
	plan_cycle();
	apex = (long)apex_px;

	bx = (float)W * 0.25f;
	by = floor_y - Rf - apex_px;
	vx = vx0;
	vy = 0.0f;

	log_open();

	/* Measure the headroom before the demo settles into the retrace, with
	 * drawing moved off the visible page first so none of it is seen. */
	page = 0;
	if (scr.pages > 1) {
		plat_present();
		if (++page >= scr.pages)
			page = 0;
	}
	{
		float sx = bx, sy = by, svx = vx, svy = vy;
		bench();
		bx = sx;
		by = sy;
		vx = svx;
		vy = svy;
	}

	/* The one and only full-screen paint: every page has to be given the
	 * background once. From here on nothing outside a dirty rectangle is
	 * ever written again. */
	for (i = 0; i < scr.pages; i++) {
		rect full;
		full.x0 = 0;
		full.y0 = 0;
		full.x1 = W;
		full.y1 = H;
		update_shape();
		compose(full);
		blit(full);
		prev_rect[page] = shape_rect();
		plat_present();
		if (++page >= scr.pages)
			page = 0;
	}

	o_reset();
	o_str("BALL: screen ");
	o_dec((long)W);
	o_ch('x');
	o_dec((long)H);
	o_str(", ");
	o_dec((long)scr.pages);
	o_str(" page(s), ball d=");
	o_dec((long)D);
	o_str(" px, apex=");
	o_dec(apex);
	o_str(" px");
	o_out(0);

	o_str("BALL: texture ");
	o_dec((long)tex.w);
	o_ch('x');
	o_dec((long)tex.h);
	o_str(", ");
	o_dec((long)tex.nmat);
	o_str(" hues, from ");
	o_str(tex.source);
	o_out(0);

	o_str("BALL: sphere map ");
	o_dec(map_px);
	o_str(" px in ");
	o_fix2((double)map_ticks * 1000.0 / (double)PHYS_HZ);
	o_str(" ms (");
	o_dec(map_ticks);
	o_str(" ticks of ");
	o_dec((long)PHYS_HZ);
	o_str(" Hz)");
	o_out(0);

	o_str("BALL: bounce ");
	o_dec(cyc_v);
	o_str(" ticks, traverse ");
	o_dec(cyc_h);
	o_str(" ticks, roll ");
	o_fix2((double)vx0);
	o_str(" px/s; elastic, no decay; path holds ");
	o_dec(cyc_hold);
	o_str(" traverses (");
	o_fix2((double)cyc_hold * (double)cyc_h / (double)PHYS_HZ);
	o_str(" s) before coming within ");
	o_dec((long)PATTERN_EPS);
	o_str(" % of retracing");
	o_out(0);

	o_str("BALL: headroom ");
	o_dec(bench_frames);
	o_str(" frames in ");
	o_fix2((double)bench_ticks / (double)PHYS_HZ);
	o_str(" s free-running = ");
	o_fix2(bench_ticks > 0 ? (double)bench_frames * (double)PHYS_HZ /
				 (double)bench_ticks : 0.0);
	o_str(" fps, ");
	o_fix2(bench_frames > 0 ? (double)bench_ticks * 1000.0 /
				  ((double)PHYS_HZ * (double)bench_frames) : 0.0);
	o_str(" ms/frame");
	o_out(0);

	t_start = get_sysvar(_hz_200);
	t_last = t_start;
	t_mark = t_start;

	for (;;) {
		long now;
		int steps;
		rect cur, dirty;

		/* Quit on any key. Bconstat is the non-blocking half of the
		 * console: a blocking read is what hangs real hardware. */
		if (Bconstat(2)) {
			(void)Bconin(2);
			break;
		}

		now = get_sysvar(_hz_200);
		steps = (int)(now - t_last);
		t_last = now;
		/* At 60 frames a second the clock advances 3 or 4 ticks per
		 * frame. More than that means a retrace went by unused --
		 * a dropped frame, and the one performance figure a 200 Hz
		 * clock can honestly resolve here. */
		if (steps > DROP_STEPS)
			dropped++;
		if (steps > MAX_STEPS)
			steps = MAX_STEPS;
		while (steps-- > 0)
			physics_step();

		update_shape();
		cur = shape_rect();
		dirty = rect_union(cur, prev_rect[page]);
		compose(dirty);
		blit(dirty);

		prev_rect[page] = cur;
		plat_present();
		if (++page >= scr.pages)
			page = 0;
		frames++;

		/* One performance line every LOG_SECS seconds: the interval's
		 * own rate, so a warm-up or a slow patch is visible and not
		 * averaged away. */
		if (now - t_mark >= LOG_EVERY) {
			double iv = (double)(now - t_mark) / (double)PHYS_HZ;
			long df = frames - f_mark;

			o_str("BALL: t=");
			o_fix2((double)(now - t_start) / (double)PHYS_HZ);
			o_str(" s  ");
			o_dec(df);
			o_str(" frames  ");
			o_fix2(iv > 0.0 ? (double)df / iv : 0.0);
			o_str(" fps  ");
			o_dec(dropped - d_mark);
			o_str(" dropped");
			o_out(0);

			t_mark = now;
			f_mark = frames;
			d_mark = dropped;
		}
	}

	t_end = get_sysvar(_hz_200);
	plat_close();

	secs = (double)(t_end - t_start) / (double)PHYS_HZ;

	o_reset();
	o_str("BALL: ");
	o_dec(frames);
	o_str(" frames in ");
	o_fix2(secs);
	o_str(" s = ");
	o_fix2(secs > 0.0 ? (double)frames / secs : 0.0);
	o_str(" fps presented, ");
	o_dec(dropped);
	o_str(" dropped");
	o_flush();

done:
	b_free(tex_buf);
	b_free(run1);
	b_free(run0);
	b_free(map_shade);
	b_free(map_off);
	b_free(bg_row);
	b_free(idx);
	return rc;
}
