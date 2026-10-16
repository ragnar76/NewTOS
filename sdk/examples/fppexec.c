/*
 * fppexec.c -- does a Pexec caller get its FP/SIMD registers back?
 * (TASK/ARCH02)
 *
 * The GEMDOS Pexec contract is "the call returns to an intact caller, with
 * the child's exit code in x0" (docs/api/gemdos/process/4b-pexec.md, native
 * ABI: the exit code is delivered "via the parent's saved trap frame"). On
 * AArch64 the caller's state includes the FP/SIMD register file v0..v31 and
 * the two status/control registers FPCR and FPSR (ARM ARM DDI 0487M.d,
 * C5.2.8 FPCR, C5.2.10 FPSR). A hard-float compiler keeps live values there
 * across a call -- AAPCS64 makes d8..d15 callee-saved, so clang parks loop
 * constants in them across an `svc` (TASK/ACC03 measured exactly that) -- and
 * a child program in between rewrites the whole file.
 *
 * One binary, two roles, selected by the first character of the command tail
 * (BasePage p_cmdlin, 0x80):
 *
 *   (empty)  PARENT: three rounds, each one asm block that loads a distinct
 *            128-bit pattern into every one of v0..v31, sets FPCR (AHP, DN,
 *            FZ, RMode = RZ) and FPSR (QC, IDC and the five cumulative
 *            flags), traps into Pexec and stores v0..v31/FPCR/FPSR right
 *            after the trap returns -- no compiler code runs in between, so
 *            nothing can spill or reload them. Rounds:
 *              A  Pexec(0 PE_LOADGO, C:\FPPEXEC.NCP, "k")
 *              B  Pexec(3 PE_LOAD) outside the block, then the block traps
 *                 Pexec(6 PE_GOTHENFREE, NULL, pd)
 *              C  as B with Pexec(4 PE_GO); the parent owns the PD afterwards
 *                 and frees nothing (its own Pterm releases it)
 *            One line per round and a final verdict:
 *              FPPEXEC: <round> rc=<n> v=<intact>/32 lost=<mask> FPCR <pre>/<post> FPSR <pre>/<post> [OK|FAIL]
 *              FPPEXEC: RESULT PASS | RESULT FAIL
 *   k        CHILD: loads a different pattern into every v register, sets a
 *            different FPCR (RMode = RP, AHP/DN/FZ clear) and FPSR = 0, and
 *            ends with Pterm0 from the same asm block.
 *
 * "pre" is the value read back from FPCR/FPSR after the parent wrote them, so
 * an implementation that does not keep a bit (RES0, RAZ/WI fields) does not
 * turn into a false FAIL; the round is only meaningful when that read-back
 * differs from what the child writes, which the program checks and reports
 * ("[INCONCLUSIVE]").
 *
 * Runs from C:\AUTO\ (console only, before the AES); the child is the same
 * file in the root, C:\FPPEXEC.NCP.
 *
 * Calls used, all against docs/api/:
 *   Pexec   GEMDOS 0x4B  docs/api/gemdos/process/4b-pexec.md
 *   Cconws  GEMDOS 0x09  docs/api/gemdos/console/09-cconws.md
 *   Pterm0  GEMDOS 0x00  docs/api/gemdos/process/00-pterm0.md
 */
#include <newtos.h>

/* Offset of `p_cmdlin` in the BasePage (tos-types/src/basepage.rs). */
#define P_CMDLIN 0x80

#define CHILD_PATH "C:\\FPPEXEC.NCP"

/* FPCR/FPSR values (ARM ARM C5.2.8 / C5.2.10 field layout). */
#define PARENT_FPCR 0x07C00000UL /* AHP[26] DN[25] FZ[24] RMode[23:22]=0b11 (RZ) */
#define PARENT_FPSR 0x0800009FUL /* QC[27] IDC[7] IXC[4] UFC[3] OFC[2] DZC[1] IOC[0] */
#define CHILD_FPCR  0x00400000UL /* RMode = 0b01 (RP), AHP/DN/FZ clear */
#define CHILD_FPSR  0x00000000UL

/* Our own BasePage: `*(_sysbase->os_run)` -- as sdk/examples/ptermch.c. */
static const unsigned char *own_basepage(void)
{
    unsigned long sysbase = *(volatile unsigned int *)0x4F2UL;
    unsigned long run_cell = *(volatile unsigned int *)(sysbase + 0x28);
    return (const unsigned char *)(unsigned long)*(volatile unsigned int *)run_cell;
}

