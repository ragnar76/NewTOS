/*
 * kbdvec.c -- KEYS02 proof: a program hooks the IKBD `kbdvec`, sees every
 * make AND break as a scancode, and chains to the old vector so the normal
 * keyboard path (BIOS queue, AES MU_KEYBD, key repeat) keeps working.
 *
 *     sdk/build.sh sdk/examples/kbdvec.c /somewhere/KBDVEC.NAP
 *
 * Spec: docs/api/xbios/22-kbdvbase.md (A6 = what a program does, B1-B4 =
 * the NewTOS binding). Not installed by tools/make-sd-image.sh; the KEYS02
 * measurement seeds it onto a scratch card and starts it via #Z.
 *
 * ---------------------------------------------------------------------------
 * What it does
 * ---------------------------------------------------------------------------
 *   appl_init
 *   p = Kbdvbase()            XBIOS 0x22, the process's own EL0 table
 *   old = p[-1]               the kbdvec slot, right before Kbdvbase()
 *   check: old and the nine struct kbdvecs slots are non-zero (B3/B8)
 *   p[-1] = hook              hook(x0 = scancode, x1 = 0): record, chain
 *   phase A (AES, O1): evnt_multi(MU_KEYBD) with NO timer until the RIGHT
 *       key word arrived, then MU_KEYBD|MU_TIMER until the hook saw 4 codes
 *   phase B (blocking BIOS read): Bconin(2) once, then Bconstat until the
 *       hook saw 6 codes
 *   p[-1] = old               restore before exit (A6.4)
 *   print what the hook saw and what the AES / BIOS delivered, PASS/FAIL
 *   appl_exit, return (crt0 -> Pterm)
 *
 * The measurement injects: up make, right make, right break, up break, then
 * 'a' make, 'a' break. Expected hook sequence 48 4D CD C8 1E 9E (repeats are
 * not hook events, EmuTOS ikbd.c:524); AES words: 4800 (+ repeats) 4D00
 * (+ repeats); Bconin 001E0061.
 *
 * The hook runs as an EL0 upcall at the next syscall return. It touches only
 * integer registers -- an upcall does not save the interrupted code's
 * FP/SIMD registers (see the KEYS02 report), so a hook must not use them.
 */
#include <newtos.h>

/* ------------------------------------------------------------------------- */
/* AES binding (classic parameter block, LP64 pointers), as in sysacc.c.     */
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

#define MU_KEYBD 0x0001
#define MU_TIMER 0x0020

/* evnt_multi (AES 25): returns the event mask, *key = kreturn. */
static short evnt_multi(short flags, short ms, short *key)
{
    static short msg[8];
    int i;
    for (i = 0; i < 16; i++)
        aes_intin[i] = 0;
    aes_intin[0] = flags;
    aes_intin[14] = ms;         /* locount */
    aes_intin[15] = 0;          /* hicount */
    aes_addrin[0] = (long)msg;
    aes_call(25, 16, 7, 1);
    *key = aes_intout[5];
    return aes_intout[0];
}

/* ------------------------------------------------------------------------- */
/* Output                                                                    */
/* ------------------------------------------------------------------------- */

static void puts_(const char *s) { gemdos1(0x09, (NLONG)s); }

static void hexn(char *dst, unsigned long v, int digits)
{
    static const char hx[] = "0123456789ABCDEF";
    int i;
    for (i = digits - 1; i >= 0; i--) {
        dst[i] = hx[v & 0xF];
        v >>= 4;
    }
    dst[digits] = '\0';
}

static void puthex(unsigned long v, int digits)
{
    char b[20];
    hexn(b, v, digits);
    puts_(b);
}

/* ------------------------------------------------------------------------- */
/* The hook                                                                  */
/* ------------------------------------------------------------------------- */

typedef void (*kbdvec_fn)(long scancode, long iorec);

static kbdvec_fn old_kbdvec;
static volatile unsigned char seen[32];
static volatile int nseen;

