/*
 * physbase.c -- proof that XBIOS `Physbase` (0x02) / `Logbase` (0x03) hand an
 * EL0 program an address it can actually draw into.
 *
 * A native NewTOS console program (`.NCP`, ADR-018). It does exactly three
 * things, and deliberately nothing more -- it is a check, not a demo:
 *
 *   1. calls `Physbase` and `Logbase` and prints both addresses, so the log
 *      shows whether they are user addresses (ADR-023; since ARCH03 stage 1
 *      the screen's own physical address, VA = PA) or the kernel high half
 *      (`0xFFFF...`, unreachable from EL0);
 *   2. stores three bands of solid colour straight into that address -- a
 *      plain `*p++ = colour` loop with **no cache maintenance whatsoever**,
 *      because the pages `Physbase` publishes are mapped Normal Non-Cacheable
 *      exactly so that a program never has to know about caches;
 *   3. blocks on `Cconin`, so the bands stay on screen for a screendump.
 *
 * The pattern is written as one linear run from the base, which needs no
 * knowledge of the display geometry: whatever the width and pitch, the first
 * 120000 pixels are the top of the screen. The pixel format is the framebuffer's
 * own XRGB8888 (`0xFF_RR_GG_BB`, `kernel/src/hal/framebuffer.rs`) -- NOT Atari
 * bitplanes. Shifter-format direct-screen writers are out of scope for NewTOS
 * (CLAUDE.md "Non-goals"); a program written *for* NewTOS gets a linear
 * 32-bit-per-pixel buffer.
 *
 * Spec: docs/api/xbios/02-physbase.md, docs/api/xbios/03-logbase.md.
 */
#include <newtos.h>

/* Pixels per colour band. 3 x 40000 = 120000 pixels = 480 KiB, which is a
 * visible slab at every geometry we boot (62 rows per band at 640 wide,
 * 20 rows per band at 1920 wide). */
#define BAND_PIXELS 40000u

/* XRGB8888, opaque: 0xFF_RR_GG_BB (hal::framebuffer::rgb). */
#define RED   0xFFFF0000u
#define GREEN 0xFF00FF00u
#define BLUE  0xFF0000FFu

static void puts_(const char *s)
{
    Cconws(s);
}

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

int main(void)
{
    unsigned long phys = (unsigned long)xbios0(0x02); /* Physbase */
    unsigned long log  = (unsigned long)xbios0(0x03); /* Logbase  */
    volatile unsigned int *p;
    unsigned int i;

    puts_("PHYSBASE.NCP -- direct screen access from EL0\r\n");
    puts_("Physbase() = ");
    puthex(phys);
    puts_("Logbase()  = ");
    puthex(log);

    if (phys == 0) {
        puts_("no display up -- nothing to draw\r\n");
        return 1;
    }

    /* The whole point: a plain store loop, no cache maintenance, no flush
     * call. If the bands appear, the mapping attribute is right. */
    p = (volatile unsigned int *)phys;
    for (i = 0; i < BAND_PIXELS; i++)
        p[i] = RED;
    for (i = 0; i < BAND_PIXELS; i++)
        p[BAND_PIXELS + i] = GREEN;
    for (i = 0; i < BAND_PIXELS; i++)
        p[2 * BAND_PIXELS + i] = BLUE;

    puts_("wrote 3 bands; press a key\r\n");
    Cconin();
    return 0;
}
