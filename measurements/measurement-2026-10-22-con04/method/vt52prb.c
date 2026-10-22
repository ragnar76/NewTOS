/*
 * vt52prb.c -- CON04 probe of the VT52 console (Bconout(2)) and the raw
 * console (Bconout(5)).
 *
 * One source, two targets, the same bytes on both screens:
 *   - m68k (EmuTOS on Hatari Falcon, the reference):  VT52A.TOS / VT52B.TOS
 *       m68k-atari-mint-gcc -O2 -DSCREEN=1 vt52prb.c -o VT52A.TOS
 *   - AArch64 (NewTOS, AUTO folder):                   VT52A.NCP / VT52B.NCP
 *       CFLAGS=-DSCREEN=1 sdk/build.sh sdk/examples/vt52prb.c VT52A.NCP
 *
 * Screen 1 (SCREEN=1): every non-destructive sequence and control character,
 * the Q3 reverse-video border-cell rows, colours (ESC b / ESC c), the raw
 * Handle-5 path and a visible cursor. Screen 2 (SCREEN=2): the erasing and
 * scrolling sequences (ESC J, ESC d, ESC K with a background colour, LF at
 * the bottom, ESC I at the top). Both end in Cnecin, so the screen stays up
 * for a screendump; the caller kills the machine.
 *
 * Only BIOS Bconout, XBIOS Cursconf and GEMDOS Cnecin are used, so nothing
 * between the program and the console (GEMDOS tab expansion, libc newline
 * translation) touches the bytes.
 *
 * Spec: docs/api/bios/_vt52.md, docs/api/bios/03-bconout.md.
 */

#ifndef SCREEN
#define SCREEN 1
#endif

#define UNUSED __attribute__((unused))

#if defined(__mc68000__) || defined(__m68k__)
#include <osbind.h>
static void out2(int c) { Bconout(2, c & 0xff); }
UNUSED static void out5(int c) { Bconout(5, c & 0xff); }
UNUSED static void cursconf(int f, int r) { Cursconf(f, r); }
static void hold(void) { (void)Cnecin(); }
#else
#include <newtos.h>
static void out2(int c) { bios2(3, 2, c & 0xff); }
UNUSED static void out5(int c) { bios2(3, 5, c & 0xff); }
UNUSED static void cursconf(int f, int r) { xbios4(0x15, f, r, 0, 0); }
static void hold(void) { (void)gemdos0(0x08); }
#endif

#define ESC 0x1b

static void s2(const char *s)
{
    while (*s)
        out2((unsigned char)*s++);
}

static void esc(int c)
{
    out2(ESC);
    out2(c);
}

/* ESC Y: row and column as byte = 32 + value. */
static void at(int row, int col)
{
    out2(ESC);
    out2('Y');
    out2(32 + row);
    out2(32 + col);
}

/* A whole row of `c`, then CR LF (never on the last row: LF would scroll). */
UNUSED static void fill_row(int c)
{
    int i;
    for (i = 0; i < 80; i++)
        out2(c);
}

