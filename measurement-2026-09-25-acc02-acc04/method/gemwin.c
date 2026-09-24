/*
 * gemwin.c -- the DESK35 measurement fixture: the smallest GEM application
 * (`.NAP`) that (a) paints only into its own rectangle and (b) terminates by
 * itself.
 *
 * Both properties are what the ticket needs and what no program already on the
 * card has:
 *
 *   * DUMPVDI.NAP starts every case with `v_clrwk`, which clears the WHOLE
 *     screen -- so it would wipe the desktop's leftover windows in the "before"
 *     build too and the before/after pictures would be identical for the wrong
 *     reason;
 *   * QED.NAP paints only its own rectangles (good), but it is interactive and
 *     ends through its own menu, which the scripted IKBD injector would have to
 *     drive -- so there is no self-terminating "and now the desktop is back"
 *     picture in the same run.
 *
 * What it does, in order:
 *
 *   1. `appl_init`;
 *   2. `wind_get(0, WF_WXYWH)` for the desktop work area (the window's maximum
 *      extent), `wind_create(0x0FEF, ...)`, `wind_set(WF_NAME, "DESK35 GEM")`,
 *      `wind_open` at the FIXED rectangle (160, 120, 320, 200) -- fixed so the
 *      same picture is expected in both builds;
 *   3. `graf_handle` + `v_opnvwk`, then one `v_bar` filling the window's work
 *      area and one `v_gtext` in it. Deliberately NO `v_clrwk`: the whole point
 *      is that this program touches nothing outside its own window;
 *   4. `evnt_timer(6000)` -- the screendump of the running program is taken
 *      here;
 *   5. `v_clsvwk`, `wind_close`, `wind_delete`, `appl_exit`, return -- the crt0
 *      `Pterm`s and the desktop resumes.
 *
 * It prints nothing: console output would land on the screen through the fbcon
 * and spoil the dump. The QMP driver takes its timing from the scratch kernel's
 * injector marks.
 *
 * Build: sdk/build.sh <this file> <out>/GEMWIN.NAP  (method/seed.sh installs it
 * as C:\GEMWIN.NAP).
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

/* ---- VDI parameter block (selector 115) ---- */

static short vdi_contrl[12];
static short vdi_intin[128];
static short vdi_ptsin[256];
static short vdi_intout[128];
static short vdi_ptsout[256];

static void vdi_trap(void)
{
    long block[5];
    register long x0 __asm__("x0") = 115;
    register long x1 __asm__("x1");
    block[0] = (long)vdi_contrl;
    block[1] = (long)vdi_intin;
    block[2] = (long)vdi_ptsin;
    block[3] = (long)vdi_intout;
    block[4] = (long)vdi_ptsout;
    x1 = (long)block;
    __asm__ __volatile__("svc\t#2" : "+r"(x0) : "r"(x1) : "memory", "cc");
    (void)x0;
}

static void vdi_clear(void)
{
    int i;
    for (i = 0; i < 12; i++)
        vdi_contrl[i] = 0;
}

#define WF_NAME   2
#define WF_WXYWH  4
/* NAME|CLOSER|FULLER|MOVER|SIZER|UPARROW|DNARROW|VSLIDE|LFARROW|RTARROW|HSLIDE */
#define KIND_0FEF 0x0FEF

static void evnt_timer_ms(unsigned long ms)
{
    aes_intin[0] = (short)(ms & 0xFFFF);
    aes_intin[1] = (short)(ms >> 16);
    aes_call(24, 2, 1, 0); /* evnt_timer */
}

static const char wname[] = "DESK35 GEM";
static const char banner[] = "GEMWIN.NAP owns the screen";