/* 32 registers x 16 bytes. */
static unsigned char pat_parent[512] __attribute__((aligned(16)));
static unsigned char pat_child[512] __attribute__((aligned(16)));
static unsigned char pat_out[512] __attribute__((aligned(16)));

/* Control block shared with the asm (offsets are load-bearing). */
struct fpctl {
    unsigned long fpcr_set;  /* 0x00 in  */
    unsigned long fpsr_set;  /* 0x08 in  */
    unsigned long fpcr_pre;  /* 0x10 out: read back after the write */
    unsigned long fpsr_pre;  /* 0x18 out */
    unsigned long fpcr_post; /* 0x20 out: right after the trap */
    unsigned long fpsr_post; /* 0x28 out */
};

static void fill_patterns(void)
{
    int i;
    for (i = 0; i < 512; i++) {
        /* Every byte of every register distinct within the file, and the
         * child's the bitwise complement of the parent's. */
        unsigned char b = (unsigned char)((i * 7 + 0x5A) ^ (i >> 4));
        pat_parent[i] = b;
        pat_child[i] = (unsigned char)~b;
    }
}

#define LDQ(base)                                                              \
    "ldp q0, q1, [" base ", #0x000]\n\tldp q2, q3, [" base ", #0x020]\n\t"     \
    "ldp q4, q5, [" base ", #0x040]\n\tldp q6, q7, [" base ", #0x060]\n\t"     \
    "ldp q8, q9, [" base ", #0x080]\n\tldp q10, q11, [" base ", #0x0a0]\n\t"   \
    "ldp q12, q13, [" base ", #0x0c0]\n\tldp q14, q15, [" base ", #0x0e0]\n\t" \
    "ldp q16, q17, [" base ", #0x100]\n\tldp q18, q19, [" base ", #0x120]\n\t" \
    "ldp q20, q21, [" base ", #0x140]\n\tldp q22, q23, [" base ", #0x160]\n\t" \
    "ldp q24, q25, [" base ", #0x180]\n\tldp q26, q27, [" base ", #0x1a0]\n\t" \
    "ldp q28, q29, [" base ", #0x1c0]\n\tldp q30, q31, [" base ", #0x1e0]\n\t"

#define STQ(base)                                                              \
    "stp q0, q1, [" base ", #0x000]\n\tstp q2, q3, [" base ", #0x020]\n\t"     \
    "stp q4, q5, [" base ", #0x040]\n\tstp q6, q7, [" base ", #0x060]\n\t"     \
    "stp q8, q9, [" base ", #0x080]\n\tstp q10, q11, [" base ", #0x0a0]\n\t"   \
    "stp q12, q13, [" base ", #0x0c0]\n\tstp q14, q15, [" base ", #0x0e0]\n\t" \
    "stp q16, q17, [" base ", #0x100]\n\tstp q18, q19, [" base ", #0x120]\n\t" \
    "stp q20, q21, [" base ", #0x140]\n\tstp q22, q23, [" base ", #0x160]\n\t" \
    "stp q24, q25, [" base ", #0x180]\n\tstp q26, q27, [" base ", #0x1a0]\n\t" \
    "stp q28, q29, [" base ", #0x1c0]\n\tstp q30, q31, [" base ", #0x1e0]\n\t"

#define ALL_V                                                                  \
    "v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11",  \
    "v12", "v13", "v14", "v15", "v16", "v17", "v18", "v19", "v20", "v21",      \
    "v22", "v23", "v24", "v25", "v26", "v27", "v28", "v29", "v30", "v31"

/*
 * The parent's round: load pat_parent into v0..v31, write FPCR/FPSR, trap
 * Pexec(mode, a1, a2, a3), store v0..v31 into pat_out and FPCR/FPSR into
 * `c`, put the caller's own FPCR/FPSR back. One asm block, so the values sit
 * in the registers -- and nowhere else -- across the trap. The pointers are
 * pinned to callee-saved x19..x21 and the saved FPCR/FPSR to x22/x23, which
 * the compiler therefore preserves around the block.
 */
