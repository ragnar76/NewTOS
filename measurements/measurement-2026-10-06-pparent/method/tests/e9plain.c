/*
 * e9plain.c -- ARCH03 E9 probe (plain sdk/crt0.S, no libc): the start-up code
 * shrinks the largest-block TPA to image + MINKEEP stack (as mintlib's
 * _crtinit with its default _stksize), so the block cannot grow by a page
 * afterwards and Malloc(-1) reports the rest.
 */
#include <newtos.h>

extern char _end[];

static void puthex(unsigned long v)
{
	static const char d[] = "0123456789abcdef";
	char b[11];
	int i;
	b[0] = '0';
	b[1] = 'x';
	for (i = 0; i < 8; i++)
		b[2 + i] = d[(v >> ((7 - i) * 4)) & 0xf];
	b[10] = 0;
	Cconws(b);
}

static NLONG mshrink(unsigned long blk, unsigned long len)
{
	register NLONG x8 __asm__("x8") = 0x4A;
	register NLONG r0 __asm__("x0") = 0;
	register NLONG r1 __asm__("x1") = (NLONG)blk;
	register NLONG r2 __asm__("x2") = (NLONG)len;
	__asm__ volatile("svc #1" : "+r"(r0) : "r"(x8), "r"(r1), "r"(r2) : "memory");
	return r0;
}

int main(unsigned long bp)
{
	unsigned long hitpa = *(volatile unsigned int *)(bp + 4);
	unsigned long kept = (((unsigned long)_end - bp + 15) & ~15UL) + 0x10000;
	unsigned long sp = (unsigned long)&hitpa;
	NLONG same = mshrink(bp, kept), grow = mshrink(bp, kept + 0x1000);
	NLONG largest = gemdos1(0x48, -1);
	int ok = same == 0 && grow == -67 && sp > bp && sp < bp + kept && hitpa - bp > 0x400000;

	Cconws("[E9PLAIN] PD=");
	puthex(bp);
	Cconws(" TPA=");
	puthex(hitpa - bp);
	Cconws(" kept=");
	puthex(kept);
	Cconws(" Mshrink(kept)=");
	puthex((unsigned long)same);
	Cconws(" Mshrink(kept+4K)=");
	puthex((unsigned long)grow);
	Cconws(" Malloc(-1)=");
	puthex((unsigned long)largest);
	Cconws(ok ? "\r\nE9PLAIN: PASS\r\n" : "\r\nE9PLAIN: FAIL\r\n");
	return ok ? 0 : 1;
}