int main(void)
{
    short apid, h, vh;
    short wx, wy, ww, wh_;
    int i;

    apid = aes_call(10, 0, 1, 0); /* appl_init */
    if (apid < 0)
        return 1;

    /* The desktop's work area is the window's maximum extent. */
    aes_intin[0] = 0;
    aes_intin[1] = WF_WXYWH;
    aes_call(104, 2, 5, 0); /* wind_get */

    aes_intin[0] = KIND_0FEF;
    aes_intin[1] = aes_intout[1];
    aes_intin[2] = aes_intout[2];
    aes_intin[3] = aes_intout[3];
    aes_intin[4] = aes_intout[4];
    h = aes_call(100, 5, 1, 0); /* wind_create */
    if (h < 1) {
        aes_call(19, 0, 1, 0); /* appl_exit */
        return 1;
    }

    /* Native form: the string pointer in addr_in[0], control[3] = 1. */
    aes_intin[0] = h;
    aes_intin[1] = WF_NAME;
    aes_intin[2] = aes_intin[3] = aes_intin[4] = aes_intin[5] = 0;
    aes_addrin[0] = (long)wname;
    aes_call(105, 6, 1, 1); /* wind_set(WF_NAME) */
    aes_addrin[0] = 0;

    aes_intin[0] = h;
    aes_intin[1] = 160;
    aes_intin[2] = 120;
    aes_intin[3] = 320;
    aes_intin[4] = 200;
    aes_call(101, 5, 1, 0); /* wind_open */

    /* The work area this program may paint in -- and nothing else. */
    aes_intin[0] = h;
    aes_intin[1] = WF_WXYWH;
    aes_call(104, 2, 5, 0); /* wind_get */
    wx  = aes_intout[1];
    wy  = aes_intout[2];
    ww  = aes_intout[3];
    wh_ = aes_intout[4];

    /* graf_handle -> physical VDI handle; v_opnvwk on it. */
    vh = aes_call(77, 0, 5, 0); /* graf_handle */
    vdi_clear();
    for (i = 0; i < 10; i++)
        vdi_intin[i] = 1;
    vdi_intin[10] = 2;              /* raster coordinates */
    vdi_contrl[0] = 100;            /* v_opnvwk */
    vdi_contrl[3] = 11;
    vdi_contrl[6] = vh;
    vdi_trap();
    vh = vdi_contrl[6];

    /* vsf_interior(SOLID) + vsf_color(RED) */
    vdi_clear();
    vdi_contrl[0] = 23;
    vdi_contrl[3] = 1;
    vdi_contrl[6] = vh;
    vdi_intin[0] = 1;
    vdi_trap();
    vdi_clear();
    vdi_contrl[0] = 25;
    vdi_contrl[3] = 1;
    vdi_contrl[6] = vh;
    vdi_intin[0] = 2;               /* pen 2 = red in the default palette */
    vdi_trap();

    /* v_bar over the window work area (GDP 1) */
    vdi_clear();
    vdi_contrl[0] = 11;
    vdi_contrl[1] = 2;
    vdi_contrl[5] = 1;              /* sub-op v_bar */
    vdi_contrl[6] = vh;
    vdi_ptsin[0] = wx;
    vdi_ptsin[1] = wy;
    vdi_ptsin[2] = (short)(wx + ww - 1);
    vdi_ptsin[3] = (short)(wy + wh_ - 1);
    vdi_trap();

    /* v_gtext in it */
    vdi_clear();
    vdi_contrl[0] = 8;
    vdi_contrl[1] = 1;
    vdi_contrl[6] = vh;
    for (i = 0; banner[i]; i++)
        vdi_intin[i] = (short)(unsigned char)banner[i];
    vdi_contrl[3] = (short)i;
    vdi_ptsin[0] = (short)(wx + 8);
    vdi_ptsin[1] = (short)(wy + 24);
    vdi_trap();

    /* The screendump of the running program is taken here. */
    evnt_timer_ms(6000);

    vdi_clear();
    vdi_contrl[0] = 101;            /* v_clsvwk */
    vdi_contrl[6] = vh;
    vdi_trap();

    aes_intin[0] = h;
    aes_call(102, 1, 1, 0); /* wind_close */
    aes_intin[0] = h;
    aes_call(103, 1, 1, 0); /* wind_delete */
    aes_call(19, 0, 1, 0);  /* appl_exit */
    return 0;
}
