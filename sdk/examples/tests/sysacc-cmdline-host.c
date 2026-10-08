/*
 * sysacc-cmdline-host.c -- SYSINFO02 host test: the boot-argument parse and
 * merge of the System Info accessory (sdk/examples/sysacc-cmdline.h), the
 * very header the accessory includes, compiled for the host.
 *
 *   cc -O1 -g -fsanitize=address,undefined -Wall -Wextra -Werror \
 *      -o /tmp/t sdk/examples/tests/sysacc-cmdline-host.c && /tmp/t
 *
 * (sdk/run-host-tests.sh runs exactly this.) The parse cases are the
 * kernel's own bootargs.rs test inputs where they apply; the merge cases are
 * the ticket's: unknown tokens kept in order, managed tokens replaced in
 * place, defaults removed, empty and missing file, CRLF / LF, trailing
 * blanks, duplicates, invalid numbers, a too-small buffer.
 */
#include <stdio.h>
#include <string.h>

#include "../sysacc-cmdline.h"

static int fails, checks;

static void expect_str(const char *what, const char *got, int gotlen,
                       const char *want)
{
    checks++;
    if (gotlen < 0 || (int)strlen(want) != gotlen ||
        memcmp(got, want, (size_t)gotlen) != 0) {
        fails++;
        printf("FAIL %s\n  got  [%.*s] (%d)\n  want [%s]\n", what,
               gotlen < 0 ? 0 : gotlen, got, gotlen, want);
    }
}

static void expect_long(const char *what, long got, long want)
{
    checks++;
    if (got != want) {
        fails++;
        printf("FAIL %s: got %ld, want %ld\n", what, got, want);
    }
}

/* merge(old, settings) must give want. */
static void merge_case(const char *what, const char *old, const cl_settings *s,
                       const char *want)
{
    char out[512];
    int n = cl_merge(old, old ? (int)strlen(old) : 0, s, out, (int)sizeof out);
    expect_str(what, out, n, want);
}

/* parse(old) then merge with no change must keep the meaning: the parse of
 * the merged text equals the parse of the original. */
static void roundtrip(const char *what, const char *old)
{
    cl_settings a, b;
    char out[512];
    int n, k;
    cl_parse(old, (int)strlen(old), &a);
    n = cl_merge(old, (int)strlen(old), &a, out, (int)sizeof out);
    checks++;
    if (n < 0) {
        fails++;
        printf("FAIL %s: merge overflow\n", what);
        return;
    }
    cl_parse(out, n, &b);
    for (k = 0; k < CL_COUNT; k++)
        expect_long(what, b.v[k], a.v[k]);
}

