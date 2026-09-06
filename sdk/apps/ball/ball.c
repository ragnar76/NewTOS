/*
 * ball.c -- BALL.NAP, the NewTOS bouncing-ball demo, milestone 1.
 *
 * A ball falls, bounces, rolls along the floor and off the side walls, and
 * casts a shadow that shrinks and fades as it rises. It is drawn as a shaded
 * sphere -- one light, a diffuse term and a specular highlight -- which is
 * where the three-dimensional impression comes from. Milestone 1 deliberately
 * stops there: no texture on the ball, no wireframe room, no reflection, no
 * status readout, and exactly ONE ball (the illustration's trail of
 * half-transparent spheres is the illustrator's way of drawing motion, not
 * something the program does).
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
 * The ball is not shaded per frame. Its size and lighting never change, so
 * the sphere is rendered once at start-up into a sprite with a per-row
 * coverage run, and each frame copies rows of it. That also keeps the square
 * root -- there is no libm here -- out of the frame loop entirely.
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

/* ------------------------------------------------------------------------ */
/* Palette layout                                                            */
/*                                                                           */
/* 256 entries, spent where the picture needs them:                          */
/*   0 .. 63    backdrop, 64 bands from the top of the screen to the horizon */
/*  64 ..191    floor, 16 bands x 8 shadow levels (level 0 = no shadow), so   */
/*              a shadowed floor pixel is simply "floor band + level"        */
/* 192 ..255    the ball, 64 shades from silhouette-dark to specular white   */
/*                                                                           */
/* This is the layout an 8-bit palette machine forces, so it is the layout    */
/* both targets use.                                                         */
/* ------------------------------------------------------------------------ */
#define SKY_N		64
#define SKY_BASE	0
#define FLOOR_N		16
#define SHADOW_N	8
#define FLOOR_BASE	64
#define BALL_N		64
#define BALL_BASE	192

/* ------------------------------------------------------------------------ */
/* Shape and motion, all as fractions of the screen the VDI reported          */
/* ------------------------------------------------------------------------ */
#define DIA_FRAC	0.16f	/* ball diameter / screen width		     */
#define DIA_MAX_FRAC	0.18f	/* hard ceiling on the same		     */
#define HORIZON_FRAC	0.62f	/* where the floor begins		     */
#define FLOOR_FRAC	0.90f	/* the line the ball's underside touches     */
#define GRAVITY_H	1.90f	/* screen heights per second squared	     */
#define RESTITUTION	0.80f	/* speed kept across a bounce		     */
#define ROLL_FRICTION	0.995f	/* horizontal speed kept across a bounce     */
#define VX0_W		0.30f	/* start speed, screen widths per second     */
#define APEX_HEADROOM	0.92f	/* of the room above the floor, so the ball  */
				/* stays fully on screen at its highest       */
#define APEX_MIN_FRAC	0.16f	/* below this share of the apex, serve again */
#define VX_MIN_FRAC	0.35f	/* below this share of VX0, push again	     */

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
static unsigned char *sprite;		/* D*D, the pre-shaded ball	     */
static short *run0, *run1;		/* D each, the sprite's row coverage */

/* ---- the ball ----------------------------------------------------------- */
static int R, D;			/* radius / diameter in pixels	     */
static float Rf;
static float bx, by;			/* centre, pixels		     */
static float vx, vy;			/* pixels per second		     */
static float gravity;
static float v_launch, v_min;		/* serve / re-serve speeds	     */
static float vx0, vx_min;

/* ---- derived per frame -------------------------------------------------- */
static int b_left, b_top;		/* sprite origin on screen	     */
static float sh_cx, sh_cy, sh_a, sh_b, sh_al;	/* the shadow ellipse	     */
static int sh_x0, sh_y0, sh_x1, sh_y1;

static ball_rgb pal[BALL_PAL_N];
static rect prev_rect[2];		/* what each screen page still shows */

