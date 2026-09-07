/*
 * sim.c -- run the bouncing-ball demo's own source on the host, and measure
 * what it does.
 *
 * The demo is deterministic by construction: a fixed wind seed, a fixed time
 * step, and plan_cycle() choosing the traverse from measured integers. So a
 * question like "which frames are expensive, and what is the ball doing at
 * those moments" has an answer that does not need the Atari, and this is how
 * it is asked. The point of the exercise was a burst of dropped frames every
 * 16.7 seconds in two PiStorm logs; what it established was that the demo's
 * per-frame cost has no such period, and the arithmetic that followed is in
 * the STATUS entry of 2026-09-07.
 *
 * ---------------------------------------------------------------------------
 * How it runs the real source
 * ---------------------------------------------------------------------------
 * ball.c is #included, not linked. That is deliberate and it is the whole
 * trick: everything in ball.c is static, so including it puts the physics,
 * the geometry and the buffers in THIS translation unit where an instrument
 * can read them -- bx, by, the wind phases, the dirty rectangle -- without
 * ball.c being modified by so much as a comment to make itself measurable.
 * The file measured is the file that ships.
 *
 * Everything ball.c calls that the host does not have is supplied here: the
 * six functions of the seam (ball.h), and the handful of TOS calls, through
 * the shim headers in mint/.
 *
 * ---------------------------------------------------------------------------
 * The virtual 200 Hz clock, which is the interesting part
 * ---------------------------------------------------------------------------
 * The demo reads the system clock, and what it reads has to behave like an
 * Atari's or nothing measured here means anything. So:
 *
 *   - reading the clock is what makes time pass. Every read adds the host
 *     CPU time actually spent since the last one, scaled up (-s) so that a
 *     composed frame costs about what it costs on the target. The demo's own
 *     four-part split therefore charges each section what that section
 *     really did.
 *   - blitting costs what the host does not pay: plat_blit() adds a modelled
 *     cost per pixel (-k), because the host has no card bus.
 *   - plat_present() snaps the clock to the next retrace (-r), exactly the
 *     way Vsync() does. Set -r to a whole number of ticks and the frame's
 *     phase within the tick never moves, which is a real property of a 50 Hz
 *     screen on a 200 Hz clock and the reason the demo's sub-tick figures
 *     are noisy there; set it to 49.985 and the phase walks as it does on
 *     the real machine.
 *
 * With -s 0 the clock is driven by the blit model alone and the whole run is
 * deterministic, which is what the picture-for-picture comparisons use.
 *
 * ---------------------------------------------------------------------------
 * What it can be asked
 * ---------------------------------------------------------------------------
 *   -n N     stop after N presented frames
 *   -o FILE  per-frame CSV: tick, work, dirty area, blits, ball state, the
 *            dirty rectangle, and the host CPU time of the frame body
 *   -S FILE  an FNV hash of the whole composed picture, one line per frame.
 *            This is how a change is shown to draw the same picture as the
 *            revision before it.
 *   -D N -P FILE   dump frame N as a PPM, to look at
 *   -L FILE  where the demo's own log goes (it thinks it is C:\BALL.LOG)
 *   -r HZ    the retrace to lock to          (default 50)
 *   -s F     virtual seconds per host second (default 120; 0 = deterministic)
 *   -k T     ticks per blitted pixel         (default 2.5e-5)
 *   -c T     ticks per blit call             (default 0.02)
 *
 * Build with build-hostsim.sh, which builds it at both composition depths.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <time.h>

static double host_ns(void)
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	return (double)ts.tv_sec * 1e9 + (double)ts.tv_nsec;
}

#define main ball_main
#include "../ball.c"
#undef main

/* ---- the virtual machine ------------------------------------------------ */
static double now_f;			/* 200 Hz clock, fractional	     */
static double vbl_f;			/* when the next retrace lands	     */
static double VBL_PERIOD = 200.0 / 50.0;
static double K_PX = 2.5e-5;		/* ticks per blitted pixel	     */
static double K_CALL = 0.02;		/* ticks per blit call		     */

static long frame_no;
static double frame_t0_ns;
static double frame_ns;
static double frame_start_f;
static long frame_area;
static int fr_x0, fr_y0, fr_x1, fr_y1, fr_have;
static int frame_blits;
static FILE *csv;
static FILE *sumf;
static long sim_frames_max = 4600;
static int quit_now;

