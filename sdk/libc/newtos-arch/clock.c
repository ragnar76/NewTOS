/*
 * clock.c -- NewTOS clock(), replacing mintlib/time/clock.c.
 *
 * mintlib's clock() reads the Atari 200 Hz system-tick sysvar `_hz_200`
 * (address 0x4ba) via get_sysvar() (the m68k assembly mintlib/getsysvar.S).
 * On NewTOS/AArch64 there is no low-memory Atari sysvar map in a process TPA,
 * and Ssystem S_GETLVAL (the portable get_sysvar path) is not implemented by
 * the kernel (tos-api/src/gemdos/ssystem.rs only has S_{GET,SET,DEL}COOKIE),
 * so get_sysvar(_hz_200) has no analog. We therefore replace the whole file
 * (build-libc.sh excludes time/clock.c) rather than shim get_sysvar, which is
 * inherently an m68k-address read.
 *
 * mintlib's clock() is itself wall-time based: `get_sysvar(_hz_200) -
 * _starttime`, i.e. 200 Hz ticks of real elapsed time since the program
 * started (NOT CPU time). We reproduce exactly that contract on top of the
 * real NewTOS wall clock: GEMDOS Tgetdate (0x2A) + Tgettime (0x2C), which the
 * kernel backs with the monotonic ARM generic timer
 * (tos-api/src/gemdos/time.rs, CNTPCT_EL0). CLOCKS_PER_SEC is 200 on TOS, so we
 * scale seconds by 200.
 *
 * Resolution is 2 s: Tgettime packs seconds as sec/2 (bits 0-4). QED's only
 * clock() user is prn_out.c prn_check(), a printer-ready poll timeout
 * (`timer = 200 * wait_time + clock(); while (timer > clock()) ...`), for which
 * 2 s granularity is adequate. This is a real elapsed-time source, not a stub;
 * a finer user-readable monotonic tick would require a new kernel trap (noted
 * for the lead). Date is folded in so the value stays monotonic across
 * midnight.
 */

#include <time.h>
#include <mint/mintbind.h>	/* Tgetdate, Tgettime */
#include "libc-symbols.h"	/* strong_alias */

/* Days since the Unix epoch for a proleptic-Gregorian y/m/d (Hinnant's
 * days_from_civil); used only to make the tick monotonic across day/month/year
 * boundaries. Exact for all valid GEMDOS dates (1980-01-01 .. 2107-12-31). */
static long
days_from_epoch(int y, unsigned m, unsigned d)
{
	y -= m <= 2;
	long era = (y >= 0 ? y : y - 399) / 400;
	unsigned yoe = (unsigned)(y - era * 400);
	unsigned doy = (153u * (m + (m > 2 ? (unsigned)-3 : 9u)) + 2u) / 5u + d - 1u;
	unsigned doe = yoe * 365u + yoe / 4u - yoe / 100u + doy;
	return era * 146097L + (long)doe - 719468L;
}

static clock_t
now_ticks(void)
{
	unsigned dt = (unsigned) Tgetdate();
	unsigned tm = (unsigned) Tgettime();

	unsigned day   =  dt         & 0x1f;
	unsigned month = (dt >> 5)   & 0x0f;
	int      year  = 1980 + (int)((dt >> 9) & 0x7f);

	unsigned sec  = (tm         & 0x1f) * 2;
	unsigned min  = (tm >> 5)   & 0x3f;
	unsigned hour = (tm >> 11)  & 0x1f;

	long secs = days_from_epoch(year, month, day) * 86400L
	          + (long)hour * 3600L + (long)min * 60L + (long)sec;

	return (clock_t)(secs * 200L);	/* CLOCKS_PER_SEC == 200 on TOS */
}

static clock_t start_ticks;
static int      started;

clock_t
_clock(void)
{
	if (!started) {
		start_ticks = now_ticks();
		started = 1;
	}
	return now_ticks() - start_ticks;
}

strong_alias(_clock, clock)
