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
/*   96 .. 119   the room: ONE ramp, from just above the brightest backdrop   */
/*               to a blue-white, and it does three jobs. It is the grid's    */
/*               glow (a line's brightness IS its coverage times its depth    */
/*               fade, so the room is anti-aliased for nothing), it is the    */
/*               lettering, and it is what lets a line be composited by       */
/*               BRIGHTNESS rather than by blending -- a glowing line may     */
/*               only ever lighten what is under it, whatever that is.       */
/*  120 .. 127   still reserved, still black, still used by nothing          */
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
#define GRID_BASE	96	/* the room's lines and the lettering	     */
#define GRID_N		24
#define RSVD_BASE	120	/* what milestone 2b did not need	     */
#define RSVD_N		8
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
	 FLOOR_BASE + FLOOR_N * SHADOW_N == GRID_BASE &&
	 GRID_BASE + GRID_N == RSVD_BASE &&
	 RSVD_BASE + RSVD_N == BALL_BASE &&
	 BALL_BASE + BALL_N == BALL_PAL_N) ? 1 : -1];
static unsigned char hue_base[TEX_MAT_MAX];
static ball_px shade_lut[BALL_LEVELS * TEX_MAT_MAX];

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

/*
 * Wind: a sideways push that bends every arc a little differently.
 *
 * It acts on the HORIZONTAL axis and on nothing else. That is not a
 * simplification, it is the whole reason this is safe: the vertical motion --
 * and therefore the apex, and therefore the promise that the ball never loses
 * energy -- is not merely unaffected in practice, it is untouched by
 * construction. No line below reads or writes vy or by.
 *
 * It is written as the VELOCITY the wind produces, not as an acceleration
 * integrated step by step. A per-step random push has no restoring force: the
 * offset it accumulates is a random walk, so after an hour the ball is either
 * hopping in place or has run away, and "within one per cent" has long since
 * stopped being true. A sum of sines is the integral of a sideways force
 * written down in closed form -- so it is BOUNDED BY CONSTRUCTION, exactly
 * WIND_FRAC of the roll speed, with no clamp to hit and nothing to drift.
 * That bound is also what makes the ball safe at the walls: the effective
 * speed can never reach zero, so it can never hover against one.
 *
 * A mean-reverting random process would also stay near the nominal, but only
 * with high probability and only with a clamp to make the bound real. The
 * sines give the same look with a theorem instead of a probability, and they
 * need no generator state at all -- only three phases.
 *
 * The periods are the measured bounce times successive powers of the golden
 * ratio. Their ratios to each other and to the bounce are then as far from
 * simple fractions as numbers get, which is the same reason plan_cycle()
 * exists and the reason the wind cannot itself fall into step with the
 * bouncing. Note that they are NOT whole ticks -- unlike the bounce and the
 * traverse -- so the wind alone already breaks the exact rationality that
 * plan_cycle() has to work around. The two compose; neither replaces the
 * other, and with the wind switched off the good period is still there.
 *
 * WIND_SEED chooses which wind you get, not how strong it is. Fixed, because
 * there is no entropy on a bare machine anyway and because a benchmark that
 * runs a different path every time measures a different workload every time.
 * Set it to 0 to seed from the clock instead and have every run differ.
 *
 * HOW STRONG. The idea came with "say, the 1 % range" attached, and 1 % turns
 * out to be invisible. Measured at 1920x1080 as how far an arc departs from
 * the straight horizontal line it would follow at a constant speed -- which
 * is exactly zero without wind, so it isolates what the wind does -- averaged
 * over eighteen wall-free arcs:
 *
 *      wind      0 %    1 %    4 %    6 %    8 %   10 %
 *      bend     0.8    1.4    3.7    5.4    7.1    8.6  px    (mean)
 *               0.9    2.1    7.0   10.4   13.6   16.9  px    (worst)
 *
 * 0.8 px is the noise floor of the measurement, so 1 % is a little over half
 * a pixel of real effect on a 306-pixel ball: not subtle, absent. 8 % puts
 * the bend at about 2 % of the ball's diameter -- enough that consecutive
 * arcs are visibly not the same arc, while the speed changes by at most about
 * 5 % within any one arc, which reads as a push rather than as a stutter.
 * Turn it down if it is too much; the number is here and the table says what
 * you will get.
 */
#define WIND_FRAC	0.08f	/* peak deviation, as a share of the roll     */
#define WIND_TERMS	3
#define WIND_SEED	0x5EEDBA11u

/* Physics step: exactly one tick of the 200 Hz system clock. */
#define PHYS_HZ		200
#define PHYS_DT		(1.0f / (float)PHYS_HZ)
/* Never try to catch up more than a fifth of a second at once: after a stall
 * the backlog is dropped instead of chased, which is what keeps a slow frame
 * from turning into a spiral of slower ones. */
#define MAX_STEPS	40
/*
 * Ticks a frame may consume before a retrace went by unused. It is derived
 * from the retrace the demo MEASURED (measure_retrace below), not from an
 * assumed refresh rate, and the difference is not academic: at 60 Hz a frame
 * is 3.33 ticks of a 200 Hz clock and a threshold of 4 sits two thirds of a
 * tick clear of it, but at 50 Hz a frame is 4.00 ticks EXACTLY and the same
 * threshold sits on top of the nominal value. Everything that then jogs the
 * sampling by one tick -- and the two clocks are independent crystals, so
 * the phase between them walks all day -- reads as a dropped frame that
 * never happened. That is precisely what the PiStorm logs of 2026-09-07
 * show: bursts of 7 to 13 "dropped" frames in intervals that presented the
 * same 249 frames in the same 1000 ticks as the quiet ones.
 *
 * A frame that really did let a retrace go by cost two retrace periods, so
 * one and a half is the line between the two, wherever the retrace happens
 * to be. Below it is the clock's own quantisation and nothing else.
 */
#define DROP_FRAC_NUM	3	/* one and a half retraces ... */
#define DROP_FRAC_DEN	2

/* ------------------------------------------------------------------------ */
/* The room                                                                  */
/*                                                                           */
/* A wireframe box seen from inside it, in ONE-POINT perspective: a back wall */
/* square to the screen, a floor, and the two side walls. There is no         */
/* ceiling -- the walls are tall enough to leave the top of the frame, so the */
/* room reads as one that continues upwards rather than one with a lid.       */
/*                                                                           */
/* All of it comes from a vanishing point and a far rectangle, and the whole  */
/* projection is then ONE number per depth. Write                             */
/*                                                                           */
/*      m = (y - vp_y) / drop                                                 */
/*                                                                           */
/* for a floor row -- drop being how far the far wall's foot sits below the   */
/* vanishing point -- and every point of the far rectangle at that depth is   */
/*                                                                           */
/*      screen = vp + m * (far_point - vp)                                    */
/*                                                                           */
/* so the floor's transverse line, the two side walls' uprights and the       */
/* ceiling line that would go with them all fall out of the same m and line   */
/* up at the corners by construction rather than by tuning.                   */
/*                                                                           */
/* A grid whose cells are the same size in the ROOM has                       */
/* 1/(y - vp_y) evenly spaced -- depth is what is uniform, not screen         */
/* distance -- so the depths are m_i = 1 / (1 - i*e), with e chosen to put    */
/* the last one exactly on the bottom row of the screen. That is the only     */
/* place perspective arithmetic appears in this file, and it runs once.       */
/*                                                                           */
/* WHERE THE ROOM IS PINNED. The floor meets the back wall on the horizon     */
/* row the backdrop gradient already had (HORIZON_FRAC), so the room needs no */
/* new gradient and no new palette: above that line the backdrop is the       */
/* walls' haze, below it the floor's is the floor's. Only the two thin        */
/* wedges where a side wall passes BELOW the horizon are new, and they take   */
/* the backdrop's colour instead of the floor's -- which is what makes the    */
/* wall look like a wall there and not like a bright triangle of floor.       */
/*                                                                           */
/* The numbers below are read off the reference render, converted into these  */
/* units: its vanishing point sits 0.15 screen heights above the floor's      */
/* junction with the wall, the wall is about 3.2 of those drops to each side  */
/* and 2.5 of them tall above the vanishing point, and the floor is ten cells */
/* wide and ten deep. The half-width is capped as a share of the screen so    */
/* that on a 4:3 display -- where the same vertical setup would make the wall */
/* nearly as wide as the picture -- both side walls still show.               */
/* ------------------------------------------------------------------------ */
#define ROOM_DROP_FRAC	0.155f	/* horizon - vanishing point, over H	     */
#define ROOM_HW_U	3.60f	/* far wall half-width, in drops	     */
#define ROOM_HW_MAX	0.36f	/* ... but never more than this much of W    */
#define ROOM_UP_U	2.50f	/* far wall top above the VP, in drops	     */
#define ROOM_M		10	/* cells across the room		     */
#define ROOM_N		10	/* cells from the wall to the bottom row     */
#define ROOM_K_MAX	16	/* ceiling on the wall's own row count	     */
#define ROOM_FADE_FAR	0.55f	/* how bright a line is at the back wall     */

/*
 * A line is not one pixel wide: it has a flat core, a soft shoulder and a
 * wide, faint halo, and all three are evaluated as a function of the pixel's
 * PERPENDICULAR distance from the segment. That is what makes the converging
 * floor lines smooth instead of stepped, and it is free -- the room is drawn
 * once into its own image and never again.
 *
 * The core and the shoulder scale with the screen so the room looks the same
 * at 640x480 as at 1920x1080; the minimum keeps a line visible when the
 * scaling would make it thinner than a pixel.
 */
#define LINE_CORE_FRAC	0.00042f
#define LINE_CORE_MIN	0.35f
#define LINE_SOFT_FRAC	0.00071f
#define LINE_SOFT_MIN	0.90f
#define LINE_GLOW	0.16f	/* the halo, as a share of the core	     */
#define LINE_GLOW_MUL	3.0f	/* ... and how far out it reaches	     */

/* ------------------------------------------------------------------------ */
/* The floor reflection                                                      */
/*                                                                           */
/* The floor is a mirror, so what stands on it is drawn again upside down.    */
/* In this picture that is exact rather than approximate: the floor's depth   */
/* is foreshortened (FLOOR_SQUASH) but its height is not, so a point h above  */
/* the contact line reflects to h BELOW it, at the same column -- the mirror  */
/* is a mirror in screen space too. Turning the ball's sprite upside down     */
/* also turns its shading and its texture upside down, which is precisely     */
/* what a mirror does to them, so the reflection is the sphere map read with  */
/* the rows reversed and costs not one new pixel of set-up.                   */
/*                                                                           */
/* It is DARKENED without a single new palette entry. Every hue's ramp        */
/* already runs from black through its own colour to the highlight, so a      */
/* darker copy of the ball is the same ramp read at a lower LIGHT LEVEL --    */
/* refl_lut is shade_lut with the level scaled, and the inner loop is the     */
/* ball's inner loop with a different table. Spending 128 more entries on a   */
/* dimmed duplicate would have had to come out of the sphere's own ramps.     */
/*                                                                           */
/* It fades with distance below the contact line in two ways at once, because */
/* an 8-bit palette cannot blend: the level scale falls (REFL_LUT_N steps of  */
/* it), and the pixel is only written when the ordered dither threshold lets  */
/* it through, so the floor -- grid lines and all -- shows through more and   */
/* more of it. That is also what makes the reflection recede as the ball      */
/* rises: its top edge is exactly as far below the contact line as the ball   */
/* is above it, so it dims and slides off the bottom of the screen by itself. */
/* ------------------------------------------------------------------------ */
#define REFL_DARK	0.72f	/* level scale where it touches the ball     */
#define REFL_TAIL	0.40f	/* ... and at the far end of the fade	     */
#define REFL_LUT_N	8	/* steps between the two		     */

/* ------------------------------------------------------------------------ */
/* The readout                                                               */
/*                                                                           */
/* Top right, in the demo's own font -- the SAME 5x7 block font the ball's    */
/* panels are lettered with (ball-tex.h). There is no second font here and    */
/* there is not going to be one.                                             */
/*                                                                           */
/* It is part of the scene, not a layer over it: compose() draws it last      */
/* inside whatever rectangle it was given, so a rectangle restored for the    */
/* ball also restores the lettering that crosses it, and the ball passes      */
/* BEHIND the readout instead of erasing it. Only the four numbers change, so */
/* only their four little rectangles are redrawn and blitted -- the titles,   */
/* the rule and the labels are never written to the screen again after the    */
/* first paint unless the ball flies through them.                            */
/* ------------------------------------------------------------------------ */
#define HUD_CHARS	20	/* the block's width, in characters	     */
#define HUD_LINES	7
#define HUD_FIRST_VAL	3	/* the first line with a number in it	     */
#define HUD_MARGIN	0.020f	/* off the screen edge, as a share of it     */
#define HUD_SCRIM_NUM	1	/* the room is dimmed to a quarter behind    */
#define HUD_SCRIM_DEN	4	/* the block, so the numbers stay readable   */
#define TXT_MAX		32
#define FPS_EVERY	(PHYS_HZ / 2)	/* how often the rate is re-measured */

typedef struct {
	int x0, y0, x1, y1;	/* x1 / y1 exclusive */
} rect;

/* ---- screen ------------------------------------------------------------- */
static ball_screen scr;
static int W, H;
static int horizon;			/* first floor row		     */
static float floor_y;			/* the ball's contact line	     */

