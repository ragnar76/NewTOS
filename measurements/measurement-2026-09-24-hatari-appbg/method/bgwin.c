/*
 * bgwin.c -- minimal m68k GEM application for the background question.
 *
 * Opens ONE window in the middle of the screen, paints nothing outside it,
 * waits, closes, exits. Deliberately never calls v_clrwk: whatever is behind
 * the window is what the AES/shell left there, which is the whole point.
 *
 * Raw AES traps (gem_aesP.h's sequence), no gemlib, like tosdump/abi.
 */
#include <stdint.h>

static int16_t contrl[5], global[16], intin[16], intout[16], addrin[4], addrout[4];

typedef struct { int16_t *c, *g, *ii, *io, *ai, *ao; } AESPB;
static AESPB pb = { contrl, global, intin, intout, addrin, addrout };

static void aes(int16_t op, int16_t ni, int16_t no, int16_t na, int16_t nao)
{
    contrl[0] = op; contrl[1] = ni; contrl[2] = no; contrl[3] = na; contrl[4] = nao;
    __asm__ volatile (
        "move.l %0,%%d1\n\t"
        "move.w #200,%%d0\n\t"
        "trap   #2\n\t"
        : : "g" (&pb)
        : "d0","d1","d2","a0","a1","a2","memory","cc");
}

int main(void)
{
    int16_t handle, i;

    aes(10, 0, 1, 0, 0);                       /* appl_init */

    /* wind_create(NAME|MOVER|CLOSER, 120,80,400,300) */
    intin[0] = 0x0001 | 0x0004 | 0x0008;       /* CLOSER|NAME|MOVER */
    intin[1] = 120; intin[2] = 80; intin[3] = 400; intin[4] = 300;
    aes(100, 5, 1, 0, 0);
    handle = intout[0];

    intin[0] = handle;
    intin[1] = 120; intin[2] = 80; intin[3] = 400; intin[4] = 300;
    aes(101, 5, 1, 0, 0);                      /* wind_open */

    /* evnt_timer(3000 ms) a few times so the screen can be grabbed */
    for (i = 0; i < 6; i++) {
        intin[0] = 3000 & 0xffff;
        intin[1] = (3000 >> 16) & 0xffff;
        aes(24, 2, 1, 0, 0);                   /* evnt_timer */
    }

    intin[0] = handle; aes(102, 1, 1, 0, 0);   /* wind_close */
    intin[0] = handle; aes(103, 1, 1, 0, 0);   /* wind_delete */
    aes(19, 0, 1, 0, 0);                       /* appl_exit */
    return 0;
}
