/*
 * n_v_delete_itab.c -- gemlib v_delete_itab() binding, retargeted for
 * NewTOS/AArch64.
 *
 * NewTOS overlay (sdk/gemlib/overlay/n_v_delete_itab.c, TASK/T04 class 2):
 * the pristine binding (gemlib-0.44.0/n_v_delete_itab.c:23-25) hands the
 * ITAB_REF pointer to the VDI as the intin array itself,
 *
 *      VDI_PARAMS(vdi_control, (short*)&itab, 0L, vdi_intout, vdi_dummy);
 *      VDI_TRAP_ESC (vdi_params, handle, 208,1, 0,2);
 *
 * and declares TWO intin words for it -- the m68k count, where a pointer is
 * 4 bytes. On AArch64 LP64 the bytes at &itab are the 8-byte pointer, which
 * is the layout the NewTOS contract asks for (a pointer in a WORD array
 * occupies the caller's own pointer width, docs/COMPATIBILITY.md T04/A81),
 * but the declared count stays 2: contrl[3] under-declares the pointer by
 * N_PTRINTS - 2 words. This overlay declares N_PTRINTS (sizeof(void *) /
 * sizeof(short), 4 on LP64, 2 on the m68k) the way the sibling bindings that
 * pack a pointer already do (n_v_create_itab.c:22-30 `3 + N_PTRINTS`).
 *
 * Everything else is the pristine gemlib text.
 */
#include "gem_vdiP.h"
#include "mt_gemx.h"

/** 
 *
 *  @param handle Device handle
 *  @param itab reference to inverse color table
 *
 *  @return 0=error, 1=all ok
 *
 *  @since NVDI 5 ?
 *
 *
 *
 */

short
v_delete_itab (short handle, ITAB_REF itab)
{
	short vdi_control[VDI_CNTRLMAX]; 
	short vdi_intout[1]; 

	VDI_PARAMS(vdi_control, (short*)&itab, 0L, vdi_intout, vdi_dummy);

	VDI_TRAP_ESC (vdi_params, handle, 208,1, 0,N_PTRINTS);

	return vdi_intout[0];
}