/* ---- buffers (all from Mxalloc, TT-RAM preferred) ----------------------- */
static ball_px *idx;			/* W*H composition pixels	     */
/*
 * The room, painted once, W*H bytes of it.
 *
 * Milestone 2a restored the background under the ball from ONE index per row
 * (bg_row), which made the restore a memset. A grid room is not uniform along
 * a row, so that is over: the choice is between re-drawing the room's lines
 * clipped to the dirty rectangle every frame and keeping a picture of the
 * room to copy from. The picture wins, and not because it is simpler:
 *
 *   - it is what makes the room ANTI-ALIASED AND GLOWING FOR NOTHING. The
 *     rasteriser runs once at start-up, so it can afford a perpendicular
 *     distance, a soft shoulder and a halo per pixel. A per-frame line
 *     re-draw would have to be cheap, and cheap lines at 640x480 are stepped
 *     ones.
 *   - the shadow needs to know whether the pixel it is about to darken is
 *     floor or a glowing line, and the reflection needs the clean floor
 *     underneath it. Both read this image; neither needs a second answer.
 *   - the cost is a memcpy where there was a memset, over the dirty rectangle
 *     only. It is the same work on both machines, which is the point.
 *
 * It costs W*H bytes: 2.0 MB at 1920x1080 and 300 KB at 640x480, next to the
 * same again for the composition buffer. Both machines have it.
 */
static ball_px *bg;		/* W*H, the room with nothing in it  */
static ball_px *bg_row;			/* H, the background pixel per row   */
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
static int roll_dir;			/* which way it is rolling, +1 / -1  */
static float wind_a[WIND_TERMS];	/* amplitudes, pixels per second     */
static float wind_w[WIND_TERMS];	/* radians per tick		     */
static float wind_p[WIND_TERMS];	/* running phases		     */
static float wind_v;			/* what the wind adds right now	     */
static float spin_u;			/* spin, in texture columns	     */

/* ---- the room ----------------------------------------------------------- */
static float vp_x, vp_y;		/* the vanishing point		     */
static float rm_drop;			/* horizon - vp_y, in pixels	     */
static float rm_hw;			/* far wall half-width		     */
static float rm_up;			/* far wall top above vp_y	     */
static float rm_mn;			/* m on the bottom row of the screen */
static float rm_slope;			/* rm_hw / rm_drop, the wedge's edge */
static int rm_k;			/* rows of cells on the walls	     */
static float line_core, line_soft, line_glow_r;

/* ---- the reflection ----------------------------------------------------- */
static ball_px refl_lut[REFL_LUT_N][BALL_LEVELS * TEX_MAT_MAX];
static int fl_i;			/* the contact line, as a row	     */
static float refl_inv;			/* 1 / the fade length		     */
static int rf_top;			/* where the mirrored disc starts    */
static int rf_y0, rf_y1;		/* ... and the rows of it on screen  */

/* ---- the readout -------------------------------------------------------- */
static int txt_sc;			/* one scale for all the lettering   */
static int txt_halo;			/* the dark outline, in pixels	     */
static int hud_x, hud_y, hud_cw, hud_lh;
static rect scrim[3];		/* readout, and the two corner blocks  */
static int scrim_n;
static rect hud_vr[HUD_LINES];		/* the part of a line that changes   */
static char hud_line[HUD_LINES][TXT_MAX];
static char hud_page[2][HUD_LINES][TXT_MAX];
static long bounces;
static double hud_fps;

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

/*
 * The three things that move, and therefore the three rectangles that can be
 * dirty. What each screen page still shows is kept PER PART and PER PAGE:
 * a part's dirty rectangle is where it is now together with where it was when
 * this page was last drawn, and nothing else. See "The dirty rectangles".
 */
enum { DP_BALL, DP_SHADOW, DP_REFLECT, DP_N };

typedef struct {
	int n;
	rect r[DP_N];
} dirty_set;

static rect prev_part[2][DP_N];		/* what each page still shows	     */
static rect cur_part[DP_N];		/* ... and where the parts are now   */

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

/*
 * Sine, single precision, no libm. Same idea as the arc sine above and the
 * same discipline: the coefficients are the reciprocal odd factorials, and
 * they are DERIVED at start-up from the recurrence that defines them, not
 * typed in.
 *
 * The argument is folded into [-pi/2, pi/2] first, where nine terms leave the
 * error near 1e-13 -- the last one is about 4e-14 at pi/2. Used three times
 * per physics step for the wind, i.e. 600 times a second; the cost does not
 * register against a frame.
 */
#define SIN_TERMS	9
static float sin_c[SIN_TERMS];

static void sin_init(void)
{
	float f = 1.0f;			/* (2k+1)! */
	int k;

	sin_c[0] = 1.0f;
	for (k = 1; k < SIN_TERMS; k++) {
		f *= (float)(2 * k) * (float)(2 * k + 1);
		sin_c[k] = ((k & 1) ? -1.0f : 1.0f) / f;
	}
}

