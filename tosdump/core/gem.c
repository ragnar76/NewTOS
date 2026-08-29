/*
 * gem.c -- workstation handling (spec section 5).
 *
 * Portable core code: it lays out the GEM parameter arrays and hands them to
 * the ABI backend.  Both targets run exactly this sequence, which is the
 * sequence a real GEM application runs -- appl_init, graf_handle, then
 * v_opnvwk on the handle graf_handle returned.
 *
 * Parameter counts are taken from gemlib's own callers, which are the
 * authoritative in-tree GEM clients:
 *   appl_init    a_appl_init.c:69    AES_PARAMS(10, 0, 1, 0, 0)
 *   appl_exit    a_appl_exit.c:18    AES_PARAMS(19, 0, 1, 0, 0)
 *   graf_handle  a_graf_handle.c:40  AES_PARAMS(77, 0, 5, 0, 0)
 *   v_opnvwk     v_opnvwk.c:26       VDI_TRAP(.., 100, sub 0, 0 ptsin, 11 intin)
 *   v_clsvwk     v_clsvwk.c:18       VDI_TRAP_00(.., 101)
 *
 * v_opnvwk returns DEV_TAB in intout[0..44] and SIZ_TAB in ptsout[0..11]
 * (EmuTOS vdi_control.c:344-352), which is where the three device-defined
 * attribute defaults come from -- see runner.c.
 */
#include <string.h>

#include "abi.h"
#include "gem.h"

/* AES parameter block arrays.  global[] is the AES's own; the AES writes it
 * on appl_init and reads it afterwards, so it lives for the whole run. */
static int16_t aes_control[5];
static int16_t aes_global[15];
static int16_t aes_int_in[16];
static int16_t aes_int_out[7];
static void   *aes_addr_in[3];
static void   *aes_addr_out[1];

static void aes_call(int16_t opcode, int16_t n_intin, int16_t n_intout,
                     int16_t n_addrin, int16_t n_addrout)
{
	aes_control[0] = opcode;
	aes_control[1] = n_intin;
	aes_control[2] = n_intout;
	aes_control[3] = n_addrin;
	aes_control[4] = n_addrout;
	td_abi_aes(aes_control, aes_global, aes_int_in, aes_int_out,
	           aes_addr_in, aes_addr_out);
}

/* VDI parameter block arrays for the workstation calls.  The runner has its
 * own set: these must never be the poisoned per-case arrays. */
static int16_t wk_contrl[TD_VDI_CONTRL_N];
static int16_t wk_intin[TD_VDI_INTIN_N];
static int16_t wk_ptsin[TD_VDI_PTSIN_N];
static int16_t wk_intout[TD_VDI_INTOUT_N];
static int16_t wk_ptsout[TD_VDI_PTSOUT_N];

static void vdi_call(int16_t opcode, int16_t n_ptsin, int16_t n_intin,
                     int16_t sub, int16_t handle)
{
	memset(wk_contrl, 0, sizeof wk_contrl);
	wk_contrl[0] = opcode;
	wk_contrl[1] = n_ptsin;
	wk_contrl[3] = n_intin;
	wk_contrl[5] = sub;
	wk_contrl[6] = handle;
	td_abi_vdi(wk_contrl, wk_intin, wk_ptsin, wk_intout, wk_ptsout);
}

int td_gem_open(struct td_wk *wk)
{
	int i;

	memset(wk, 0, sizeof *wk);

	/*
	 * The work_in every measurement is taken against, stated once and
	 * identical on both targets.  This is the classic GEM application
	 * open: every attribute index 1, raster coordinates.  EmuTOS' init_wk
	 * derives the open-time attribute state from exactly these values
	 * (vdi_control.c:269-311), so the reset sequence in runner.c restores
	 * the same ones.
	 */
	for (i = 0; i < 10; i++)
		wk->work_in[i] = 1;
	wk->work_in[10] = 2;              /* 2 = raster coordinates */

	aes_call(10, 0, 1, 0, 0);         /* appl_init */
	wk->ap_id = aes_int_out[0];
	if (wk->ap_id < 0)
		return TD_GEM_ERR_APPL_INIT;

	aes_call(77, 0, 5, 0, 0);         /* graf_handle */
	wk->phys_handle = aes_int_out[0];
	if (wk->phys_handle <= 0)
		return TD_GEM_ERR_GRAF_HANDLE;

	memset(wk_intin, 0, sizeof wk_intin);
	for (i = 0; i < TD_WORK_IN_N; i++)
		wk_intin[i] = wk->work_in[i];
	vdi_call(100, 0, TD_WORK_IN_N, 0, wk->phys_handle);   /* v_opnvwk */

	wk->handle = wk_contrl[6];
	if (wk->handle <= 0)
		return TD_GEM_ERR_OPNVWK;

	for (i = 0; i < TD_WORK_OUT_N; i++)
		wk->work_out[i] = wk_intout[i];
	for (i = 0; i < TD_WORK_PTS_N; i++)
		wk->work_pts[i] = wk_ptsout[i];

	return 0;
}

void td_gem_close(struct td_wk *wk)
{
	if (wk->handle > 0) {
		vdi_call(101, 0, 0, 0, wk->handle);   /* v_clsvwk */
		wk->handle = 0;
	}
	if (wk->ap_id >= 0) {
		aes_call(19, 0, 1, 0, 0);             /* appl_exit */
		wk->ap_id = -1;
	}
}

const char *td_gem_error_text(int rc)
{
	switch (rc) {
	case 0:                       return "ok";
	case TD_GEM_ERR_APPL_INIT:    return "appl_init failed";
	case TD_GEM_ERR_GRAF_HANDLE:  return "graf_handle returned no handle";
	case TD_GEM_ERR_OPNVWK:       return "v_opnvwk returned no handle";
	default:                      return "unknown error";
	}
}