#if SCREEN == 1
static void screen1(void)
{
    int i, row;

    esc('E');                  /* clear + home */
    esc('f');                  /* cursor off for the drawing */

    /* r16: the INITIAL wrap state (no ESC v / ESC w sent yet): wrap off
     * leaves "abcg" at columns 76..79; wrap on would put "efg" on r17. */
    at(16, 76);
    s2("abcdefg");

    at(0, 1);
    s2("CON04 VT52 probe 1");

    /* r2..r7: Q3 -- prefill '#', then ESC p + ESC K / ESC o at even and odd
     * columns, at the last column (ESC K) and at column 0 (ESC o). */
    for (row = 2; row <= 7; row++) {
        at(row, 0);
        fill_row('#');
    }
    at(2, 10); esc('p'); esc('K'); esc('q');   /* even column, to EOL */
    at(3, 11); esc('p'); esc('K'); esc('q');   /* odd column, to EOL */
    at(4, 10); esc('p'); esc('o'); esc('q');   /* even column, from BOL */
    at(5, 11); esc('p'); esc('o'); esc('q');   /* odd column, from BOL */
    at(6, 79); esc('p'); esc('K'); esc('q');   /* last column, to EOL */
    at(7, 0);  esc('p'); esc('o'); esc('q');   /* column 0, from BOL */

    /* r8: foreground registers 0..15 ('0'..'?' & 0x0f), two cells each. */
    at(8, 0);
    for (i = 0; i < 16; i++) {
        esc('b'); out2('0' + i);
        s2("XX");
    }
    esc('b'); out2('?');
    /* r9: background registers 0..15 with foreground 15. */
    at(9, 0);
    for (i = 0; i < 16; i++) {
        esc('c'); out2('0' + i);
        s2("oo");
    }
    esc('c'); out2('0');

    /* r10: reverse video, also with a coloured foreground. */
    at(10, 0);
    s2("normal ");
    esc('p'); s2("reverse"); esc('q');
    s2(" normal ");
    esc('b'); out2('1'); esc('p'); s2("revred"); esc('q'); esc('b'); out2('?');

    /* r11: BS steps left without erasing; BS at column 0 does nothing. */
    at(11, 0);
    s2("ABCD\b\bxy|");
    s2("\r\bZ");               /* -> "ZBxy|" */

    /* r12: HT to the next multiple of 8; at the end it clamps to col 79. */
    at(12, 0);
    s2("a\tb\tc");
    at(12, 75);
    s2("\t\tT");

    /* r13..r15: LF keeps the column; VT and FF are LFs. */
    at(13, 0);
    s2("L1\nL2");
    at(13, 10);
    s2("V\013W\014X");

    /* r17..r18: wrap on -- "abcd" on r17, "efg" on r18. */
    esc('v');
    at(17, 76);
    s2("abcdefg");
    esc('w');

    /* r19..r20: ESC j / ESC k; a second ESC k without ESC j goes home. */
    at(19, 5);
    esc('j');
    s2("SAVE");
    at(20, 0);
    s2("Z");
    esc('k');
    s2("R");                   /* -> r19 "RAVE" at column 5 */
    esc('k');
    s2("@");                   /* -> (0,0) */

    /* ESC Y clamping: bytes below 0x20 -> 0, beyond the screen -> last. */
    out2(ESC); out2('Y'); out2(0x01); out2(32 + 21);
    s2("!");                   /* -> (0,21) */

    /* r21: ESC I below the top row moves up without scrolling. */
    at(22, 3);
    esc('I');
    s2("i");                   /* -> (21,3) */

    /* r22: invalid sequences are swallowed, ESC ESC eats the second ESC. */
    at(22, 10);
    s2("[");
    esc('a'); esc('Z'); esc('x'); esc('F'); esc('N');
    out2(ESC); out2(ESC); out2('Q');
    s2("]");                   /* -> "[Q]" */

    /* r23: control characters 0..6, 14..26, 28..31 are discarded, BEL too. */
    at(23, 0);
    s2("<");
    for (i = 1; i <= 6; i++)
        out2(i);
    for (i = 14; i <= 26; i++)
        out2(i);
    for (i = 28; i <= 31; i++)
        out2(i);
    out2(7);
    out2(0);
    s2(">");                   /* -> "<>" */

    /* r24: the raw console draws every byte as a glyph ... */
    at(24, 0);
    out5(ESC); out5('E'); out5(7); out5('\r'); out5('\n'); out5(1); out5(8); out5(9);
    /* ... and does not touch a half-received Handle-2 ESC. */
    out2(' ');
    out2(ESC);
    out5('A');
    out2('p');                 /* completes ESC p */
    s2("rev");
    esc('q');

    /* r25..r29: ESC L / ESC M / ESC l. */
    at(25, 0); s2("delme");
    at(26, 0); s2("row26");
    at(27, 0); s2("row27");
    at(28, 0); s2("row28");
    at(27, 10);
    esc('L');
    s2("ins");                 /* blank line at 27, cursor col 0 */
    at(25, 3);
    esc('M');                  /* rows 26.. move up, r29 blank */
    at(29, 0);
    s2("eraseline");
    esc('l');
    s2("E");                   /* row 29 = "E" */
    out2(ESC); out2('Y'); out2(0x7f); out2(0x7f);
    s2("*");                   /* -> (29,79) */

    /* Cursor: ESC f twice, one ESC e shows it (disab_cnt forced to 0).
     * Cursconf(2) clears M_CFLASH (bios/vt52.c:902-903), which blink()
     * reads as "not flashing" (:856): a steady cursor for the screendump. */
    at(28, 40);
    esc('f');
    esc('f');
    esc('e');
    cursconf(2, 0);
}
#else
static void screen2(void)
{
    int row;

    esc('E');
    esc('f');
    esc('w');
    for (row = 0; row < 30; row++) {
        at(row, 0);
        fill_row('A' + row);
    }
    at(5, 40);
    esc('J');                  /* rest of r5 and r6..r29 erased */
    at(2, 20);
    esc('d');                  /* r0, r1 and r2 col 0..20 erased */
    esc('c'); out2('4');
    at(3, 30);
    esc('K');                  /* r3 col 30..79 in register 4 */
    esc('c'); out2('0');
    at(7, 0);
    s2("after ESC J");
    at(29, 0);
    s2("S29");
    s2("\n");                  /* LF on the last row scrolls up */
    s2("S30");
    esc('H');
    esc('I');                  /* ESC I on the top row scrolls down */
    s2("top");
    at(10, 0);
    esc('b'); out2('2');
    s2("green fg ");
    esc('c'); out2('1');
    s2("on red bg");
    esc('b'); out2('?');
    esc('c'); out2('0');
}
#endif

int main(void)
{
#if SCREEN == 1
    screen1();
#else
    screen2();
#endif
    hold();
    return 0;
}
