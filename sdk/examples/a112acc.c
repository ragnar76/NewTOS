/*
 * a112acc.c -- TASK/A112 companion desk accessory (A112ACC.NAC): makes the
 * AES post a message to the running program from ANOTHER process while that
 * program waits in evnt_multi(MU_MESAG|MU_TIMER) (case G of a112mesg.c).
 *
 *   appl_init
 *   evnt_multi(MU_MESAG|MU_TIMER, 0)   -- once: issues a message wait, so the
 *                                         AP_MESAG boot barrier sees it
 *                                         (EmuTOS gemsuper.c:147-148)
 *   loop: evnt_timer(200 ms); wind_get(0, WF_TOP)
 *         the first time a window is on top (the probe's own window), open a
 *         full-screen window of our own, close it and delete it: closing it
 *         uncovers the probe's window, and the AES posts WM_REDRAW to the
 *         window's owner (EmuTOS wind_close -> w_redraw -> ap_sendmsg,
 *         aes/gemwmlib.c:887-907); repeat that once 10 timer ticks later
 *         (for case G2), then evnt_mesag forever.
 *
 * Progress lines go to the console (Cconws -> serial), prefix "A112ACC:".
 * Build: sdk/build.sh sdk/examples/a112acc.c <out>/A112ACC.NAC
 */
#include <newtos.h>

typedef struct {
    short *control;
    short *global;
    short *intin;
    short *intout;
    long  *addrin;
    long  *addrout;
} AESPB;

static short aes_control[6];
static short aes_global[16];
static short aes_intin[16];
static short aes_intout[16];
static long  aes_addrin[4];
static long  aes_addrout[4];

static void aes_trap(void *pb)
{
    register long x0 __asm__("x0") = 200;
    register long x1 __asm__("x1") = (long)pb;
    __asm__ __volatile__("svc\t#2" : "+r"(x0) : "r"(x1) : "memory", "cc");
    (void)x0;
}

static short aes_call(short opcode, short nin, short nout, short naddrin)
{
    AESPB pb;
    int i;
    aes_control[0] = opcode;
    aes_control[1] = nin;
    aes_control[2] = nout;
    aes_control[3] = naddrin;
    aes_control[4] = 0;
    for (i = 0; i < 16; i++)
        aes_intout[i] = 0;
    pb.control = aes_control;
    pb.global  = aes_global;
    pb.intin   = aes_intin;
    pb.intout  = aes_intout;
    pb.addrin  = aes_addrin;
    pb.addrout = aes_addrout;
    aes_trap(&pb);
    return aes_intout[0];
}

static char numbuf[16];
static const char *dec(long v)
{
    char t[16]; int n = 0, i = 0;
    unsigned long u = v < 0 ? (unsigned long)-v : (unsigned long)v;
    if (v < 0) numbuf[i++] = '-';
    do { t[n++] = (char)('0' + u % 10); u /= 10; } while (u);
    while (n) numbuf[i++] = t[--n];
    numbuf[i] = 0;
    return numbuf;
}

#define MU_MESAG 0x0010
#define MU_TIMER 0x0020
#define WF_TOP   10
#define WF_WORKXYWH 4

static short msg[8];

/* Open a full-screen window of our own, close it and delete it. */
static void cycle(int n)
{
    short wh, x, y, w, h;
    aes_intin[0] = 0; aes_intin[1] = WF_WORKXYWH;
    aes_call(104, 2, 5, 0); /* wind_get(0, WF_WORKXYWH) */
    x = aes_intout[1]; y = aes_intout[2]; w = aes_intout[3]; h = aes_intout[4];
    aes_intin[0] = 0; aes_intin[1] = x; aes_intin[2] = y; aes_intin[3] = w; aes_intin[4] = h;
    wh = aes_call(100, 5, 1, 0); /* wind_create */
    aes_intin[0] = wh; aes_intin[1] = x; aes_intin[2] = y; aes_intin[3] = w; aes_intin[4] = h;
    aes_call(101, 5, 1, 0); /* wind_open */
    aes_intin[0] = wh; aes_call(102, 1, 1, 0); /* wind_close */
    aes_intin[0] = wh; aes_call(103, 1, 1, 0); /* wind_delete */
    Cconws("A112ACC: cycle "); Cconws(dec(n)); Cconws(": own window "); Cconws(dec(wh));
    Cconws(" opened+closed\r\n");
}

int main(void)
{
    short top, ticks = 0;
    int i;

    aes_call(10, 0, 1, 0); /* appl_init */

    for (i = 0; i < 16; i++) aes_intin[i] = 0;
    aes_intin[0] = MU_MESAG | MU_TIMER; /* timer 0 */
    aes_addrin[0] = (long)msg;
    aes_call(25, 16, 7, 1); /* evnt_multi */
    Cconws("A112ACC: up\r\n");

    for (;;) {
        aes_intin[0] = 200; aes_intin[1] = 0;
        aes_call(24, 2, 1, 0); /* evnt_timer(200) */
        ticks++;
        aes_intin[0] = 0; aes_intin[1] = WF_TOP;
        aes_call(104, 2, 5, 0); /* wind_get(0, WF_TOP) */
        top = aes_intout[1];
        if (top > 0)
            break;
    }
    Cconws("A112ACC: window on top after ticks="); Cconws(dec(ticks));
    Cconws(" handle="); Cconws(dec(top)); Cconws("\r\n");

    cycle(1);
    for (i = 0; i < 10; i++) {
        aes_intin[0] = 200; aes_intin[1] = 0;
        aes_call(24, 2, 1, 0); /* evnt_timer(200) */
    }
    cycle(2);

    for (;;) {
        aes_addrin[0] = (long)msg;
        aes_call(23, 0, 1, 1); /* evnt_mesag */
        Cconws("A112ACC: message "); Cconws(dec(msg[0]));
        Cconws(" handle "); Cconws(dec(msg[3])); Cconws("\r\n");
    }
}
