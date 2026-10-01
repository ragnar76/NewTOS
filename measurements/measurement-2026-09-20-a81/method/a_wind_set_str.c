/*
 * a_wind_set_str.c -- gemlib wind_set_str() binding, retargeted for
 * NewTOS/AArch64.
 *
 * NewTOS overlay (sdk/gemlib/overlay/a_wind_set_str.c, TASK/A81): the
 * pristine binding (gemlib-0.44.0/a_wind_set_str.c:25-26) packs the string
 * pointer into the int_in WORD array and zeroes a second pointer-sized slot
 * behind it,
 *
 *      *(const char**)(aes_intin +2) = str;
 *      *(const char**)(aes_intin +4) = 0;
 *
 * the m68k layout (4-byte pointer in int_in[2..3], EmuTOS aes/gemwmlib.c:
 * 1660-1668 reads `*(char **)pinwds` with pinwds = &int_in[2] for WF_NAME /
 * WF_INFO). On AArch64 LP64 both stores are EIGHT bytes: the pointer spans
 * int_in[2..5] and the zero store spans int_in[4..7], overwriting bits
 * 32..63 of the pointer just written. That was harmless only because every
 * EL0 address currently lies under 4 GiB (its upper half is already 0), not
 * because the overlap is benign -- see the sibling a_wind_set_ptr_int.c,
 * where the same slot carries a real value.
 *
 * NewTOS's wind_set accepts the pointer in addr_in[0] whenever control[3]
 * (AIN_LEN) is >= 1 (docs/api/aes/window-graphics.md, "Pointers in
 * `addr_in[]`"; tos-api/src/aes/window.rs, wf_str_ptr). This overlay uses
 * that width-independent native form: num_addrin 1, the pointer in
 * aes_addrin[0], the declared int_in words after the field zero.
 *
 * Everything else is the pristine gemlib text.
 */
#include "gem_aesP.h"

/** sets various window attributes.
 *
 *  @param WindowHandle specifies the window handle of the window to modify.
 *  @param What specifies the attribute to change
 *  @param str a string
 *  @param global_aes global AES array
 *
 *  see mt_wind_set() documentation for more details.
 *
 *  the \a str pointer is sent in \a addrin[0] (NewTOS native convention; the
 *  m68k binding packs it over the two first parameters \a w1 and \a w2 of
 *  mt_wind_set()). Parameters \a w1 to \a w4 of mt_wind_set() are sent as 0.
 */

short 
mt_wind_set_str (short WindowHandle, short What, const char *str, short *global_aes)
{
	AES_PARAMS(105,6,1,1,0);

	aes_intin[0]                  = WindowHandle;
	aes_intin[1]                  = What;
	aes_intin[2]                  = 0;
	aes_intin[3]                  = 0;
	aes_intin[4]                  = 0;
	aes_intin[5]                  = 0;

	aes_addrin[0]                 = (long)str;
	
	AES_TRAP(aes_params);

	return (aes_intout[0]);
}
