/*
 * e10usr.c -- the ARCH03 stage-3c E10 probe: a G_USERDEF routine of one
 * process drawn while another process is current.
 *
 * EmuTOS runs a USERBLK's ub_code wherever the draw happens -- a plain jsr
 * on the current stack (aes/gemoblib.c:411-426, from ob_user :433-448 and
 * just_draw :792-793). The classic case: a desktop registers a background
 * tree with wind_set(0, WF_NEWDESK) that holds a G_USERDEF, then starts a
 * program; every background repaint the program causes runs the desktop's
 * routine inside the program. NewTOS does the same since stage 3c (E10 (a));
 * with memory protection on, the program holds the owner's rights for the
 * duration of the callout only (FreeMiNT prot_temp).
 *
 * This parent (the "desktop"):
 *   1. appl_init; registers a WF_NEWDESK tree: a G_BOX root over the
 *      desktop work area with one G_USERDEF child; the routine
 *      (ud_draw) counts its calls, records which process it ran in (the
 *      os_run cell of the page 0 it sees: _sysbase->os_run, which names
 *      the CURRENT process), checks pb_parm and returns CROSSED, so the
 *      AES draws a cross over the box (visible in a screendump);
 *   2. 's': repaints its own background (form_dial FMD_FINISH) -- the
 *      routine runs in the parent itself (worked before 3c too);
 *   3. 'd': loads C:\E10KID.NAP with Pexec 3 (so it knows the child's PD)
 *      and starts it with Pexec 6 (a child with a memory space of its own,
 *      like Pexec 0); the child repaints the background twice (form_dial,
 *      and a window opened and closed over it) and ends with 0. Expected:
 *      the routine ran in the CHILD (os_run == the child's PD), as often as
 *      the child repainted, with the right pb_parm. Before 3c (A97 rule)
 *      it did not run at all;
 *   4. 'r': the same child reads one of the parent's private (PROT_P)
 *      words after its repaints: protection on (default) -> it may (it
 *      inherited the parent's blocks at its start, E5); memprot=strict ->
 *      it may not -- the temporary right of the callout ended (crash
 *      report, Pexec = 0x0000FFFF); memprot=off -> it may.
 *   5. prints "E10USR: PASS"/"FAIL", keeps its background 3 x 4 s (for a
 *      screendump), removes it, appl_exit.
 *
 * The expected answers depend on the boot argument, chosen at build time
 * with E10_MODE: 0 protection on (E10USR.NAP), 1 memprot=strict
 * (E10USRS.NAP), 2 memprot=off (E10USRO.NAP).
 *
 * Build: sdk/build.sh sdk/examples/e10usr.c <out>/E10USR.NAP
 *        (+ sdk/examples/e10kid.c -> E10KID.NAP)
 */
#include <newtos.h>

#ifndef E10_MODE
#define E10_MODE 0
#endif
#define STRICT (E10_MODE == 1)

#define CRASH 0xFFFFUL
#define MAGIC 0x45313055UL /* "E10U", the USERBLK's ub_parm */
#define PATTERN 0x50415254UL

typedef unsigned long UL;

/* ---- AES (classic parameter block, LP64 pointers) ---- */

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

/* ---- the tree (LP64 OBJECT: 6 WORDs, ob_spec @16, x/y/w/h @24) ---- */

typedef struct {
    short ob_next, ob_head, ob_tail;
    unsigned short ob_type, ob_flags, ob_state;
    long ob_spec;
    short ob_x, ob_y, ob_width, ob_height;
} OBJECT;

typedef struct {
    OBJECT *pb_tree;
    short pb_obj, pb_prevstate, pb_currstate;
    short pb_x, pb_y, pb_w, pb_h;
    short pb_xc, pb_yc, pb_wc, pb_hc;
    long pb_parm;
} PARMBLK;

typedef struct {
    short (*ub_code)(PARMBLK *);
    long ub_parm;
} USERBLK;

