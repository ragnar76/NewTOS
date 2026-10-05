/*
 * mpkid.c -- the child of the ARCH03 stage-3b memory-protection probe
 * (driver: mprot3b.c). The command tail is "<case> <8 hex digits>": the
 * case selects what the child does with the address (a block of its
 * parent's, allocated with a given protection mode):
 *
 *   p  read the parent's PROT_P block      P  write it
 *   g  read + write the parent's PROT_G block
 *   r  read the parent's PROT_PR block     R  write it
 *   t  read + write the parent's PROT_P block (started as a Pexec 4 thread)
 *   f  MEM04: Malloc a page, write, Mfree it, write it again
 *   .  nothing
 *
 * Exit code: 0 = the access went through and read the parent's pattern,
 * 2 = it read something else. A refused access does not come back: the
 * EmuTOS crash report, a key, Pterm(-1) -- the parent sees 0x0000FFFF.
 *
 * Build: sdk/build.sh sdk/examples/mpkid.c <out>/MPKID.NAP
 */
#include <newtos.h>

#define P_CMDLIN 0x80
#define PATTERN 0x50415254UL /* "PART" */
#define MARK 0x4B494431UL    /* "KID1" */

typedef unsigned long UL;

static const unsigned char *own_basepage(void)
{
    UL sysbase = *(volatile unsigned int *)0x4F2UL;
    UL run_cell = *(volatile unsigned int *)(sysbase + 0x28);
    return (const unsigned char *)(UL)*(volatile unsigned int *)run_cell;
}

static UL hex8(const unsigned char *s)
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

int main(void)
{
    const unsigned char *tail = own_basepage() + P_CMDLIN;
    char c = (char)tail[1];
    volatile unsigned int *a = (volatile unsigned int *)hex8(tail + 3);
    UL v;

    switch (c) {
    case 'p':
    case 'r':
        v = *a;
        Cconws("MPKID: read done\r\n");
        Pterm(v == PATTERN ? 0 : 2);
    case 'P':
    case 'R':
        *a = (unsigned int)MARK;
        Cconws("MPKID: write done\r\n");
        Pterm(0);
    case 'g':
    case 't':
        v = *a;
        *a = (unsigned int)MARK;
        Cconws("MPKID: read+write done\r\n");
        Pterm(v == PATTERN ? 0 : 2);
    case 'f': {
        volatile unsigned int *m = (volatile unsigned int *)gemdos1(0x48, 4096); /* Malloc */
        if (!m)
            Pterm(3);
        *m = (unsigned int)MARK;
        gemdos1(0x49, (NLONG)m); /* Mfree */
        Cconws("MPKID: freed, writing again\r\n");
        *m = (unsigned int)MARK;
        Cconws("MPKID: write after Mfree went through\r\n");
        Pterm(0);
    }
    default:
        Pterm(0);
    }
}