static NLONG fp_pexec(NLONG mode, NLONG a1, NLONG a2, NLONG a3, struct fpctl *c)
{
    register NLONG r0 __asm__("x0") = mode;
    register NLONG r1 __asm__("x1") = a1;
    register NLONG r2 __asm__("x2") = a2;
    register NLONG r3 __asm__("x3") = a3;
    register unsigned char *rin __asm__("x19") = pat_parent;
    register unsigned char *rout __asm__("x20") = pat_out;
    register struct fpctl *rc __asm__("x21") = c;

    __asm__ volatile(
        "mrs x22, fpcr\n\t"
        "mrs x23, fpsr\n\t"
        "ldr x9, [x21, #0x00]\n\t"
        "msr fpcr, x9\n\t"
        "ldr x9, [x21, #0x08]\n\t"
        "msr fpsr, x9\n\t"
        "mrs x9, fpcr\n\t"
        "str x9, [x21, #0x10]\n\t"
        "mrs x9, fpsr\n\t"
        "str x9, [x21, #0x18]\n\t"
        LDQ("x19")
        "mov x8, #0x4b\n\t"
        "svc #1\n\t"
        "mrs x9, fpcr\n\t"
        "str x9, [x21, #0x20]\n\t"
        "mrs x9, fpsr\n\t"
        "str x9, [x21, #0x28]\n\t"
        STQ("x20")
        "msr fpcr, x22\n\t"
        "msr fpsr, x23\n\t"
        : "+r"(r0), "+r"(r1), "+r"(r2), "+r"(r3)
        : "r"(rin), "r"(rout), "r"(rc)
        : "x8", "x9", "x22", "x23", ALL_V, "memory", "cc");
    return r0;
}

/* The child: clobber the whole FP/SIMD file and both registers, then Pterm0
 * from the same block (nothing of ours runs after the clobber). */
__attribute__((noreturn)) static void child_clobber_and_exit(void)
{
    register unsigned char *rin __asm__("x19") = pat_child;
    register unsigned long fpcr __asm__("x20") = CHILD_FPCR;
    register unsigned long fpsr __asm__("x21") = CHILD_FPSR;

    __asm__ volatile(
        LDQ("x19")
        "msr fpcr, x20\n\t"
        "msr fpsr, x21\n\t"
        "mov x8, #0\n\t"
        "svc #1\n\t"
        :
        : "r"(rin), "r"(fpcr), "r"(fpsr)
        : "x0", "x8", ALL_V, "memory", "cc");
    __builtin_unreachable();
}

/* ---- output ------------------------------------------------------------ */

static int puts_at(char *dst, int j, const char *s)
{
    while (*s)
        dst[j++] = *s++;
    dst[j] = '\0';
    return j;
}

static int dec_at(char *dst, int j, NLONG v)
{
    char tmp[24];
    int n = 0;
    unsigned long u;

    if (v < 0) {
        dst[j++] = '-';
        u = (unsigned long)(-(v + 1)) + 1UL;
    } else {
        u = (unsigned long)v;
    }
    do {
        tmp[n++] = (char)('0' + (int)(u % 10UL));
        u /= 10UL;
    } while (u);
    while (n)
        dst[j++] = tmp[--n];
    dst[j] = '\0';
    return j;
}

static int hex_at(char *dst, int j, unsigned long v, int digits)
{
    static const char hx[] = "0123456789abcdef";
    int k;
    for (k = digits - 1; k >= 0; k--)
        dst[j++] = hx[(v >> (k * 4)) & 0xF];
    dst[j] = '\0';
    return j;
}

/* Pexec(3 PE_LOAD, path, tail, NULL): the child's PD pointer, or a negative
 * error. */
static NLONG pexec_load(const char *path, const char *tail)
{
    register NLONG r0 __asm__("x0") = 3;
    register NLONG r1 __asm__("x1") = (NLONG)path;
    register NLONG r2 __asm__("x2") = (NLONG)tail;
    register NLONG r3 __asm__("x3") = 0;
    register NLONG x8 __asm__("x8") = 0x4B;
    __asm__ volatile("svc #1"
                     : "+r"(r0)
                     : "r"(x8), "r"(r1), "r"(r2), "r"(r3)
                     : "memory");
    return r0;
}

/* ---- the parent's rounds ------------------------------------------------ */

