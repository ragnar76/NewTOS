/*
 * sysacc.c -- ACC06 part 2: the desk accessory that shows the machine's ARM
 * clock and die temperature in a `form_alert` box.
 *
 * ===========================================================================
 * THIS PROGRAM CANNOT RUN YET. IT IS SOURCE, NOT A DELIVERABLE.
 * ===========================================================================
 *
 * It is written now so that the accessory framework has something to be
 * measured against when it lands -- the same order in which CURDIR.NCP was
 * written before the launcher could set a working directory (DESK18).
 *
 * What is missing, precisely:
 *
 *   * `menu_register` (AES 35) exists and is real: it records the title
 *     pointer and the owning application id and hands back a stable slot
 *     number (tos-api/src/aes/menu.rs:1105-1130). That is all it does.
 *   * Nothing puts that slot into the Desk drop-down, and nothing posts
 *     `AC_OPEN` when it is picked. In EmuTOS that is `gemctrl.c:337-346`
 *     (the click on a Desk item at or past `gl_dafirst` -> `mn_getownid` ->
 *     `ct_msgup(AC_OPEN, owner, THEDESK, menu-id, 0,0,0)`), and the
 *     counterpart `mn_cleanup` (`gemmnlib.c:740-748`) which sends `AC_CLOSE`
 *     to every registered accessory.
 *   * There is also no accessory *loader*: classic TOS starts accessories
 *     from the boot drive's root before the desktop, as processes that never
 *     terminate.
 *
 * Those three are tickets ACC02/ACC03 and deliberately NOT built here. Run
 * against today's kernel this program registers successfully and then blocks
 * in `evnt_mesag` forever, because the message it waits for is never sent.
 * That is the honest state, not a defect of this file.
 *
 * FILE EXTENSION: not decided. The classic extension is `.ACC`; our own
 * native formats are `.NAP` (GEM) and `.NCP` (console), deliberately
 * different from the m68k ones so the later emulator can own `.PRG`/`.TOS`/
 * `.TTP`/`.ACC`. Which one a NewTOS accessory gets is ACC01's decision and
 * is NOT pre-empted here: build this file as `SYSACC.TBD` -- an extension
 * that is obviously a placeholder and that nothing in the system claims:
 *
 *     sdk/build.sh sdk/examples/sysacc.c /somewhere/SYSACC.TBD
 *
 * tools/make-sd-image.sh deliberately does NOT install it: a program that
 * cannot run has no business on the card.
 *
 * ---------------------------------------------------------------------------
 * What it does once ACC02/ACC03 exist
 * ---------------------------------------------------------------------------
 *   appl_init                       -> our application id
 *   menu_register(id, " NewTOS System")
 *   forever:
 *       evnt_mesag
 *       AC_OPEN  (40) with our menu id -> read the two values, show the box
 *       AC_CLOSE (41)                  -> nothing to tear down; keep running
 *
 * An accessory never terminates, so there is no `appl_exit` and no `return`
 * out of the loop.
 *
 * The box is a single `form_alert` (lead decision, 2026-09-24): one AES call,
 * no object tree of our own, the best chance of working the day the framework
 * lands. A window version is a later ticket.
 *
 *     [i] NewTOS System
 *         ARM clock : 1500.0 MHz
 *         CPU temp  : 47.2 C
 *         [  OK  ]
 *
 * Calls used, all against docs/api/ and the EmuTOS AES:
 *   appl_init       AES  10
 *   menu_register   AES  35   tos-api/src/aes/menu.rs:1105
 *   evnt_mesag      AES  23   tos-api/src/aes/event.rs:783
 *   form_alert      AES  52   tos-api/src/aes/form.rs:136
 *   Ssystem         GEMDOS 0x154, modes 2000/2001
 *                             docs/api/gemdos/154-ssystem.md
 */
#include <newtos.h>

/* ------------------------------------------------------------------------- */
/* GEMDOS: Ssystem takes three arguments; newtos.h ships gemdos0/1/2 only.    */
/* ------------------------------------------------------------------------- */

static inline NLONG gemdos3(NLONG fn, NLONG a0, NLONG a1, NLONG a2)
{
    register NLONG x8 __asm__("x8") = fn;
    register NLONG r0 __asm__("x0") = a0;
    register NLONG r1 __asm__("x1") = a1;
    register NLONG r2 __asm__("x2") = a2;
    __asm__ volatile("svc #1"
                     : "+r"(r0)
                     : "r"(x8), "r"(r1), "r"(r2)
                     : "memory");
    return r0;
}

#define SSYSTEM            0x154
#define S_NEWTOS_CPUCLOCK  2000
#define S_NEWTOS_CPUTEMP   2001
#define E_OK               0L

/* The kernel delivers the pointer form with one 32-bit store, so the target
 * is an `int` (32 bit under our LP64 app ABI), never a `long`. */
static NLONG sys_value(NLONG mode, int *out)
{
    return gemdos3(SSYSTEM, mode, (NLONG)out, 0);
}