#define G_BOX 20
#define G_USERDEF 24
#define LASTOB 0x0020
#define CROSSED 0x0002
#define WF_WORKXYWH 4
#define WF_NEWDESK 14
#define FMD_FINISH 3

static OBJECT tree[2];
static USERBLK ublk;

/* What the routine saw -- written from whichever process it runs in. */
static volatile UL calls;
static volatile UL ran_in;
static volatile UL bad_parm;

/* The parent's private word the 'r' child reads (PROT_P: its BSS). */
static volatile unsigned int secret;

static UL current_pd(void)
{
    /* _sysbase (0x4F2) -> OSHEADER, os_run (+0x28) -> the cell holding the
       running process's PD. Page 0 is the current process's own. */
    UL sysbase = *(volatile unsigned int *)0x4F2UL;
    UL run_cell = *(volatile unsigned int *)(sysbase + 0x28);
    return (UL)*(volatile unsigned int *)run_cell;
}

static short ud_draw(PARMBLK *pb)
{
    calls++;
    ran_in = current_pd();
    if ((UL)pb->pb_parm != MAGIC || pb->pb_tree != tree || pb->pb_obj != 1)
        bad_parm++;
    return (short)(pb->pb_currstate | CROSSED);
}

/* ---- output ---- */

static __attribute__((noinline)) void put_hex(UL v)
{
    volatile char buf[11];
    int i;
    buf[0] = '0';
    buf[1] = 'x';
    for (i = 0; i < 8; i++) {
        unsigned d = (unsigned)(v >> (28 - 4 * i)) & 0xF;
        buf[2 + i] = (char)(d < 10 ? '0' + d : 'A' + d - 10);
    }
    buf[10] = '\0';
    Cconws((const char *)buf);
}

static int fails;

static void verdict(char c, const char *what, UL got, UL want)
{
    char line[] = "E10USR: case ? ";
    line[13] = c;
    Cconws(line);
    Cconws(what);
    Cconws(" got=");
    put_hex(got);
    Cconws(" want=");
    put_hex(want);
    if (got == want) {
        Cconws(" OK\r\n");
    } else {
        Cconws(" WRONG\r\n");
        fails++;
    }
}

static NLONG Pexec(short mode, const char *path, const char *tail, const char *env)
{
    register NLONG x8 __asm__("x8") = 0x4B;
    register NLONG r0 __asm__("x0") = mode;
    register NLONG r1 __asm__("x1") = (NLONG)path;
    register NLONG r2 __asm__("x2") = (NLONG)tail;
    register NLONG r3 __asm__("x3") = (NLONG)env;
    __asm__ volatile("svc #1" : "+r"(r0) : "r"(x8), "r"(r1), "r"(r2), "r"(r3) : "memory");
    return r0;
}

static void make_tail(char *tail, char c, UL addr)
{
    static const char hex[] = "0123456789abcdef";
    int i;
    tail[0] = 10;
    tail[1] = c;
    tail[2] = ' ';
    for (i = 0; i < 8; i++)
        tail[3 + i] = hex[(addr >> ((7 - i) * 4)) & 0xF];
    tail[11] = '\0';
}

static short dx, dy, dw, dh;

static void repaint_background(void)
{
    int i;
    for (i = 0; i < 4; i++)
        aes_intin[1 + i] = 0;
    aes_intin[0] = FMD_FINISH;
    aes_intin[5] = dx;
    aes_intin[6] = dy;
    aes_intin[7] = dw;
    aes_intin[8] = dh;
    aes_call(51, 9, 1, 0); /* form_dial */
}

static void set_newdesk(OBJECT *t)
{
    aes_intin[0] = 0;
    aes_intin[1] = WF_NEWDESK;
    aes_intin[2] = aes_intin[3] = aes_intin[4] = aes_intin[5] = 0;
    aes_addrin[0] = (long)t;
    aes_call(105, 6, 1, 1); /* wind_set, native addr_in[0] form */
    aes_addrin[0] = 0;
}

/* Load the child (Pexec 3: we learn its PD), start it with its own memory
   space (Pexec 6). Returns the exit code; *pd the child's PD. */
