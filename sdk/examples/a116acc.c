/*
 * a116acc.c -- TASK/A116 companion desk accessory (A116ACC.NAC) of a116w.c.
 * Reads its own queue normally (evnt_mesag) until START1 arrives; then stops
 * reading for 3 s (15 x evnt_timer(200)) so the program fills the queue and
 * its 9th write blocks; then reads 9 messages with evnt_mesag. After START2
 * it waits 2 s and reads the PROGRAM's queue once with appl_read(0, 16) --
 * the read that releases the program's parked self-write -- then reads its
 * own messages forever (AC_OPEN/AC_CLOSE included).
 *
 * Lines go to the console (Cconws -> serial), prefix "A116ACC:".
 * Build: sdk/build.sh sdk/examples/a116acc.c <out>/A116ACC.NAC
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

static short m[8];

static void say(const char *what, long a, long b)
{
    Cconws("A116ACC: #"); Cconws(dec(++seq)); Cconws(" ");
    Cconws(what); Cconws(" "); Cconws(dec(a)); Cconws(" -> "); Cconws(dec(b)); Cconws("\r\n");
}

int main(void)
{
    short r;
    int i;

    aes_call(10, 0, 1, 0); /* appl_init */
    say("up id", 0, aes_global[2]);

    for (;;) {
        r = evnt_mesag(m);
        say("read type", m[0], m[3]);
        if (m[0] == A116_START1) {
            say("deaf 3s", 0, 0);
            for (i = 0; i < 15; i++) evnt_timer(200);
            say("reading 9", 0, 0);
            for (i = 0; i < 9; i++) {
                r = evnt_mesag(m);
                say("P1 read type", m[0], m[3]);
            }
        } else if (m[0] == A116_START2) {
            say("wait 2s", 0, 0);
            for (i = 0; i < 10; i++) evnt_timer(200);
            r = appl_read(0, 16, m);
            say("P2 appl_read(0) ret", r, m[3]);
        }
        (void)r;
    }
}