/* 0 = OK, 1 = FAIL, 2 = INCONCLUSIVE (the child's writes would not differ). */
static int report_round(const char *name, NLONG rc, NLONG want_rc, const struct fpctl *c)
{
    char line[200];
    unsigned long lost = 0;
    int intact = 0, r, i, j, verdict;

    for (r = 0; r < 32; r++) {
        int same = 1;
        for (i = 0; i < 16; i++)
            if (pat_out[r * 16 + i] != pat_parent[r * 16 + i])
                same = 0;
        if (same)
            intact++;
        else
            lost |= 1UL << r;
    }
    if (c->fpcr_pre == CHILD_FPCR || c->fpsr_pre == CHILD_FPSR)
        verdict = 2;
    else if (rc == want_rc && intact == 32 && c->fpcr_post == c->fpcr_pre &&
             c->fpsr_post == c->fpsr_pre)
        verdict = 0;
    else
        verdict = 1;

    j = puts_at(line, 0, "FPPEXEC: ");
    j = puts_at(line, j, name);
    j = puts_at(line, j, " rc=");
    j = dec_at(line, j, rc);
    j = puts_at(line, j, " v=");
    j = dec_at(line, j, intact);
    j = puts_at(line, j, "/32 lost=");
    j = hex_at(line, j, lost, 8);
    j = puts_at(line, j, " FPCR ");
    j = hex_at(line, j, c->fpcr_pre, 8);
    j = puts_at(line, j, "/");
    j = hex_at(line, j, c->fpcr_post, 8);
    j = puts_at(line, j, " FPSR ");
    j = hex_at(line, j, c->fpsr_pre, 8);
    j = puts_at(line, j, "/");
    j = hex_at(line, j, c->fpsr_post, 8);
    j = puts_at(line, j, verdict == 0 ? " [OK]\r\n"
                         : verdict == 1 ? " [FAIL]\r\n"
                                        : " [INCONCLUSIVE]\r\n");
    Cconws(line);
    return verdict;
}

static void reset_ctl(struct fpctl *c)
{
    int i;
    c->fpcr_set = PARENT_FPCR;
    c->fpsr_set = PARENT_FPSR;
    c->fpcr_pre = c->fpsr_pre = c->fpcr_post = c->fpsr_post = 0;
    for (i = 0; i < 512; i++)
        pat_out[i] = 0;
}

static int run_parent(void)
{
    /* Command tail image: length byte 1, then "k" (the child role). */
    static const char tail[] = "\001k";
    struct fpctl c;
    NLONG rc, pd;
    int bad = 0, v;

    Cconws("FPPEXEC: FP/SIMD state across Pexec (ARCH02)\r\n");

    /* A: PE_LOADGO. */
    reset_ctl(&c);
    rc = fp_pexec(0, (NLONG)CHILD_PATH, (NLONG)tail, 0, &c);
    v = report_round("A Pexec(0)", rc, 0, &c);
    bad |= v;

    /* B: PE_LOAD, then PE_GOTHENFREE from inside the block. */
    pd = pexec_load(CHILD_PATH, tail);
    if (pd <= 0) {
        Cconws("FPPEXEC: B Pexec(3) failed [FAIL]\r\n");
        bad |= 1;
    } else {
        reset_ctl(&c);
        rc = fp_pexec(6, 0, pd, 0, &c);
        v = report_round("B Pexec(3)+Pexec(6)", rc, 0, &c);
        bad |= v;
    }

    /* C: PE_LOAD, then PE_GO. */
    pd = pexec_load(CHILD_PATH, tail);
    if (pd <= 0) {
        Cconws("FPPEXEC: C Pexec(3) failed [FAIL]\r\n");
        bad |= 1;
    } else {
        reset_ctl(&c);
        rc = fp_pexec(4, 0, pd, 0, &c);
        v = report_round("C Pexec(3)+Pexec(4)", rc, 0, &c);
        bad |= v;
    }

    Cconws(bad == 0 ? "FPPEXEC: RESULT PASS\r\n"
           : (bad & 1) ? "FPPEXEC: RESULT FAIL\r\n"
                       : "FPPEXEC: RESULT INCONCLUSIVE\r\n");
    return bad == 0 ? 0 : 1;
}

int main(void)
{
    const unsigned char *tail = own_basepage() + P_CMDLIN;
    char mode = tail[0] ? (char)tail[1] : '\0';

    fill_patterns();
    if (mode == 'k')
        child_clobber_and_exit();
    return run_parent();
}
