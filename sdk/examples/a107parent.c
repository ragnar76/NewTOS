/*
 * a107parent.c -- screendump probe for TASK/A107: a GEM program that stays
 * alive while a GEM child it started with `Pexec` ends without cleaning up.
 *
 * This is the shape EmuTOS never has. There a GEM program that starts another
 * one goes through the shell: `shel_write(SHW_EXEC, ...)` ends the parent,
 * `sh_ldapp` runs the child and sweeps the window table after it
 * (`if (gl_shgem) wm_new();`, `aes/gemshlib.c:676-677`), and the parent is
 * started again. NewTOS runs an EL0 `Pexec` child **nested** inside its living
 * parent -- TeraDesk starting QED (`sdk/apps/teradesk/startprg.c:259,309`) --
 * so no shell-loop sweep ever happens for the child. A96 added the sweep at
 * the point the built-in desktop regains control; this probe is the case that
 * one cannot reach.
 *
 *   1. `appl_init`;
 *   2. its own window "A107 PARENT" at (360, 300, 260, 160) -- the window that
 *      must **survive** the child's death;
 *   3. `Pexec(0, "C:\A96ZOMB.NAP", "", 0)`: the child opens "A96 ZOMBIE" at
 *      (120, 80, 300, 200), stands 3 s and `Pterm`s **without** `wind_close`;
 *   4. `evnt_timer(2500)`: the parent's next AES call -- the dispatch at which
 *      the per-slot sweep runs (`aes::sweep_dead_clients`);
 *   5. `Pterm(0)`.
 *
 * The measurement is dump C: with the A107 sweep the child's frame is gone
 * (the background is repainted by `wm_close`'s `draw_change`) and the parent's
 * window is untouched; without it the frame stands on the screen for the rest
 * of the parent's life. Launched by the `#Z` autorun line of C:\NEWDESK.INF,
 * so the run needs no mouse at all. Build: sdk/build.sh
 * sdk/examples/a107parent.c <out>/A107PAR.NAP.
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

/* `Pexec` (GEMDOS 0x4B): mode in x0, path in x1, command tail in x2,
   environment in x3 (0 = inherit the parent's). The SDK header has no
   four-argument GEMDOS helper, so the trap is issued here. */
static long Pexec(short mode, const char *path, const char *tail, const char *env)
{
    register long x8 __asm__("x8") = 0x4B;
    register long r0 __asm__("x0") = mode;
    register long r1 __asm__("x1") = (long)path;
    register long r2 __asm__("x2") = (long)tail;
    register long r3 __asm__("x3") = (long)env;
    __asm__ volatile("svc #1" : "+r"(r0) : "r"(x8), "r"(r1), "r"(r2), "r"(r3) : "memory");
    return r0;
}

#define WF_NAME   2
/* NAME|CLOSER|MOVER */
#define KIND_000B 0x000B

static void evnt_timer_ms(unsigned long ms)
{
    aes_intin[0] = (short)(ms & 0xFFFF);
    aes_intin[1] = (short)(ms >> 16);
    aes_call(24, 2, 1, 0); /* evnt_timer */
}

static const char name[] = "A107 PARENT";
static const char child[] = "C:\\A96ZOMB.NAP";
/* An empty command tail: the classic length byte 0, NUL-terminated. */
static const char tail[] = { 0, 0 };

int main(void)
{
    short apid, h;

    apid = aes_call(10, 0, 1, 0); /* appl_init */
    if (apid < 0)
        return 1;

    aes_intin[0] = KIND_000B;
    aes_intin[1] = 0;
    aes_intin[2] = 19;
    aes_intin[3] = 640;
    aes_intin[4] = 461;
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
    aes_intin[1] = 360;
    aes_intin[2] = 300;
    aes_intin[3] = 260;
    aes_intin[4] = 160;
    aes_call(101, 5, 1, 0); /* wind_open */

    /* Let the picture settle before the child appears (dump A). */
    evnt_timer_ms(1000);

    /* The nested GEM child: it opens a window, stands 3 s and dies without
       closing it. `Pexec(PE_LOADGO)` returns here when it has terminated. */
    Pexec(0, child, tail, 0);

    /* The parent's next AES call -- where the per-slot sweep runs (dump C). */
    evnt_timer_ms(2500);

    /* The parent closes its own window properly and leaves. */
    aes_intin[0] = h;
    aes_call(102, 1, 1, 0); /* wind_close */
    aes_intin[0] = h;
    aes_call(103, 1, 1, 0); /* wind_delete */
    aes_call(19, 0, 1, 0);  /* appl_exit */
    Pterm(0);
}