/* ------------------------------------------------------------------------ */
/* Small helpers: no libm, no printf                                         */
/* ------------------------------------------------------------------------ */

/*
 * Square root. There is no libm here and none is wanted: this is called while
 * the sprite is built and to work out the two serve speeds, never inside the
 * frame loop. The seed halves the IEEE-754 exponent (binary32 on both
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
/* Palette                                                                   */
/* ------------------------------------------------------------------------ */
static void build_palette(void)
{
	int i, band, lv, r, g, b;
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

	/* The ball: Atari red, black at the silhouette, blowing out to white
	 * in the specular highlight. */
	for (i = 0; i < BALL_N; i++) {
		t = (float)i / (float)(BALL_N - 1);
		if (t <= 0.72f) {
			float k = t / 0.72f;
			r = mixc(0, 222, k);
			g = mixc(0, 40, k);
			b = mixc(0, 34, k);
		} else {
			float u = (t - 0.72f) / 0.28f;
			r = mixc(222, 255, u);
			g = mixc(40, 255, u);
			b = mixc(34, 255, u);
		}
		pal[BALL_BASE + i].r = (unsigned char)r;
		pal[BALL_BASE + i].g = (unsigned char)g;
		pal[BALL_BASE + i].b = (unsigned char)b;
	}
}

static void build_bg_rows(void)
{
	int y;

	for (y = 0; y < H; y++) {
		if (y < horizon) {
			int band = (int)((long)y * SKY_N / (long)horizon);
			if (band > SKY_N - 1)
				band = SKY_N - 1;
			bg_row[y] = (unsigned char)(SKY_BASE + band);
		} else {
			int band = (int)((long)(y - horizon) * FLOOR_N /
					 (long)(H - horizon));
			if (band > FLOOR_N - 1)
				band = FLOOR_N - 1;
			bg_row[y] = (unsigned char)(FLOOR_BASE +
						    band * SHADOW_N);
		}
	}
}

/* ------------------------------------------------------------------------ */
/* The ball sprite: shaded once, copied ever after                           */
/* ------------------------------------------------------------------------ */
static void build_sprite(void)
{
	float lx = -0.45f, ly = -0.60f, lz = 0.66f;
	float hx, hy, hz, n;
	int sx, sy;

	n = b_sqrt(lx * lx + ly * ly + lz * lz);
	lx /= n;
	ly /= n;
	lz /= n;

	/* Blinn-Phong halfway vector, the viewer looking straight down +z. */
	hx = lx;
	hy = ly;
	hz = lz + 1.0f;
	n = b_sqrt(hx * hx + hy * hy + hz * hz);
	hx /= n;
	hy /= n;
	hz /= n;

	for (sy = 0; sy < D; sy++) {
		float fy = ((float)sy + 0.5f - Rf) / Rf;
		int first = -1, last = -1;

		for (sx = 0; sx < D; sx++) {
			float fx = ((float)sx + 0.5f - Rf) / Rf;
			float q = fx * fx + fy * fy;
			float nz, dif, sp, lit;
			int s;

			if (q > 1.0f)
				continue;

			/* The sphere's surface normal at this pixel. */
			nz = b_sqrt(1.0f - q);

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

			lit = 0.10f + 0.80f * dif + 0.80f * sp;
			if (lit > 1.0f)
				lit = 1.0f;

			s = (int)(lit * (float)(BALL_N - 1) + 0.5f);
			if (s < 0)
				s = 0;
			if (s > BALL_N - 1)
				s = BALL_N - 1;

			sprite[(long)sy * D + sx] =
				(unsigned char)(BALL_BASE + s);
			if (first < 0)
				first = sx;
			last = sx;
		}

		/* A circle covers one contiguous run per row, which is what
		 * lets the frame loop copy rows instead of testing a mask. */
		run0[sy] = (short)(first < 0 ? 0 : first);
		run1[sy] = (short)(first < 0 ? 0 : last + 1);
	}
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
		by = floor_y - Rf;
		vy = -vy * RESTITUTION;
		vx *= ROLL_FRICTION;
		/* A demo has to keep going. When the bounce has decayed to
		 * almost nothing, serve again at full height -- a strong
		 * bounce rather than a teleport, so the motion stays honest. */
		if (-vy < v_min)
			vy = -v_launch;
	}

	if (bx - Rf < 0.0f) {
		bx = Rf;
		vx = -vx;
	} else if (bx + Rf > (float)W) {
		bx = (float)W - Rf;
		vx = -vx;
	}

	if (b_abs(vx) < vx_min)
		vx = (vx < 0.0f) ? -vx0 : vx0;
}