/*
 * The virtual 200 Hz clock. It advances with the host CPU time actually
 * spent since the last reading, scaled up so that the composed frame costs
 * about what it costs on the Atari, plus the virtual cost of the blits the
 * host does not perform. Reading it is therefore what makes time pass --
 * which is exactly the property the demo's own four-part split relies on.
 */
static double last_ns;
static double CPU_SCALE = 120.0;	/* virtual seconds per host second   */

static void clock_advance(void)
{
	double t = host_ns();

	if (last_ns != 0.0)
		now_f += (t - last_ns) * 1e-9 * 200.0 * CPU_SCALE;
	last_ns = t;
}

long host_hz200(void) { clock_advance(); return (long)now_f; }
void host_cconws(const char *s) { fputs(s, stderr); }
int host_bconstat(int d) { (void)d; return quit_now; }
long host_bconin(int d) { (void)d; return 13; }
static FILE *hf[8];
static const char *logname = "BALL.LOG";
long host_fopen(const char *p, int m)
{
	int i;
	(void)m;
	for (i = 1; i < 8; i++) if (!hf[i]) break;
	if (i == 8) return -35;
	if (!strstr(p, "BALL.LOG")) return -33;	/* generated texture, as on the ST */
	hf[i] = fopen(logname, "r+b");
	return hf[i] ? i : -33;
}
long host_fcreate(const char *p, int a)
{
	int i;
	(void)a;
	for (i = 1; i < 8; i++) if (!hf[i]) break;
	if (i == 8) return -35;
	if (!strstr(p, "BALL.LOG")) return -33;
	hf[i] = fopen(logname, "w+b");
	return hf[i] ? i : -36;
}
long host_fclose(int fd) { if (fd>0&&fd<8&&hf[fd]) { fclose(hf[fd]); hf[fd]=0; } return 0; }
long host_fread(int fd, long n, void *b)
{ return (fd>0&&fd<8&&hf[fd]) ? (long)fread(b,1,(size_t)n,hf[fd]) : -1; }
long host_fwrite(int fd, long n, const void *b)
{ return (fd>0&&fd<8&&hf[fd]) ? (long)fwrite(b,1,(size_t)n,hf[fd]) : -1; }
long host_fseek(long o, int fd, int m)
{ if (fd<=0||fd>=8||!hf[fd]) return -1; fseek(hf[fd], o, m==2?SEEK_END:(m==1?SEEK_CUR:SEEK_SET)); return ftell(hf[fd]); }
void *host_malloc(long n) { return calloc(1, (size_t)n); }
void host_free(void *p) { free(p); }
void host_vsync(void) { }

int plat_open(ball_screen *s)
{
	s->w = 640; s->h = 480; s->pages = 1;
	return 0;
}
#if !BALL_TRUECOLOR
void plat_palette(const ball_rgb *p) { (void)p; }
#endif

/* One frame of the composed picture, as a PPM, so it can be looked at. */
static long dump_at = -1;
static const char *dump_name = "frame.ppm";

static void dump_ppm(void)
{
	FILE *f = fopen(dump_name, "wb");
	long x, y;

	if (!f) return;
	fprintf(f, "P6\n%d %d\n255\n", W, H);
	for (y = 0; y < H; y++)
		for (x = 0; x < W; x++) {
			ball_px p = idx[y * (long)W + x];
			unsigned char c[3];
#if BALL_TRUECOLOR
			c[0] = (unsigned char)((p >> 16) & 0xFF);
			c[1] = (unsigned char)((p >> 8) & 0xFF);
			c[2] = (unsigned char)(p & 0xFF);
#else
			c[0] = pal[p].r; c[1] = pal[p].g; c[2] = pal[p].b;
#endif
			fwrite(c, 1, 3, f);
		}
	fclose(f);
}
const char *plat_error(void) { return ""; }
void plat_close(void) { }

void plat_blit(const ball_px *src, long pitch, int x, int y, int w, int h)
{
	(void)src; (void)pitch; (void)x; (void)y;
	if (!fr_have) { fr_x0 = x; fr_y0 = y; fr_x1 = x + w; fr_y1 = y + h; fr_have = 1; }
	frame_area += (long)w * (long)h;
	frame_blits++;
	now_f += (double)w * (double)h * K_PX + K_CALL;
}

