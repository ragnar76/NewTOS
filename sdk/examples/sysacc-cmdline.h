/*
 * sysacc-cmdline.h -- SYSINFO02: the boot-argument logic of the System Info
 * accessory (sdk/examples/sysacc.c), as pure byte logic without libc, so the
 * same code runs in the accessory and in the host test
 * (sdk/examples/tests/sysacc-cmdline-host.c, sdk/run-host-tests.sh).
 *
 * Two jobs:
 *
 *  1. cl_parse(): what the NEXT start will run with, read from the text of
 *     C:\CMDLINE.TXT exactly the way the kernel reads its boot arguments
 *     (kernel/src/bootargs.rs + the switches in kernel/src/lib.rs):
 *       - tokens are separated by any ASCII whitespace, the text ends at NUL;
 *       - an on/off token matches only as a whole token (`has_token`);
 *       - `memprot=off` wins over `memprot=strict` (memprot_switch);
 *       - `asid=tiny` wins over `asid=on` (asid_switch);
 *       - `bcache=` / `bcachepct=`: the FIRST token with that prefix decides;
 *         its value is 1..10 decimal digits within the maximum, otherwise the
 *         kernel default applies (numeric_setting, Setting::Invalid).
 *
 *  2. cl_merge(): the new file text. Every token the accessory does not
 *     manage stays, in its order (firmware / Linux tokens, unknown ones,
 *     `memprot=verify`, misspellings such as `cpuclk=max`). A managed token is
 *     exactly one the kernel interprets for one of the eight settings:
 *       memprot=off memprot=strict asid=on asid=tiny emmc=compat
 *       cpuclk=firmware input=aes serial=on bcache=<any> bcachepct=<any>
 *     (`bcache=`/`bcachepct=` with any value: the kernel reads the first of
 *     them, even an invalid one). The first managed token of a setting is
 *     replaced in place by the setting's new token, every further one is
 *     dropped; a setting equal to the kernel default writes no token. A
 *     setting with a token but none in the old text is appended at the end,
 *     in the order of the table below. The result is one line (tokens joined
 *     by one space, `references/circle/doc/cmdline.txt:3-4`: "Multiple
 *     options must be placed on a single line delimited by a space
 *     character"), ended by CR LF when the old text used CR LF, else by LF.
 *
 * Kernel defaults (kernel/src/bootargs.rs, kernel/src/lib.rs,
 * tos-api/src/bios/bcache.rs DEFAULT_KIB / DEFAULT_PCT / MAX_KIB):
 *   memory protection on, ASIDs off, SD fast mode, CPU clock raised to the
 *   maximum, block cache 32768 KiB, cache share 5 %, pointer motion from the
 *   timer tick (no input=aes), serial console off.
 *
 * Value codes follow the Ssystem modes of docs/api/gemdos/154-ssystem.md
 * where one exists (2005 memprot, 2006 ASID, 2007 SD mode, 2009 KiB,
 * 2010 percent, 2018 serial); CPU clock 1 = fixed at maximum, 0 = firmware
 * (2004's 0/1); input 1 = pointer moves while busy, 0 = input=aes.
 */
#ifndef SYSACC_CMDLINE_H
#define SYSACC_CMDLINE_H

enum {
    CL_MEMPROT,   /* 0 off, 1 on, 2 strict           */
    CL_ASID,      /* 0 off, 1 on, 2 test (asid=tiny) */
    CL_SDMODE,    /* 0 fast, 1 compatibility         */
    CL_CPUCLK,    /* 0 firmware-controlled, 1 max    */
    CL_BCACHE,    /* KiB, 0 = off                    */
    CL_BCACHEPCT, /* percent                         */
    CL_INPUT,     /* 1 moves while busy, 0 input=aes */
    CL_SERIAL,    /* 0 off, 1 on                     */
    CL_COUNT
};

#define CL_BCACHE_DEFAULT 32768L  /* bcache.rs DEFAULT_KIB  */
#define CL_BCACHE_MAX     262144L /* bcache.rs MAX_KIB      */
#define CL_PCT_DEFAULT    5L      /* bcache.rs DEFAULT_PCT  */
#define CL_PCT_MAX        100L    /* lib.rs bcache_switch   */

typedef struct {
    long v[CL_COUNT];
} cl_settings;

static void cl_defaults(cl_settings *s)
{
    s->v[CL_MEMPROT] = 1;
    s->v[CL_ASID] = 0;
    s->v[CL_SDMODE] = 0;
    s->v[CL_CPUCLK] = 1;
    s->v[CL_BCACHE] = CL_BCACHE_DEFAULT;
    s->v[CL_BCACHEPCT] = CL_PCT_DEFAULT;
    s->v[CL_INPUT] = 1;
    s->v[CL_SERIAL] = 0;
}