/* ------------------------------------------------------------------------- */
/* AES parameter block (the classic layout, LP64 pointers) -- the same shape  */
/* every native NewTOS GEM program uses.                                      */
/* ------------------------------------------------------------------------- */

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

/* AES message types (references/emutos/include/aesdefs.h:242-243). */
#define AC_OPEN   40
#define AC_CLOSE  41

/* ------------------------------------------------------------------------- */
/* Text building -- no libc in a freestanding native program.                 */
/* ------------------------------------------------------------------------- */

static int sapp(char *dst, int j, const char *s)
{
    while (*s)
        dst[j++] = *s++;
    dst[j] = '\0';
    return j;
}

static int napp(char *dst, int j, long v)
{
    char tmp[24];
    int i = 0;
    unsigned long u;

    if (v < 0) {
        dst[j++] = '-';
        u = (unsigned long)(-v);
    } else {
        u = (unsigned long)v;
    }
    do {
        tmp[i++] = (char)('0' + (int)(u % 10));
        u /= 10;
    } while (u);
    while (i)
        dst[j++] = tmp[--i];
    dst[j] = '\0';
    return j;
}

/* `v / div` with one decimal, rounding down -- no floating point here. */
static int fix1app(char *dst, int j, long v, long div)
{
    long rest;

    if (v < 0) {
        dst[j++] = '-';
        v = -v;
    }
    rest = v % div;
    j = napp(dst, j, v / div);
    dst[j++] = '.';
    dst[j++] = (char)('0' + (int)((rest / (div / 10)) % 10));
    dst[j] = '\0';
    return j;
}

/*
 * Build the alert string. Each of the two lines either carries its value or
 * says plainly that this board cannot measure it -- a machine that has no
 * sensor must not be given a number.
 *
 * EmuTOS caps an alert at 5 message lines of 40 characters
 * (`MAX_LINENUM`/`MAX_LINELEN`, gem_rsc.h; tos-api/src/aes/form.rs:128-130);
 * three short lines stay well inside that.
 */
static void build_alert(char *dst)
{
    int hz = 0, mdeg = 0;
    int j;

    j = sapp(dst, 0, "[1][NewTOS System|");

    if (sys_value(S_NEWTOS_CPUCLOCK, &hz) == E_OK) {
        j = sapp(dst, j, "ARM clock : ");
        j = fix1app(dst, j, (long)hz, 1000000L);
        j = sapp(dst, j, " MHz");
    } else {
        j = sapp(dst, j, "ARM clock : not available");
    }
    j = sapp(dst, j, "|");

    if (sys_value(S_NEWTOS_CPUTEMP, &mdeg) == E_OK) {
        j = sapp(dst, j, "CPU temp  : ");
        j = fix1app(dst, j, (long)mdeg, 1000L);
        j = sapp(dst, j, " C");
    } else {
        j = sapp(dst, j, "CPU temp  : not available");
    }

    sapp(dst, j, "][  OK  ]");
}

/* The accessory's entry in the Desk menu. A leading space is the classic
 * look: the drop-down puts no padding of its own in front of the title. */
static const char acc_title[] = " NewTOS System";

int main(void)
{
    char alert[200];
    short msg[8];
    short apid, menu_id;
    int i;

    apid = aes_call(10, 0, 1, 0); /* appl_init */
    if (apid < 0)
        return 1;

    aes_intin[0] = apid;
    aes_addrin[0] = (long)acc_title;
    menu_id = aes_call(35, 1, 1, 1); /* menu_register */
    aes_addrin[0] = 0;
    if (menu_id < 0) {
        /* No free accessory slot. Classic accessories simply give up here;
         * there is nothing to clean up and nothing to show. */
        return 1;
    }

    /*
     * An accessory never terminates: it waits for messages for as long as the
     * machine runs. TODAY THIS LOOP BLOCKS ON ITS FIRST ITERATION, because
     * nothing posts AC_OPEN yet (see the header).
     */
    for (;;) {
        for (i = 0; i < 8; i++)
            msg[i] = 0;
        aes_addrin[0] = (long)msg;
        aes_call(23, 0, 1, 1); /* evnt_mesag */
        aes_addrin[0] = 0;

        /*
         * AC_OPEN carries the accessory's menu id in msg[4]
         * (EmuTOS gemctrl.c:337-346 -> ap_sendmsg, gemwmlib.c:851-862);
         * AC_CLOSE carries it in msg[3] (gemmnlib.c:747). The asymmetry is
         * EmuTOS's, not a typo here.
         */
        if (msg[0] == AC_OPEN && msg[4] == menu_id) {
            build_alert(alert);
            aes_intin[0] = 1;            /* default button = OK */
            aes_addrin[0] = (long)alert;
            aes_call(52, 1, 1, 1);       /* form_alert */
            aes_addrin[0] = 0;
        } else if (msg[0] == AC_CLOSE && msg[3] == menu_id) {
            /* The application that was running is going away. This accessory
             * owns no window and no allocation, so there is nothing to close
             * -- but it must keep running and keep listening. */
        }
    }
}
