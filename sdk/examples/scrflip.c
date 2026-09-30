/*
 * scrflip.c -- proof that XBIOS `Setscreen` (0x05) really moves the logical
 * and the physical screen, i.e. that NewTOS can double-buffer.
 *
 * A native NewTOS console program (`.NCP`, ADR-018). It is a check, not a
 * demo -- it draws nothing that moves, and it puts every base back before it
 * exits so the boot can carry on into the desktop.
 *
 * What it proves, in the order a screendump can see it:
 *
 *   1. `Logbase()` / `Physbase()` start out equal: one screen, and it is the
 *      one you see.
 *   2. There is a *second* screen page. The program finds it without
 *      assuming any geometry: it walks candidate offsets and asks
 *      `Setscreen(base + off, -1, -1, -1)` for each, then reads `Logbase()`
 *      back. `Setscreen`'s contract is that a base the display cannot use
 *      leaves that base untouched and `Logbase()` keeps reporting the truth
 *      -- so the first offset that *does* move the logical base is the page
 *      size. (A program that already knows its screen layout would just
 *      compute it; this one deliberately does not, so the check does not
 *      depend on a guess.)
 *   3. Drawing into the logical page does NOT change the picture: the
 *      program fills the whole hidden page with three colour bands, puts the
 *      logical base back on page 0 so its own console output does not land
 *      there, and waits. A screendump taken now shows the console -- the
 *      bands are invisible.
 *   4. `Setscreen(-1, page1, -1, -1)` makes that page the physical screen.
 *      A screendump taken now shows the bands. Nothing was copied; only the
 *      display's scan-out base moved.
 *   5. Both bases go back to page 0 and the program exits.
 *
 * The stores into the hidden page use no cache maintenance of any kind: the
 * pages `Physbase`/`Logbase` publish are mapped Normal Non-Cacheable exactly
 * so that a program never has to know about caches (ADR-023 amendment).
 *
 * Deliberately **no** newtos_pause() (TASK/T13), for two reasons. It keeps a
 * live \AUTO\ route -- tools/make-sd-image.sh copies it there on
 * SD_SCRFLIP_AUTO=1, which this program is built for ("it restores
 * everything and exits, so the boot can carry on"); a key wait would stop
 * that boot. And it already holds what it wants shown: each of the two
 * screendump windows in steps 3 and 4 stands ~10 s by itself.
 *
 * Spec: docs/api/xbios/05-setscreen.md, 02-physbase.md, 03-logbase.md,
 *       25-vsync.md.
 */
#include <newtos.h>

#define XB_PHYSBASE  0x02
#define XB_LOGBASE   0x03
#define XB_SETSCREEN 0x05
#define XB_VSYNC     0x25

/* XRGB8888, opaque: 0xFF_RR_GG_BB (hal::framebuffer::rgb). */
#define RED   0xFFFF0000u
#define GREEN 0xFF00FF00u
#define BLUE  0xFF0000FFu

/* Page-size search: the display's screen page is a whole number of 4-KiB
 * frames on every geometry NewTOS boots (pitch * height), and the published
 * screen memory is at most a few tens of MiB, so this bound always covers
 * it while keeping the probe short. */
#define PROBE_STEP  4096uL
#define PROBE_LIMIT (64uL * 1024uL * 1024uL)

/* Seconds each screendump window stays open (frames at the ~60 Hz the
 * NewTOS frame clock runs at -- see docs/api/xbios/25-vsync.md). */
#define WINDOW_FRAMES (10 * 60)

static NLONG Physbase(void)  { return xbios0(XB_PHYSBASE); }
static NLONG Logbase(void)   { return xbios0(XB_LOGBASE); }
static void  Vsync(void)     { xbios0(XB_VSYNC); }

static NLONG Setscreen(NLONG logLoc, NLONG physLoc, NLONG rez, NLONG videlmode)
{
    return xbios4(XB_SETSCREEN, logLoc, physLoc, rez, videlmode);
}

static void puts_(const char *s) { Cconws(s); }

/* Print `v` as 16 hex digits followed by CR/LF. No libc, no printf. */
static void puthex(unsigned long v)
{
    static const char digits[] = "0123456789ABCDEF";
    char buf[19];
    int i;

    for (i = 0; i < 16; i++)
        buf[i] = digits[(v >> ((15 - i) * 4)) & 0xF];
    buf[16] = '\r';
    buf[17] = '\n';
    buf[18] = 0;
    Cconws(buf);
}

static void wait_frames(unsigned n)
{
    while (n--)
        Vsync();
}

int main(void)
{
    NLONG page0 = Logbase();
    NLONG phys0 = Physbase();
    unsigned long off, page = 0;
    volatile unsigned int *p;
    unsigned long i, band;

    puts_("SCRFLIP.NCP -- Setscreen page flip\r\n");
    puts_("Logbase()  = ");
    puthex((unsigned long)page0);
    puts_("Physbase() = ");
    puthex((unsigned long)phys0);

    if (page0 == 0) {
        puts_("no display up -- nothing to flip\r\n");
        return 1;
    }

    /* 2. Find the second screen page through the Setscreen contract. */
    for (off = PROBE_STEP; off < PROBE_LIMIT; off += PROBE_STEP) {
        Setscreen(page0 + (NLONG)off, -1, -1, -1);
        if (Logbase() != page0) {
            page = off;
            break;
        }
    }
    if (page == 0) {
        puts_("this display has a single screen page -- no flipping\r\n");
        return 1;
    }
    puts_("screen page size = ");
    puthex(page);
    puts_("hidden page at   = ");
    puthex((unsigned long)Logbase());

    /* 3. Paint the hidden page. Linear bands, so no geometry is assumed. */
    p = (volatile unsigned int *)Logbase();
    band = (page / 4) / 3;
    for (i = 0; i < band; i++)
        p[i] = RED;
    for (; i < 2 * band; i++)
        p[i] = GREEN;
    for (; i < page / 4; i++)
        p[i] = BLUE;

    /* Put drawing back on the visible page, so the lines below land where
     * they can be read and not on top of the bands we just painted. */
    Setscreen(page0, -1, -1, -1);
    puts_("HIDDEN PAGE PAINTED -- screendump now must NOT show the bands\r\n");
    wait_frames(WINDOW_FRAMES);

    /* 4. Show it. One Vsync first, so the scan-out moves on a frame
     * boundary (docs/api/xbios/25-vsync.md). */
    Vsync();
    Setscreen(-1, page0 + (NLONG)page, -1, -1);
    puts_("FLIPPED -- screendump now must show the bands\r\n");
    puts_("Physbase() = ");
    puthex((unsigned long)Physbase());
    wait_frames(WINDOW_FRAMES);

    /* 5. Put everything back, so the boot can go on to the desktop. */
    Vsync();
    Setscreen(page0, page0, -1, -1);
    puts_("RESTORED\r\n");
    puts_("Logbase()  = ");
    puthex((unsigned long)Logbase());
    puts_("Physbase() = ");
    puthex((unsigned long)Physbase());
    return 0;
}
