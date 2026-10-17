/* rawtrap.c -- X01 check 4: raw svc, bypassing the osbind.h overlay, to show the KERNEL side is unchanged:
 *   GEMDOS 0x20 (Super)   -> EINVFN (-32) for arg 1L, 0L and a pointer
 *   XBIOS  0x26 (Supexec) -> 38 (= the function number), callee NOT run
 * Scratch probe, built with sdk/build.sh; not part of the product. */
#include <newtos.h>
static volatile long ran;
static long callee(void) { ran++; return 0x1234; }
static void hex(const char *tag, NLONG v)
{
    char b[48]; int i = 0; const char *t = tag;
    while (*t) b[i++] = *t++;
    b[i++] = '0'; b[i++] = 'x';
    for (int s = 28; s >= 0; s -= 4) b[i++] = "0123456789ABCDEF"[((unsigned long)v >> s) & 15];
    b[i++] = '\r'; b[i++] = '\n'; b[i] = 0;
    Cconws(b);
}
int main(void)
{
    hex("RAW Super(1L)    = ", gemdos1(0x20, 1));
    hex("RAW Super(0L)    = ", gemdos1(0x20, 0));
    hex("RAW Super(ptr)   = ", gemdos1(0x20, (NLONG)(unsigned long)&ran));
    hex("RAW Supexec(fn)  = ", xbios1(0x26, (NLONG)(unsigned long)&callee));
    hex("RAW callee ran   = ", ran);
    Cconws("RAW DONE\r\n");
    Pterm0();
}