/* ------------------------------------------------------------------------- */
/* Tokens                                                                     */
/* ------------------------------------------------------------------------- */

/* The ASCII whitespace of Rust's u8::is_ascii_whitespace (bootargs.rs):
 * space, tab, LF, form feed, CR. (Vertical tab is NOT in that set.) */
static int cl_ws(char c)
{
    return c == ' ' || c == '\t' || c == '\n' || c == '\f' || c == '\r';
}

/* Length of the text: `len` bytes or up to the first NUL. */
static int cl_textlen(const char *t, int len)
{
    int n = 0;
    while (n < len && t[n] != '\0')
        n++;
    return n;
}

/* Next token at or after *pos: its start in *start, length returned (0 = no
 * more tokens). *pos moves behind it. */
static int cl_next(const char *t, int n, int *pos, int *start)
{
    int i = *pos, s;
    while (i < n && cl_ws(t[i]))
        i++;
    s = i;
    while (i < n && !cl_ws(t[i]))
        i++;
    *pos = i;
    *start = s;
    return i - s;
}

static int cl_is(const char *tok, int tl, const char *lit)
{
    int i = 0;
    while (i < tl && lit[i] != '\0' && tok[i] == lit[i])
        i++;
    return i == tl && lit[i] == '\0';
}

static int cl_prefix(const char *tok, int tl, const char *lit)
{
    int i = 0;
    while (lit[i] != '\0') {
        if (i >= tl || tok[i] != lit[i])
            return 0;
        i++;
    }
    return 1;
}

/* numeric_setting's value check: 1..10 decimal digits, <= max. Returns the
 * value, or -1 when invalid. */
static long cl_number(const char *d, int n, long max)
{
    long v = 0;
    int i;
    if (n < 1 || n > 10)
        return -1;
    for (i = 0; i < n; i++) {
        if (d[i] < '0' || d[i] > '9')
            return -1;
        v = v * 10 + (d[i] - '0');
        if (v > max)
            return -1; /* also keeps v from overflowing */
    }
    return v;
}

/* Which setting a token is a managed token of, or -1. */
static int cl_managed(const char *tok, int tl)
{
    if (cl_is(tok, tl, "memprot=off") || cl_is(tok, tl, "memprot=strict"))
        return CL_MEMPROT;
    if (cl_is(tok, tl, "asid=on") || cl_is(tok, tl, "asid=tiny"))
        return CL_ASID;
    if (cl_is(tok, tl, "emmc=compat"))
        return CL_SDMODE;
    if (cl_is(tok, tl, "cpuclk=firmware"))
        return CL_CPUCLK;
    if (cl_prefix(tok, tl, "bcachepct="))
        return CL_BCACHEPCT;
    if (cl_prefix(tok, tl, "bcache="))
        return CL_BCACHE;
    if (cl_is(tok, tl, "input=aes"))
        return CL_INPUT;
    if (cl_is(tok, tl, "serial=on"))
        return CL_SERIAL;
    return -1;
}

/* ------------------------------------------------------------------------- */
/* Parse                                                                      */
/* ------------------------------------------------------------------------- */

/* The settings a boot with `text` (len bytes, or up to a NUL) runs with. */
static void cl_parse(const char *text, int len, cl_settings *s)
{
    int n = text ? cl_textlen(text, len) : 0;
    int pos = 0, st, tl;
    int off = 0, strict = 0, on = 0, tiny = 0;
    int seen_kib = 0, seen_pct = 0;

    cl_defaults(s);
    while ((tl = cl_next(text, n, &pos, &st)) > 0) {
        const char *t = text + st;
        if (cl_is(t, tl, "memprot=off"))
            off = 1;
        else if (cl_is(t, tl, "memprot=strict"))
            strict = 1;
        else if (cl_is(t, tl, "asid=on"))
            on = 1;
        else if (cl_is(t, tl, "asid=tiny"))
            tiny = 1;
        else if (cl_is(t, tl, "emmc=compat"))
            s->v[CL_SDMODE] = 1;
        else if (cl_is(t, tl, "cpuclk=firmware"))
            s->v[CL_CPUCLK] = 0;
        else if (cl_is(t, tl, "input=aes"))
            s->v[CL_INPUT] = 0;
        else if (cl_is(t, tl, "serial=on"))
            s->v[CL_SERIAL] = 1;
        else if (cl_prefix(t, tl, "bcachepct=")) {
            if (!seen_pct) {
                long v = cl_number(t + 10, tl - 10, CL_PCT_MAX);
                if (v >= 0)
                    s->v[CL_BCACHEPCT] = v;
                seen_pct = 1;
            }
        } else if (cl_prefix(t, tl, "bcache=")) {
            if (!seen_kib) {
                long v = cl_number(t + 7, tl - 7, CL_BCACHE_MAX);
                if (v >= 0)
                    s->v[CL_BCACHE] = v;
                seen_kib = 1;
            }
        }
    }
    s->v[CL_MEMPROT] = off ? 0 : strict ? 2 : 1;
    s->v[CL_ASID] = tiny ? 2 : on ? 1 : 0;
}

