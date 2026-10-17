/* kbprobe.c -- RST01-test scratch probe (not product code): NAP that polls BIOS Kbshift(-1) (svc #13, fn 11) every 50 ms
 * via evnt_timer and prints each change to the serial console. Armed by tools/inject-arm.py up:KBPROBE. */
#include <newtos.h>
typedef struct { short *control, *global, *intin, *intout; long *addrin, *addrout; } AESPB;
static short ctl[6], glb[16], ii[16], io[16];
static long ai[4], ao[4];
static short aes(short op, short nin, short nout, short nai)
{
    AESPB pb; int i;
    ctl[0] = op; ctl[1] = nin; ctl[2] = nout; ctl[3] = nai; ctl[4] = 0;
    for (i = 0; i < 16; i++) io[i] = 0;
    pb.control = ctl; pb.global = glb; pb.intin = ii; pb.intout = io; pb.addrin = ai; pb.addrout = ao;
    register long x0 __asm__("x0") = 200;
    register long x1 __asm__("x1") = (long)&pb;
    __asm__ __volatile__("svc\t#2" : "+r"(x0) : "r"(x1) : "memory", "cc");
    return io[0];
}
static void say(const char *a, unsigned v)
{
    char b[8]; int i;
    for (i = 0; i < 2; i++) { unsigned d = (v >> (4 - 4 * i)) & 15; b[i] = d < 10 ? '0' + d : 'A' + d - 10; }
    b[2] = 0; Cconws(a); Cconws(b); Cconws("\r\n");
}
int main(void)
{
    long last = -2, k;
    aes(10, 0, 1, 0);
    Cconws("KBPROBE start\r\n");
    for (;;) {
        ii[0] = 50; ii[1] = 0; aes(24, 2, 1, 0);
        k = bios1(11, -1) & 0xFF;
        if (k != last) { say("KBPROBE Kbshift=0x", (unsigned)k); last = k; }
    }
}
