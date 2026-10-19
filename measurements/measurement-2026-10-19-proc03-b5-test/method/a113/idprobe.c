/* A113 fix run (copy of measurements/measurement-2026-10-17-a113-a114/method/idprobe/idprobe.c,
 * extended: each accessory reads WF_OWNER of its own window; MAIN also writes to pid 4
 * (>= totpds with two accessories) and to pid 1).
 * Original header: A113/A114 auditor probe: AES ids of accessories vs. main program,
 * appl_find / appl_write addressing, msg[1] stamps, WF_OWNER, and the
 * AC_CLOSE handshake of appl_exit.  Roles by #define:
 *   ROLE_ACC (with ACCTAG "X"/"Y")  desk accessory
 *   ROLE_MAIN                       #Z autorun program; chains SECOND via shel_write
 *   ROLE_SECOND                     second program, started after MAIN's appl_exit
 */
#include <newtos.h>

typedef struct { short *control, *global, *intin, *intout; long *addrin, *addrout; } AESPB;
static short ctl[6], glb[16], ii[16], io[16];
static long ai[4], ao[4];

static short aes(short op, short nin, short nout, short nai)
{
    AESPB pb; int i;
    ctl[0] = op; ctl[1] = nin; ctl[2] = nout; ctl[3] = nai; ctl[4] = 0;
    for (i = 0; i < 16; i++) io[i] = 0;
    pb.control = ctl; pb.global = glb; pb.intin = ii; pb.intout = io;
    pb.addrin = ai; pb.addrout = ao;
    register long x0 __asm__("x0") = 200;
    register long x1 __asm__("x1") = (long)&pb;
    __asm__ __volatile__("svc\t#2" : "+r"(x0) : "r"(x1) : "memory", "cc");
    return io[0];
}

static void puthex(unsigned v)
{
    char b[8]; int i;
    for (i = 0; i < 4; i++) { unsigned d = (v >> (12 - 4 * i)) & 15; b[i] = d < 10 ? '0' + d : 'A' + d - 10; }
    b[4] = 0; Cconws(b);
}
static void kv(const char *k, unsigned v) { Cconws(" "); Cconws(k); Cconws("="); puthex(v & 0xffff); }
static void nl(void) { Cconws("\r\n"); }

static short find(const char *name) { ai[0] = (long)name; return aes(13, 0, 1, 1); }
static short write_msg(short to, short *msg) { ii[0] = to; ii[1] = 16; ai[0] = (long)msg; return aes(12, 2, 1, 1); }
static void timer(unsigned ms) { ii[0] = (short)(ms & 0xffff); ii[1] = (short)(ms >> 16); aes(24, 2, 1, 0); }
static short mesag(short *msg) { ai[0] = (long)msg; return aes(23, 0, 1, 1); }
static short multi_mt(unsigned ms, short *msg)
{
    int i; for (i = 0; i < 16; i++) ii[i] = 0;
    ii[0] = 0x30; ii[14] = (short)(ms & 0xffff); ii[15] = (short)(ms >> 16);
    ai[0] = (long)msg; return aes(25, 16, 7, 1);
}

static void finds(const char *tag)
{
    Cconws(tag); Cconws(" appl_find");
    kv("ACCX", find("ACCX    "));
    kv("ACCY", find("ACCY    "));
    kv("MAIN", find("MAIN    "));
    kv("SECOND", find("SECOND  "));
    kv("SCRENMGR", find("SCRENMGR"));
    kv("NOSUCH", find("NOSUCH  "));
    nl();
}

static void pmsg(const char *tag, short ret, short *m)
{
    Cconws(tag); kv("ret", ret); kv("m0", m[0]); kv("m1", m[1]); kv("m2", m[2]);
    kv("m3", m[3]); kv("m4", m[4]); kv("m5", m[5]); nl();
}

