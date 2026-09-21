/*
 * u_vqt_fontheader.c -- gemlib udef_vqt_fontheader() binding, retargeted for
 * NewTOS/AArch64.
 *
 * NewTOS overlay (sdk/gemlib/overlay/u_vqt_fontheader.c, TASK/T04 class 2):
 * the pristine binding (gemlib-0.44.0/u_vqt_fontheader.c:12-14) hands the
 * header buffer pointer to the VDI as the intin array itself,
 *
 *      VDI_PARAMS(_VDIParBlk.vdi_control, (short*)&buffer, 0L, ...);
 *      VDI_TRAP (vdi_params, handle, 232, 0,2);
 *
 * and declares TWO intin words for it -- the m68k count, where a pointer is
 * 4 bytes. On AArch64 LP64 the bytes at &buffer are the 8-byte pointer,
 * which is the layout the NewTOS contract asks for (a pointer in a WORD
 * array occupies the caller's own pointer width, docs/COMPATIBILITY.md
 * T04/A81), but the declared count stays 2: contrl[3] under-declares the
 * pointer by N_PTRINTS - 2 words. This overlay declares N_PTRINTS
 * (sizeof(void *) / sizeof(short), 4 on LP64, 2 on the m68k), the same fix
 * as the non-UDEF binding n_vqt_fontheader.c.
 *
 * Everything else is the pristine gemlib text.
 */
#include "gem_vdiP.h"
#include "mt_gemx.h"
#include "vdi_userdef.h"


/** UDEF version of vqt_fontheader(). See \ref overviewUDEF for details about UDEF feature
 */

void
udef_vqt_fontheader (short handle, char *buffer, char *pathname)
{
	VDI_PARAMS(_VDIParBlk.vdi_control, (short*)&buffer, 0L, _VDIParBlk.vdi_intout, vdi_dummy);
		
	VDI_TRAP (vdi_params, handle, 232, 0,N_PTRINTS);
	
	vdi_array2str (_VDIParBlk.vdi_intout, pathname, _VDIParBlk.vdi_control[4]);
}
