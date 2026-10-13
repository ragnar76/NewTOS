/*
 * asidcommon.h -- helpers shared by the ARCH03 stage-4 ASID probes
 * (asidstr.c, asidkid.c, asidacc.c). Plain sdk/build.sh programs
 * (sdk/crt0.S, <newtos.h>), no libc.
 *
 * The probes look for a STALE TRANSLATION of the one address that maps to a
 * different physical page in every process: page 0 (VA [0, 0x1000), the
 * process's own system-variable page, plan E3). Each process writes its own
 * marker into a free cell of its page 0 (CELL, 0xFF8, above the KEYS02 stub
 * area that ends at 0xA40) and checks it after every address-space switch.
 * With neither a TLB invalidation nor an ASID per root, the previous root's
 * page-0 translation survives the switch and the cell shows the other
 * process's marker (ARCH03 0b, S0b section 4: 8/16 wrong).
 */
#include <newtos.h>

typedef unsigned long UL;

#define CELL ((volatile unsigned int *)0xFF8UL)
#define HZ200 (*(volatile unsigned int *)0x4BAUL) /* _hz_200, mirrored per tick */
#define CRASH 0xFFFFUL
#define PATTERN 0x50415254UL /* "PART" */
#define MARK 0x4B494431UL    /* "KID1" */

static __attribute__((unused)) NLONG Pexec(short mode, const char *path, const char *tail, const char *env)
{
    register NLONG x8 __asm__("x8") = 0x4B;
    register NLONG r0 __asm__("x0") = mode;
    register NLONG r1 __asm__("x1") = (NLONG)path;
    register NLONG r2 __asm__("x2") = (NLONG)tail;
    register NLONG r3 __asm__("x3") = (NLONG)env;
    __asm__ volatile("svc #1" : "+r"(r0) : "r"(x8), "r"(r1), "r"(r2), "r"(r3) : "memory");
    return r0;
}

static __attribute__((unused)) __attribute__((noinline)) void put_hex(UL v)
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

static __attribute__((unused)) __attribute__((noinline)) void put_dec(UL v)
{
    volatile char buf[24];
    char tmp[24];
    int i = 0, j = 0;
    do {
        tmp[i++] = (char)('0' + (int)(v % 10));
        v /= 10;
    } while (v);
    while (i)
        buf[j++] = tmp[--i];
    buf[j] = '\0';
    Cconws((const char *)buf);
}

/* The current PD the classic way: *(_sysbase->os_run) (OSHEADER +0x28). */
static __attribute__((unused)) UL os_run_pd(void)
{
    UL sysbase = *(volatile unsigned int *)0x4F2UL;
    UL cell = *(volatile unsigned int *)(sysbase + 0x28);
    return *(volatile unsigned int *)cell;
}

/* Command tail "<c> <8 hex digits>" (length byte 10). */
static __attribute__((unused)) void make_tail(char *tail, char c, UL v)
{
    static const char hex[] = "0123456789abcdef";
    int i;
    tail[0] = 10;
    tail[1] = c;
    tail[2] = ' ';
    for (i = 0; i < 8; i++)
        tail[3 + i] = hex[(v >> ((7 - i) * 4)) & 0xF];
    tail[11] = '\0';
}

static __attribute__((unused)) UL hex8(const unsigned char *s)
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

/* ---- AES (classic parameter block, LP64 pointers; as sysacc.c) ---- */

typedef struct {
    short *control;
    short *global;
    short *intin;
    short *intout;
    long *addrin;
    long *addrout;
} AESPB;

static __attribute__((unused)) short aes_control[6];
static __attribute__((unused)) short aes_global[16];
static __attribute__((unused)) short aes_intin[16];
static __attribute__((unused)) short aes_intout[16];
static __attribute__((unused)) long aes_addrin[4];
static __attribute__((unused)) long aes_addrout[4];

static __attribute__((unused)) short aes_call(short opcode, short nin, short nout, short naddrin)
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
        (void)x0;
    }
    return aes_intout[0];
}

static __attribute__((unused)) short evnt_timer(unsigned ms)
{
    aes_intin[0] = (short)(ms & 0xFFFF);
    aes_intin[1] = (short)(ms >> 16);
    return aes_call(24, 2, 1, 0);
}
