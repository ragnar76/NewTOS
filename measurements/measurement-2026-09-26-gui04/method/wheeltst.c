/*
 * wheeltst.c -- GUI04 acceptance program: a GEM application that registers a
 * mouse-wheel routine with vex_wheelv() (VDI opcode 134) and reports every
 * wheel event that routine is handed.
 *
 * This is the proof that the CHAIN is served, not just the desktop: the
 * desktop's own scrolling goes through the AES (wheel_change -> WM_ARROWED),
 * which would still look right if the VDI vector were never called.  A
 * program sitting on vex_wheelv can only see something if
 *
 *   USB report byte 3  ->  IKBD scancode 0x59 / 0x5a
 *                      ->  mousexvec  ->  vdi_mousex_handler
 *                      ->  call_user_wheel  ->  THIS function
 *
 * is complete end to end.
 *
 * Everything is reported with Cconws, so a headless QEMU raspi4b run says on
 * the serial console exactly how far it got and what it saw.
 *
 * The handler is invoked as an EL0 upcall
 * (docs/analysis/coop-kernel/10-upcall-signal-design.md, consumer "vex"): the
 * kernel redirects this process's next return-to-EL0 into wheel_handler with
 * x0 = wheel_number, x1 = wheel_amount (AAPCS64) and the link register set to
 * the per-process trampoline, whose Psigreturn resumes the interrupted code.
 * That is why the main loop has to come back from a syscall regularly -- each
 * evnt_timer() return is a delivery point.  A wheel event therefore shows up
 * at most one timer period late; NewTOS has no lower-EL IRQ route that could
 * deliver it mid-instruction the way a real m68k ISR does.
 *
 * Built and launched exactly like measurement-2026-09-25-port05/method's two
 * programs: the QED/TeraDesk toolchain (build-wheel.sh copies the recipe) and
 * the `#Z` autorun line of NEWDESK.INF.
 */

#include <mint/osbind.h>   /* Cconws, Pterm */
#include <gem.h>           /* appl_init, graf_handle, v_opnvwk, vex_wheelv */

/* ---- what the wheel routine records ---------------------------------- */

static volatile short w_total;   /* every call */
static volatile short w_up;      /* number 0, amount < 0 */
static volatile short w_dn;      /* number 0, amount > 0 */
static volatile short w_lf;      /* number 1, amount < 0 */
static volatile short w_rt;      /* number 1, amount > 0 */
static volatile short w_number;  /* arguments of the most recent call */
static volatile short w_amount;

/*
 * The vex_wheelv routine.  gemlib documents the arguments as "d0.w contains
 * the wheel_number, d1.w the wheel_amount" (vex_wheelv.c); on AArch64 that is
 * w0 / w1, i.e. an ordinary two-argument function.
 */
static void wheel_handler(short number, short amount)
{
	w_number = number;
	w_amount = amount;
	w_total++;
	if (number == 0) {
		if (amount < 0)
			w_up++;
		else if (amount > 0)
			w_dn++;
	} else {
		if (amount < 0)
			w_lf++;
		else if (amount > 0)
			w_rt++;
	}
}

/* ---- tiny output helpers (no libc printf in this toolchain) ---------- */

static void puthex(unsigned long v)
{
	static const char d[] = "0123456789abcdef";
	char b[19];
	int i;

	b[0] = '0';
	b[1] = 'x';
	for (i = 0; i < 16; i++)
		b[2 + i] = d[(v >> ((15 - i) * 4)) & 0xf];
	b[18] = '\0';
	Cconws(b);
}

static void putdec(long v)
{
	char b[12];
	int i = 11;

	b[i] = '\0';
	if (v < 0) {
		Cconws("-");
		v = -v;
	}
	do {
		b[--i] = (char)('0' + (v % 10));
		v /= 10;
	} while (v && i > 0);
	Cconws(b + i);
}

/* ---- the program ------------------------------------------------------ */

#ifndef WHEEL_POLLS
#define WHEEL_POLLS 400        /* 400 * 50 ms = 20 s of wheel-watching */
#endif
#ifndef WHEEL_ENOUGH
#define WHEEL_ENOUGH 8         /* stop early once this many events arrived */
#endif

int main(void)
{
	short work_in[11], work_out[57];
	short handle, phys, wchar, hchar, wbox, hbox;
	void *old = (void *)0;
	void *back = (void *)0;
	short seen = 0;
	int i;

	Cconws("[WHL] 1 main entry\r\n");

	(void)appl_init();
	Cconws("[WHL] 2 appl_init done\r\n");

	phys = graf_handle(&wchar, &hchar, &wbox, &hbox);
	for (i = 0; i < 10; i++)
		work_in[i] = 1;
	work_in[10] = 2;                 /* RC coordinates */
	handle = phys;
	for (i = 0; i < 57; i++)
		work_out[i] = 0;
	v_opnvwk(work_in, &handle, work_out);
	Cconws("[WHL] 3 v_opnvwk handle=");
	putdec(handle);
	Cconws("\r\n");

	/* The line this whole measurement is about. */
	vex_wheelv(handle, (void *)wheel_handler, &old);
	Cconws("[WHL] 4 vex_wheelv installed, mine=");
	puthex((unsigned long)(void *)wheel_handler);
	Cconws(" old=");
	puthex((unsigned long)old);
	Cconws("\r\n");

	/*
	 * Watch.  Each evnt_timer() return is an upcall delivery point, so the
	 * loop both waits and collects.
	 */
	for (i = 0; i < WHEEL_POLLS && w_total < WHEEL_ENOUGH; i++) {
		(void)evnt_timer(50UL);
		if (w_total != seen) {
			seen = w_total;
			Cconws("[WHL] 5 wheel number=");
			putdec(w_number);
			Cconws(" amount=");
			putdec(w_amount);
			Cconws(" total=");
			putdec(seen);
			Cconws("\r\n");
		}
	}

	Cconws("[WHL] 6 summary up=");
	putdec(w_up);
	Cconws(" down=");
	putdec(w_dn);
	Cconws(" left=");
	putdec(w_lf);
	Cconws(" right=");
	putdec(w_rt);
	Cconws(" total=");
	putdec(w_total);
	Cconws("\r\n");

	/* Hand the vector back, the way a well-behaved program does. */
	vex_wheelv(handle, old, &back);
	Cconws("[WHL] 7 vex_wheelv restored, got back=");
	puthex((unsigned long)back);
	Cconws("\r\n");

	v_clsvwk(handle);
	Cconws("[WHL] 8 v_clsvwk done\r\n");

	/*
	 * No appl_exit(): with desk accessories loaded it blocks in
	 * wait_for_accs(AP_ACCLOSE), a separate pre-existing defect that has
	 * nothing to do with the wheel -- see measurement-2026-09-25-port05's
	 * pexkid.c header.  Pterm(0) directly, so the run ends cleanly.
	 */
	Cconws("[WHL] 9 Pterm(0)\r\n");
	Pterm(0);
	return 0;
}
