/*
 * wfscreen.c -- proof that `wind_get(WF_SCREEN)` hands an EL0 program the
 * AES menu/alert screen buffer as an address it can really write through.
 *
 * A native NewTOS GEM program (`.NAP`, started through the `#Z` autorun line
 * of C:\NEWDESK.INF -- NOT from \AUTO\, which runs before the AES goes
 * graphic and before the buffer exists). It is a check, not a demo:
 *
 *   1. `appl_init`, then `graf_handle` + `v_opnvwk` for the screen size
 *      (`work_out[0..1]` = xmax/ymax), then `v_clsvwk`;
 *   2. `wind_get(0, WF_SCREEN)` with OUT_LEN 5 and reads the two LONGs the
 *      way EmuTOS stores them -- `*(LONG *)&int_out[1]` is the address,
 *      `*(LONG *)&int_out[3]` the length (`aes/gemwmlib.c:1564`,
 *      `aes/gemgsxif.c:125-129`) -- as native 32-bit loads at the
 *      WORD-aligned slots (what TeraDesk's union and cflib do);
 *   3. checks ret == 1, address != 0 and inside the EL0 window (< 32 MiB),
 *      length == stride16(W) * H * 2 (NewTOS' `gsx_mcalc`: one screen in
 *      device pixels, `docs/COMPATIBILITY.md`);
 *   4. writes a byte pattern over the WHOLE length through the pointer, reads
 *      it back, and prints "write ok" or the first mismatch. No fault means
 *      the pages are really published read-write into this process
 *      (ADR-023, `PROT_G`);
 *   5. `appl_exit`, returns. Every line goes out through `Cconws`, so the
 *      serial log carries the verdict.
 *
 * Deliberately **no** key pause (TASK/T13): this is measurement scaffolding,
 * not a card program. tools/make-sd-image.sh does not install it; it is built
 * into a scratch image for TASK/A57 and started by the `#Z` autorun line with
 * no mouse and no keyboard in the run, so a key wait would simply park it.
 *
 * Spec: docs/api/aes/window-graphics.md (`WF_SCREEN`), TASK/A57.md.
 */
#include <newtos.h>

/* ---- AES parameter block (the classic layout, LP64 pointers) ---- */

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

/* Issue one AES call; returns int_out[0]. `int_out` is poisoned first so a
 * word the AES did not write is visible as such. */
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
        aes_intout[i] = (short)0xDEAD;
    pb.control = aes_control;
    pb.global  = aes_global;
    pb.intin   = aes_intin;
    pb.intout  = aes_intout;
    pb.addrin  = aes_addrin;
    pb.addrout = aes_addrout;
    aes_trap(&pb);
    return aes_intout[0];
}

/* ---- VDI parameter block (selector 115) ---- */

static short vdi_contrl[12];
static short vdi_intin[128];
static short vdi_ptsin[256];
static short vdi_intout[128];
static short vdi_ptsout[256];

static void vdi_trap(void)
{
    long block[5];
    register long x0 __asm__("x0") = 115;
    register long x1 __asm__("x1");
    block[0] = (long)vdi_contrl;
    block[1] = (long)vdi_intin;
    block[2] = (long)vdi_ptsin;
    block[3] = (long)vdi_intout;
    block[4] = (long)vdi_ptsout;
    x1 = (long)block;
    __asm__ __volatile__("svc\t#2" : "+r"(x0) : "r"(x1) : "memory", "cc");
    (void)x0;
}

/* ---- output helpers (no libc, no printf) ---- */

static void puts_(const char *s)
{
    Cconws(s);
}

static void puthex32(unsigned long v)
{
    static const char digits[] = "0123456789ABCDEF";
    char buf[11];
    int i;
    buf[0] = '0';
    buf[1] = 'x';
    for (i = 0; i < 8; i++)
        buf[2 + i] = digits[(v >> ((7 - i) * 4)) & 0xF];
    buf[10] = 0;
    Cconws(buf);
}

static void putdec(unsigned long v)
{
    char buf[24];
    int i = 23;
    buf[i] = 0;
    if (v == 0)
        buf[--i] = '0';
    while (v) {
        buf[--i] = (char)('0' + v % 10);
        v /= 10;
    }
    Cconws(buf + i);
}

/* The LONG EmuTOS stored at &int_out[word]: one native 32-bit load at the
 * WORD-aligned address (gemwmlib.c:1564 does `*(LONG *)poutwds = ...`). */
static unsigned long long_at(int word)
{
    unsigned int v;
    __builtin_memcpy(&v, &aes_intout[word], 4);
    return v;
}

