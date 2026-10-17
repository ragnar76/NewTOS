/* rstprobe.c -- RST01 check: the OSHEADER reset entry, reached the way TeraDesk's LP64-fixed "Reboot" reaches it.
 *   reads _sysbase ($4F2) as a 32-bit word, prints reseth (+0x04) and os_res1 (+0x10),
 *   clears memvalid/memval2 (32-bit, like the TeraDesk overlay) and calls Supexec(*(uint32_t *)(os_start + 0x10)).
 *   Expected on board-rpi4: no return -- "[reset] OSHEADER reset entry (Cold) requested" on serial, then the restart.
 * Scratch probe, built with sdk/libc/build-libc.sh (mintlib + the SDK's <mint/osbind.h> Supexec overlay); not product code. */
#include <stdint.h>
#include <mint/osbind.h>

static void hex(const char *tag, unsigned long v)
{
	char b[64]; int i = 0; const char *t = tag;
	while (*t) b[i++] = *t++;
	b[i++] = '0'; b[i++] = 'x';
	for (int s = 28; s >= 0; s -= 4) b[i++] = "0123456789ABCDEF"[(v >> s) & 15];
	b[i++] = '\r'; b[i++] = '\n'; b[i] = 0;
	Cconws(b);
}

int main(void)
{
	uint32_t os = *(volatile uint32_t *)0x4F2L;
	uint32_t reseth = *(volatile uint32_t *)(uintptr_t)(os + 0x04);
	uint32_t res1 = *(volatile uint32_t *)(uintptr_t)(os + 0x10);
	long rv;

	hex("RSTPROBE sysbase   = ", os);
	hex("RSTPROBE reseth    = ", reseth);
	hex("RSTPROBE os_res1   = ", res1);
	hex("RSTPROBE insn@res1 = ", *(volatile uint32_t *)(uintptr_t)res1);
	*(volatile uint32_t *)0x420L = 0;	/* memvalid */
	*(volatile uint32_t *)0x43AL = 0;	/* memval2 */
	Cconws("RSTPROBE calling Supexec(os_res1)\r\n");
	rv = Supexec((long (*)(void))(uintptr_t)res1);
	hex("RSTPROBE RETURNED rv = ", (unsigned long)rv);
	Pterm0();
	return 0;
}