void plat_present(void)
{
	double work;

	clock_advance();
	work = now_f - frame_start_f;

	frame_ns = host_ns() - frame_t0_ns;
	double hgt = (double)floor_y - ((double)by + (double)Rf);
	long tick_in = (long)frame_start_f;

	{
		unsigned long h = 2166136261UL;
		long k, np = (long)W * (long)H;
		for (k = 0; k < np; k++) { h ^= idx[k]; h *= 16777619UL; }
		if (sumf) fprintf(sumf, "%ld %08lx\n", frame_no, h & 0xffffffffUL);
	}
	if (frame_no == dump_at)
		dump_ppm();
	if (csv)
		fprintf(csv,
			"%ld,%ld,%.6f,%.6f,%ld,%d,%.3f,%.3f,%.3f,%.3f,%.4f,%.3f,%d,%d,%d,%d,%d,%d,%d\n",
			frame_no, tick_in, frame_start_f, work,
			frame_area, frame_blits,
			(double)bx, (double)by, hgt, (double)vy,
			(double)wind_v, frame_ns / 1000.0,
			fr_x0, fr_y0, fr_x1, fr_y1, b_left, b_top,
			(b_left < scrim[0].x1 && b_left + D > scrim[0].x0 &&
			 b_top < scrim[0].y1 && b_top + D > scrim[0].y0) ? 1 : 0);

	/* wait for the retrace, with optional sampling jitter */
	while (vbl_f <= now_f)
		vbl_f += VBL_PERIOD;
	now_f = vbl_f;

	frame_no++;
	frame_start_f = now_f;
	frame_t0_ns = host_ns();
	frame_area = 0;
	frame_blits = 0;
	fr_have = 0;
	if (frame_no >= sim_frames_max)
		quit_now = 1;
}

int main(int argc, char **argv)
{
	const char *out = "frames.csv";
	int i;

	for (i = 1; i < argc; i++) {
		if (!strcmp(argv[i], "-o") && i + 1 < argc) out = argv[++i];
		else if (!strcmp(argv[i], "-n") && i + 1 < argc) sim_frames_max = atol(argv[++i]);
		else if (!strcmp(argv[i], "-k") && i + 1 < argc) K_PX = atof(argv[++i]);
		else if (!strcmp(argv[i], "-c") && i + 1 < argc) K_CALL = atof(argv[++i]);
		else if (!strcmp(argv[i], "-r") && i + 1 < argc) VBL_PERIOD = 200.0 / atof(argv[++i]);
		else if (!strcmp(argv[i], "-s") && i + 1 < argc) CPU_SCALE = atof(argv[++i]);
		else if (!strcmp(argv[i], "-L") && i + 1 < argc) logname = argv[++i];
		else if (!strcmp(argv[i], "-S") && i + 1 < argc) sumf = fopen(argv[++i], "w");
		else if (!strcmp(argv[i], "-D") && i + 1 < argc) dump_at = atol(argv[++i]);
		else if (!strcmp(argv[i], "-P") && i + 1 < argc) dump_name = argv[++i];
	}
	csv = fopen(out, "w");
	{
		unsigned long h = 2166136261UL;
		long k, np = (long)W * (long)H;
		for (k = 0; k < np; k++) { h ^= idx[k]; h *= 16777619UL; }
		if (sumf) fprintf(sumf, "%ld %08lx\n", frame_no, h & 0xffffffffUL);
	}
	if (frame_no == dump_at)
		dump_ppm();
	if (csv)
		fprintf(csv, "frame,tick,tick_f,work,area,blits,bx,by,hgt,vy,wind,cpu_us,x0,y0,x1,y1,bl,bt,hudhit\n");
	frame_t0_ns = host_ns();
	ball_main();
	if (csv) fclose(csv);
	fprintf(stderr, "sim: bench_frames=%ld bench_ticks=%ld cyc_v=%ld cyc_h=%ld frames=%ld\n",
		bench_frames, bench_ticks, cyc_v, cyc_h, frame_no);
	return 0;
}
