/*
 * e10kid.c -- the child of the ARCH03 stage-3c E10 probe (driver:
 * e10usr.c). Its parent has registered a WF_NEWDESK background tree with a
 * G_USERDEF in it; this program makes the AES repaint that background twice
 * while it is the current process -- form_dial(FMD_FINISH) over the
 * desktop work area, and a window opened and closed over it -- so the
 * parent's routine runs here (EmuTOS: wherever the draw happens).
 *
 * Command tail "<case> <8 hex digits>":
 *   d  repaint, end with 0
 *   r  repaint, then read the parent's private word at the address: 0 if it
 *      held the parent's pattern, 2 if not. A refused read does not come
 *      back (crash report, Pterm(-1): the parent sees 0x0000FFFF).
 *
 * Build: sdk/build.sh sdk/examples/e10kid.c <out>/E10KID.NAP
 */
#include <newtos.h>

#define P_CMDLIN 0x80
#define PATTERN 0x50415254UL

typedef unsigned long UL;

typedef struct {
    short *control;
    short *global;
    short *intin;
    short *intout;
    long *addrin;
    long *addrout;
} AESPB;

static short aes_control[6];
static short aes_global[16];
static short aes_intin[16];
static short aes_intout[16];
static long aes_addrin[4];
static long aes_addrout[4];

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
    pb.global = aes_global;
    pb.intin = aes_intin;
    pb.intout = aes_intout;
    pb.addrin = aes_addrin;
    pb.addrout = aes_addrout;
    {
        register long x0 __asm__("x0") = 200;
        register long x1 __asm__("x1") = (long)&pb;
        __asm__ __volatile__("svc\t#2" : "+r"(x0) : "r"(x1) : "memory", "cc");
    }
    return aes_intout[0];
}

static const unsigned char *own_basepage(void)
{
    UL sysbase = *(volatile unsigned int *)0x4F2UL;
    UL run_cell = *(volatile unsigned int *)(sysbase + 0x28);
    return (const unsigned char *)(UL)*(volatile unsigned int *)run_cell;
}

static UL hex8(const unsigned char *s)
{
    UL v = 0;
    int i;
    for (i = 0; i < 8; i++) {
        unsigned char c = s[i];
        v <<= 4;
        if (c >= '0' && c <= '9') v |= (UL)(c - '0');
        else if (c >= 'a' && c <= 'f') v |= (UL)(c - 'a' + 10);
        else if (c >= 'A' && c <= 'F') v |= (UL)(c - 'A' + 10);
    }
    return v;
}

int main(void)
{
    const unsigned char *tail = own_basepage() + P_CMDLIN;
    char c = (char)tail[1];
    volatile unsigned int *a = (volatile unsigned int *)hex8(tail + 3);
    short dx, dy, dw, dh, h;
    UL v;

    if (aes_call(10, 0, 1, 0) < 0) /* appl_init */
        return 3;
    aes_intin[0] = 0;
    aes_intin[1] = 4; /* WF_WORKXYWH */
    aes_call(104, 2, 5, 0); /* wind_get */
    dx = aes_intout[1];
    dy = aes_intout[2];
    dw = aes_intout[3];
    dh = aes_intout[4];

    /* Repaint 1: form_dial(FMD_FINISH) over the work area (w_drawdesk). */
    aes_intin[0] = 3;
    aes_intin[1] = aes_intin[2] = aes_intin[3] = aes_intin[4] = 0;
    aes_intin[5] = dx;
    aes_intin[6] = dy;
    aes_intin[7] = dw;
    aes_intin[8] = dh;
    aes_call(51, 9, 1, 0);

    /* Repaint 2: a window over the background, opened and closed. */
    aes_intin[0] = 0x000B; /* NAME|CLOSER|MOVER */
    aes_intin[1] = dx;
    aes_intin[2] = dy;
    aes_intin[3] = dw;
    aes_intin[4] = dh;
    h = aes_call(100, 5, 1, 0); /* wind_create */
    if (h > 0) {
        aes_intin[0] = h;
        aes_intin[1] = dx + 20;
        aes_intin[2] = dy + 20;
        aes_intin[3] = 240;
        aes_intin[4] = 160;
        aes_call(101, 5, 1, 0); /* wind_open */
        aes_intin[0] = h;
        aes_call(102, 1, 1, 0); /* wind_close */
        aes_intin[0] = h;
        aes_call(103, 1, 1, 0); /* wind_delete */
    }
    Cconws("E10KID: repainted\r\n");

    if (c == 'r') {
        v = *a;
        Cconws("E10KID: read done\r\n");
        aes_call(19, 0, 1, 0); /* appl_exit */
        return v == PATTERN ? 0 : 2;
    }
    aes_call(19, 0, 1, 0); /* appl_exit */
    return 0;
}