/* ------------------------------------------------------------------------- */
/* Merge                                                                      */
/* ------------------------------------------------------------------------- */

/* Output buffer with an overflow flag. */
typedef struct {
    char *b;
    int n, cap, over;
} cl_out;

static void cl_put(cl_out *o, const char *s, int len)
{
    int i;
    for (i = 0; i < len; i++) {
        if (o->n >= o->cap) {
            o->over = 1;
            return;
        }
        o->b[o->n++] = s[i];
    }
}

static void cl_puts(cl_out *o, const char *s)
{
    int n = 0;
    while (s[n] != '\0')
        n++;
    cl_put(o, s, n);
}

static void cl_putnum(cl_out *o, long v)
{
    char tmp[24];
    int i = 0;
    unsigned long u = (unsigned long)(v < 0 ? 0 : v);
    do {
        tmp[i++] = (char)('0' + (int)(u % 10));
        u /= 10;
    } while (u);
    while (i)
        cl_put(o, &tmp[--i], 1);
}

/* Space between tokens: none before the first one. */
static void cl_sep(cl_out *o, int *first)
{
    if (!*first)
        cl_put(o, " ", 1);
    *first = 0;
}

/* The token setting `k` writes for value `v`, or nothing for the default.
 * Returns 1 if a token was written. */
static int cl_token(cl_out *o, int *first, int k, long v)
{
    const char *lit = 0;
    switch (k) {
    case CL_MEMPROT:
        lit = v == 0 ? "memprot=off" : v == 2 ? "memprot=strict" : 0;
        break;
    case CL_ASID:
        lit = v == 1 ? "asid=on" : v == 2 ? "asid=tiny" : 0;
        break;
    case CL_SDMODE:
        lit = v ? "emmc=compat" : 0;
        break;
    case CL_CPUCLK:
        lit = v ? 0 : "cpuclk=firmware";
        break;
    case CL_INPUT:
        lit = v ? 0 : "input=aes";
        break;
    case CL_SERIAL:
        lit = v ? "serial=on" : 0;
        break;
    case CL_BCACHE:
        if (v == CL_BCACHE_DEFAULT)
            return 0;
        cl_sep(o, first);
        cl_puts(o, "bcache=");
        cl_putnum(o, v);
        return 1;
    case CL_BCACHEPCT:
        if (v == CL_PCT_DEFAULT)
            return 0;
        cl_sep(o, first);
        cl_puts(o, "bcachepct=");
        cl_putnum(o, v);
        return 1;
    default:
        return 0;
    }
    if (!lit)
        return 0;
    cl_sep(o, first);
    cl_puts(o, lit);
    return 1;
}

/* Build the new file text from the old one (`old`, `oldlen` bytes or up to a
 * NUL; NULL/0 = no file) and the settings `s` into `out` (capacity `cap`).
 * Returns the length written, or -1 if `cap` is too small. No NUL is added. */
static int cl_merge(const char *old, int oldlen, const cl_settings *s,
                    char *out, int cap)
{
    cl_out o;
    int n = old ? cl_textlen(old, oldlen) : 0;
    int pos = 0, st, tl, k, i;
    int first = 1, crlf = 0;
    int done[CL_COUNT];

    o.b = out;
    o.n = 0;
    o.cap = cap;
    o.over = 0;
    for (k = 0; k < CL_COUNT; k++)
        done[k] = 0;
    for (i = 0; i + 1 < n; i++) {
        if (old[i] == '\r' && old[i + 1] == '\n') {
            crlf = 1;
            break;
        }
    }

    while ((tl = cl_next(old, n, &pos, &st)) > 0) {
        const char *t = old + st;
        k = cl_managed(t, tl);
        if (k < 0) {
            cl_sep(&o, &first);
            cl_put(&o, t, tl);
        } else if (!done[k]) {
            /* The setting's first managed token: its new value goes here. */
            cl_token(&o, &first, k, s->v[k]);
            done[k] = 1;
        }
        /* Further managed tokens of the same setting are dropped. */
    }
    for (k = 0; k < CL_COUNT; k++)
        if (!done[k])
            cl_token(&o, &first, k, s->v[k]);

    cl_puts(&o, crlf ? "\r\n" : "\n");
    return o.over ? -1 : o.n;
}

#endif /* SYSACC_CMDLINE_H */
