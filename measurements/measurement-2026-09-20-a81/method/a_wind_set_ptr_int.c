/*
 * a_wind_set_ptr_int.c -- gemlib wind_set_ptr_int() binding, retargeted for
 * NewTOS/AArch64.
 *
 * NewTOS overlay (sdk/gemlib/overlay/a_wind_set_ptr_int.c, TASK/A81): the
 * pristine binding (gemlib-0.44.0/a_wind_set_ptr_int.c:27-28) packs the
 * pointer into the int_in WORD array and puts the extra word behind it,
 *
 *      *(void**)(aes_intin +2) = v;
 *      aes_intin[4]            = W3;
 *
 * which is the m68k layout: a 4-byte pointer in int_in[2..3] and W3 in
 * int_in[4], exactly what EmuTOS's wm_set(WF_NEWDESK) reads
 * (`gl_newdesk = *(OBJECT **)pinwds; gl_newroot = pinwds[2];`,
 * aes/gemwmlib.c:1685-1689, pinwds = &int_in[2]). On AArch64 LP64 the
 * pointer store is EIGHT bytes and spans int_in[2..5]; the W3 store then
 * lands on int_in[4] = bits 32..47 of the pointer just written, so a
 * non-zero W3 (a WF_NEWDESK root other than 0) silently corrupts the tree
 * pointer instead of reaching the AES as the root (TASK/A81.md: tree
 * 0x0000_0001_0177_E4C0 with root 2 arrives as 0x0000_0002_0177_E4C0).
 *
 * NewTOS's wind_set accepts the pointer in addr_in[0] -- the same slot every
 * other AES pointer-class argument uses (cf. a_menu_bar.c) -- with the extra
 * word in int_in[2] whenever control[3] (AIN_LEN) is >= 1
 * (docs/api/aes/window-graphics.md, "Pointers in `addr_in[]`";
 * tos-api/src/aes/window.rs, wf_str_ptr / set_newdesk). That native form is
 * width-independent, so this overlay uses it: num_addrin 1, the pointer in
 * aes_addrin[0], W3 in aes_intin[2], the remaining declared words zero. The
 * int_in words after index 2 are no longer written over by anything.
 *
 * Everything else is the pristine gemlib text.
 */
#include "gem_aesP.h"

/** sets various window attributes.
 *
 *  @param WindowHandle specifies the window handle of the window to modify.
 *  @param What specifies the attribute to change
 *  @param v a pointer
 *  @param W3 see hereafter
 *  @param global_aes global AES array
 *
 *  see mt_wind_set() documentation for more details
 *  
 *  the \a v pointer is sent in \a addrin[0] (NewTOS native convention; the
 *  m68k binding packs it over the two first parameters \a w1 and \a w2 of
 *  mt_wind_set()). The \a W3 value (16 bits) is sent in place of the first
 *  parameter \a w1 of mt_wind_set(), which is where the NewTOS AES reads the
 *  companion word of an \a addrin[0] pointer. Parameters \a w2, \a w3 and
 *  \a w4 of mt_wind_set() are sent as 0.
 */

short 
mt_wind_set_ptr_int (short WindowHandle, short What, void *v, short W3, short *global_aes)
{
	AES_PARAMS(105,6,1,1,0);

	aes_intin[0]                  = WindowHandle;
	aes_intin[1]                  = What;
	aes_intin[2]                  = W3;
	aes_intin[3]                  = 0;
	aes_intin[4]                  = 0;
	aes_intin[5]                  = 0;

	aes_addrin[0]                 = (long)v;

	AES_TRAP(aes_params);

	return (aes_intout[0]);
}
