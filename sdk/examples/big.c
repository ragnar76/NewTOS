/*
 * big.c -- NewTOS multi-page-TPA proof.
 *
 * Deliberately larger than a single 4-KiB page so it exercises the multi-page
 * Transient Program Area the loader now hands out:
 *
 *   - an 8-KiB *initialized* table (.data) makes the on-disk `.NCP` and the
 *     mapped image span several pages;
 *   - a 32-KiB *uninitialized* array (.bss) forces multi-page BSS the loader
 *     must have zeroed;
 *   - deep recursion exercises the multi-page stack (well past one page);
 *   - a bump allocation over the free TPA (from the linker's `_end` up) uses
 *     the in-TPA heap.
 *
 * It prints each computed result via GEMDOS Cconws (0x09); crt0 then calls
 * Pterm (0x4C) with main's return value. A clean run + zero faults proves the
 * whole image + BSS + heap + stack live in one mapped multi-page TPA.
 */
#include <newtos.h>

/* 8 KiB of initialized data. REPn(x) emits n comma-separated copies of x, so
   the array is fully initialized (non-zero -> .data -> real file bytes) and
   `volatile` keeps the compiler from folding the fill loop away. */
#define REP4(x)    x, x, x, x
#define REP16(x)   REP4(x), REP4(x), REP4(x), REP4(x)
#define REP64(x)   REP16(x), REP16(x), REP16(x), REP16(x)
#define REP256(x)  REP64(x), REP64(x), REP64(x), REP64(x)
#define REP1024(x) REP256(x), REP256(x), REP256(x), REP256(x)
static volatile unsigned int table[2048] = {
    REP1024(0x00000001u), REP1024(0x00000002u)
};

/* 32 KiB of BSS (NOLOAD): the loader must zero it before main runs. */
static volatile unsigned int scratch[8192];

/* Linker-provided first free byte of the TPA: the start of the heap. */
extern char _end[];

/* Accumulator recursion to drive the stack many frames deep. Marked noinline
   so the call actually happens (the main prologue already uses the stack, but
   this pushes well past a single 4-KiB page). */
__attribute__((noinline)) static unsigned long sum_to(unsigned long n)
{
    if (n == 0)
        return 0;
    return n + sum_to(n - 1);
}

/* Minimal unsigned-decimal formatter (no libc in Stage 1). */
static char *utoa(unsigned long v, char *buf)
{
    char tmp[24];
    int i = 0;
    if (v == 0)
        tmp[i++] = '0';
    while (v) {
        tmp[i++] = (char)('0' + (v % 10u));
        v /= 10u;
    }
    int j = 0;
    while (i)
        buf[j++] = tmp[--i];
    buf[j] = '\0';
    return buf;
}

static void put_num(const char *label, unsigned long v)
{
    char num[24];
    Cconws(label);
    Cconws(utoa(v, num));
    Cconws("\r\n");
}

int main(void)
{
    unsigned long i;

    Cconws("BIG: multi-page TPA test\r\n");

    /* 1. Read the 8-KiB initialized table (spans multiple pages). */
    unsigned long checksum = 0;
    for (i = 0; i < 2048; i++)
        checksum += table[i];
    put_num("  table checksum = ", checksum); /* 1024*1 + 1024*2 = 3072 */

    /* 2. Write + read the 32-KiB BSS (loader zeroed it; we refill it). */
    unsigned long bss_sum = 0;
    for (i = 0; i < 8192; i++)
        scratch[i] = (unsigned int)(i * 3u);
    for (i = 0; i < 8192; i++)
        bss_sum += scratch[i];
    put_num("  scratch sum    = ", bss_sum);

    /* 3. Exercise the multi-page stack via deep recursion (8192 frames is
       several hundred KiB of stack — many pages past a single 4-KiB page). */
    put_num("  sum_to(8192)   = ", sum_to(8192)); /* 8192*8193/2 = 33558528 */

    /* 4. Use a heap in the free TPA: a 16-KiB bump region from _end. */
    volatile unsigned char *heap = (volatile unsigned char *)_end;
    unsigned long heap_sum = 0;
    for (i = 0; i < 16384; i++)
        heap[i] = (unsigned char)(i & 0xFFu);
    for (i = 0; i < 16384; i++)
        heap_sum += heap[i];
    put_num("  heap sum       = ", heap_sum);

    Cconws("BIG: done\r\n");
    return 0;
}
