/*
 * a_wind_sget.c -- gemlib wind_sget() binding, retargeted for NewTOS/AArch64.
 *
 * NewTOS overlay (sdk/gemlib/overlay/a_wind_sget.c, TASK/T04 class 3c): the
 * pristine binding (gemlib-0.44.0/a_wind_sget.c:21-25) packs the caller's
 * string buffer pointer into the int_in WORD array and declares FOUR int_in
 * words for the call,
 *
 *      AES_PARAMS(104,4,1,0,0);
 *      aes_intin_ptr(2, char *) = str;
 *
 * the m68k count: two words of handle/field plus a 4-byte pointer. On
 * AArch64 LP64 the pointer is 8 bytes, int_in[2..5], but IN_LEN stays 4 --
 * the AES copies exactly IN_LEN words into its local buffer (EmuTOS
 * aes/gemsuper.c:436-437; tos-api/src/aes/mod.rs from_raw, A19) and would
 * see the pointer's upper half as 0. The layout itself is the NewTOS
 * contract (a pointer packed into a WORD array occupies the caller's own
 * pointer width, docs/COMPATIBILITY.md T04/A81); the count is what this
 * overlay fixes: 2 + N_PTRINTS (6 on LP64, 4 on the m68k, gem_aesP.h), the
 * same repair as a_wind_get.c's WF_NAME/WF_INFO case.
 *
 * Everything else is the pristine gemlib text.
 */
#include "gem_aesP.h"

/** returns the current window title or the current info line of a window.
 *
 *  @param WindowHandle specifies the handle of the window to return
 *         information about (0 is the desktop window)
 *  @param What specifies the information to return and the value
 *         placed into \a str. Can be WF_INFO or WF_NAME
 *  @param str is a pointer to a memory buffer where the window title or the
 *         current info line should be copied to
 *
 *  @param global_aes global AES array
 *
 *  see mt_wind_get() documentation for more details.
 */

short
mt_wind_sget (short WindowHandle, short What,
             char *str, short *global_aes)
{
	AES_PARAMS(104,2 + N_PTRINTS,1,0,0);

	aes_intin[0] = WindowHandle;
	aes_intin[1] = What;
	aes_intin_ptr(2, char *) = str;

	AES_TRAP(aes_params);

	return (aes_intout[0]);
}