static UL run_kid(char c, UL addr, UL *pd)
{
    char tail[12];
    NLONG p;
    make_tail(tail, c, addr);
    p = Pexec(3, "C:\\E10KID.NAP", tail, 0);
    *pd = (UL)p;
    if (p <= 0)
        return (UL)p;
    return (UL)Pexec(6, 0, (const char *)p, 0);
}

int main(void)
{
    UL me = current_pd();
    UL before, kid, rc;

    Cconws(STRICT ? "E10USR: start (expecting memprot=strict)\r\n"
           : E10_MODE == 2 ? "E10USR: start (expecting memprot=off)\r\n"
                           : "E10USR: start (expecting protection on, default)\r\n");
    if (aes_call(10, 0, 1, 0) < 0) { /* appl_init */
        Cconws("E10USR: FAIL (appl_init)\r\n");
        return 1;
    }
    aes_intin[0] = 0;
    aes_intin[1] = WF_WORKXYWH;
    aes_call(104, 2, 5, 0); /* wind_get */
    dx = aes_intout[1];
    dy = aes_intout[2];
    dw = aes_intout[3];
    dh = aes_intout[4];
    secret = PATTERN;

    tree[0].ob_next = -1;
    tree[0].ob_head = 1;
    tree[0].ob_tail = 1;
    tree[0].ob_type = G_BOX;
    tree[0].ob_spec = 0x00001143L; /* the AES's own desktop fill */
    tree[0].ob_x = dx;
    tree[0].ob_y = dy;
    tree[0].ob_width = dw;
    tree[0].ob_height = dh;
    ublk.ub_code = ud_draw;
    ublk.ub_parm = (long)MAGIC;
    tree[1].ob_next = 0;
    tree[1].ob_head = -1;
    tree[1].ob_tail = -1;
    tree[1].ob_type = G_USERDEF;
    tree[1].ob_flags = LASTOB;
    tree[1].ob_spec = (long)&ublk;
    tree[1].ob_x = 40;
    tree[1].ob_y = 40;
    tree[1].ob_width = 160;
    tree[1].ob_height = 96;
    set_newdesk(tree);

    /* s: our own repaint -- the routine runs in us. */
    calls = 0;
    repaint_background();
    verdict('s', "own repaint: routine ran", calls ? 1 : 0, 1);
    verdict('s', "  in this process", ran_in, me);

    /* d: the child's repaints -- the routine runs in the child. */
    before = calls;
    ran_in = 0;
    rc = run_kid('d', 0, &kid);
    Cconws("E10USR: child PD ");
    put_hex(kid);
    Cconws(", routine calls during the child ");
    put_hex(calls - before);
    Cconws("\r\n");
    verdict('d', "child exit", rc, 0);
    verdict('d', "routine ran during the child", calls - before >= 2 ? 1 : 0, 1);
    verdict('d', "  in the child (os_run)", ran_in, kid);
    verdict('d', "  pb_parm/pb_tree/pb_obj right", bad_parm, 0);

    /* r: after its repaints the child reads our private word. */
    rc = run_kid('r', (UL)&secret, &kid);
    verdict('r', "child reads parent PROT_P after the callouts", rc, STRICT ? CRASH : 0);

    /* We are unharmed. */
    verdict('.', "own private word intact", secret, PATTERN);

    /* The verdict first, then 4 s with the background still registered
       (a screendump shows the CROSSED box), then clean up. */
    Cconws(fails ? "E10USR: FAIL\r\n" : "E10USR: PASS\r\n");
    {
        int i;
        for (i = 0; i < 3; i++) {
            aes_intin[0] = 4000;
            aes_intin[1] = 0;
            aes_call(24, 2, 1, 0); /* evnt_timer */
        }
    }
    Cconws("E10USR: hold over\r\n");
    set_newdesk(0);
    repaint_background();
    aes_call(19, 0, 1, 0); /* appl_exit */
    return fails;
}
