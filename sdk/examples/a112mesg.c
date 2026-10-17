/*
 * a112mesg.c -- TASK/A112 probe: does evnt_multi(MU_MESAG) see a message that
 * is already queued when it is entered, and one that arrives while it waits?
 *
 * EmuTOS reference: ev_multi's quick check (aes/gemevlib.c:220-228) returns a
 * queued message at once (`if (rlr->p_qindex > 0 ...) { ev_mesag(); what |=
 * MU_MESAG; }`); a message posted while the caller waits completes the parked
 * MU_MESAG EVB from the writer's side (aqueue, aes/gemqueue.c:100-118).
 *
 * Cases, in this order (a case that blocks forever leaves its RESULT line out;
 * the blocking-prone ones come last):
 *   E  empty queue, evnt_multi(MU_MESAG|MU_TIMER, 500)  -> 32 (timer only)
 *   A  appl_write(self,16), evnt_multi(MU_MESAG|MU_TIMER, 1000)
 *                                                       -> 1, the 16 bytes
 *   B  appl_write(self,16), evnt_mesag                  -> 1, the 16 bytes
 *   C  appl_write(self,16), appl_init again, evnt_multi(MU_MESAG|MU_TIMER,
 *      1000)                                            -> 1, the 16 bytes
 *      (the A25 attempt-2 sequence)
 *   D  two appl_write(self,16), two evnt_multi          -> 1 + 1, FIFO order
 *   F  wind_create+wind_open of an own window: the AES queues WM_REDRAW in
 *      the caller's own trap; evnt_multi(MU_MESAG|MU_TIMER, 1000) -> 1,
 *      msg[0] = WM_REDRAW (20), msg[3] = the handle
 *   G  queue drained, own window open and on top.
 *      Default build (no accessory on the card): evnt_multi(MU_MESAG|
 *      MU_TIMER, 5000) -> 32 (timer only).
 *      Build with -DA112_ACC (card carries the companion accessory
 *      A112ACC.NAC, which opens and closes a full-screen window of its own
 *      while we wait, so the AES posts WM_REDRAW for our window from ANOTHER
 *      process): case E is left out (it sleeps), then
 *        G1 evnt_mesag                              -> 1, WM_REDRAW, our handle
 *        G2 evnt_multi(MU_MESAG|MU_TIMER, 5000)     -> 16, WM_REDRAW, our handle
 *           (the accessory repeats its window cycle 10 ticks later)
 *      The -DA112_ACC build is made by a one-line wrapper,
 *      `#define A112_ACC 1` + `#include ".../a112mesg.c"` (method/run-all.sh).
 *
 * Every evnt_multi call clears intin[0..15] first and passes a 16-byte
 * buffer in addrin[0]; the timer is intin[14] (low) / intin[15] (high).
 * Result lines go to the console (Cconws -> serial) and to C:\A112MESG.LOG.
 * A GEM .NAP: launched by the #Z autorun line of C:\NEWDESK.INF.
 * Build: sdk/build.sh sdk/examples/a112mesg.c <out>/A112MESG.NAP
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

static long gemdos3(long fn, long a0, long a1, long a2)
{
    register long x8 __asm__("x8") = fn;
    register long r0 __asm__("x0") = a0;
    register long r1 __asm__("x1") = a1;
    register long r2 __asm__("x2") = a2;
    __asm__ volatile("svc #1" : "+r"(r0) : "r"(x8), "r"(r1), "r"(r2) : "memory");
    return r0;
}

static long logh = -1;

static void out(const char *s)
{
    long n = 0;
    while (s[n]) n++;
    Cconws(s);
    if (logh >= 0)
        gemdos3(0x40, logh, n, (long)s); /* Fwrite */
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

#define MU_MESAG  0x0010
#define MU_TIMER  0x0020
#define WM_REDRAW 20
#define WF_TOP    10
#define WF_WORKXYWH 4

static short apid;
static short sent[8];
static short got[8];
static short fails;

static short write_self(short tag)
{
    int i;
    for (i = 0; i < 8; i++) sent[i] = (short)(0x1120 + tag * 0x100 + i);
    aes_intin[0] = apid;
    aes_intin[1] = 16;
    aes_addrin[0] = (long)sent;
    return aes_call(12, 2, 1, 1); /* appl_write */
}

static short evnt_multi_mt(long ms)
{
    int i;
    for (i = 0; i < 16; i++) aes_intin[i] = 0;
    for (i = 0; i < 8; i++) got[i] = 0;
    aes_intin[0] = MU_MESAG | MU_TIMER;
    aes_intin[14] = (short)(ms & 0xFFFF);
    aes_intin[15] = (short)((ms >> 16) & 0xFFFF);
    aes_addrin[0] = (long)got;
    return aes_call(25, 16, 7, 1);
}

static short evnt_mesag(void)
{
    int i;
    for (i = 0; i < 8; i++) got[i] = 0;
    aes_addrin[0] = (long)got;
    return aes_call(23, 0, 1, 1);
}

static int diff_words(void)
{
    int i, d = 0;
    for (i = 0; i < 8; i++) if (got[i] != sent[i]) d++;
    return d;
}

static void verdict(const char *c, int ok)
{
    out("A112: case "); out(c); out(ok ? " PASS\r\n" : " FAIL\r\n");
    if (!ok) fails++;
}

static void line(const char *c, const char *what, long v)
{
    out("A112: "); out(c); out(" "); out(what); out("="); out(dec(v)); out("\r\n");
}

