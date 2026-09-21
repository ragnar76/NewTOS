/*
 * a96zombie.c -- screendump probe for TASK/A96: a GEM program that leaves a
 * window open when it ends.
 *
 * A native NewTOS GEM program (`.NAP`), double-clicked from the built-in
 * desktop (C:\A96ZOMB.NAP -- NOT `#Z`, not \AUTO\: the ticket is the
 * desktop's own launch path, `run_gem_app` -> `desktop_resume`). It prints
 * nothing (console output would land on the screen through the fbcon and
 * spoil the dump); the QMP dump driver takes its timing from the scratch
 * kernel's injector marks.
 *
 *   1. `appl_init`;
 *   2. `wind_create(NAME|CLOSER|MOVER, desktop work area)`,
 *      `wind_set(WF_NAME, "A96 ZOMBIE")` (native `addr_in[0]` form),
 *      `wind_open` at (120, 80, 300, 200);
 *   3. `evnt_timer(3000)`: the window stands (dump);
 *   4. `Pterm(0)` -- **no** `wind_close`, no `wind_delete`, no `appl_exit`:
 *      the crashed/sloppy program of `sh_ldapp`'s cleanup
 *      (`aes/gemshlib.c:650-657,676-677`).
 *
 * What the desktop shows when it comes back is the measurement: with the
 * A96 sweep no frame of this window is composited any more; without it the
 * window stays in the AES table and its frame is painted over the desktop
 * (the zombie). Spec: docs/api/aes/window-graphics.md § `wind_new (109)`,
 * NewTOS rule A96. Build: sdk/build.sh sdk/examples/a96zombie.c
 * <out>/A96ZOMB.NAP.
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
/* NAME|CLOSER|MOVER */
#define KIND_000B 0x000B

static void evnt_timer_ms(unsigned long ms)
{
    aes_intin[0] = (short)(ms & 0xFFFF);
    aes_intin[1] = (short)(ms >> 16);
    aes_call(24, 2, 1, 0); /* evnt_timer */
}

static const char name[] = "A96 ZOMBIE";

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

    aes_intin[0] = KIND_000B;
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
    aes_intin[1] = 120;
    aes_intin[2] = 80;
    aes_intin[3] = 300;
    aes_intin[4] = 200;
    aes_call(101, 5, 1, 0); /* wind_open */

    /* The window stands for the dump. */
    evnt_timer_ms(3000);

    /* Gone without closing anything: the window is the AES's problem now. */
    Pterm(0);
}