static float b_sin(float x)
{
	float x2, p, s;
	int k;

	while (x > B_PI)
		x -= 2.0f * B_PI;
	while (x < -B_PI)
		x += 2.0f * B_PI;
	if (x > 0.5f * B_PI)
		x = B_PI - x;
	else if (x < -0.5f * B_PI)
		x = -B_PI - x;

	x2 = x * x;
	p = x;
	s = x;
	for (k = 1; k < SIN_TERMS; k++) {
		p *= x2;
		s += sin_c[k] * p;
	}
	return s;
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

/* Four decimals, for a setting rather than a measurement: 0.16 and 0.1625
 * are different balls and two places cannot tell them apart. */
static void o_fix4(double v)
{
	long q;
	int i;

	if (v < 0.0)
		v = 0.0;
	q = (long)(v * 10000.0 + 0.5);
	o_dec(q / 10000);
	o_ch('.');
	for (i = 1000; i >= 1; i /= 10)
		o_ch((char)('0' + (int)((q / (long)i) % 10)));
}

/*
 * The readout builds its lines the same way, but into its own buffer and
 * without a log to flush: each of these appends and hands back the new end,
 * and the caller terminates. Three of them, plus a length and a compare, is
 * everything the demo needs of a string library -- which is why it has none.
 */
static char *f_str(char *p, const char *v)
{
	while (*v)
		*p++ = *v++;
	return p;
}

static char *f_dec(char *p, long v)
{
	char t[24];
	int n = 0;
	unsigned long u;

	if (v < 0) {
		*p++ = '-';
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
		*p++ = t[--n];
	return p;
}

/* One decimal, which is as much as a frame rate deserves. */
static char *f_fix1(char *p, double v)
{
	long q;

	if (v < 0.0)
		v = 0.0;
	q = (long)(v * 10.0 + 0.5);
	p = f_dec(p, q / 10);
	*p++ = '.';
	*p++ = (char)('0' + (int)(q % 10));
	return p;
}

static int s_len(const char *a)
{
	int n = 0;

	while (a[n])
		n++;
	return n;
}

static int s_eq(const char *a, const char *b)
{
	while (*a && *a == *b) {
		a++;
		b++;
	}
	return *a == *b;
}

static void s_cpy(char *d, const char *v)
{
	while ((*d++ = *v++) != 0)
		;
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
/* The settings, if there are any. Same reasoning as the two paths above: an
 * absolute one in the root of C:, because nothing sets a current directory
 * before a program is launched and the demo is meant to be one double-click
 * away. */
#define CFGPATH		"C:\\BALL.CFG"
#define LOG_SECS	5
#define LOG_EVERY	(LOG_SECS * PHYS_HZ)

static int log_ok;
/*
 * The lines are HELD IN MEMORY and committed in one go, and that is a
 * measurement decision rather than a tidiness one. A commit is
 * Fopen -> Fseek -> Fwrite -> Fclose: a directory search, a sector
 * read-modify-write and a directory update. On the Atari that is longer than
 * a whole 50 Hz frame, so a commit inside the frame loop lets exactly one
 * retrace go by, once every LOG_SECS seconds, for ever -- the demo hitching
 * on its own instrument. (It shows up in the two PiStorm logs as a floor of
 * one dropped frame in every five-second interval, in every interval of both
 * runs, and in the very first interval of a run -- the one whose log line is
 * written after the interval closes -- as none at all.)
 *
 * What that trades away is the old behaviour of a log that was valid after
 * every single line, so a run killed by the power switch still read back.
 * The header block is still committed on its own before the frame loop
 * starts, so an interrupted run still says which machine it was and what the
 * screen was; the per-interval lines arrive when the demo is asked to stop,
 * or whenever the buffer fills, whichever comes first.
 */
#define LOG_BUF_MAX	32768L		/* ~9 minutes of interval lines	     */
static char *log_buf;			/* b_alloc'd in main()		     */
static long log_len;

static void log_open(void)
{
	long fd = Fcreate(LOGPATH, 0);

	if (fd >= 0) {
		Fclose((short)fd);
		if (log_buf)
			log_ok = 1;
	}
}

/* The only thing in this file that touches the disk. Never called from
 * inside the frame loop unless the buffer has actually filled up. */
static void log_commit(void)
{
	long fd;

	if (!log_ok || log_len <= 0)
		return;
	fd = Fopen(LOGPATH, 2);			/* read/write */
	if (fd < 0)
		return;
	if (Fseek(0L, (short)fd, 2) < 0) {	/* SEEK_END: GEMDOS has no append */
		(void)Fclose((short)fd);
		return;
	}
	(void)Fwrite((short)fd, log_len, log_buf);
	(void)Fclose((short)fd);		/* commits the directory entry */
	log_len = 0;
}

static void log_line(const char *s)
{
	long n = 0;

	if (!log_ok)
		return;
	while (s[n])
		n++;
	if (log_len + n > LOG_BUF_MAX)
		log_commit();
	if (log_len + n > LOG_BUF_MAX)		/* one line longer than the lot */
		return;
	memcpy(log_buf + log_len, s, (size_t)n);
	log_len += n;
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
/* Settings                                                                  */
/*                                                                           */
/* One number can only be decided in front of the screen, and it is how big   */
/* the ball is. It sets what the demo looks like, and -- since the ST's       */
/* display is single-buffered and its driver cannot page-flip, so the blit IS */
/* the presentation -- it also sets how long the blit takes and therefore     */
/* how much of the frame the scanout can catch. Those two pull against each   */
/* other, the trade is a judgement, and a judgement needs the picture. So it  */
/* comes out of the program: C:\BALL.CFG, beside the texture and the log.     */
/*                                                                           */
/* It is a FRACTION OF THE SCREEN WIDTH and not a pixel count, because that   */
/* is the property the whole demo is built on -- every size here is a         */
/* fraction of the screen the VDI reported, so the picture is the same        */
/* picture at 640x480 and at 1920x1080. A pixel count would be right on one   */
/* machine and wrong on the next, and the machine at the meeting is not ours  */
/* to choose. The log says the fraction AND the diameter it came to, so a log */
/* on its own still says what was run.                                        */
/*                                                                           */
/* The parser is total on purpose, and it fails the way the texture loader    */
/* fails: no file, a line it cannot read, a key it does not know, a value out */
/* of range -- none of them stop the demo, all of them leave the built-in     */
/* default in place, and every one of them is COUNTED AND REPORTED. A         */
/* configuration file that goes silently unread is worse than none at all,    */
/* because the run then measures something other than what was asked for.     */
/*                                                                           */
/* Format, in full: one `key value` per line, `#` to end of line is a         */
/* comment, blank lines are ignored, both line endings are accepted. One key  */
/* is known:                                                                  */
/*                                                                            */
/*   ball  F    the ball's diameter as a fraction of the screen width.        */
/*              Default DIA_FRAC; the demo's own geometry ceiling             */
/*              (DIA_MAX_FRAC, and a third of the room's height) still        */
/*              applies and a value above it is clamped and said so.          */
/*                                                                            */
/* Nothing else is exposed, and that is a decision rather than an oversight.  */
/* The next candidate is the shadow's radius (SHADOW_R): it is the second     */
/* biggest rectangle the frame redraws, about a sixth of the dirty area       */
/* against the ball's two thirds, so it is the only other number that moves   */
/* both the look and the blit. It is not here because the ball is where the   */
/* lever is and a settings file that grows without a reason becomes its own   */
/* problem.                                                                   */
/* ------------------------------------------------------------------------ */
#define CFG_MAX		512	/* more than the one key needs, and bounded  */
#define BALL_FRAC_MIN	0.02f	/* below this there is nothing to look at    */

static char cfg_buf[CFG_MAX];
static float cfg_ball = DIA_FRAC;	/* what was asked for		     */
static float cfg_ball_used = DIA_FRAC;	/* ... and what the geometry allowed */
static const char *cfg_from = "built-in default";
static int cfg_ignored;			/* lines that said nothing usable    */
static int cfg_clamped;

static int c_space(char c)
{
	return c == ' ' || c == '\t' || c == '\r';
}

/*
 * A decimal fraction, and nothing else: no exponent, no hexadecimal, no
 * locale. Returns the number of characters consumed, or 0 if the text is not
 * a number -- which is what makes the caller able to ignore a line instead of
 * believing a zero.
 */
static int c_num(const char *p, float *out)
{
	float v = 0.0f, scale = 0.1f;
	int i = 0, digits = 0, neg = 0;

	/* Written on every path, including every failing one, so that a
	 * caller cannot hold an uninitialised value even by misreading the
	 * return. */
	*out = 0.0f;

	if (p[i] == '-') {
		neg = 1;
		i++;
	} else if (p[i] == '+') {
		i++;
	}
	while (p[i] >= '0' && p[i] <= '9') {
		v = v * 10.0f + (float)(p[i] - '0');
		digits++;
		i++;
	}
	if (p[i] == '.') {
		i++;
		while (p[i] >= '0' && p[i] <= '9') {
			v += (float)(p[i] - '0') * scale;
			scale *= 0.1f;
			digits++;
			i++;
		}
	}
	if (!digits)
		return 0;
	/* Trailing rubbish on the value makes the line a mistake, not a
	 * number followed by a comment: `ball 0.12x` is not 0.12. */
	if (p[i] && !c_space(p[i]) && p[i] != '\n' && p[i] != '#')
		return 0;
	*out = neg ? -v : v;
	return i;
}

/* Read the settings. Always succeeds; cfg_from says which way. */
static void cfg_read(void)
{
	long fd = Fopen(CFGPATH, 0);
	long n;
	int i = 0;

	if (fd < 0)
		return;
	n = Fread((short)fd, (long)CFG_MAX, cfg_buf);
	(void)Fclose((short)fd);
	if (n < 0)
		n = 0;
	if (n >= CFG_MAX)
		n = CFG_MAX - 1;
	cfg_buf[n] = '\0';
	cfg_from = CFGPATH;

	while (i < (int)n) {
		int start, klen, used;
		float v;

		while (i < (int)n && (c_space(cfg_buf[i]) ||
				      cfg_buf[i] == '\n'))
			i++;
		if (i >= (int)n)
			break;
		if (cfg_buf[i] == '#') {
			while (i < (int)n && cfg_buf[i] != '\n')
				i++;
			continue;
		}

		start = i;
		while (i < (int)n && !c_space(cfg_buf[i]) &&
		       cfg_buf[i] != '\n')
			i++;
		klen = i - start;
		while (i < (int)n && c_space(cfg_buf[i]))
			i++;

		used = c_num(cfg_buf + i, &v);
		if (klen == 4 && cfg_buf[start] == 'b' &&
		    cfg_buf[start + 1] == 'a' && cfg_buf[start + 2] == 'l' &&
		    cfg_buf[start + 3] == 'l' && used > 0 &&
		    v >= BALL_FRAC_MIN)
			cfg_ball = v;
		else
			cfg_ignored++;

		while (i < (int)n && cfg_buf[i] != '\n')
			i++;
	}
}

/*
 * The ball's radius in pixels, from its diameter as a fraction of the screen
 * width, with the demo's own geometry as the ceiling: never wider than
 * DIA_MAX_FRAC of the screen and never more than a third of the room it has
 * to bounce in.
 *
 * It is a function of the fraction and of nothing else on purpose. A later
 * milestone wants the ball to move in DEPTH -- towards and away from the
 * viewer, inside a room that is already drawn in perspective -- and its
 * on-screen radius would then vary continuously, which means a SET of sphere
 * maps at stepped radii instead of the one built here. Every step of that
 * would call this, once per step, and this is the part that would not have to
 * change. The rest would: build_sphere_map() writes one pair of tables and
 * one coverage run for the single D that draw_ball(), draw_reflect() and
 * compose() all index by, so a set means choosing among them per frame.
 * That is the depth milestone and it is not built here.
 */
static int radius_of(float frac)
{
	int r = (int)((float)W * frac * 0.5f);
	int wmax = (int)((float)W * DIA_MAX_FRAC * 0.5f);
	int hmax = (int)(floor_y * 0.30f);

	if (r > wmax) {
		r = wmax;
		cfg_clamped = 1;
	}
	if (r > hmax) {
		r = hmax;
		cfg_clamped = 1;
	}
	if (r < 4) {
		r = 4;
		cfg_clamped = 1;
	}
	return r;
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
/* The composition pixel                                                     */
/*                                                                           */
/* Everything the renderer writes goes through the handful of operations     */
/* below, and each of them is stated twice: once for a machine whose picture */
/* is 256 designed colours, once for a machine whose picture is RGB. The     */
/* renderer itself reads the same either way -- that is the point of naming  */
/* them -- and the build script says which half is compiled (see ball.h).    */
/*                                                                           */
/* WHAT THE PALETTE LAYOUT WAS DOING, and how the wide pixel does it. The    */
/* 256-entry design is not only a set of colours, it also ENCODES two facts  */
/* about every background pixel, and the renderer reads them straight out of */
/* the index:                                                                */
/*                                                                           */
/*   "the shadow may darken this"  -- true exactly for the floor bands,      */
/*                                    i.e. FLOOR_BASE <= i < GRID_BASE, and  */
/*                                    darkening is then an ADDITION because  */
/*                                    each band is stored SHADOW_N times     */
/*   "this is a grid line, at      -- i - GRID_BASE + 1, which is what lets  */
/*    level L"                        the scrim take a line down to a        */
/*                                    quarter of itself without redrawing it */
/*                                                                           */
/* An RGB pixel says neither. It does not have to: 0x00RRGGBB leaves the top */
/* byte unused, and that is where both answers go. They cost nothing -- they */
/* ride in a pixel that was going to be copied anyway -- and the screen      */
/* never sees them, because the composition pixel is handed to plat_blit()   */
/* and it is the platform that decides what reaches the card. The renderer   */
/* asks the same two questions on both machines; only the answer's hiding    */
/* place differs.                                                            */
/* ------------------------------------------------------------------------ */
#if BALL_TRUECOLOR

#define PX_RGB_MASK	0x00FFFFFFu
#define PX_GRID_SHIFT	24		/* grid level + 1, 0 = not a line    */
#define PX_GRID_MASK	0x1Fu		/* GRID_N is 24, so five bits	     */
#define PX_NOSHADE	0x20000000u	/* the floor shadow must skip this   */

#define px_pack(r, g, b) \
	((ball_px)(((unsigned)(r) << 16) | ((unsigned)(g) << 8) | \
		   (unsigned)(b)))

/* A designed colour, carrying whatever its place in the design implies. */
static ball_px px_of(int i)
{
	ball_px p = px_pack(pal[i].r, pal[i].g, pal[i].b);

	if (i >= GRID_BASE && i < GRID_BASE + GRID_N)
		return p | PX_NOSHADE |
		       ((ball_px)(i - GRID_BASE + 1) << PX_GRID_SHIFT);
	if (i >= FLOOR_BASE && i < GRID_BASE)
		return p;		/* plain floor: shadeable	     */
	return p | PX_NOSHADE;
}

#define px_shadeable(p)		(((p) & PX_NOSHADE) == 0)
#define px_grid_level(p)	((int)(((p) >> PX_GRID_SHIFT) & PX_GRID_MASK))
#define px_lum(p)		(2 * (int)(((p) >> 16) & 0xFFu) + \
				 5 * (int)(((p) >> 8) & 0xFFu) + \
				 (int)((p) & 0xFFu))

/*
 * The floor shadow. On the palette machine this is one addition, because the
 * design stores every floor band SHADOW_N times over; here it is the same
 * darkening done to the colour itself, through a table so that it is still
 * three lookups and no multiply.
 */
static unsigned char shade_tab[SHADOW_N][256];

static void px_shade_init(void)
{
	int lv, v;

	for (lv = 0; lv < SHADOW_N; lv++) {
		float d = 1.0f - 0.88f * (float)lv / (float)(SHADOW_N - 1);

		for (v = 0; v < 256; v++)
			shade_tab[lv][v] =
				(unsigned char)((float)v * d + 0.5f);
	}
}

static ball_px px_shade(ball_px p, int lv)
{
	const unsigned char *t = shade_tab[lv];

	return (p & ~PX_RGB_MASK) |
	       px_pack(t[(p >> 16) & 0xFFu], t[(p >> 8) & 0xFFu], t[p & 0xFFu]);
}

static void px_fill(ball_px *row, ball_px p, long n)
{
	while (n-- > 0)
		*row++ = p;
}

#else	/* the 256-entry design: the index IS the answer */

/* Luminance, near enough: the classic 2:5:1 weighting, in integers. */
static int pal_lum(int i)
{
	return 2 * (int)pal[i].r + 5 * (int)pal[i].g + (int)pal[i].b;
}

#define px_of(i)		((ball_px)(i))
#define px_shadeable(p)		((p) >= FLOOR_BASE && (p) < GRID_BASE)
#define px_grid_level(p)	((int)(p) - GRID_BASE + 1)
#define px_lum(p)		pal_lum((int)(p))
#define px_shade(p, lv)		((ball_px)((int)(p) + (lv)))
#define px_shade_init()		((void)0)
#define px_fill(row, p, n)	memset((row), (int)(p), (size_t)(n))

#endif

#define px_copy(dst, src, n) \
	memcpy((dst), (src), (size_t)(n) * sizeof(ball_px))

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

	/* The room's one ramp. Its BOTTOM matters as much as its top: it
	 * starts a shade above the brightest thing the backdrop and the floor
	 * can be, so that the faintest pixel of an anti-aliased line is still
	 * at least as bright as whatever it landed on -- a line here is a
	 * glowing line, and a glowing line that darkens the floor it crosses
	 * is a drawing mistake nobody can un-see. The top is a blue-white
	 * rather than a white: the room is lit by its own grid. */
	for (i = 0; i < GRID_N; i++) {
		t = (float)i / (float)(GRID_N - 1);
		pal[GRID_BASE + i].r = (unsigned char)mixc(44, 232, t);
		pal[GRID_BASE + i].g = (unsigned char)mixc(62, 246, t);
		pal[GRID_BASE + i].b = (unsigned char)mixc(96, 255, t);
	}

	/* What milestone 2b did not need. Black, and touched by nothing until
	 * it has an owner -- an entry left at a plausible-looking colour is an
	 * entry somebody eventually draws with by accident. */
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

	/* And the table that turns (light level, hue) into a pixel.
	 *
	 * On a palette machine that is one of the entries just laid out: a
	 * ramp shorter than BALL_LEVELS is sampled out of the same levels, so
	 * every hue answers to the same light level and the renderer never
	 * needs to know how long any ramp is. A hue whose ramp is 20 entries
	 * long therefore shows 20 steps where the light asks for 48, and on a
	 * big sphere that is visible as banding -- the price of 256 colours,
	 * paid where it is least avoidable.
	 *
	 * A truecolour machine is not asked to pay it. The ramp is the same
	 * ramp, from the same two knees, evaluated at the level ITSELF rather
	 * than at the nearest entry of a shortened copy of it. Same table,
	 * same size in entries, same lookup in the inner loop; the hue_len
	 * quantisation simply does not happen. That is the whole of what this
	 * build gets for free, and it is deliberately the only thing changed:
	 * every other colour in the picture is still the designed one, so the
	 * two builds can be compared. */
	for (i = 0; i < BALL_LEVELS; i++)
		for (m = 0; m < TEX_MAT_MAX; m++) {
#if BALL_TRUECOLOR
			int mr = tex.rgb[m][0];
			int mg = tex.rgb[m][1];
			int mb = tex.rgb[m][2];

			t = (float)i / (float)(BALL_LEVELS - 1);
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
			shade_lut[i * TEX_MAT_MAX + m] =
				px_pack(r, g, b) | PX_NOSHADE;
#else
			int n = hue_len[m] - 1;
			int e = (i * n + (BALL_LEVELS - 1) / 2) /
				(BALL_LEVELS - 1);

			shade_lut[i * TEX_MAT_MAX + m] =
				px_of(hue_base[m] + e);
#endif
		}
}

/*
 * The reflection's tables: the ball's own shade table, read at a lower light
 * level. Table 0 is the far, faint end of the fade and REFL_LUT_N-1 the end
 * that touches the ball. 1536 bytes for all of them, so the whole set is in
 * L1 on either machine, and the reflection's inner loop is the sphere's inner
 * loop with a different pointer. No palette entry is spent.
 */
static void build_refl_lut(void)
{
	int k, i, m;

	for (k = 0; k < REFL_LUT_N; k++) {
		float f = REFL_DARK * (REFL_TAIL + (1.0f - REFL_TAIL) *
				       (float)k / (float)(REFL_LUT_N - 1));

		for (i = 0; i < BALL_LEVELS; i++) {
			int lv = (int)((float)i * f + 0.5f);

			if (lv > BALL_LEVELS - 1)
				lv = BALL_LEVELS - 1;
			for (m = 0; m < TEX_MAT_MAX; m++)
				refl_lut[k][i * TEX_MAT_MAX + m] =
					shade_lut[lv * TEX_MAT_MAX + m];
		}
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
			bg_row[y] = px_of(SKY_BASE + band);
		} else {
			band = (int)((float)(y - horizon) *
				     (float)(FLOOR_N - 1) /
				     (float)(H - horizon) + d);
			if (band > FLOOR_N - 1)
				band = FLOOR_N - 1;
			bg_row[y] = px_of(FLOOR_BASE + band * SHADOW_N);
		}
	}
}

/* ------------------------------------------------------------------------ */
/* The lettering                                                             */
/*                                                                           */
/* One renderer, one font. The glyphs are ball-tex.c's -- the same table the  */
/* ball's own panels are lettered from -- and only the destination differs:   */
/* there it is a texture of material indices, here it is the screen's index   */
/* buffer. Every string is drawn CLIPPED TO A RECTANGLE, because that is what */
/* lets the readout be part of the composed frame instead of a layer painted  */
/* over it afterwards.                                                        */
/*                                                                           */
/* Each string is drawn five times: four one-pixel offsets in the darkest     */
/* backdrop colour, then the string itself. That outline is what keeps a      */
/* number readable where a glowing grid line runs behind it, and it costs     */
/* nothing that matters -- the four numbers are the only text redrawn per     */
/* frame, and they are eleven characters between them.                        */
/* ------------------------------------------------------------------------ */
#define TXT_BRIGHT	(GRID_BASE + GRID_N - 4)
/* The quiet lettering has to win against a grid line the scrim has left
 * behind it, so it is defined from the scrim rather than picked. */
#define TXT_DIM		(GRID_BASE + GRID_N * HUD_SCRIM_NUM / HUD_SCRIM_DEN + 3)
#define TXT_HALO_IDX	(SKY_BASE)

/* Pixels a string of `n` characters occupies at the current scale. */
static int text_span(int n)
{
	return n > 0 ? n * (GLYPH_W + 1) * txt_sc - txt_sc : 0;
}

static void text_pass(int x, int y, const char *v, ball_px col, rect r,
		      int ox, int oy)
{
	int gx, gy, sx, sy;

	for (; *v; v++) {
		const unsigned char *g = font_glyph(*v);

		for (gy = 0; gy < GLYPH_H; gy++) {
			unsigned bits = g[gy];

			for (gx = 0; gx < GLYPH_W; gx++) {
				if (!(bits & (1u << (GLYPH_W - 1 - gx))))
					continue;
				for (sy = 0; sy < txt_sc; sy++) {
					int py = y + oy + gy * txt_sc + sy;
					ball_px *row;

					if (py < r.y0 || py >= r.y1)
						continue;
					row = idx + (long)py * W;
					for (sx = 0; sx < txt_sc; sx++) {
						int px = x + ox +
							 gx * txt_sc + sx;

						if (px < r.x0 || px >= r.x1)
							continue;
						row[px] = col;
					}
				}
			}
		}
		x += (GLYPH_W + 1) * txt_sc;
	}
}

static void text_draw(int x, int y, const char *v, ball_px col, rect r)
{
	static const signed char off[4][2] = {
		{ -1, 0 }, { 1, 0 }, { 0, -1 }, { 0, 1 }
	};
	int i;

	for (i = 0; i < 4; i++)
		text_pass(x, y, v, px_of(TXT_HALO_IDX), r,
			  off[i][0] * txt_halo, off[i][1] * txt_halo);
	text_pass(x, y, v, col, r, 0, 0);
}

/*
 * Everything written in words: the readout's seven lines and the four in the
 * bottom corners. compose() walks this list last, so a rectangle composed for
 * any reason comes out with the lettering that crosses it already on it --
 * which is the whole trick that lets the ball pass BEHIND the readout without
 * a single test in the ball's own loop.
 *
 * `s` for a readout line points at hud_line[], which is rewritten in place,
 * so the list never has to be rebuilt; `w` reserves the block's full width
 * rather than the current string's, so a number that grows a digit does not
 * grow past its own rectangle.
 */
typedef struct {
	int x, y, w, h;
	ball_px col;
	const char *s;
} ov_item;

#define OV_MAX		16
static ov_item ov[OV_MAX];
static int ov_n;

static void ov_add(int x, int y, int chars, ball_px col, const char *v)
{
	ov_item *o;

	if (ov_n >= OV_MAX)
		return;
	o = &ov[ov_n++];
	o->x = x - txt_halo;
	o->y = y - txt_halo;
	o->w = text_span(chars) + 2 * txt_halo;
	o->h = GLYPH_H * txt_sc + 2 * txt_halo;
	o->col = col;
	o->s = v;
}

static void ov_draw(rect r)
{
	int i;

	for (i = 0; i < ov_n; i++) {
		const ov_item *o = &ov[i];

		if (o->x >= r.x1 || o->x + o->w <= r.x0 ||
		    o->y >= r.y1 || o->y + o->h <= r.y0)
			continue;
		text_draw(o->x + txt_halo, o->y + txt_halo, o->s, o->col, r);
	}
}

/*
 * Where the words go. The readout is a block HUD_CHARS wide in the top right
 * corner; the two pairs of corner lines are the demo's own -- what it is and
 * what it runs on. The reference render's third line there claims a
 * partnership between two companies that does not exist, and is not
 * reproduced; neither are anybody's trademarks, here or on the ball.
 *
 * Sizes are in CHARACTERS and the scale comes off the screen width, so the
 * block is the same fraction of the picture at 640x480 as at 1920x1080.
 */
static const char *const hud_label[HUD_LINES] = {
	"NEWTOS DEMO",
	"BOUNCING BALL",
	"--------------------",
	"FPS: ",
	"POS: ",
	"VEL: ",
	"BOUNCES: "
};

static const char *const corner_l[2] = { "SMALL SYSTEMS.", "BIG IDEAS." };
static const char *const corner_r[2] = { "NEWTOS.", "POWERED BY CURIOSITY." };

/* One rectangle of the room to be taken down behind lettering. */
static void scrim_add(int x0, int y0, int x1, int y1)
{
	rect *r;

	if (scrim_n >= (int)(sizeof scrim / sizeof scrim[0]))
		return;
	r = &scrim[scrim_n++];
	r->x0 = x0 < 0 ? 0 : x0;
	r->y0 = y0 < 0 ? 0 : y0;
	r->x1 = x1 > W ? W : x1;
	r->y1 = y1 > H ? H : y1;
}

static void text_layout(void)
{
	int i, mx, my, bx0, by0;

	txt_sc = W / 640;
	if (txt_sc < 1)
		txt_sc = 1;
	txt_halo = txt_sc / 2;
	if (txt_halo < 1)
		txt_halo = 1;
	hud_cw = (GLYPH_W + 1) * txt_sc;
	hud_lh = (GLYPH_H + 2) * txt_sc;

	mx = (int)(HUD_MARGIN * (float)W);
	my = (int)(HUD_MARGIN * (float)H);

	hud_x = W - mx - HUD_CHARS * hud_cw;
	if (hud_x < 0)
		hud_x = 0;
	hud_y = my;

	scrim_n = 0;
	scrim_add(hud_x - hud_cw, hud_y - txt_sc,
		  hud_x + (HUD_CHARS + 1) * hud_cw, hud_y + HUD_LINES * hud_lh);

	ov_n = 0;
	for (i = 0; i < HUD_LINES; i++) {
		int ly = hud_y + i * hud_lh;
		int lab = s_len(hud_label[i]);

		s_cpy(hud_line[i], hud_label[i]);
		ov_add(hud_x, ly, HUD_CHARS,
		       px_of(i == 2 ? TXT_DIM : TXT_BRIGHT), hud_line[i]);

		/* The part of the line a new value can change: from the end
		 * of the label to the end of the block, and one halo wider
		 * all round so the outline is restored with it. */
		hud_vr[i].x0 = hud_x + text_span(lab + 1) - hud_cw - txt_halo;
		hud_vr[i].x1 = hud_x + HUD_CHARS * hud_cw + txt_halo;
		hud_vr[i].y0 = ly - txt_halo;
		hud_vr[i].y1 = ly + GLYPH_H * txt_sc + txt_halo;
		if (hud_vr[i].x0 < 0)
			hud_vr[i].x0 = 0;
		if (hud_vr[i].y0 < 0)
			hud_vr[i].y0 = 0;
		if (hud_vr[i].x1 > W)
			hud_vr[i].x1 = W;
		if (hud_vr[i].y1 > H)
			hud_vr[i].y1 = H;
	}

	/* The two corner pairs, and a scrim for each: they sit on the near
	 * floor, where the grid is at its brightest, and a line crossing a
	 * word at full strength is a word nobody reads. */
	by0 = H - my - 2 * hud_lh;
	bx0 = mx;
	{
		int wl = 0, wr = 0;

		for (i = 0; i < 2; i++) {
			int nl = s_len(corner_l[i]), nr = s_len(corner_r[i]);

			if (text_span(nl) > wl)
				wl = text_span(nl);
			if (text_span(nr) > wr)
				wr = text_span(nr);
			ov_add(bx0, by0 + i * hud_lh, nl, px_of(TXT_DIM),
			       corner_l[i]);
			ov_add(W - mx - text_span(nr), by0 + i * hud_lh, nr,
			       px_of(TXT_DIM), corner_r[i]);
		}
		scrim_add(bx0 - hud_cw, by0 - txt_sc,
			  bx0 + wl + hud_cw, by0 + 2 * hud_lh);
		scrim_add(W - mx - wr - hud_cw, by0 - txt_sc,
			  W - mx + hud_cw, by0 + 2 * hud_lh);
	}
}

/*
 * The four numbers, rebuilt into their lines. Position and velocity are the
 * ball's own, in screen pixels and in pixels per second -- the units the rest
 * of the program is written in, so what the readout says can be checked
 * against what the picture does.
 */
static void hud_values(void)
{
	char *q;

	q = f_str(hud_line[3], hud_label[3]);
	q = f_fix1(q, hud_fps);
	*q = 0;

	q = f_str(hud_line[4], hud_label[4]);
	*q++ = '(';
	q = f_dec(q, (long)(bx + 0.5f));
	*q++ = ',';
	q = f_dec(q, (long)(by + 0.5f));
	*q++ = ')';
	*q = 0;

	q = f_str(hud_line[5], hud_label[5]);
	*q++ = '(';
	q = f_dec(q, (long)vx);
	*q++ = ',';
	q = f_dec(q, (long)vy);
	*q++ = ')';
	*q = 0;

	q = f_str(hud_line[6], hud_label[6]);
	q = f_dec(q, bounces);
	*q = 0;
}

/* ------------------------------------------------------------------------ */
/* The room, painted once into its own image                                 */
/* ------------------------------------------------------------------------ */

/*
 * What the room's bare surface is at (x, y). Above the horizon everything is
 * backdrop; below it, the floor is what lies between the two side walls, and
 * the wedges outside them are wall -- flat at the backdrop's brightest, which
 * is where the backdrop's own gradient had got to by the horizon.
 */
static ball_px room_fill(int x, int y)
{
	float t;

	if (y < horizon)
		return bg_row[y];
	t = ((float)y + 0.5f - vp_y) * rm_slope;
	if ((float)x + 0.5f < vp_x - t || (float)x + 0.5f > vp_x + t)
		return px_of(SKY_BASE + SKY_N - 1);
	return bg_row[y];
}

/*
 * One pixel of a line, at brightness `v` in [0, 1].
 *
 * The level is ordered-dithered, so the halo falls off smoothly through 24
 * entries instead of banding into rings. And it is composited by BRIGHTNESS:
 * a line may lighten what is under it and never darken it, which is what a
 * glowing line does and what removes the whole question of blending a cyan
 * ramp with whichever of two gradients it happened to land on.
 */
static void room_px(int x, int y, float v)
{
	int lv;
	ball_px e;
	ball_px *q;

	if (v <= 0.0f || x < 0 || y < 0 || x >= W || y >= H)
		return;
	lv = (int)(v * (float)GRID_N + dith8[y & 7][x & 7]);
	if (lv <= 0)
		return;
	if (lv > GRID_N)
		lv = GRID_N;
	e = px_of(GRID_BASE + lv - 1);
	q = bg + (long)y * W + x;
	if (px_lum(e) <= px_lum(*q))
		return;
	*q = e;
}

/* Flat core, soft shoulder, wide faint halo -- as a function of the
 * perpendicular distance from the segment, in pixels. */
static float line_prof(float d)
{
	float v;

	if (d <= line_core)
		v = 1.0f;
	else if (d < line_core + line_soft)
		v = 1.0f - (d - line_core) / line_soft;
	else
		v = 0.0f;
	if (d < line_glow_r) {
		float g = LINE_GLOW * (1.0f - d / line_glow_r);

		if (g > v)
			v = g;
	}
	return v;
}

/*
 * A segment, brightness b0 at one end and b1 at the other. It walks the major
 * axis and covers the minor one out to the halo's reach, converting the
 * distance it steps into the PERPENDICULAR distance the profile wants with
 * the segment's own cosine -- so a line at 30 degrees is exactly as thick as
 * one at 0.
 */
static void room_line(float x0, float y0, float x1, float y1,
		      float b0, float b1)
{
	float dx = x1 - x0, dy = y1 - y0;
	float rad = line_core + line_soft;
	float t;
	int i, j;

	if (line_glow_r > rad)
		rad = line_glow_r;

	if (b_abs(dx) >= b_abs(dy)) {
		float sl, cs, rv;
		int ia, ib;

		if (dx < 0.0f) {
			t = x0; x0 = x1; x1 = t;
			t = y0; y0 = y1; y1 = t;
			t = b0; b0 = b1; b1 = t;
			dx = -dx;
			dy = -dy;
		}
		if (dx < 0.001f)
			return;
		sl = dy / dx;
		cs = 1.0f / b_sqrt(1.0f + sl * sl);
		rv = rad / cs;
		ia = (int)(x0 + 0.5f);
		ib = (int)(x1 + 0.5f);
		if (ia < 0)
			ia = 0;
		if (ib > W - 1)
			ib = W - 1;
		for (i = ia; i <= ib; i++) {
			float fx = (float)i + 0.5f;
			float yc = y0 + (fx - x0) * sl;
			float u = (fx - x0) / dx;
			float bri;
			int j0, j1;

			if (u < 0.0f)
				u = 0.0f;
			if (u > 1.0f)
				u = 1.0f;
			bri = b0 + (b1 - b0) * u;
			j0 = (int)(yc - rv);
			j1 = (int)(yc + rv) + 1;
			if (j0 < 0)
				j0 = 0;
			if (j1 > H)
				j1 = H;
			for (j = j0; j < j1; j++)
				room_px(i, j,
					line_prof(b_abs((float)j + 0.5f - yc)
						  * cs) * bri);
		}
	} else {
		float sl, cs, rh;
		int ja, jb;

		if (dy < 0.0f) {
			t = x0; x0 = x1; x1 = t;
			t = y0; y0 = y1; y1 = t;
			t = b0; b0 = b1; b1 = t;
			dx = -dx;
			dy = -dy;
		}
		if (dy < 0.001f)
			return;
		sl = dx / dy;
		cs = 1.0f / b_sqrt(1.0f + sl * sl);
		rh = rad / cs;
		ja = (int)(y0 + 0.5f);
		jb = (int)(y1 + 0.5f);
		if (ja < 0)
			ja = 0;
		if (jb > H - 1)
			jb = H - 1;
		for (j = ja; j <= jb; j++) {
			float fy = (float)j + 0.5f;
			float xc = x0 + (fy - y0) * sl;
			float u = (fy - y0) / dy;
			float bri;
			int i0, i1;

			if (u < 0.0f)
				u = 0.0f;
			if (u > 1.0f)
				u = 1.0f;
			bri = b0 + (b1 - b0) * u;
			i0 = (int)(xc - rh);
			i1 = (int)(xc + rh) + 1;
			if (i0 < 0)
				i0 = 0;
			if (i1 > W)
				i1 = W;
			for (i = i0; i < i1; i++)
				room_px(i, j,
					line_prof(b_abs((float)i + 0.5f - xc)
						  * cs) * bri);
		}
	}
}

/* Bright near the viewer, dim at the back wall -- the one depth cue the room
 * has, and the reason the far end of it reads as far away at all. */
static float room_fade(float m)
{
	float t = (m - 1.0f) / (rm_mn - 1.0f);

	if (t < 0.0f)
		t = 0.0f;
	if (t > 1.0f)
		t = 1.0f;
	return ROOM_FADE_FAR + (1.0f - ROOM_FADE_FAR) * t;
}

static long room_ticks;			/* what painting it cost, in ticks   */

static void build_room(void)
{
	float mi[ROOM_N + 1];
	float e, cellw, wallh, f1, fn;
	long t0 = get_sysvar(_hz_200);
	int i, j, k, x, y;

	/* --- the projection, once ------------------------------------- */
	vp_x = 0.5f * (float)W;
	rm_drop = ROOM_DROP_FRAC * (float)H;
	if (rm_drop < 1.0f)
		rm_drop = 1.0f;
	vp_y = (float)horizon - rm_drop;
	rm_hw = ROOM_HW_U * rm_drop;
	if (rm_hw > ROOM_HW_MAX * (float)W)
		rm_hw = ROOM_HW_MAX * (float)W;
	rm_up = ROOM_UP_U * rm_drop;
	rm_slope = rm_hw / rm_drop;
	rm_mn = ((float)H - vp_y) / rm_drop;
	if (rm_mn < 1.5f)
		rm_mn = 1.5f;

	line_core = LINE_CORE_FRAC * (float)W;
	if (line_core < LINE_CORE_MIN)
		line_core = LINE_CORE_MIN;
	line_soft = LINE_SOFT_FRAC * (float)W;
	if (line_soft < LINE_SOFT_MIN)
		line_soft = LINE_SOFT_MIN;
	line_glow_r = LINE_GLOW_MUL * (line_core + line_soft);

	/* Depth is what is evenly spaced, not screen distance: 1/(y - vp_y)
	 * steps by a constant, and the constant is the one that lands the
	 * last division exactly on the bottom row of the screen. */
	e = (1.0f - 1.0f / rm_mn) / (float)ROOM_N;
	for (i = 0; i <= ROOM_N; i++)
		mi[i] = 1.0f / (1.0f - (float)i * e);

	/* Rows of cells on the walls, chosen so a cell on the back wall is
	 * square rather than so it is a round number. */
	cellw = 2.0f * rm_hw / (float)ROOM_M;
	wallh = rm_drop + rm_up;
	rm_k = (int)(wallh / cellw + 0.5f);
	if (rm_k < 2)
		rm_k = 2;
	if (rm_k > ROOM_K_MAX)
		rm_k = ROOM_K_MAX;

	f1 = room_fade(1.0f);
	fn = room_fade(rm_mn);

	/* --- the bare surfaces ---------------------------------------- */
	for (y = 0; y < H; y++) {
		ball_px *row = bg + (long)y * W;

		if (y < horizon) {
			px_fill(row, bg_row[y], (long)W);
			continue;
		}
		{
			float t = ((float)y + 0.5f - vp_y) * rm_slope;
			int xl = (int)(vp_x - t + 0.5f);
			int xr = (int)(vp_x + t + 0.5f);
			ball_px wc = px_of(SKY_BASE + SKY_N - 1);

			if (xl < 0)
				xl = 0;
			if (xr > W)
				xr = W;
			if (xr < xl)
				xr = xl;
			if (xl > 0)
				px_fill(row, wc, (long)xl);
			if (xr > xl)
				px_fill(row + xl, bg_row[y], (long)(xr - xl));
			if (xr < W)
				px_fill(row + xr, wc, (long)(W - xr));
		}
	}

	/* --- the floor ------------------------------------------------- */
	for (i = 0; i <= ROOM_N; i++) {		/* across, one per depth */
		float m = mi[i], f = room_fade(m);

		room_line(vp_x - m * rm_hw, vp_y + m * rm_drop,
			  vp_x + m * rm_hw, vp_y + m * rm_drop, f, f);
	}
	for (j = 0; j <= ROOM_M; j++) {		/* away, one per cell    */
		float xf = vp_x + (2.0f * (float)j / (float)ROOM_M - 1.0f)
			   * rm_hw;

		room_line(xf, vp_y + rm_drop,
			  vp_x + rm_mn * (xf - vp_x), vp_y + rm_mn * rm_drop,
			  f1, fn);
	}

	/* --- the back wall, square to the screen ----------------------- */
	for (j = 0; j <= ROOM_M; j++) {
		float xf = vp_x + (2.0f * (float)j / (float)ROOM_M - 1.0f)
			   * rm_hw;

		room_line(xf, vp_y - rm_up, xf, vp_y + rm_drop, f1, f1);
	}
	for (k = 0; k < rm_k; k++) {		/* k == rm_k is the floor */
		float yf = vp_y - rm_up + (float)k * wallh / (float)rm_k;

		room_line(vp_x - rm_hw, yf, vp_x + rm_hw, yf, f1, f1);
	}

	/* --- the two side walls, off the same depths ------------------- */
	for (i = 1; i <= ROOM_N; i++) {		/* i == 0 is the wall's edge */
		float m = mi[i], f = room_fade(m);

		room_line(vp_x - m * rm_hw, vp_y - m * rm_up,
			  vp_x - m * rm_hw, vp_y + m * rm_drop, f, f);
		room_line(vp_x + m * rm_hw, vp_y - m * rm_up,
			  vp_x + m * rm_hw, vp_y + m * rm_drop, f, f);
	}
	for (k = 0; k < rm_k; k++) {		/* k == rm_k is the floor */
		float yf = vp_y - rm_up + (float)k * wallh / (float)rm_k;
		float dyf = yf - vp_y;

		room_line(vp_x - rm_hw, yf,
			  vp_x - rm_mn * rm_hw, vp_y + rm_mn * dyf, f1, fn);
		room_line(vp_x + rm_hw, yf,
			  vp_x + rm_mn * rm_hw, vp_y + rm_mn * dyf, f1, fn);
	}

	/* --- the scrims behind the lettering --------------------------- */
	/* The room does not stop where a word starts, it recedes behind it:
	 * every line inside one of these rectangles is taken down to a
	 * quarter of its brightness, and a line too faint to survive that
	 * gives its pixel back to the surface. Clearing the rectangles would
	 * have been simpler and would have left three holes in the room. */
	for (i = 0; i < scrim_n; i++) {
		for (y = scrim[i].y0; y < scrim[i].y1; y++) {
			ball_px *row = bg + (long)y * W;

			for (x = scrim[i].x0; x < scrim[i].x1; x++) {
				int lv = px_grid_level(row[x]);

				if (lv < 1 || lv > GRID_N)
					continue;
				lv = lv * HUD_SCRIM_NUM / HUD_SCRIM_DEN;
				row[x] = lv < 1
					 ? room_fill(x, y)
					 : px_of(GRID_BASE + lv - 1);
			}
		}
	}

	room_ticks = get_sysvar(_hz_200) - t0;
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
	int i;

	/* The wind, and the only place the ball's horizontal speed is
	 * decided. It is the sum of three sines about the nominal roll, so it
	 * is bounded by WIND_FRAC of that roll however long the demo runs --
	 * and, because it is added to the SIGNED velocity rather than to the
	 * magnitude, it pushes the same way whichever way the ball is going,
	 * which is what a wind does. */
	wind_v = 0.0f;
	for (i = 0; i < WIND_TERMS; i++) {
		wind_p[i] += wind_w[i];
		if (wind_p[i] >= 2.0f * B_PI)
			wind_p[i] -= 2.0f * B_PI;
		wind_v += wind_a[i] * b_sin(wind_p[i]);
	}
	vx = (float)roll_dir * vx0 + wind_v;

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

	/* The walls only turn it round, and they set the direction outright
	 * rather than negating it -- at the left wall the only way out is
	 * right. Nothing anywhere scales the roll speed, so it never winds
	 * down; the wind moves it about the nominal and always gives it back.
	 */
	if (bx - Rf < 0.0f) {
		bx = Rf;
		roll_dir = 1;
	} else if (bx + Rf > (float)W) {
		bx = (float)W - Rf;
		roll_dir = -1;
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

/*
 * Set the wind up: three sines, equal amplitudes summing to exactly
 * WIND_FRAC of the roll speed, periods the measured bounce times phi, phi^2
 * and phi^3, and phases from the seed. Everything is scaled off numbers the
 * program measured, so the wind is the same wind at 640x480 as at 1920x1080.
 */
static void wind_init(void)
{
	unsigned r = WIND_SEED ? (unsigned)WIND_SEED
				: ((unsigned)get_sysvar(_hz_200) | 1u);
	float phi = 0.5f * (1.0f + b_sqrt(5.0f));
	float mult = phi;
	int i;

	for (i = 0; i < WIND_TERMS; i++) {
		float period = (float)cyc_v * mult;	/* in ticks */

		wind_a[i] = vx0 * WIND_FRAC / (float)WIND_TERMS;
		wind_w[i] = 2.0f * B_PI / period;
		/* xorshift32, purely to spread the three phases; nothing
		 * downstream cares about its quality. */
		r ^= r << 13;
		r ^= r >> 17;
		r ^= r << 5;
		wind_p[i] = 2.0f * B_PI * (float)(r >> 8) / 16777216.0f;
		mult *= phi;
	}
	wind_v = 0.0f;
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

	/* The reflection. The ball's underside is `hgt` above the contact
	 * line, so its mirror image begins exactly `hgt` below it -- which is
	 * why nothing here needs a special case for a ball in the air: as it
	 * rises its reflection slides down the floor, fades, and leaves the
	 * bottom of the screen on its own. Same columns as the ball. */
	rf_top = (int)(2.0f * floor_y - by - Rf);
	rf_y0 = rf_top < fl_i ? fl_i : rf_top;
	rf_y1 = rf_top + D;
	if (rf_y1 > H)
		rf_y1 = H;
	if (rf_y1 < rf_y0)
		rf_y1 = rf_y0;
}

/* ------------------------------------------------------------------------ */
/* The dirty rectangles                                                      */
/*                                                                           */
/* Three things move: the ball, its shadow on the floor, and its reflection  */
/* in the floor. They are nowhere near each other. A ball 306 pixels across  */
/* flying 600 pixels above its own shadow put into ONE bounding box gives a  */
/* rectangle 874 rows tall -- measured, on the ST with PiStorm at 1920x1072  */
/* -- of which the ball is a third and the rest is room that did not change. */
/*                                                                           */
/* That box was what the demo handed over every frame, and it cost twice:    */
/*                                                                           */
/*   - compose and blit both scale with it. 14.91 ms and 7.94 ms per frame   */
/*     against a 16.6 ms retrace is 30 fps on a 60 Hz screen, and that is    */
/*     exactly what that machine measured.                                   */
/*   - the ST's screen is SINGLE-BUFFERED and its driver cannot page-flip,   */
/*     so the blit is not a copy to a hidden page, it IS the presentation.   */
/*     A blit that runs for half a refresh is overtaken by the scanout part  */
/*     way down the rectangle and the viewer sees the ball twice, torn       */
/*     across the join. Without a flip the only cure is a blit short enough  */
/*     that the beam cannot catch it, so the height of that rectangle is not */
/*     a performance detail -- it is whether the picture holds together.     */
/*                                                                           */
/* So each part carries its own rectangle, and the frame hands over a SET.   */
/*                                                                           */
/* What makes that safe is that compose() rebuilds a rectangle from the room */
/* upwards -- background, shadow, reflection, ball, lettering -- so it       */
/* depends on nothing but the rectangle and the frame's state. It is         */
/* therefore idempotent and order-free: the set need not be disjoint, no     */
/* rectangle need know about any other, and the picture is right as long as  */
/* the set covers everywhere a part IS and everywhere it WAS on this page.   */
/* Which is what a per-part, per-page previous rectangle gives.              */
/*                                                                           */
/* Where splitting stops paying is MEASURED, not assumed, because it differs */
/* by an order of magnitude between the three machines this runs on. Two     */
/* rectangles are two blits, and a blit has a fixed price as well as a       */
/* per-pixel one: on the Atari it is a VDI vro_cpyfm call, on the Pi it is a */
/* function call. blit_cost() times both on the machine it finds itself on   */
/* and states the fixed one AS A NUMBER OF PIXELS (slack_px). Two            */
/* rectangles are then merged when their bounding box costs no more than the */
/* two of them plus that number -- so on a machine where a blit call is dear */
/* the demo keeps the single rectangle it always had, and on one where it is */
/* cheap it splits. Two overlapping rectangles usually have a bounding box   */
/* smaller than their two areas added, so the same rule merges them without  */
/* needing to know that they overlap, and no pixel is composed twice for     */
/* want of a special case.                                                   */
/*                                                                           */
/* slack_px is deliberately the CONSERVATIVE form of that price. The pixels  */
/* a merge adds are paid for twice over -- once composing them and once      */
/* blitting them -- while the call it saves is a blit call only, so the true */
/* break-even is a smaller number of pixels than this one. Erring this way   */
/* splits less readily than the optimum and never more readily, and it needs */
/* one measurement instead of two.                                           */
/* ------------------------------------------------------------------------ */

/* The fixed cost of one blit, in pixels of that same blit. Measured by
 * blit_cost() before the first frame; until then nothing splits. */
static long slack_px;

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

static long rect_area(rect r)
{
	if (r.x1 <= r.x0 || r.y1 <= r.y0)
		return 0;
	return (long)(r.x1 - r.x0) * (long)(r.y1 - r.y0);
}

/* Onto the screen, with anything left over normalised to the one empty
 * rectangle so that area and union both read it the same way. */
static rect rect_clip(rect r)
{
	if (r.x0 < 0)
		r.x0 = 0;
	if (r.y0 < 0)
		r.y0 = 0;
	if (r.x1 > W)
		r.x1 = W;
	if (r.y1 > H)
		r.y1 = H;
	if (r.x1 <= r.x0 || r.y1 <= r.y0) {
		r.x0 = 0;
		r.y0 = 0;
		r.x1 = 0;
		r.y1 = 0;
	}
	return r;
}

/*
 * Where the three moving parts are this frame, one rectangle each.
 *
 * The reflection is drawn in the ball's own columns, so its rectangle shares
 * the ball's x range exactly. It cannot reach ABOVE the ball either: its top
 * row is 2*floor_y - by - Rf, which is below the ball's top row b_top by
 * twice the ball's height above the contact line, and the ball is never below
 * that line. So the union of these three is the single rectangle the demo
 * used to compose -- the same pixels, differently grouped.
 */
static void shape_parts(rect *p)
{
	rect r;

	r.x0 = b_left;
	r.y0 = b_top;
	r.x1 = b_left + D;
	r.y1 = b_top + D;
	p[DP_BALL] = rect_clip(r);

	/* update_shape() has already put the shadow on the screen and on the
	 * floor; clipping again only normalises an empty one. */
	r.x0 = sh_x0;
	r.y0 = sh_y0;
	r.x1 = sh_x1;
	r.y1 = sh_y1;
	p[DP_SHADOW] = rect_clip(r);

	r.x0 = b_left;
	r.x1 = b_left + D;
	r.y0 = rf_y0;
	r.y1 = rf_y1;
	p[DP_REFLECT] = rect_clip(r);
}

/*
 * Merge the pair that gains most, until the best pair on offer costs more
 * than a blit call is worth. With three rectangles that is three comparisons
 * a round and at most two rounds, so the loop is written for clarity rather
 * than for a count that could ever matter.
 */
static void dirty_merge(dirty_set *s)
{
	for (;;) {
		long best = 0;
		int bi = -1, bj = -1, i, j;

		for (i = 0; i < s->n; i++)
			for (j = i + 1; j < s->n; j++) {
				long d = rect_area(rect_union(s->r[i],
							      s->r[j])) -
					 rect_area(s->r[i]) -
					 rect_area(s->r[j]);

				if (bi < 0 || d < best) {
					best = d;
					bi = i;
					bj = j;
				}
			}
		if (bi < 0 || best > slack_px)
			return;
		s->r[bi] = rect_union(s->r[bi], s->r[bj]);
		s->r[bj] = s->r[--s->n];
	}
}

/*
 * The set this frame has to redraw: for every part, where it is now together
 * with where it was when THIS page was last drawn, merged where merging is
 * cheaper than not.
 */
static void dirty_from(dirty_set *s, const rect *cur, const rect *prev)
{
	int i;

	s->n = 0;
	for (i = 0; i < DP_N; i++) {
		rect d = rect_union(cur[i], prev[i]);

		if (d.x1 > d.x0 && d.y1 > d.y0)
			s->r[s->n++] = d;
	}
	dirty_merge(s);
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
		ball_px *row = idx + (long)y * W;
		const ball_px *src = bg + (long)y * W;
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
			ball_px base;
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
			/* Whatever is shaded is read from the ROOM, not from
			 * what is already on the row -- a floor surface, and
			 * only a floor surface. The room's own lines are
			 * skipped by the same test, because a glowing line is
			 * not dimmed by a shadow falling across it. Both the
			 * test and the darkening are one question each of the
			 * composition pixel; see "The composition pixel". */
			base = src[x];
			if (!px_shadeable(base))
				continue;
			row[x] = px_shade(base, lv);
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
	const ball_px *lut = shade_lut;
	unsigned du = (unsigned)tex_du;
	int sy;

	for (sy = 0; sy < D; sy++) {
		int y = b_top + sy;
		int a, b, n;
		const unsigned short *op;
		const unsigned char *shp;
		ball_px *dp;

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
 * The same loop again, upside down and darker.
 *
 * The sphere map is read from the BOTTOM row up (D - 1 - sy), which turns the
 * ball, its shading and its texture over together -- exactly what the mirror
 * does to all three. The spin is unchanged: a mirror in the floor does not
 * reverse a horizontal rotation.
 *
 * Two things fade it with distance below the contact line. The shade table is
 * one of REFL_LUT_N progressively darker copies, and the pixel is only
 * written where the ordered dither threshold is under the coverage -- so the
 * floor beneath, grid lines included, comes through more and more of it. Per
 * pixel that is the ball's four loads and two adds plus one threshold and one
 * compare.
 */
static void draw_reflect(rect r)
{
	const unsigned char *tp = tex.px;
	unsigned du = (unsigned)tex_du;
	int y0 = rf_y0, y1 = rf_y1;
	int y;

	if (y0 < r.y0)
		y0 = r.y0;
	if (y1 > r.y1)
		y1 = r.y1;

	for (y = y0; y < y1; y++) {
		int msy = D - 1 - (y - rf_top);
		const ball_px *lut;
		const float *dr;
		const unsigned short *op;
		const unsigned char *shp;
		ball_px *dp;
		float cov;
		int a, b, n, x;

		if (msy < 0 || msy >= D)
			continue;
		cov = 1.0f - (float)(y - fl_i) * refl_inv;
		if (cov <= 0.0f)
			continue;
		if (cov > 1.0f)
			cov = 1.0f;
		lut = refl_lut[(int)(cov * (float)(REFL_LUT_N - 1) + 0.5f)];
		/* The shade table dims linearly with distance, but the screen
		 * door opens with the SQUARE ROOT of it: a reflection that
		 * loses both at the same rate is gone before it has been seen.
		 * Both still reach nothing at the same place, the bottom row
		 * of the floor, so nothing is cut off there either. */
		cov = b_sqrt(cov);
		dr = dith8[y & 7];

		a = b_left + run0[msy];
		b = b_left + run1[msy];
		if (a < r.x0)
			a = r.x0;
		if (b > r.x1)
			b = r.x1;
		if (b <= a)
			continue;

		n = a - b_left;
		op = map_off + (long)msy * D + n;
		shp = map_shade + (long)msy * D + n;
		dp = idx + (long)y * W + a;
		for (x = a; x < b; x++) {
			if (dr[x & 7] < cov)
				*dp = lut[*shp + tp[(unsigned)*op + du]];
			dp++;
			op++;
			shp++;
		}
	}
}

/*
 * Compose rectangle `r`: the room, then the shadow, then the reflection, then
 * the ball, then the lettering. The room is not copied underneath the ball's
 * own run -- the ball covers it whole -- which is why one pass over the
 * rectangle is enough and no separate background restore is needed anywhere
 * in this program.
 *
 * The order is what puts the picture together: the shadow darkens the floor
 * the reflection then lies on and shows through, and the lettering goes on
 * last so that a rectangle restored for the ball comes back with the words
 * that cross it -- the ball passes behind the readout and behind the corner
 * lines without either of them costing a test in the ball's loop.
 */
static void compose(rect r)
{
	int y;

	for (y = r.y0; y < r.y1; y++) {
		ball_px *row = idx + (long)y * W;
		const ball_px *src = bg + (long)y * W;
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
				px_copy(row + r.x0, src + r.x0, a - r.x0);
			if (r.x1 > b)
				px_copy(row + b, src + b, r.x1 - b);
		} else {
			px_copy(row + r.x0, src + r.x0, r.x1 - r.x0);
		}
	}

	draw_shadow(r);
	draw_reflect(r);
	draw_ball(r);
	ov_draw(r);
}

static void blit(rect r)
{
	if (r.x1 <= r.x0 || r.y1 <= r.y0)
		return;
	plat_blit(idx + (long)r.y0 * W + r.x0, (long)W,
		  r.x0, r.y0, r.x1 - r.x0, r.y1 - r.y0);
}

/*
 * The set, composed and then blitted -- in that order and not interleaved,
 * for the reason the frame loop already had: composing writes the index
 * buffer and blitting reads it, so doing all of one and then all of the other
 * lets the two halves be timed apart with a single clock read between them
 * instead of one per rectangle. It also puts the whole frame on the screen in
 * one uninterrupted run of blits, which is what a single-buffered display
 * needs if the beam is not to arrive in the middle of the picture.
 */
static void compose_set(const dirty_set *s)
{
	int i;

	for (i = 0; i < s->n; i++)
		compose(s->r[i]);
}

static void blit_set(const dirty_set *s)
{
	int i;

	for (i = 0; i < s->n; i++)
		blit(s->r[i]);
}

static long set_area(const dirty_set *s)
{
	long a = 0;
	int i;

	for (i = 0; i < s->n; i++)
		a += rect_area(s->r[i]);
	return a;
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
	rect prev[DP_N], cur[DP_N];
	dirty_set ds;
	long t0;
	/* The benchmark advances the physics to have something to draw, and
	 * how far it gets depends on how fast the machine is. It must
	 * therefore put the demo back EXACTLY where it found it, or the run
	 * that follows starts from a state the machine chose -- and this
	 * program is meant to be a benchmark, where the same demo has to be
	 * the same workload on every machine it is pointed at. All of the
	 * state, not just the obvious half: the wind's three running phases
	 * and the spin are integrated by physics_step() as surely as the
	 * position is, and leaving them where the benchmark stopped is what
	 * made two runs on one machine differ (the PiStorm logs of
	 * 2026-09-07 measured 50 and 52 benchmark frames, so their winds
	 * were 10 ms out of step with each other). */
	float sbx = bx, sby = by, svx = vx, svy = vy;
	float sspin = spin_u, swind = wind_v;
	float sph[WIND_TERMS];
	int sdir = roll_dir, i;

	for (i = 0; i < WIND_TERMS; i++)
		sph[i] = wind_p[i];

	update_shape();
	shape_parts(prev);
	bench_frames = 0;
	t0 = get_sysvar(_hz_200);
	do {
		physics_step();
		update_shape();
		shape_parts(cur);
		dirty_from(&ds, cur, prev);
		compose_set(&ds);
		blit_set(&ds);
		for (i = 0; i < DP_N; i++)
			prev[i] = cur[i];
		bench_frames++;
		bench_ticks = get_sysvar(_hz_200) - t0;
	} while (bench_ticks < BENCH_TICKS && bench_frames < BENCH_MAX);

	bx = sbx;
	by = sby;
	vx = svx;
	vy = svy;
	spin_u = sspin;
	wind_v = swind;
	roll_dir = sdir;
	for (i = 0; i < WIND_TERMS; i++)
		wind_p[i] = sph[i];
	update_shape();			/* the geometry follows the state */
}

/* ------------------------------------------------------------------------ */
/* Where the frame went                                                      */
/*                                                                           */
/* One number per frame -- "was it late" -- cannot say WHERE a frame went,   */
/* and on this demo that is the whole question: composing happens in this    */
/* file, blitting happens on the other side of plat_blit(), and the two are  */
/* not the same machine's problem. So the loop is timed in four parts:       */
/*                                                                          */
/*   step     the physics, the readout's strings and the frame's geometry    */
/*   compose  building the dirty rectangle in the index buffer -- ours       */
/*   blit     handing those rectangles to the platform -- theirs             */
/*   wait     what was left, i.e. the retrace                                */
/*                                                                          */
/* The 200 Hz clock cannot resolve ONE frame: a frame here is a fraction of  */
/* a tick, and worse, a retrace-locked frame starts at a fixed phase of the  */
/* tick, so a per-frame subtraction can read a flat zero however much work   */
/* it is doing. Nothing here pretends otherwise. Each section's tick         */
/* difference is ACCUMULATED over the whole logging interval, and the sum of */
/* a few hundred truncated differences is the number of tick edges that fell */
/* inside that section -- which over 250 frames is the section's real total  */
/* to within a tick. Coarse arithmetic done honestly, not a fine clock       */
/* invented.                                                                 */
/*                                                                           */
/* The same interval also keeps the RAW material the dropped-frame count is  */
/* derived from: a histogram of the ticks each frame consumed. That cannot   */
/* mislead the way a threshold can. A display whose retrace period is a      */
/* whole number of ticks -- 50 Hz on a 200 Hz clock is exactly 4 -- puts the */
/* nominal frame right on the threshold, so a frame that merely sampled the  */
/* clock a hair late reads as one tick long; a LOST retrace cannot hide,     */
/* because it costs a whole extra retrace period and lands two bins further  */
/* along. If the two ever disagree, the histogram is the one that is true.   */
/*                                                                           */
/* What this CANNOT do, and the log has to be read knowing it: a section     */
/* shorter than one tick is only seen when it happens to straddle a tick     */
/* edge, so its estimate is unbiased only if the frame's phase within the    */
/* tick moves. A display whose retrace period is an exact whole number of    */
/* ticks does not move it at all, and one whose period is 4.0012 ticks --    */
/* which is what a 50 Hz screen measures against a 200 Hz clock -- moves it  */
/* about a third of a tick in a five-second interval. So the SUB-TICK        */
/* per-interval figures are noisy, and the run totals in the closing summary */
/* -- taken over a phase that has gone round many times -- are the ones to   */
/* trust. What is not affected either way is the thing this is here to       */
/* catch: a section costing a whole tick or more shows up frame by frame,    */
/* and worst_blit records it exactly.                                        */
/*                                                                           */
/* It costs three extra reads of the system clock per frame, and on TOS a    */
/* clock read is a trap, not a load. What that costs is not guessed either:  */
/* clock_cost() measures it and the log says so.                             */
/* ------------------------------------------------------------------------ */
#define STEP_BINS	12		/* last bin is "this many or more"   */

static long acc_step, acc_compose, acc_blit, acc_wait;
static long tot_step, tot_compose, tot_blit, tot_wait;
/* How much of the screen the frame actually redrew, and in how many pieces.
 * The pixel count is the one figure that says whether the split is doing its
 * job, and unlike the tick sums it is exact per frame. Per interval it is a
 * long; over a whole run it would not fit one on a 32-bit machine, so the run
 * total is a double. */
static long acc_px, acc_rects;
static double tot_px;
static long tot_rects;
static long step_hist[STEP_BINS];
static long worst_blit, worst_compose, worst_frame;
static dirty_set worst_set;

static void budget_reset(void)
{
	int i;

	tot_step += acc_step;
	tot_compose += acc_compose;
	tot_blit += acc_blit;
	tot_wait += acc_wait;
	tot_px += (double)acc_px;
	tot_rects += acc_rects;
	acc_step = 0;
	acc_compose = 0;
	acc_blit = 0;
	acc_wait = 0;
	acc_px = 0;
	acc_rects = 0;
	worst_blit = 0;
	worst_compose = 0;
	worst_frame = 0;
	worst_set.n = 0;
	for (i = 0; i < STEP_BINS; i++)
		step_hist[i] = 0;
}

/* ------------------------------------------------------------------------ */
/* The retrace, measured rather than assumed                                 */
/*                                                                           */
/* Everything the demo says about dropped frames rests on how long a retrace */
/* is, and that is not a number a program may assume: this one runs on a     */
/* 60 Hz Pi, a 50 Hz PiStorm screen, a 71 Hz Atari mono monitor and whatever */
/* the benchmark plan points it at next. So it is asked, once, of the        */
/* machine -- wait for a run of retraces and see how many ticks went by.     */
/*                                                                           */
/* It runs AFTER the first full-screen paint, so every page already holds    */
/* the picture and the flipping is invisible; it hands back how many times   */
/* it presented, because on a two-page display the caller's idea of which    */
/* page is being drawn has to follow.                                        */
/*                                                                           */
/* A platform with no retrace to wait for (plat_present() returns at once)   */
/* measures zero, and says so: there is nothing there to drop, and the log   */
/* prints what the threshold means instead of implying a retrace that is not */
/* running.                                                                  */
/* ------------------------------------------------------------------------ */
#define RETRACE_N	32L		/* 0.64 s at 50 Hz, +-0.8 %	     */

static long retrace_q8;			/* retrace period, 1/256 of a tick   */
static int drop_steps;			/* ... as the test the loop applies  */

static long measure_retrace(void)
{
	long t0, t1, i;

	plat_present();			/* land on one before timing any     */
	t0 = get_sysvar(_hz_200);
	for (i = 0; i < RETRACE_N; i++)
		plat_present();
	t1 = get_sysvar(_hz_200);

	retrace_q8 = ((t1 - t0) * 256L) / RETRACE_N;
	if (retrace_q8 < 256L)
		retrace_q8 = 0;		/* under one tick: nothing waited    */

	if (retrace_q8) {
		drop_steps = (int)((retrace_q8 * DROP_FRAC_NUM) /
				   (256L * DROP_FRAC_DEN));
		if (drop_steps < 2)
			drop_steps = 2;
	} else {
		/* Free-running: a frame cannot miss what is not there, so the
		 * only thing left worth counting is a stall, and MAX_STEPS is
		 * already this program's definition of one. */
		drop_steps = MAX_STEPS;
	}
	return RETRACE_N + 1L;
}

/*
 * What one reading of the system clock costs, in nanoseconds. Measured the
 * only way a 200 Hz clock can measure something far below its own resolution:
 * do it until enough ticks have passed to divide by.
 */
#define CLOCK_COST_TICKS	40L	/* 0.2 s				*/
#define CLOCK_COST_MAX		2000000L

static long clock_ns;			/* per get_sysvar() call		*/

static void clock_cost(void)
{
	long t0 = get_sysvar(_hz_200);
	long n = 0, d;

	do {
		n++;
		d = get_sysvar(_hz_200) - t0;
	} while (d < CLOCK_COST_TICKS && n < CLOCK_COST_MAX);
	/* d ticks over n calls, in nanoseconds; the multiply is done in
	 * floating point because d * 1000000000 does not fit a long. */
	clock_ns = n > 0 ? (long)((double)d * 1e9 /
				  ((double)PHYS_HZ * (double)n) + 0.5) : 0;
}

/* ------------------------------------------------------------------------ */
/* What one more rectangle costs                                             */
/*                                                                           */
/* The frame is split into as many rectangles as it is worth splitting into, */
/* and "worth" is a property of the machine, not of the demo. A blit costs   */
/* something per pixel and something per call, and the ratio between the two */
/* is what decides whether two rectangles far apart are cheaper than the box */
/* around them. On the Pi a blit call is a function call; on the Atari it is */
/* a VDI vro_cpyfm, which on the ST's fVDI driver is a single host call that */
/* does all of the work at once -- so the fixed half is not something this   */
/* program can reason about from first principles, and it does not try.      */
/*                                                                           */
/* It times two blits of very different sizes and solves the straight line   */
/* through them: the slope is the per-pixel cost and the intercept is the    */
/* per-call one. The answer that matters is the intercept expressed in the   */
/* slope's units -- how many pixels a call is worth -- because that is       */
/* exactly the number dirty_merge() needs.                                   */
/*                                                                           */
/* Three details make it honest rather than approximately right:             */
/*                                                                           */
/*   - it blits from the ROOM, which build_room() has already painted, so    */
/*     what appears during the probe is a corner of the picture rather than  */
/*     uninitialised memory. It runs before the first full-screen paint, so  */
/*     nothing of it survives into the demo.                                 */
/*   - the small blit can be quicker than a reading of the system clock, so  */
/*     the clock is read once per batch and the readings are SUBTRACTED,     */
/*     using the cost clock_cost() has just measured. Without that, on a     */
/*     machine with a fast blit, this would mostly measure the trap.         */
/*   - if the two probes cannot be told apart the demo says the call is      */
/*     free rather than inventing a slope, and splits freely.                */
/* ------------------------------------------------------------------------ */
#define BCOST_TICKS	20L		/* 0.1 s per probe		     */
#define BCOST_MAX	2000000L
#define BCOST_BATCH	32L		/* blits between two clock readings  */
#define BCOST_SMALL	8		/* the small probe's side	     */

static long blit_px_ps;			/* picoseconds per blitted pixel     */
static long blit_call_ns;		/* ... and nanoseconds per call	     */

static long blit_probe(int w, int h)
{
	long t0 = get_sysvar(_hz_200);
	long n = 0, reads = 0, d;
	double ns;

	do {
		long k;

		for (k = 0; k < BCOST_BATCH; k++)
			plat_blit(bg, (long)W, 0, 0, w, h);
		n += BCOST_BATCH;
		reads++;
		d = get_sysvar(_hz_200) - t0;
	} while (d < BCOST_TICKS && n < BCOST_MAX);

	ns = (double)d * 1e9 / (double)PHYS_HZ -
	     (double)reads * (double)clock_ns;
	if (ns < 0.0)
		ns = 0.0;
	return (long)(ns / (double)n + 0.5);
}

static void blit_cost(void)
{
	long ns_big, ns_small, a_big, a_small;
	double px_ns, call_ns;
	int wb = D, hb = D;

	if (wb > W)
		wb = W;
	if (hb > H)
		hb = H;
	/* Nothing to fit a line through: split freely and say nothing. */
	if (wb < 4 * BCOST_SMALL || hb < 4 * BCOST_SMALL) {
		slack_px = 0;
		return;
	}

	a_big = (long)wb * (long)hb;
	a_small = (long)BCOST_SMALL * (long)BCOST_SMALL;

	ns_small = blit_probe(BCOST_SMALL, BCOST_SMALL);
	ns_big = blit_probe(wb, hb);

	if (ns_big <= ns_small) {
		blit_px_ps = 0;
		blit_call_ns = ns_small;
		slack_px = 0;
		return;
	}

	px_ns = (double)(ns_big - ns_small) / (double)(a_big - a_small);
	call_ns = (double)ns_small - px_ns * (double)a_small;
	if (call_ns < 0.0)
		call_ns = 0.0;

	blit_px_ps = (long)(px_ns * 1000.0 + 0.5);
	blit_call_ns = (long)(call_ns + 0.5);

	/* A call worth more than a whole screen of pixels can never be worth
	 * paying to split, so that is where the number stops. */
	if (px_ns <= 0.0 || call_ns / px_ns > (double)W * (double)H)
		slack_px = (long)W * (long)H;
	else
		slack_px = (long)(call_ns / px_ns + 0.5);
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
	long t_fps = 0, f_fps = 0;
	long t_b1_prev = 0;		/* where the last frame's blitting ended */
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

	/* How big the ball is: 16 % of the screen width unless C:\BALL.CFG
	 * says otherwise, capped at 18 % and never taller than a third of the
	 * room it has to bounce in. Read here because everything after this
	 * point is sized from R. */
	cfg_read();
	R = radius_of(cfg_ball);
	D = 2 * R;
	Rf = (float)R;
	cfg_ball_used = 2.0f * (float)R / (float)W;

	idx = (ball_px *)b_alloc((long)W * H * (long)sizeof(ball_px));
	bg = (ball_px *)b_alloc((long)W * H * (long)sizeof(ball_px));
	bg_row = (ball_px *)b_alloc((long)H * (long)sizeof(ball_px));
	map_off = (unsigned short *)b_alloc((long)D * D *
					    (long)sizeof(unsigned short));
	map_shade = (unsigned char *)b_alloc((long)D * D);
	run0 = (short *)b_alloc((long)D * (long)sizeof(short));
	run1 = (short *)b_alloc((long)D * (long)sizeof(short));
	tex_buf = (unsigned char *)b_alloc((long)TEX_MAX_BYTES);
	log_buf = (char *)b_alloc(LOG_BUF_MAX);
	if (!idx || !bg || !bg_row || !map_off || !map_shade || !run0 ||
	    !run1 || !tex_buf || !log_buf) {
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
	sin_init();
	light_init();
	tex_build(&tex, tex_buf, TEXPATH);
	build_palette();
	build_refl_lut();
#if !BALL_TRUECOLOR
	plat_palette(pal);
#endif
	px_shade_init();
	build_bg_rows();
	build_sphere_map();

	/* The contact line as a row, and the length over which the reflection
	 * fades: the whole of the floor that is visible below it, so it runs
	 * out exactly at the bottom of the screen instead of being cut off
	 * there. */
	fl_i = (int)floor_y;
	{
		float rl = (float)H - floor_y;

		if (rl < 1.0f)
			rl = 1.0f;
		refl_inv = 1.0f / rl;
	}

	/* The readout has to be laid out before the room is painted: the room
	 * dims itself behind the block, and it can only do that once it knows
	 * where the block is. */
	text_layout();
	build_room();

	/* Serve speeds. The apex is as high as the ball can go and still be
	 * fully on screen, with a little headroom. */
	gravity = GRAVITY_H * (float)H;
	apex_px = (floor_y - (float)D) * APEX_HEADROOM;
	if (apex_px < (float)D)
		apex_px = (float)D;
	v_launch = b_sqrt(2.0f * gravity * apex_px);
	plan_cycle();
	wind_init();
	apex = (long)apex_px;

	bx = (float)W * 0.25f;
	by = floor_y - Rf - apex_px;
	roll_dir = 1;
	vx = vx0;			/* physics_step recomputes it at once */
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
	clock_cost();
	/* Before bench(), because bench() has to measure the headroom under
	 * the very policy the run will use, and dirty_merge() has no policy
	 * until this has run. */
	blit_cost();
	bench();			/* leaves the demo exactly as it was */

	/* The numbers, so the first picture already carries them rather than
	 * a line of zeroes. */
	hud_values();

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
		shape_parts(prev_part[page]);
		plat_present();
		if (++page >= scr.pages)
			page = 0;
	}

	/* How long a retrace is, asked of the machine now that both pages
	 * hold the picture and the flipping cannot be seen. It presents a
	 * number of times, so the page being drawn moves with it. */
	{
		long np = measure_retrace();

		if (scr.pages > 1)
			page = (int)((page + np) % (long)scr.pages);
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

	/* What size ball this actually ran, where the number came from, and
	 * everything that was not taken at face value on the way. */
	o_str("BALL: ball ");
	o_fix4((double)cfg_ball);
	o_str(" of screen width = ");
	o_dec((long)D);
	o_str(" px");
	if (cfg_clamped) {
		o_str(" (clamped to ");
		o_fix4((double)cfg_ball_used);
		o_str(" by the room)");
	}
	o_str(", from ");
	o_str(cfg_from);
	if (cfg_ignored) {
		o_str("; ");
		o_dec((long)cfg_ignored);
		o_str(" line(s) ignored");
	}
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

	o_str("BALL: room ");
	o_dec((long)ROOM_M);
	o_ch('x');
	o_dec((long)ROOM_N);
	o_str(" floor cells, wall ");
	o_dec((long)ROOM_M);
	o_ch('x');
	o_dec((long)rm_k);
	o_str(", ");
	o_dec((long)GRID_N);
	o_str(" palette entries, background image ");
	o_dec((long)W * (long)H);
	o_str(" bytes painted in ");
	o_fix2((double)room_ticks * 1000.0 / (double)PHYS_HZ);
	o_str(" ms; reflection fades over ");
	o_dec((long)(H - fl_i));
	o_str(" rows");
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

	o_str("BALL: wind ");
	o_fix2((double)(vx0 * WIND_FRAC));
	o_str(" px/s peak (");
	o_dec((long)(WIND_FRAC * 100.0f + 0.5f));
	o_str(" % of the roll), ");
	o_dec((long)WIND_TERMS);
	o_str(" sines, periods ");
	{
		int wi;
		for (wi = 0; wi < WIND_TERMS; wi++) {
			if (wi)
				o_str(" / ");
			o_fix2((double)(2.0f * B_PI / wind_w[wi]) /
			       (double)PHYS_HZ);
		}
	}
	o_str(" s, seed ");
	o_dec((long)(unsigned)WIND_SEED);
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

	if (retrace_q8) {
		o_str("BALL: retrace ");
		o_fix2(256.0 * (double)PHYS_HZ / (double)retrace_q8);
		o_str(" Hz measured (");
		o_fix2((double)retrace_q8 / 256.0);
		o_str(" ticks of ");
		o_dec((long)PHYS_HZ);
		o_str(" Hz); a frame counts as dropped above ");
		o_dec((long)drop_steps);
		o_str(" ticks");
	} else {
		o_str("BALL: no retrace to wait for; nothing to drop, so a "
		      "frame is only counted above ");
		o_dec((long)drop_steps);
		o_str(" ticks, which is a stall");
	}
	o_out(0);

	o_str("BALL: clock read ");
	o_dec(clock_ns);
	o_str(" ns; the frame budget is split with 3 extra reads a frame = ");
	o_fix2((double)clock_ns * 3.0 / 1e7);
	o_str(" % of a 20 ms frame");
	o_out(0);

	/* What this machine says a rectangle is worth. slack_px is the whole
	 * of the split's policy: 0 means every part of the picture travels
	 * on its own, a whole screen means the demo keeps the one rectangle
	 * it used to hand over. */
	o_str("BALL: blit ");
	o_fix2((double)blit_px_ps / 1000.0);
	o_str(" ns/px + ");
	o_fix2((double)blit_call_ns / 1000.0);
	o_str(" us/call, so one more rectangle is worth ");
	o_dec(slack_px);
	o_str(" px; the frame splits up to ");
	o_dec((long)DP_N);
	o_str(" ways");
	o_out(0);

	/* The header block, committed before the loop starts, where a disk
	 * write costs nothing. From here on the log is held in memory --
	 * see the LOGPATH commentary for why. */
	log_commit();

	budget_reset();
	t_start = get_sysvar(_hz_200);
	t_last = t_start;
	t_mark = t_start;
	t_fps = t_start;
	f_fps = 0;

	for (;;) {
		long now, t_c0, t_c1, t_b1;
		int steps, nd = 0;
		int hud_d[HUD_LINES];
		dirty_set dirty;

		/* The clock first, the keyboard second: the wait for the
		 * retrace ends where this read is, so anything before it
		 * would be charged to the retrace instead of to the frame. */
		now = get_sysvar(_hz_200);
		if (t_b1_prev)
			acc_wait += now - t_b1_prev;

		/* Quit on any key. Bconstat is the non-blocking half of the
		 * console: a blocking read is what hangs real hardware. */
		if (Bconstat(2)) {
			(void)Bconin(2);
			break;
		}

		steps = (int)(now - t_last);
		t_last = now;
		step_hist[steps < 0 ? 0 :
			  (steps >= STEP_BINS ? STEP_BINS - 1 : steps)]++;
		/* At 60 frames a second the clock advances 3 or 4 ticks per
		 * frame. More than that means a retrace went by unused --
		 * a dropped frame, and the one performance figure a 200 Hz
		 * clock can honestly resolve here. */
		if (steps > drop_steps)
			dropped++;
		if (steps > MAX_STEPS)
			steps = MAX_STEPS;
		while (steps-- > 0) {
			float pvy = vy;

			physics_step();
			/* A bounce is where the ball stops falling and starts
			 * rising, counted OUTSIDE physics_step so the step
			 * itself stays exactly the step it was. */
			if (pvy > 0.0f && vy < 0.0f)
				bounces++;
		}

		/* The rate the readout shows, re-measured twice a second: an
		 * interval short enough to react and long enough that a 200 Hz
		 * clock can divide by it. */
		if (now - t_fps >= FPS_EVERY) {
			hud_fps = (double)(frames - f_fps) * (double)PHYS_HZ /
				  (double)(now - t_fps);
			t_fps = now;
			f_fps = frames;
		}
		hud_values();

		update_shape();
		shape_parts(cur_part);
		dirty_from(&dirty, cur_part, prev_part[page]);
		acc_px += set_area(&dirty);
		acc_rects += dirty.n;

		/* Only the numbers that changed, and only their own little
		 * rectangles. Tracked per page, because on a two-page display
		 * a number written to one page has not been written to the
		 * other; compose() puts the ball back into the rectangle it
		 * re-draws, so a number changing under the ball costs nothing
		 * but its own area.
		 *
		 * Composing is done for all of them before any of them is
		 * blitted. The picture is the same either way -- compose()
		 * writes the index buffer, blit() reads it, and compose(dirty)
		 * already re-letters the readout where the two overlap -- and
		 * it is what lets the two halves be timed apart with one clock
		 * read between them instead of one per rectangle. */
		t_c0 = get_sysvar(_hz_200);
		acc_step += t_c0 - now;
		compose_set(&dirty);
		for (i = HUD_FIRST_VAL; i < HUD_LINES; i++) {
			if (s_eq(hud_line[i], hud_page[page][i]))
				continue;
			compose(hud_vr[i]);
			hud_d[nd++] = i;
		}

		t_c1 = get_sysvar(_hz_200);
		acc_compose += t_c1 - t_c0;
		blit_set(&dirty);
		for (i = 0; i < nd; i++) {
			blit(hud_vr[hud_d[i]]);
			s_cpy(hud_page[page][hud_d[i]], hud_line[hud_d[i]]);
		}

		t_b1 = get_sysvar(_hz_200);
		acc_blit += t_b1 - t_c1;
		t_b1_prev = t_b1;		/* the retrace starts here    */
		/* The dearest blit of the interval, and the rectangle that
		 * bought it: a driver with a slow path takes it for a SHAPE,
		 * so the shape is what gets recorded, not just the cost. */
		if (t_b1 - t_c1 > worst_blit) {
			worst_blit = t_b1 - t_c1;
			worst_set = dirty;
			worst_frame = frames;
		}
		if (t_c1 - t_c0 > worst_compose)
			worst_compose = t_c1 - t_c0;

		for (i = 0; i < DP_N; i++)
			prev_part[page][i] = cur_part[i];
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

			/* Where those frames went. ms per frame, from ticks
			 * accumulated over the whole interval -- see "Where
			 * the frame went" for why it cannot be done per
			 * frame on a 200 Hz clock. */
			{
				double per = df > 0 ? 1000.0 /
					((double)PHYS_HZ * (double)df) : 0.0;
				int bi;

				o_str("BALL:   step ");
				o_fix2((double)acc_step * per);
				o_str(" ms  compose ");
				o_fix2((double)acc_compose * per);
				o_str(" ms  blit ");
				o_fix2((double)acc_blit * per);
				o_str(" ms  wait ");
				o_fix2((double)acc_wait * per);
				o_str(" ms");
				o_out(0);

				/* The ticks each frame consumed, unreduced:
				 * the raw material "dropped" is derived from,
				 * and the only figure here that cannot be
				 * fooled by where the clock was sampled. */
				o_str("BALL:   frame ticks");
				for (bi = 0; bi < STEP_BINS; bi++) {
					if (!step_hist[bi])
						continue;
					o_ch(' ');
					o_dec((long)bi);
					if (bi == STEP_BINS - 1)
						o_ch('+');
					o_ch(':');
					o_dec(step_hist[bi]);
				}
				o_out(0);

				/* What the frame actually redrew: the pixels
				 * and the pieces they came in. This is the
				 * figure the rectangle split exists to move,
				 * and unlike the tick sums above it is exact.
				 */
				o_str("BALL:   redrew ");
				o_fix2(df > 0 ? (double)acc_px / (double)df
					      : 0.0);
				o_str(" px in ");
				o_fix2(df > 0 ? (double)acc_rects / (double)df
					      : 0.0);
				o_str(" rect/frame (");
				o_fix2((double)acc_px * 100.0 /
				       ((double)W * (double)H *
					(df > 0 ? (double)df : 1.0)));
				o_str(" % of the screen)");
				o_out(0);

				o_str("BALL:   worst blit ");
				o_fix2((double)worst_blit * 1000.0 /
				       (double)PHYS_HZ);
				o_str(" ms f");
				o_dec(worst_frame);
				o_str("; worst compose ");
				o_fix2((double)worst_compose * 1000.0 /
				       (double)PHYS_HZ);
				o_str(" ms");
				o_out(0);

				/* And the rectangles that bought that blit: a
				 * driver with a slow path takes it for a
				 * SHAPE, so the shapes are what is recorded,
				 * odd origin and odd width included. */
				o_str("BALL:   worst rects");
				for (bi = 0; bi < worst_set.n; bi++) {
					rect wr = worst_set.r[bi];

					o_ch(' ');
					o_dec((long)(wr.x1 - wr.x0));
					o_ch('x');
					o_dec((long)(wr.y1 - wr.y0));
					o_ch('@');
					o_dec((long)wr.x0);
					o_ch(',');
					o_dec((long)wr.y0);
					o_ch('(');
					o_dec((long)(wr.x0 & 1));
					o_ch(',');
					o_dec((long)((wr.x1 - wr.x0) & 1));
					o_ch(')');
				}
				o_str(" = ");
				o_dec(set_area(&worst_set));
				o_str(" px");
				o_out(0);
			}

			budget_reset();
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
	o_str(" dropped, ");
	o_dec(bounces);
	o_str(" bounces");
	o_out(1);

	/* The whole run's budget. Over this many frames the frame's phase
	 * within the 200 Hz tick has gone round many times, so unlike the
	 * per-interval figures these are not biased by where it sat. */
	budget_reset();				/* fold the last interval in   */
	{
		double per = frames > 0 ? 1000.0 /
			((double)PHYS_HZ * (double)frames) : 0.0;

		o_str("BALL: whole run per frame: step ");
		o_fix2((double)tot_step * per);
		o_str(" ms  compose ");
		o_fix2((double)tot_compose * per);
		o_str(" ms  blit ");
		o_fix2((double)tot_blit * per);
		o_str(" ms  wait ");
		o_fix2((double)tot_wait * per);
		o_str(" ms");
		o_out(1);

		o_str("BALL: whole run redrew ");
		o_fix2(frames > 0 ? tot_px / (double)frames : 0.0);
		o_str(" px in ");
		o_fix2(frames > 0 ? (double)tot_rects / (double)frames : 0.0);
		o_str(" rect/frame = ");
		o_fix2(frames > 0 ? tot_px * 100.0 /
				    ((double)W * (double)H * (double)frames)
				  : 0.0);
		o_str(" % of the screen");
		o_flush();
	}
	log_commit();

done:
	b_free(log_buf);
	b_free(tex_buf);
	b_free(run1);
	b_free(run0);
	b_free(map_shade);
	b_free(map_off);
	b_free(bg_row);
	b_free(bg);
	b_free(idx);
	return rc;
}
