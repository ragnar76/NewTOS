/*
 * a_evnt_timer.c -- gemlib evnt_timer() binding, retargeted for NewTOS/AArch64.
 *
 * NewTOS overlay (sdk/gemlib/overlay/a_evnt_timer.c, TASK/H03): the pristine
 * binding (gemlib-0.44.0/a_evnt_timer.c:27-32) splits the `unsigned long`
 * millisecond count into the two AES words by pointer punning,
 *
 *      unsigned short *i = (unsigned short *)&interval;
 *      aes_intin[0] = i[1];
 *      aes_intin[1] = i[0];
 *
 * which is only correct on the m68k, where `long` is 32 bits and big-endian:
 * i[0] is the HIGH word, i[1] the LOW word, so int_in[0] = lo, int_in[1] = hi,
 * exactly what the AES defines (EmuTOS aes/gemsuper.c:142-143,
 * `ev_timer(MAKE_ULONG(T_HICOUNT, T_LOCOUNT))`, with T_LOCOUNT = int_in[0] and
 * T_HICOUNT = int_in[1] in include/aesdefs.h:190-191). On AArch64 LP64 `long`
 * is 64 bits and little-endian, so i[0] is the LOW word and i[1] bits 16..31,
 * and the punned split arrives at the AES with the halves SWAPPED:
 * evnt_timer(150) became int_in[0] = 0, int_in[1] = 150, i.e. a request for
 * 150 << 16 = 9 830 400 ms (2 h 44 min). That is the TeraDesk hang of
 * TASK/H03: its wait(150) before the Quit dialog never returned.
 *
 * The NewTOS AES reads the words the same way EmuTOS does
 * (tos-api/src/aes/event.rs, evnt_timer: lo = int_in[0], hi = int_in[1]) and
 * is untouched. The fix belongs to the binding: derive the two words
 * arithmetically, as gemlib's own mt_evnt_multi() already does for the very
 * same interval (gemlib-0.44.0/a_evnt_multi.c:91-92, `interval` and
 * `interval >> 16`). That form is correct on every width and byte order, so it
 * needs no target guard. Same defect class as commit 065270b (BFOBSPEC).
 *
 * Everything else is the pristine gemlib text.
 */
#include "gem_aesP.h"

/** releases control to the operating system until
 *  a specified amount of time has passed.
 *
 *  @param interval time value specified in milliseconds.
 *  @param global_aes global AES array
 *
 *  @return reserved and is currently always 1.
 *
 *  @since All AES versions. Under TOS 1.0, calling this
 *         function from a desk accessory with \a interval
 *         having a value of 0 will hang the system.
 *
 *  @sa mt_evnt_multi()
 *
 *  This function should not be relyed on as an accurate clock.
 *  The time specified is used as a minimum time value only and
 *  the function will return at some point after that duration
 *  has passed.
 *
 */

short
mt_evnt_timer (unsigned long interval, short *global_aes)
{
	AES_PARAMS(24,2,1,0,0);

	aes_intin[0] = (short)(interval & 0xFFFF);	/* T_LOCOUNT: low word  */
	aes_intin[1] = (short)(interval >> 16);		/* T_HICOUNT: high word */

	AES_TRAP(aes_params);

	return (aes_intout[0]);
}