int main(void)
{
    short wr, ev, ev2, wh, wx, wy, ww, whh;
    int d;

    logh = (long)gemdos2(0x3C, (long)"C:\\A112MESG.LOG", 0); /* Fcreate */
    out("A112: start\r\n");
    apid = aes_call(10, 0, 1, 0); /* appl_init */
    line("init", "ap_id", apid);

#ifndef A112_ACC
    /* E: nothing queued -> timer only. */
    ev = evnt_multi_mt(500);
    line("E", "events", ev);
    verdict("E", ev == MU_TIMER);
#endif

    /* A: self-sent, already queued at entry. */
    wr = write_self(1);
    ev = evnt_multi_mt(1000);
    d = diff_words();
    line("A", "appl_write", wr);
    line("A", "events", ev);
    line("A", "words_differing", d);
    verdict("A", wr == 1 && ev == MU_MESAG && d == 0);

    /* B: self-sent, evnt_mesag. */
    wr = write_self(2);
    out("A112: B entering evnt_mesag\r\n");
    ev = evnt_mesag();
    d = diff_words();
    line("B", "appl_write", wr);
    line("B", "ret", ev);
    line("B", "words_differing", d);
    verdict("B", wr == 1 && ev == 1 && d == 0);

    /* C: the A25 attempt-2 sequence (re-appl_init between write and wait). */
    wr = write_self(3);
    ev2 = aes_call(10, 0, 1, 0); /* appl_init again */
    ev = evnt_multi_mt(1000);
    d = diff_words();
    line("C", "appl_write", wr);
    line("C", "appl_init2", ev2);
    line("C", "events", ev);
    line("C", "words_differing", d);
    verdict("C", wr == 1 && ev2 == apid && ev == MU_MESAG && d == 0);

    /* D: two queued, FIFO order. */
    {
        short s1[8]; int i, d1, d2;
        wr = write_self(4);
        for (i = 0; i < 8; i++) s1[i] = sent[i];
        ev2 = write_self(5);
        wr = (short)(wr + ev2);
        ev = evnt_multi_mt(1000);
        d1 = 0; for (i = 0; i < 8; i++) if (got[i] != s1[i]) d1++;
        ev2 = evnt_multi_mt(1000);
        d2 = diff_words();
        line("D", "appl_write_sum", wr);
        line("D", "events1", ev);
        line("D", "words_differing1", d1);
        line("D", "events2", ev2);
        line("D", "words_differing2", d2);
        verdict("D", wr == 2 && ev == MU_MESAG && ev2 == MU_MESAG && d1 == 0 && d2 == 0);
    }

    /* F: own window -> WM_REDRAW queued by the AES in our own trap. */
    aes_intin[0] = 0; aes_intin[1] = WF_WORKXYWH;
    aes_call(104, 2, 5, 0); /* wind_get(0, WF_WORKXYWH) */
    wx = (short)(aes_intout[1] + 40); wy = (short)(aes_intout[2] + 40);
    ww = 240; whh = 160;
    aes_intin[0] = 0; aes_intin[1] = wx; aes_intin[2] = wy; aes_intin[3] = ww; aes_intin[4] = whh;
    wh = aes_call(100, 5, 1, 0); /* wind_create(kind 0) */
    aes_intin[0] = wh; aes_intin[1] = wx; aes_intin[2] = wy; aes_intin[3] = ww; aes_intin[4] = whh;
    wr = aes_call(101, 5, 1, 0); /* wind_open */
    ev = evnt_multi_mt(1000);
    line("F", "wind_create", wh);
    line("F", "wind_open", wr);
    line("F", "events", ev);
    line("F", "msg0", got[0]);
    line("F", "msg3", got[3]);
    verdict("F", wh > 0 && wr != 0 && ev == MU_MESAG && got[0] == WM_REDRAW && got[3] == wh);
    /* drain anything else the open produced (timer 0 = poll) */
    {
        int n = 0;
        while (n < 8 && (evnt_multi_mt(0) & MU_MESAG)) n++;
        line("F", "drained_extra", n);
    }

    /* G: message from another process arriving while we wait. */
    aes_intin[0] = 0; aes_intin[1] = WF_TOP;
    aes_call(104, 2, 5, 0);
    line("G", "top_before", aes_intout[1]);
#ifdef A112_ACC
    /* G1: evnt_mesag (sleeps on the message wake key). */
    out("A112: G1 entering evnt_mesag\r\n");
    ev = evnt_mesag();
    line("G1", "ret", ev);
    line("G1", "msg0", got[0]);
    line("G1", "msg3", got[3]);
    verdict("G1(acc,evnt_mesag)", ev == 1 && got[0] == WM_REDRAW && got[3] == wh);
    /* G2: evnt_multi; the accessory repeats its window cycle ~2 s later. */
    out("A112: G2 entering evnt_multi(MU_MESAG|MU_TIMER, 5000)\r\n");
    ev = evnt_multi_mt(5000);
    line("G2", "events", ev);
    line("G2", "msg0", got[0]);
    line("G2", "msg3", got[3]);
    verdict("G2(acc,evnt_multi)", ev == MU_MESAG && got[0] == WM_REDRAW && got[3] == wh);
#else
    out("A112: G entering evnt_multi(MU_MESAG|MU_TIMER, 5000)\r\n");
    ev = evnt_multi_mt(5000);
    line("G", "events", ev);
    line("G", "msg0", got[0]);
    line("G", "msg3", got[3]);
    verdict("G(no acc: timer only)", ev == MU_TIMER);
#endif

    aes_intin[0] = wh; aes_call(102, 1, 1, 0); /* wind_close */
    aes_intin[0] = wh; aes_call(103, 1, 1, 0); /* wind_delete */
    aes_call(19, 0, 1, 0); /* appl_exit */
    out(fails ? "A112: RESULT FAIL\r\n" : "A112: RESULT PASS\r\n");
    if (logh >= 0) gemdos1(0x3E, logh); /* Fclose */
    Pterm(fails ? 1 : 0);
}
