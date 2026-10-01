/*
 * a79color.c -- screendump probe for TASK/A79: `wind_set(WF_COLOR)` reaches
 * the pixels.
 *
 * A native NewTOS GEM program (`.NAP`, started through the `#Z` autorun line
 * of C:\NEWDESK.INF). It prints nothing (console output would land on the
 * screen through the fbcon and spoil the dump); the QMP dump driver takes
 * its timing from the scratch kernel's injector marks instead.
 *
 *   1. `appl_init`;
 *   2. `wind_create(0x0FEF, desktop work area)` -- the A59 survey kind --
 *      and `wind_open` at (30, 40, 320, 240), the T7 window;
 *   3. `wind_set(WF_NAME, "A79 T7")` in the native `addr_in[0]` form;
 *   4. `evnt_timer(7000)`: the window stands with its default colour words
 *      (dump A);
 *   5. T7: `wind_set(h, WF_COLOR, W_NAME, 0x1172, 0x1172)` and the second
 *      spec case `wind_set(h, WF_COLOR, W_VSLIDE, 0x1171, 0x1171)` (dumps B/C);
 *   6. `evnt_timer(1000)` forever -- the run ends when QEMU is quit.
 *
 * Spec: docs/api/aes/window-graphics.md, "`WF_COLOR`/`WF_DCOLOR` -> pixel
 * mapping (design, A79)" § (4) worked examples. Build: sdk/build.sh
 * sdk/examples/a79color.c <out>/A79COLOR.NAP.
 */
#include <newtos.h>

/* ---- AES parameter block (the classic layout, LP64 pointers) ---- */

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

/* Issue one AES call; returns int_out[0]. */
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

#define WF_NAME   2
#define WF_WXYWH  4
#define WF_COLOR  18
#define W_NAME    3
#define W_VSLIDE  12
/* NAME|CLOSER|FULLER|MOVER|SIZER|UPARROW|DNARROW|VSLIDE|LFARROW|RTARROW|HSLIDE */
#define KIND_0FEF 0x0FEF

static void evnt_timer_ms(unsigned long ms)
{
    aes_intin[0] = (short)(ms & 0xFFFF);
    aes_intin[1] = (short)(ms >> 16);
    aes_call(24, 2, 1, 0); /* evnt_timer */
}

static short wind_set_color(short h, short gadget, short tcol, short bcol)
{
    aes_intin[0] = h;
    aes_intin[1] = WF_COLOR;
    aes_intin[2] = gadget;
    aes_intin[3] = tcol;
    aes_intin[4] = bcol;
    aes_intin[5] = 0;
    return aes_call(105, 6, 1, 0); /* wind_set */
}

static const char name[] = "A79 T7";

int main(void)
{
    short apid, h;
    short dx, dy, dw, dh;

    apid = aes_call(10, 0, 1, 0); /* appl_init */
    if (apid < 0)
        return 1;

    /* The desktop's work area is the window's maximum extent. */
    aes_intin[0] = 0;
    aes_intin[1] = WF_WXYWH;
    aes_call(104, 2, 5, 0); /* wind_get */
    dx = aes_intout[1];
    dy = aes_intout[2];
    dw = aes_intout[3];
    dh = aes_intout[4];

    aes_intin[0] = KIND_0FEF;
    aes_intin[1] = dx;
    aes_intin[2] = dy;
    aes_intin[3] = dw;
    aes_intin[4] = dh;
    h = aes_call(100, 5, 1, 0); /* wind_create */
    if (h < 1) {
        aes_call(19, 0, 1, 0); /* appl_exit */
        return 1;
    }

    /* Native form: the string pointer in addr_in[0], control[3] = 1. */
    aes_intin[0] = h;
    aes_intin[1] = WF_NAME;
    aes_intin[2] = aes_intin[3] = aes_intin[4] = aes_intin[5] = 0;
    aes_addrin[0] = (long)name;
    aes_call(105, 6, 1, 1); /* wind_set(WF_NAME) */
    aes_addrin[0] = 0;

    aes_intin[0] = h;
    aes_intin[1] = 30;
    aes_intin[2] = 40;
    aes_intin[3] = 320;
    aes_intin[4] = 240;
    aes_call(101, 5, 1, 0); /* wind_open */

    /* Dump A: the default colour words. */
    evnt_timer_ms(7000);

    /* T7 + the W_VSLIDE case (spec § (4)). */
    wind_set_color(h, W_NAME, 0x1172, 0x1172);
    wind_set_color(h, W_VSLIDE, 0x1171, 0x1171);

    /* Dumps B/C: hold the picture until QEMU is quit. */
    for (;;)
        evnt_timer_ms(1000);
}
