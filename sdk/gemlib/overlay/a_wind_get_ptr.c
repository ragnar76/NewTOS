/*
 * a_wind_get_ptr.c -- gemlib wind_get_ptr() binding, retargeted for
 * NewTOS/AArch64.
 *
 * NewTOS overlay (sdk/gemlib/overlay/a_wind_get_ptr.c, TASK/T04 class 3c):
 * the pristine binding (gemlib-0.44.0/a_wind_get_ptr.c:51) returns the
 * pointer-class answer of wind_get() with one read at the caller's pointer
 * width,
 *
 *      *v = aes_intout_ptr(1, void *);
 *
 * four bytes on the m68k, int_out[1..2]. That is right for every field the
 * AES answers with a pointer (WF_NEWDESK, WF_TOOLBAR, WF_USER_POINTER,
 * WF_WIND_ATTACH: a pointer in a WORD array occupies the caller's own
 * pointer width, docs/COMPATIBILITY.md T04/A81) -- but WF_SCREEN is not a
 * pointer field. EmuTOS answers it with two 32-bit LONGs, the buffer
 * address in int_out[1..2] and its length in int_out[3..4]
 * (`gsx_mret((LONG *)poutwds, (LONG *)(poutwds + 2))`, aes/gemwmlib.c:1564,
 * gemgsxif.c:125-129), and NewTOS writes the same two 32-bit values at every
 * caller width (tos-api/src/aes/mod.rs Aespb::set_intout_long, A57). On
 * LP64 the 8-byte read at &int_out[1] therefore spans BOTH LONGs and hands
 * the caller `address | length << 32` -- not the address
 * (tos-api/tests/aes_t04_intin_long_contract.rs pins that byte pattern).
 *
 * This overlay reads WF_SCREEN as the 32-bit LONG it is (aes_intout_long,
 * gem_aesP.h) and zero-extends it to a pointer, the 68k widening of an
 * address held in a LONG; every other field keeps the pointer-width read.
 *
 * Everything else is the pristine gemlib text.
 */
#include "gem_aesP.h"

/** returns various information about a window.
 *
 *  @param WindowHandle specifies the handle of the window to return
 *         information about (0 is the desktop window)
 *  @param What specifies the information to return and the value
 *         placed into \a v.
 *  @param v
 *  @param global_aes global AES array
 *
 *  see mt_wind_get() documentation for more details.
 */

short 
mt_wind_get_ptr (short WindowHandle, short What, void **v, short *global_aes)
{
	short *ptr;
	short aes_control[AES_CTRLMAX],
	      aes_intin[AES_INTINMAX],
		  aes_intout[AES_INTOUTMAX];
	long  aes_addrin[AES_ADDRINMAX], 
	      aes_addrout[AES_ADDROUTMAX];
	AESPB aes_params;

	aes_params.control = &aes_control[0];				/* AES Control Array */
	aes_params.global  = &global_aes[0];				/* AES Global Array */
	aes_params.intin   = &aes_intin[0];				/* input integer array */
	aes_params.intout  = &aes_intout[0];				/* output integer array */
	aes_params.addrin  = &aes_addrin[0];				/* input address array */
	aes_params.addrout = &aes_addrout[0];				/* output address array */

	aes_intin[0] = WindowHandle;
	aes_intin[1] = What;

	ptr = aes_control;
	*(ptr ++) = 104;									/* aes_control[0] */

	*(ptr ++) = 2;										/* aes_control[1] */
	*(ptr ++) = 5;										/* aes_control[2] */
	*(ptr ++) = 0;										/* aes_control[3] */
	*(ptr ++) = 0;				 						/* aes_control[4] */

	/* ol: this line is required for WF_FIRSTXYWH and WF_NEXTXYWH because
	   lot of programmers doesn't verify the return value and espect W or H
	   will be 0 it's not true for NAES */
	aes_intout[3] = aes_intout[4] = 0;

	AES_TRAP(aes_params);
	
	if (What == WF_SCREEN)
		/* a 32-bit LONG (the buffer address), not a pointer: int_out[1..2] */
		*v = (void *)(unsigned long)(unsigned int)aes_intout_long(1);
	else
		*v = aes_intout_ptr(1, void *);

	return (aes_intout[0]);
}