int main(void)
{
    cl_settings s, d;

    /* --- parse: the kernel's reading -------------------------------------- */
    cl_parse(NULL, 0, &s);
    cl_defaults(&d);
    expect_long("missing file = defaults", memcmp(&s, &d, sizeof s), 0);
    cl_parse("", 0, &s);
    expect_long("empty file = defaults", memcmp(&s, &d, sizeof s), 0);

    cl_parse("serial=on memprot=off", 21, &s);
    expect_long("memprot=off", s.v[CL_MEMPROT], 0);
    expect_long("serial=on", s.v[CL_SERIAL], 1);
    cl_parse("serial=on memprot=offx", 22, &s);
    expect_long("memprot=offx is no token", s.v[CL_MEMPROT], 1);
    cl_parse("memprot=strict\n", 15, &s);
    expect_long("memprot=strict", s.v[CL_MEMPROT], 2);
    cl_parse("memprot=strict memprot=off", 26, &s);
    expect_long("off wins over strict", s.v[CL_MEMPROT], 0);
    cl_parse("asid=tiny\r\n", 11, &s);
    expect_long("asid=tiny", s.v[CL_ASID], 2);
    cl_parse("asid=on asid=tiny", 17, &s);
    expect_long("tiny wins over on", s.v[CL_ASID], 2);
    cl_parse("serial=on asid=only", 19, &s);
    expect_long("asid=only is no token", s.v[CL_ASID], 0);
    cl_parse("emmc=COMPAT cpuclk=FIRMWARE input=aesx xinput=aes", 49, &s);
    expect_long("case matters (emmc)", s.v[CL_SDMODE], 0);
    expect_long("case matters (cpuclk)", s.v[CL_CPUCLK], 1);
    expect_long("input=aesx is no token", s.v[CL_INPUT], 1);
    cl_parse("\temmc=compat\tcpuclk=firmware\finput=aes", 38, &s);
    expect_long("tab-separated emmc", s.v[CL_SDMODE], 1);
    expect_long("cpuclk=firmware", s.v[CL_CPUCLK], 0);
    expect_long("form-feed-separated input=aes", s.v[CL_INPUT], 0);
    cl_parse("serial=on bcache=8192", 21, &s);
    expect_long("bcache=8192", s.v[CL_BCACHE], 8192);
    cl_parse("bcache=0\r\n", 10, &s);
    expect_long("bcache=0", s.v[CL_BCACHE], 0);
    cl_parse("bcache=262144", 13, &s);
    expect_long("bcache=max", s.v[CL_BCACHE], 262144);
    cl_parse("bcache=262145", 13, &s);
    expect_long("bcache above max -> default", s.v[CL_BCACHE], 32768);
    cl_parse("bcache=99999999999", 18, &s);
    expect_long("bcache 11 digits -> default", s.v[CL_BCACHE], 32768);
    cl_parse("bcache=", 7, &s);
    expect_long("bcache= empty -> default", s.v[CL_BCACHE], 32768);
    cl_parse("bcache=8M", 9, &s);
    expect_long("bcache=8M -> default", s.v[CL_BCACHE], 32768);
    cl_parse("bcache=-1", 9, &s);
    expect_long("bcache=-1 -> default", s.v[CL_BCACHE], 32768);
    cl_parse("bcache=16 bcache=32", 19, &s);
    expect_long("first bcache decides", s.v[CL_BCACHE], 16);
    cl_parse("bcache=x bcache=32", 18, &s);
    expect_long("first bcache decides even invalid", s.v[CL_BCACHE], 32768);
    cl_parse("bcachepct=20 bcache=4096", 24, &s);
    expect_long("bcachepct is not bcache", s.v[CL_BCACHE], 4096);
    expect_long("bcachepct=20", s.v[CL_BCACHEPCT], 20);
    cl_parse("bcachepct=101", 13, &s);
    expect_long("bcachepct above 100 -> default", s.v[CL_BCACHEPCT], 5);
    cl_parse("serial=on\0memprot=off", 21, &s);
    expect_long("text ends at NUL", s.v[CL_MEMPROT], 1);

    /* --- merge -------------------------------------------------------------- */
    cl_defaults(&s);
    merge_case("missing file, defaults -> empty line", NULL, &s, "\n");
    merge_case("empty file, defaults -> empty line", "", &s, "\n");
    merge_case("blank file, defaults -> empty line", "  \r\n", &s, "\r\n");

    s.v[CL_SERIAL] = 1;
    s.v[CL_BCACHE] = 8192;
    merge_case("missing file, two settings appended in table order", NULL, &s,
               "bcache=8192 serial=on\n");

    cl_defaults(&s);
    merge_case("unknown tokens kept, defaults removed",
               "console=serial0,115200 memprot=strict quiet bcache=32768 rootwait\n",
               &s, "console=serial0,115200 quiet rootwait\n");

    cl_parse("dwc_otg.lpm_enable=0 memprot=off quiet", 38, &s);
    s.v[CL_MEMPROT] = 2;
    merge_case("managed token replaced in place",
               "dwc_otg.lpm_enable=0 memprot=off quiet", &s,
               "dwc_otg.lpm_enable=0 memprot=strict quiet\n");

    cl_parse("a serial=on b bcache=0 c", 24, &s);
    s.v[CL_BCACHE] = 65536;
    s.v[CL_ASID] = 1;
    merge_case("replace in place, new setting appended, CRLF kept",
               "a serial=on b bcache=0 c\r\n", &s,
               "a serial=on b bcache=65536 c asid=on\r\n");

    cl_parse("memprot=strict x memprot=off y", 30, &s);
    s.v[CL_MEMPROT] = 2;
    merge_case("duplicates collapse to the first position",
               "memprot=strict x memprot=off y", &s,
               "memprot=strict x y\n");

    cl_parse("bcache=8M bcache=16 z", 21, &s);
    merge_case("invalid first bcache (= default) removed with the rest",
               "bcache=8M bcache=16 z", &s, "z\n");

    cl_defaults(&s);
    merge_case("unknown look-alikes and memprot=verify kept",
               "cpuclk=max memprot=verify serial=off emmc=fast input=gate", &s,
               "cpuclk=max memprot=verify serial=off emmc=fast input=gate\n");

    cl_defaults(&s);
    s.v[CL_CPUCLK] = 0;
    s.v[CL_INPUT] = 0;
    s.v[CL_SDMODE] = 1;
    s.v[CL_BCACHEPCT] = 25;
    merge_case("trailing blanks and tabs, multi-line, LF",
               "  one\ttwo   \nthree  \n\n", &s,
               "one two three emmc=compat cpuclk=firmware bcachepct=25 input=aes\n");

    cl_defaults(&s);
    s.v[CL_BCACHE] = 0;
    merge_case("bcache off is written as bcache=0", "serial=on", &s,
               "bcache=0\n");

    {
        char small[8];
        cl_defaults(&s);
        expect_long("too small a buffer -> -1",
                    cl_merge("console=tty1 quiet", 18, &s, small, 8), -1);
        expect_long("exact fit", cl_merge("abcdef", 6, &s, small, 7), 7);
    }

    roundtrip("roundtrip 1", "serial=on memprot=strict bcache=8192 bcachepct=20 x");
    roundtrip("roundtrip 2", "asid=on asid=tiny emmc=compat cpuclk=firmware input=aes\r\n");
    roundtrip("roundtrip 3", "bcache=junk bcachepct=1000 memprot=off memprot=strict");
    roundtrip("roundtrip 4", "");

    printf("sysacc-cmdline-host: %d checks, %d failed\n", checks, fails);
    return fails ? 1 : 0;
}