int main(void)
{
    short msg[8], id; int i, n;
    (void)n; (void)multi_mt; (void)mesag; (void)write_msg;
    id = aes(10, 0, 1, 0);
#if defined(ROLE_ACC)
    Cconws("[ACC" ACCTAG "] appl_init"); kv("ret", id); kv("global2", glb[2]); nl();
    ii[0] = id; ai[0] = (long)"  Probe " ACCTAG;
    Cconws("[ACC" ACCTAG "] menu_register"); kv("ret", aes(35, 1, 1, 1)); nl();
    finds("[ACC" ACCTAG "]");
    /* one window each, so MAIN can read WF_OWNER of a foreign window */
    ii[0] = 0x000B; ii[1] = 20; ii[2] = 300; ii[3] = 100; ii[4] = 80;
    {
        short hd = aes(100, 5, 1, 0);
        ii[0] = hd; ii[1] = 20 + 120 * (ACCTAG[0] - 'X'); ii[2] = 300; ii[3] = 100; ii[4] = 80; aes(101, 5, 1, 0);
        Cconws("[ACC" ACCTAG "] window"); kv("handle", hd); nl();
        ii[0] = hd; ii[1] = 20;
        { short r = aes(104, 2, 5, 0);
          Cconws("[ACC" ACCTAG "] wind_get(WF_OWNER) own window"); kv("h", hd); kv("ret", r); kv("owner", io[1]); kv("open", io[2]); nl(); }
    }
    for (;;) {
        short r;
        for (i = 0; i < 8; i++) msg[i] = 0;
        r = mesag(msg);
        pmsg("[ACC" ACCTAG "] evnt_mesag", r, msg);
        if (msg[0] == 0x4000) {
            short rep[8] = { 0x4001, id, 0, id, msg[1], 0, 0, 0 };
            Cconws("[ACC" ACCTAG "] reply appl_write"); kv("to", msg[1]); kv("ret", write_msg(msg[1], rep)); nl();
        }
    }
#elif defined(ROLE_MAIN)
    Cconws("[MAIN] appl_init"); kv("ret", id); kv("global2", glb[2]); nl();
    timer(1500);
    finds("[MAIN]");
    for (i = 1; i <= 6; i++) {           /* WF_OWNER of handles 1..6 */
        ii[0] = i; ii[1] = 20;
        short r = aes(104, 2, 5, 0);
        Cconws("[MAIN] wind_get(WF_OWNER)"); kv("h", i); kv("ret", r); kv("owner", io[1]); kv("open", io[2]); nl();
    }
    {
        short targets[7]; short fx = find("ACCX    "), fy = find("ACCY    ");
        targets[0] = fx; targets[1] = fy; targets[2] = 2; targets[3] = 3; targets[4] = 0;
        targets[5] = 4; targets[6] = 1;
        for (i = 0; i < 7; i++) {
            short m[8] = { 0x4000, id, 0, targets[i], (short)i, 0, 0, 0 };
            Cconws("[MAIN] appl_write"); kv("to", targets[i]); kv("ret", write_msg(targets[i], m)); nl();
        }
    }
    for (n = 0; n < 8; n++) {
        short r;
        for (i = 0; i < 8; i++) msg[i] = 0;
        r = multi_mt(700, msg);
        pmsg("[MAIN] evnt_multi(MESAG|TIMER)", r, msg);
    }
    ii[0] = 1; ii[1] = 1; ii[2] = 0; ai[0] = (long)"C:\\SECOND.NAP"; ai[1] = (long)"\0";
    Cconws("[MAIN] shel_write(SHW_EXEC)"); kv("ret", aes(121, 3, 1, 2)); nl();
    Cconws("[MAIN] calling appl_exit\r\n");
    { short r = aes(19, 0, 1, 0); Cconws("[MAIN] appl_exit"); kv("ret", r); nl(); return 0x40 + r; }
#elif defined(ROLE_SECOND)
    Cconws("[SECOND] appl_init"); kv("ret", id); kv("global2", glb[2]); nl();
    finds("[SECOND]");
    timer(1500);
    Cconws("[SECOND] calling appl_exit\r\n");
    { short r = aes(19, 0, 1, 0); Cconws("[SECOND] appl_exit"); kv("ret", r); nl(); return 0x50 + r; }
#endif
    return 0;
}
