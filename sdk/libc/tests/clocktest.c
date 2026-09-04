/*
 * clocktest.c -- proof that the NewTOS 200 Hz system tick is live at EL0.
 *
 * Runs as a console program (`.NCP`, e.g. from C:\AUTO\) and measures the
 * tick THROUGH mintlib's own, unmodified time/clock.c -- which reads the
 * classic Atari system variable `_hz_200` ($4ba) via get_sysvar(). If the
 * numbers below are right, then:
 *
 *   - the kernel's ARM-generic-timer interrupt runs at 200 Hz
 *     (kernel/src/drivers/timer.rs),
 *   - it keeps the running process's low-memory copy of $4ba fresh
 *     (tos-api/src/bios/tick.rs `publish`),
 *   - and the pristine mintlib clock() sees it (sdk/libc/newtos-arch/
 *     getsysvar.c is the only AArch64 piece involved).
 *
 * Measurement method. The reference clock must be independent of clock(),
 * so it is GEMDOS Tgettime (0x2c). Tgettime packs seconds as sec/2, i.e. its
 * value changes exactly every 2 s; the test aligns on one such transition,
 * then counts EDGES rather than sampling a coarse value, so the window is an
 * exact multiple of 2 s and the expected tick count is exact:
 *
 *     edges * 2 s * CLOCKS_PER_SEC (200)  ==  clock() delta
 *
 * The polling loop hammers a GEMDOS trap, which runs with IRQs masked -- so
 * this also exercises the driver's "credit every elapsed period" catch-up
 * (a plain increment-per-interrupt tick would lose time here).
 */

#include <stdio.h>
#include <time.h>
#include <mint/osbind.h>

/* Classic low-memory address of the 200 Hz counter (mint/sysvars.h:83). Read
 * raw as well, so the output shows the sysvar cell itself and not only what
 * clock() made of it. */
#define HZ_200_ADDR 0x4baUL

/* Number of 2 s Tgettime edges to measure over. */
#define EDGES 5

static unsigned long
raw_hz200 (void)
{
	return (unsigned long) *(volatile unsigned int *) HZ_200_ADDR;
}

/* Spin until Tgettime() differs from `from`; returns the new value. */
static unsigned
wait_edge (unsigned from)
{
	unsigned t;

	do
		t = (unsigned) Tgettime ();
	while (t == from);

	return t;
}

int
main (void)
{
	unsigned long r0, r1;
	unsigned t;
	clock_t c0, c1, delta, expect;
	long err;
	int i;

	printf ("clocktest: CLOCKS_PER_SEC = %ld\n", (long) CLOCKS_PER_SEC);
	printf ("clocktest: clock() at entry = %ld\n", (long) clock ());

	/* 1. The raw sysvar cell advances at all. */
	r0 = raw_hz200 ();
	t = (unsigned) Tgettime ();
	wait_edge (t);				/* >= one full 2 s window */
	r1 = raw_hz200 ();
	printf ("clocktest: $4ba %lu -> %lu (+%lu)\n", r0, r1, r1 - r0);

	/* 2. Rate, measured over an exact EDGES*2 s window. */
	t = wait_edge ((unsigned) Tgettime ());
	c0 = clock ();
	for (i = 0; i < EDGES; i++)
		t = wait_edge (t);
	c1 = clock ();

	delta = c1 - c0;
	expect = (clock_t) EDGES * 2 * CLOCKS_PER_SEC;
	err = (long) delta - (long) expect;

	printf ("clocktest: %d s window: clock() +%ld, expected %ld, error %ld tick(s)\n",
		EDGES * 2, (long) delta, (long) expect, err);
	printf ("clocktest: %s\n",
		(err > -3 && err < 3) ? "PASS (200 Hz tick live)" : "FAIL");

	fflush (stdout);
	return 0;
}