/* ------------------------------------------------------------------------ */
/* Geometry of the current frame                                             */
/* ------------------------------------------------------------------------ */
static void update_shape(void)
{
	float hgt, k, sz;

	b_left = (int)(bx - Rf);
	b_top = (int)(by - Rf);

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
	sh_a = Rf * 1.20f * sz;
	sh_b = Rf * 0.30f * sz;
	sh_cx = bx;
	sh_cy = floor_y + sh_b * 0.20f;

	sh_x0 = (int)(sh_cx - sh_a);
	sh_x1 = (int)(sh_cx + sh_a) + 1;
	sh_y0 = (int)(sh_cy - sh_b);
	sh_y1 = (int)(sh_cy + sh_b) + 1;
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
		float dy = ((float)y + 0.5f - sh_cy) / sh_b;
		float ry = dy * dy;

		if (ry >= 1.0f)
			continue;
		for (x = x0; x < x1; x++) {
			float dx = ((float)x + 0.5f - sh_cx) / sh_a;
			float q = dx * dx + ry;
			int lv;

			if (q >= 1.0f)
				continue;
			/* 1 - q^2 keeps the middle of the shadow flat and lets
			 * the rim fall off softly, which is what a real
			 * penumbra looks like at this size. */
			q = 1.0f - q * q;
			lv = (int)(sh_al * (float)(SHADOW_N - 1) * q + 0.5f);
			if (lv <= 0)
				continue;
			if (lv > SHADOW_N - 1)
				lv = SHADOW_N - 1;
			row[x] = (unsigned char)(base + lv);
		}
	}
}

static void draw_ball(rect r)
{
	int sy;

	for (sy = 0; sy < D; sy++) {
		int y = b_top + sy;
		int a, b;

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
		memcpy(idx + (long)y * W + a,
		       sprite + (long)sy * D + (a - b_left),
		       (size_t)(b - a));
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
	sprite = (unsigned char *)b_alloc((long)D * D);
	run0 = (short *)b_alloc((long)D * (long)sizeof(short));
	run1 = (short *)b_alloc((long)D * (long)sizeof(short));
	if (!idx || !bg_row || !sprite || !run0 || !run1) {
		plat_close();
		Cconws("BALL: out of memory\r\n");
		rc = 1;
		goto done;
	}

	build_palette();
	plat_palette(pal);
	build_bg_rows();
	build_sprite();

	/* Serve speeds. The apex is as high as the ball can go and still be
	 * fully on screen, with a little headroom. */
	gravity = GRAVITY_H * (float)H;
	apex_px = (floor_y - (float)D) * APEX_HEADROOM;
	if (apex_px < (float)D)
		apex_px = (float)D;
	v_launch = b_sqrt(2.0f * gravity * apex_px);
	v_min = b_sqrt(2.0f * gravity * apex_px * APEX_MIN_FRAC);
	vx0 = VX0_W * (float)W;
	vx_min = VX_MIN_FRAC * vx0;
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
	b_free(run1);
	b_free(run0);
	b_free(sprite);
	b_free(bg_row);
	b_free(idx);
	return rc;
}