#define WF_SCREEN 17
#define EL0_WINDOW_TOP (32ul * 1024 * 1024)

int main(void)
{
    short apid, vh, ret;
    int i;
    unsigned long addr, len, want, w, h, stride;
    volatile unsigned char *p;
    unsigned long bad = (unsigned long)-1;

    puts_("WFSCREEN.NAP -- wind_get(WF_SCREEN) write probe from EL0\r\n");

    apid = aes_call(10, 0, 1, 0); /* appl_init */
    if (apid < 0) {
        puts_("appl_init failed\r\n");
        return 1;
    }

    /* graf_handle -> physical handle in int_out[0]; then v_opnvwk on it for
     * the screen geometry: work_out[0] = xmax, work_out[1] = ymax. */
    vh = aes_call(77, 0, 5, 0); /* graf_handle */
    for (i = 0; i < 12; i++)
        vdi_contrl[i] = 0;
    for (i = 0; i < 10; i++)
        vdi_intin[i] = 1;
    vdi_intin[10] = 2;              /* raster coordinates */
    vdi_contrl[0] = 100;            /* v_opnvwk */
    vdi_contrl[3] = 11;
    vdi_contrl[6] = vh;
    vdi_trap();
    vh = vdi_contrl[6];
    w = (unsigned long)vdi_intout[0] + 1;
    h = (unsigned long)vdi_intout[1] + 1;
    for (i = 0; i < 12; i++)
        vdi_contrl[i] = 0;
    vdi_contrl[0] = 101;            /* v_clsvwk */
    vdi_contrl[6] = vh;
    vdi_trap();

    puts_("screen ");
    putdec(w);
    puts_("x");
    putdec(h);
    puts_("\r\n");

    /* wind_get(0, WF_SCREEN): IN_LEN 2, OUT_LEN 5 -- cases 331/332/439 of
     * the r4 sub-function table. */
    aes_intin[0] = 0;
    aes_intin[1] = WF_SCREEN;
    ret = aes_call(104, 2, 5, 0);
    addr = long_at(1);
    len = long_at(3);

    puts_("wind_get(0, WF_SCREEN) ret=");
    putdec((unsigned long)ret);
    puts_(" addr=");
    puthex32(addr);
    puts_(" len=");
    putdec(len);
    puts_("\r\n");

    if (ret != 1) {
        puts_("FAIL: ret != 1 (no screen buffer)\r\n");
        aes_call(19, 0, 1, 0);
        return 1;
    }
    if (addr == 0 || addr >= EL0_WINDOW_TOP) {
        puts_("FAIL: address is 0 or outside the EL0 window\r\n");
        aes_call(19, 0, 1, 0);
        return 1;
    }
    stride = ((w + 15) / 16) * 16;
    want = stride * h * 2;
    if (len != want) {
        puts_("FAIL: len != stride16(W)*H*2 = ");
        putdec(want);
        puts_("\r\n");
        aes_call(19, 0, 1, 0);
        return 1;
    }

    /* The whole point: store through the pointer over the WHOLE length, then
     * read it back. A fault here would be the kernel not having published the
     * pages; a mismatch would be a mapping to the wrong memory. */
    p = (volatile unsigned char *)addr;
    for (i = 0; (unsigned long)i < len; i++)
        p[i] = (unsigned char)(i * 7 + 3);
    for (i = 0; (unsigned long)i < len; i++) {
        if (p[i] != (unsigned char)(i * 7 + 3)) {
            bad = (unsigned long)i;
            break;
        }
    }
    if (bad != (unsigned long)-1) {
        puts_("FAIL: read-back mismatch at offset ");
        putdec(bad);
        puts_("\r\n");
        aes_call(19, 0, 1, 0);
        return 1;
    }
    puts_("write ok (");
    putdec(len);
    puts_(" bytes written and read back through the WF_SCREEN pointer)\r\n");

    /* A second call must hand out the same address (one buffer, mapped once). */
    aes_intin[0] = -1;
    aes_intin[1] = WF_SCREEN;
    ret = aes_call(104, 2, 5, 0);
    if (ret != 1 || long_at(1) != addr || long_at(3) != len) {
        puts_("FAIL: second wind_get(WF_SCREEN) differs\r\n");
        aes_call(19, 0, 1, 0);
        return 1;
    }
    puts_("second call: same address, same length\r\n");
    puts_("PASS\r\n");

    aes_call(19, 0, 1, 0); /* appl_exit */
    return 0;
}
