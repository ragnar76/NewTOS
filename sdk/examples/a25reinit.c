/*
 * a25reinit.c -- real-program probe for TASK/A25: a second `appl_init` keeps
 * the message queue (EmuTOS aes/gemaplib.c:54-63 only does `p_flags |= AP_OPEN`
 * and returns the same PID).
 *
 *   1. ap_id1 = appl_init;
 *   2. appl_write(ap_id1, 16, msg)           -- 16-byte message to its own PID;
 *   3. ap_id2 = appl_init                    -- the re-call under test;
 *   4. appl_read(ap_id, 16, got)             -- blocking read, only reached after
 *      the write returned 1 (a system that lost the message would block here;
 *      the missing RESULT line is then the failure); the 16 bytes read are
 *      compared with msg;
 *   5. appl_exit.
 *
 * (An evnt_multi(MU_MESAG|MU_TIMER) observation was tried first and did not
 * see a self-posted message even WITHOUT the re-call -- see RUNINFO.TXT.)
 *
 * Result lines go to the console (Cconws -> serial) and to C:\A25REINI.LOG.
 * A GEM .NAP: launched by the #Z autorun line of C:\NEWDESK.INF.
 * Build: sdk/build.sh sdk/examples/a25reinit.c <out>/A25REINI.NAP
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

static short msg[8];
static short got[8];

int main(void)
{
    short id1, id2, wr, rd = 0, i, bad = 0, fails = 0;

    logh = (long)gemdos2(0x3C, (long)"C:\\A25REINI.LOG", 0); /* Fcreate */
    out("A25: start\r\n");

    id1 = aes_call(10, 0, 1, 0); /* appl_init */
    out("A25: appl_init #1 ap_id="); out(dec(id1)); out("\r\n");

    for (i = 0; i < 8; i++) msg[i] = (short)(0x2500 + i * 0x111);
    aes_intin[0] = id1;
    aes_intin[1] = 16;
    aes_addrin[0] = (long)msg;
    wr = aes_call(12, 2, 1, 1); /* appl_write */

    id2 = aes_call(10, 0, 1, 0); /* appl_init, the re-call */

    /* No console/file I/O between appl_write and the read: nothing but the
       re-call itself may stand between the message and its observation. */
    for (i = 0; i < 8; i++) got[i] = 0;
    if (wr == 1) {
        aes_intin[0] = id1;
        aes_intin[1] = 16;
        aes_addrin[0] = (long)got;
        rd = aes_call(11, 2, 1, 1); /* appl_read */
        for (i = 0; i < 8; i++) if (got[i] != msg[i]) bad++;
    }

    out("A25: appl_write(self,16) rc="); out(dec(wr)); out("\r\n");
    if (wr != 1) fails++;
    out("A25: appl_init #2 ap_id="); out(dec(id2)); out("\r\n");
    if (id2 != id1) { out("A25: FAIL ap_id differs\r\n"); fails++; }
    out("A25: appl_read rc="); out(dec(rd)); out("\r\n");
    if (rd != 1) { out("A25: FAIL message lost by re-appl_init\r\n"); fails++; }
    out("A25: message words differing from the 16 bytes sent: "); out(dec(bad)); out("\r\n");
    if (bad) fails++;

    aes_call(19, 0, 1, 0); /* appl_exit */
    out(fails ? "A25: RESULT FAIL\r\n" : "A25: RESULT PASS\r\n");
    if (logh >= 0) gemdos1(0x3E, logh); /* Fclose */
    Pterm(fails ? 1 : 0);
}
