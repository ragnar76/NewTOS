/*
 * ball.h -- the seam between the bouncing-ball demo and the machine it runs on.
 *
 * The demo is ONE source that is meant to build for two very different
 * machines: NewTOS on a Raspberry Pi 4 (1920x1080, 4 bytes per pixel, no
 * palette) and TOS on an Atari TT with a NOVA graphics card (640x480, 1 byte
 * per pixel, a hardware palette). Everything that differs between them lives
 * behind the functions below; everything else -- physics, the fixed time
 * step, collision, damping, the palette design, the sprite, dirty-rectangle
 * management, the main loop -- is in ball.c and is compiled unchanged for
 * both.
 *
 * The pixel format is the one thing that cannot be shared, so the core never
 * touches it: it composes each frame into a buffer of its own and hands
 * rectangles of that buffer to plat_blit(). How wide one of those pixels is
 * is stated below and is the TARGET's answer, not the core's guess -- on a
 * palette machine it is an index and the platform owns the colour table, on
 * a truecolour one it is RGB and there is no table. Either way the frame is
 * built in ordinary memory and written out exactly once, because that is
 * what both machines want: on the TT every byte crosses the card's bus, and
 * on the Pi the framebuffer is mapped Normal Non-Cacheable into the process
 * (ADR-023), so a single-pixel store goes straight to DRAM.
 *
 * TOS itself is NOT part of this seam. GEMDOS/BIOS/XBIOS calls (Mxalloc,
 * Bconstat, Vsync, the 200 Hz tick) are the same API on both machines and are
 * called straight from the core through <mint/osbind.h>; only the trap
 * instruction behind those macros differs, and mintlib already knows both.
 */
#ifndef BALL_H
#define BALL_H

/*
 * ---------------------------------------------------------------------------
 * The composition pixel, and who decides how wide it is
 * ---------------------------------------------------------------------------
 * The core composes each frame into a buffer of its own and hands rectangles
 * of it to plat_blit(). How wide one of those pixels is depends on the
 * screen the target actually has, and it is the TARGET that says so: the
 * build script for a machine defines BALL_TRUECOLOR (or does not), and the
 * core is told. It never asks the screen at run time and it never assumes.
 *
 *   BALL_TRUECOLOR 0   one byte, a PALETTE INDEX. The picture is designed
 *                      into 256 entries, plat_palette() installs them, and
 *                      plat_blit() either copies the bytes (an 8-bit card)
 *                      or converts them through a 256-entry table (the Pi).
 *
 *   BALL_TRUECOLOR 1   four bytes, 0x00RRGGBB, plus a few TAG bits above
 *                      the colour that the composer uses and the screen
 *                      ignores (see ball.c, "The composition pixel"). There
 *                      is no palette and there is no plat_palette().
 *
 * It is a compile-time choice on purpose. A run-time one would either put a
 * branch in the inner loop that draws the sphere -- which is four loads, two
 * adds, one lookup and one store, and is meant to stay that -- or duplicate
 * the renderer. Two builds of one source is the cheaper answer, and it is
 * the same answer the rest of this seam gives: one source, two machines,
 * everything that differs stated once.
 */
#ifndef BALL_TRUECOLOR
#define BALL_TRUECOLOR 0
#endif

#if BALL_TRUECOLOR
typedef unsigned int ball_px;
/* 32 bits is not a hope here, it is the pixel format; say so at compile time
 * rather than discovering it in a screenshot. */
typedef char ball_px_is_32_bit[sizeof(ball_px) == 4 ? 1 : -1];
#else
typedef unsigned char ball_px;
#endif

/* What plat_open() found out about the screen. */
typedef struct {
	int w;			/* visible width in pixels		     */
	int h;			/* visible height in pixels		     */
	int pages;		/* screen pages plat_present() can flip (1|2) */
} ball_screen;

typedef struct {
	unsigned char r, g, b;
} ball_rgb;

#define BALL_PAL_N 256

/*
 * Set the screen up and report its geometry. Returns 0, or a negative value
 * whose reason plat_error() spells out. After this call the demo owns the
 * screen until plat_close().
 */
int plat_open(ball_screen *scr);

#if !BALL_TRUECOLOR
/* Install the demo's 256 colours. Called once, right after plat_open().
 * A truecolour target has no colour table to install and does not have this
 * function; the core does not call it there. */
void plat_palette(const ball_rgb *pal);
#endif

/*
 * Copy one rectangle of palette indices onto the page currently being drawn.
 * `src` points at the rectangle's top-left byte inside the core's index
 * buffer, `src_pitch` is that buffer's row stride, and (x, y) is where the
 * rectangle goes on screen. This is the demo's only write to screen memory.
 */
void plat_blit(const ball_px *src, long src_pitch,
	       int x, int y, int w, int h);

/*
 * Finish the frame: wait for the vertical retrace and, on a machine with two
 * screen pages, show the page just drawn and start drawing into the other.
 * On a single-page display it only waits -- which is the whole reason the
 * core must not assume the picture moved.
 */
void plat_present(void);

/* Put the screen back the way it was found. */
void plat_close(void);

/* Why the last plat_ call failed. Never NULL. */
const char *plat_error(void);

#endif /* BALL_H */
