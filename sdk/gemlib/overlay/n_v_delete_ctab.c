/*
 * n_v_delete_ctab.c -- gemlib v_delete_ctab() binding, retargeted for
 * NewTOS/AArch64.
 *
 * NewTOS overlay (sdk/gemlib/overlay/n_v_delete_ctab.c, TASK/T04 class 2):
 * the pristine binding (gemlib-0.44.0/n_v_delete_ctab.c:23-25) hands the
 * COLOR_TAB pointer to the VDI as the intin array itself,
 *
 *      VDI_PARAMS(vdi_control, (short*)&ctab, 0L, vdi_intout, vdi_dummy);
 *      VDI_TRAP_ESC (vdi_params, handle, 206,9, 0,2);
 *
 * and declares TWO intin words for it -- the m68k count, where a pointer is
 * 4 bytes. On AArch64 LP64 the bytes at &ctab are the 8-byte pointer, which
 * is the layout the NewTOS contract asks for (a pointer in a WORD array
 * occupies the caller's own pointer width, docs/COMPATIBILITY.md T04/A81),
 * but the declared count stays 2: contrl[3] under-declares the pointer by
 * N_PTRINTS - 2 words. The count is the one thing the binding gets wrong;
 * this overlay declares N_PTRINTS (sizeof(void *) / sizeof(short), 4 on
 * LP64, 2 on the m68k) the way the sibling bindings that pack a pointer
 * already do (n_v_create_itab.c:22-30 `3 + N_PTRINTS`).
 *
 * Everything else is the pristine gemlib text.
 */
#include "gem_vdiP.h"
#include "mt_gemx.h"

/** 
 *
 *  @param handle Device handle
 *  @param ctab reference to color table
 *
 *  @return 0=error, 1=all ok
 *
 *  @since NVDI 5 ?
 *
 *
 *
 */

short
v_delete_ctab (short handle, COLOR_TAB * ctab)
{
	short vdi_control[VDI_CNTRLMAX]; 
	short vdi_intout[1]; 

	VDI_PARAMS(vdi_control, (short*)&ctab, 0L, vdi_intout, vdi_dummy);
	
	VDI_TRAP_ESC (vdi_params, handle, 206,9, 0,N_PTRINTS);

	return vdi_intout[0];
}
