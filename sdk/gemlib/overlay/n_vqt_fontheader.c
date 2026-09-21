/*
 * n_vqt_fontheader.c -- gemlib vqt_fontheader() binding, retargeted for
 * NewTOS/AArch64.
 *
 * NewTOS overlay (sdk/gemlib/overlay/n_vqt_fontheader.c, TASK/T04 class 2):
 * the pristine binding (gemlib-0.44.0/n_vqt_fontheader.c:52-54) hands the
 * header buffer pointer to the VDI as the intin array itself,
 *
 *      VDI_PARAMS(vdi_control, (short*)&buffer, 0L, vdi_intout, vdi_dummy);
 *      VDI_TRAP (vdi_params, handle, 232, 0,2);
 *
 * and declares TWO intin words for it -- the m68k count, where a pointer is
 * 4 bytes. On AArch64 LP64 the bytes at &buffer are the 8-byte pointer,
 * which is the layout the NewTOS contract asks for (a pointer in a WORD
 * array occupies the caller's own pointer width, docs/COMPATIBILITY.md
 * T04/A81), but the declared count stays 2: contrl[3] under-declares the
 * pointer by N_PTRINTS - 2 words. This overlay declares N_PTRINTS
 * (sizeof(void *) / sizeof(short), 4 on LP64, 2 on the m68k) the way the
 * sibling bindings that pack a pointer already do (n_v_create_itab.c:22-30
 * `3 + N_PTRINTS`). The UDEF twin u_vqt_fontheader.c carries the same fix.
 *
 * Everything else is the pristine gemlib text.
 */
#include "gem_vdiP.h"
#include "mt_gemx.h"

/** copies the header of a speedo font file into \p buffer and 
 *  returns a pointer to the TDF file. The buffer should have a size of 1 Kb, 
 *  because the header size depends on the font and extensions of the font format.
 *
 *  @param handle Device handle
 *  @param buffer font header
 *  @param pathname name of the TDF
 *
 *  @since NVDI 3.00
 *
 *  If the selected (vector) font is not a Speedo font (TrueType, ...) the VDI 
 *  tries to create a virtual header with the important information.
 *  
 *  You should consider that each vqt_fontheader()-call can lead to several disk 
 *  accesses (depending on the font format) - if a user has installed 300 or more 
 *  fonts (which can happen very easily if you use high quality fonts with a lot 
 *  of font styles), this will take a few seconds. If necessary, save font 
 *  information and load (and update) it when your program is started.
 *
 *  Note:
 *  
 *  Usually the following elements of the font header are the most interesting 
 *  ones for programmers:
 *  
\verbatim
Name      Offset  Length  Description
FH_FNTNM    24     70     font name (siehe auch vqt_name()),
                          e.g. "Century 725 Italic BT"
FH_NKTKS   258      2     number of kerning tracks
FH_NKPRS   260      2     number of kerining pairs, (see vst_kern())
FH_CLFGS   263      1     classification (Italic and Monospace)
FH_SFNTN   266     32     name of the corresponding Postscript font,
                          e.g. "Century725BT-Italic"
FH_SFACN   298     16     short name of the family, e.g. "Century725 BT"
FH_FNTFM   314     14     style, e.g. "Italic"
FH_ITANG   328      2     skew in 1/256 of degrees (if it is an italic font),
                          e.g 4480 (17,5 degree)
FH_ORUPM   330      2     number of ORUs per em, e.g. 2048
\endverbatim
 *
 */

void
vqt_fontheader (short handle, char *buffer, char *pathname)
{
	short vdi_control[VDI_CNTRLMAX]; 
	short vdi_intout[VDI_INTOUTMAX]; 

	VDI_PARAMS(vdi_control, (short*)&buffer, 0L, vdi_intout, vdi_dummy);
		
	VDI_TRAP (vdi_params, handle, 232, 0,N_PTRINTS);
	
	vdi_array2str (vdi_intout, pathname, vdi_control[4]);
}
