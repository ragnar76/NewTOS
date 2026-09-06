/*
 * ball.h -- the seam between the bouncing-ball demo and the machine it runs on.
 *
 * The demo is ONE source that is meant to build for two very different
 * machines: NewTOS on a Raspberry Pi 4 (1920x1080, 4 bytes per pixel, no
 * palette) and TOS on an Atari TT with a NOVA graphics card (640x480, 1 byte
 * per pixel, a hardware palette). Everything that differs between them lives
 * behind the five functions below; everything else -- physics, the fixed time
 * step, collision, damping, the palette design, the sprite, dirty-rectangle
 * management, the main loop -- is in ball.c and is compiled unchanged for
 * both.
 *
 * The pixel format is the one thing that cannot be shared, so the core never
 * touches it: it composes each frame into a buffer of **palette indices**
 * (one byte per pixel) and hands rectangles of that buffer to plat_blit().
 * On an 8-bit palette machine that copy is a memcpy; on the Pi the platform
 * layer holds a 256-entry index -> XRGB8888 table and converts while it
 * copies. This is not a compromise for the TT's sake: the Pi's framebuffer is
 * mapped Normal Non-Cacheable into the process (ADR-023), so the frame has to
 * be built in ordinary memory and written out once either way, and writing it
 * out as one byte per pixel from a cached buffer is the cheaper half of that
 * trip on both machines.
 *
 * TOS itself is NOT part of this seam. GEMDOS/BIOS/XBIOS calls (Mxalloc,
 * Bconstat, Vsync, the 200 Hz tick) are the same API on both machines and are
 * called straight from the core through <mint/osbind.h>; only the trap
 * instruction behind those macros differs, and mintlib already knows both.
 */
#ifndef BALL_H
#define BALL_H

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

/* Install the demo's 256 colours. Called once, right after plat_open(). */
void plat_palette(const ball_rgb *pal);

/*
 * Copy one rectangle of palette indices onto the page currently being drawn.
 * `src` points at the rectangle's top-left byte inside the core's index
 * buffer, `src_pitch` is that buffer's row stride, and (x, y) is where the
 * rectangle goes on screen. This is the demo's only write to screen memory.
 */
void plat_blit(const unsigned char *src, long src_pitch,
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
