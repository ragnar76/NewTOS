/*
 * a116w.c -- TASK/A116 probe program (A116W.NAP): appl_write on a FULL queue
 * of another process (the companion accessory A116ACC.NAC) and on its OWN full
 * queue. EmuTOS (aes/gemqueue.c:96-97,108-124, QUEUE_SIZE 128 aes/struct.h:56):
 * a 16-byte write blocks while 8 messages are queued and returns 1 once a read
 * of THAT queue has made room; it never returns 0.
 *
 *   appl_init; acc = appl_find("A116ACC ")
 *   phase 1: appl_write(acc, START1); evnt_timer(500)   -- the accessory now
 *            stops reading for 3 s
 *            appl_write(acc, msg 1..8)    -> expected 1 each, at once
 *            appl_write(acc, msg 9)       -> expected: blocks until the
 *            accessory reads (~3 s later), then 1
 *   phase 2: appl_write(acc, START2); evnt_timer(500)   -- the accessory waits
 *            2 s, then reads OUR queue once with appl_read(0)
 *            appl_write(0 = self, msg 1..8) -> 1 each
 *            appl_write(0, msg 9)         -> expected: blocks until the
 *            accessory's appl_read(0), then 1
 *            evnt_mesag x8                -> msgs 2..9 in order
 *   appl_exit
 *
 * The order of the serial lines (both processes write to one console) is
 * the evidence: "msg 9 returned" must come after the accessory's first read.
 * Lines go to the console (Cconws -> serial), prefix "A116W:".
 * A GEM .NAP, started by the #Z line of C:\NEWDESK.INF.
 * Build: sdk/build.sh sdk/examples/a116w.c <out>/A116W.NAP
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

static void clr_in(void)
{
    int i;
    for (i = 0; i < 16; i++) aes_intin[i] = 0;
}

static void evnt_timer(short ms)
{
    clr_in();
    aes_intin[0] = ms; aes_intin[1] = 0;
    aes_call(24, 2, 1, 0);
}

__attribute__((unused)) static short appl_write(short id, short len, short *buf)
{
    clr_in();
    aes_intin[0] = id; aes_intin[1] = len;
    aes_addrin[0] = (long)buf;
    return aes_call(12, 2, 1, 1);
}

__attribute__((unused)) static short appl_read(short id, short len, short *buf)
{
    clr_in();
    aes_intin[0] = id; aes_intin[1] = len;
    aes_addrin[0] = (long)buf;
    return aes_call(11, 2, 1, 1);
}

static short evnt_mesag(short *buf)
{
    clr_in();
    aes_addrin[0] = (long)buf;
    return aes_call(23, 0, 1, 1);
}

/* Sequence number of the line: the serial log interleaves both processes,
 * so the order of the lines is the evidence of who waited for whom. */
static long seq;

#define A116_START1 0x7101
#define A116_START2 0x7102
#define A116_MSG    0x7110

static short m[8];

static void say(const char *what, long a, long b)
{
    Cconws("A116W: #"); Cconws(dec(++seq)); Cconws(" ");
    Cconws(what); Cconws(" "); Cconws(dec(a)); Cconws(" -> "); Cconws(dec(b)); Cconws("\r\n");
}

static void mk(short type, short n)
{
    int i;
    for (i = 0; i < 8; i++) m[i] = 0;
    m[0] = type; m[1] = aes_global[2]; m[3] = n;
}

int main(void)
{
    static char name[9] = "A116ACC ";
    short me, acc, r, n;

    me = aes_call(10, 0, 1, 0); /* appl_init */
    clr_in(); aes_addrin[0] = (long)name;
    acc = aes_call(13, 0, 1, 1); /* appl_find */
    say("appl_init id", 0, me);
    say("appl_find A116ACC", 0, acc);
    if (acc < 0) { aes_call(19, 0, 1, 0); return 1; }

    /* phase 1: the accessory's full queue */
    mk(A116_START1, 0); r = appl_write(acc, 16, m); say("P1 write START1", 0, r);
    evnt_timer(500);
    for (n = 1; n <= 8; n++) { mk(A116_MSG, n); r = appl_write(acc, 16, m); say("P1 write acc msg", n, r); }
    mk(A116_MSG, 9);
    say("P1 write acc msg 9 begin", 9, 0);
    r = appl_write(acc, 16, m);
    say("P1 write acc msg 9 returned", 9, r);

    /* phase 2: our own full queue */
    mk(A116_START2, 0); r = appl_write(acc, 16, m); say("P2 write START2", 0, r);
    evnt_timer(500);
    for (n = 1; n <= 8; n++) { mk(A116_MSG, 0x20 + n); r = appl_write(0, 16, m); say("P2 write self msg", n, r); }
    mk(A116_MSG, 0x29);
    say("P2 write self msg 9 begin", 9, 0);
    r = appl_write(0, 16, m);
    say("P2 write self msg 9 returned", 9, r);
    for (n = 1; n <= 8; n++) {
        r = evnt_mesag(m);
        say("P2 evnt_mesag type", m[0], m[3]);
    }
    say("done", 0, 0);
    aes_call(19, 0, 1, 0); /* appl_exit */
    return 0;
}