/* kbdvec: x0 = scancode (bit 7 = break), x1 = 0 (no IOREC, D2a). Record the
 * byte, then chain to the previous vector with unchanged arguments (A6.3). */
static void hook(long scancode, long iorec)
{
    int n = nseen;
    if (n < 32) {
        seen[n] = (unsigned char)scancode;
        nseen = n + 1;
    }
    old_kbdvec(scancode, iorec);
}

/* ------------------------------------------------------------------------- */

static const unsigned char want[6] = { 0x48, 0x4D, 0xCD, 0xC8, 0x1E, 0x9E };

int main(void)
{
    unsigned long *p;
    unsigned short words[64];
    int nwords = 0, i, ok, guard;
    short key, ev;
    long bc;

    aes_call(10, 0, 1, 0);      /* appl_init */

    p = (unsigned long *)xbios0(0x22);   /* Kbdvbase */
    old_kbdvec = (kbdvec_fn)p[-1];
    puts_("KBDVEC: Kbdvbase=");
    puthex((unsigned long)p, 8);
    puts_(" kbdvec=");
    puthex((unsigned long)old_kbdvec, 8);
    puts_(" ikbdsys=");
    puthex(p[8], 8);
    puts_("\r\n");
    ok = old_kbdvec != 0;
    for (i = 0; i < 9; i++)
        if (p[i] == 0)
            ok = 0;
    puts_(ok ? "KBDVEC: table OK (no slot is 0)\r\n" : "KBDVEC: table FAIL\r\n");

    p[-1] = (unsigned long)hook;
    puts_("KBDVEC: hook installed, phase A (evnt_multi MU_KEYBD, no timer)\r\n");

    /* Phase A: block in the AES with no timer until the RIGHT key word. */
    for (;;) {
        ev = evnt_multi(MU_KEYBD, 0, &key);
        if ((ev & MU_KEYBD) && nwords < 64)
            words[nwords++] = (unsigned short)key;
        if ((ev & MU_KEYBD) && ((unsigned short)key >> 8) == 0x4D)
            break;
    }
    /* …then keep consuming (repeats) until the hook saw both breaks. */
    for (guard = 0; nseen < 4 && guard < 1000; guard++) {
        ev = evnt_multi(MU_KEYBD | MU_TIMER, 20, &key);
        if ((ev & MU_KEYBD) && nwords < 64)
            words[nwords++] = (unsigned short)key;
    }
    puts_("KBDVEC: phase B (Bconin(2))\r\n");

    /* Phase B: a blocking BIOS read. */
    bc = bios1(0x02, 2);
    for (guard = 0; nseen < 6 && guard < 1000; guard++) {
        if (bios1(0x01, 2))
            (void)bios1(0x02, 2);
        evnt_multi(MU_TIMER, 20, &key);
    }
    while (bios1(0x01, 2))      /* leave nothing behind for the desktop */
        (void)bios1(0x02, 2);

    p[-1] = (unsigned long)old_kbdvec;   /* restore (A6.4) */

    puts_("KBDVEC: hook saw");
    for (i = 0; i < nseen; i++) {
        puts_(" ");
        puthex(seen[i], 2);
    }
    puts_("\r\nKBDVEC: AES words");
    for (i = 0; i < nwords; i++) {
        puts_(" ");
        puthex(words[i], 4);
    }
    puts_("\r\nKBDVEC: Bconin=");
    puthex((unsigned long)bc, 8);
    puts_("\r\n");

    ok = ok && nseen == 6 && ((unsigned long)bc & 0xFFFFFF) == 0x1E0061;
    for (i = 0; i < 6 && i < nseen; i++)
        if (seen[i] != want[i])
            ok = 0;
    if (nwords < 2 || words[0] != 0x4800)
        ok = 0;
    puts_(ok ? "KBDVEC: PASS\r\n" : "KBDVEC: FAIL\r\n");

    aes_call(19, 0, 1, 0);      /* appl_exit */
    return ok ? 0 : 1;
}
