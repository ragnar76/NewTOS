/*
 * gem.h -- workstation handling (spec section 5).
 *
 * "Open with v_opnvwk on the handle returned by graf_handle.  This is what
 * real applications do, and matching application behaviour is the point."
 *
 * Portable core code: it builds the GEM parameter arrays and hands them to the
 * ABI backend.  Both targets run exactly this sequence.
 */
#ifndef TOSDUMP_GEM_H
#define TOSDUMP_GEM_H

#include "record.h"

#define TD_WORK_IN_N   11
#define TD_WORK_OUT_N  45
#define TD_WORK_PTS_N  12

struct td_wk {
	int16_t ap_id;                       /* appl_init                     */
	int16_t phys_handle;                 /* graf_handle                   */
	int16_t handle;                      /* v_opnvwk                      */
	int16_t work_in[TD_WORK_IN_N];
	int16_t work_out[TD_WORK_OUT_N];     /* DEV_TAB                       */
	int16_t work_pts[TD_WORK_PTS_N];     /* SIZ_TAB                       */
};

#define TD_GEM_ERR_APPL_INIT    (-1)
#define TD_GEM_ERR_GRAF_HANDLE  (-2)
#define TD_GEM_ERR_OPNVWK       (-3)

int  td_gem_open(struct td_wk *wk);
void td_gem_close(struct td_wk *wk);

/*
 * The application's live AESGLOBAL array (TD_AES_GLOBAL_N words).
 *
 * appl_init writes it (EmuTOS aes/gemsuper.c:92-98) and rs_load / rs_free /
 * rs_gaddr / rs_saddr read it back (gemsuper.c:363-373), so it is live AES
 * state for the whole run and there is exactly one of it -- the same array
 * appl_init filled.  The AES runner passes it to every call and records it
 * afterwards; it must never be poisoned or replaced by a copy.
 */
int16_t *td_gem_global(void);

const char *td_gem_error_text(int rc);

#endif /* TOSDUMP_GEM_H */
