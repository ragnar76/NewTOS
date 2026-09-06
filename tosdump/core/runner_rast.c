/*
 * runner_rast.c -- the VDI raster runner (spec sections 4, 5 and 7).
 */
#include <string.h>

#include "abi.h"
#include "runner_rast.h"

/*
 * The MFDB, in the layout the target's own compiler produces -- which IS the
 * ABI both VDIs expect, because both of them read the caller's struct.  On
 * the m68k that is a 4-byte fd_addr followed by eight WORDs at offsets
 * 4..18 (EmuTOS include/gsxdefs.h:65-76); on AArch64 an 8-byte fd_addr
 * followed by the same eight WORDs at 8..22, which is what NewTOS' LP64
 * branch reads (tos-api/src/vdi/raster.rs, Mfdb::from_ptr).  Writing the
 * struct out by hand at explicit offsets -- the rule the RECORD follows --
 * would be wrong here: this is not a file, it is a parameter, and the
 * parameter's layout is the target's.
 */
struct td_mfdb {
	void   *fd_addr;
	int16_t fd_w;
	int16_t fd_h;
	int16_t fd_wdwidth;
	int16_t fd_stand;
	int16_t fd_nplanes;
	int16_t fd_r1;
	int16_t fd_r2;
	int16_t fd_r3;
};

static struct td_raster_arrays rast_arrays;
static int16_t                 rast_before[TD_RAST_CONTRL_N];
static struct td_mfdb          rast_src;
static struct td_mfdb          rast_dst;

/*
 * Pack a pointer into the control array the way every GEM binding does:
 * pointer n at contrl[7 + n * N_PTRINTS], N_PTRINTS = sizeof(void *) /
 * sizeof(WORD) (gemlib gem_vdiP.h:177,204).  One expression, correct on
 * both targets because sizeof(void *) is what differs -- so this is core
 * code and not a third ABI backend.  memcpy rather than a cast, because
 * contrl[7] is only WORD-aligned and an 8-byte store through a void ** at
 * that address is undefined C even where the hardware allows it.
 */
static void set_mfdb_ptr(int16_t *contrl, int n, void *p)
{
	const int nptrints = (int)(sizeof(void *) / sizeof(int16_t));

	memcpy(&contrl[7 + n * nptrints], &p, sizeof p);
}

static void fill_mfdb(struct td_mfdb *m, const struct td_rast_mfdb *spec,
                      struct td_raster_arrays *a)
{
	memset(m, 0, sizeof *m);
	m->fd_addr    = (spec->buffer == TD_RAST_BUF_B) ? (void *)a->buf_b
	                                                : (void *)a->buf_a;
	m->fd_w       = spec->w;
	m->fd_h       = spec->h;
	m->fd_wdwidth = spec->wdwidth;
	m->fd_stand   = spec->stand;
	m->fd_nplanes = spec->nplanes;
	/* fd_r1..fd_r3 stay 0: they are reserved, and a case that found them
	 * changed afterwards would have found a real defect. */
}

/* The eight WORDs after fd_addr, read back out of an MFDB after the call --
 * vr_trnfm overwrites fd_stand (vdi_raster.c:303-314), so the MFDB is an
 * output as well as an input. */
static void mfdb_words(const struct td_mfdb *m, int16_t *out)
{
	out[0] = m->fd_w;
	out[1] = m->fd_h;
	out[2] = m->fd_wdwidth;
	out[3] = m->fd_stand;
	out[4] = m->fd_nplanes;
	out[5] = m->fd_r1;
	out[6] = m->fd_r2;
	out[7] = m->fd_r3;
}

int td_run_raster(const struct td_rast_case_table *ct, const struct td_wk *wk,
                  uint8_t *out, size_t out_bytes)
{
	int16_t src_words[TD_RAST_MFDB_N];
	int16_t dst_words[TD_RAST_MFDB_N];
	uint16_t i;
	int32_t ret;
	int j;

	if (out_bytes < (size_t)ct->count * TD_RAST_RECORD_SIZE)
		return TD_RUN_ERR_BUFFER;

	for (i = 0; i < ct->count; i++) {
		const struct td_rast_case *c = &ct->cases[i];
		uint8_t *rec = out + (size_t)i * TD_RAST_RECORD_SIZE;

		/* 1. reset the device state this case could otherwise inherit */
		td_reset_vdi(wk);

		/* 2. lay the two pixel buffers.  Not poison: see runner_rast.h */
		td_rast_fill(rast_arrays.buf_a, TD_RAST_BUF_WORDS, c->seed_a);
		td_rast_fill(rast_arrays.buf_b, TD_RAST_BUF_WORDS, c->seed_b);

		/* 3. the two MFDBs, over the buffers the case named */
		fill_mfdb(&rast_src, &c->src, &rast_arrays);
		fill_mfdb(&rast_dst, &c->dst, &rast_arrays);

		/* 4. poison every parameter array (spec section 4) */
		td_poison_raster(&rast_arrays);

		/* 5. lay the case values over the poison */
		for (j = 0; j < (int)c->n_intin; j++)
			rast_arrays.intin[j] = c->intin[j];
		for (j = 0; j < (int)c->n_ptsin; j++)
			rast_arrays.ptsin[j] = c->ptsin[j];

		/* 6. contrl: only the input slots.  contrl[2] and contrl[4] are
		 *    outputs and stay poisoned, so a VDI that leaves them alone
		 *    stays distinguishable from one that writes 0. */
		rast_arrays.contrl[0] = c->opcode;
		rast_arrays.contrl[1] = (int16_t)(c->n_ptsin / 2);   /* points */
		rast_arrays.contrl[3] = (int16_t)c->n_intin;
		rast_arrays.contrl[5] = 0;
		rast_arrays.contrl[6] = wk->handle;
		set_mfdb_ptr(rast_arrays.contrl, 0, &rast_src);
		set_mfdb_ptr(rast_arrays.contrl, 1, &rast_dst);

		/* 7. snapshot contrl as it stands going in */
		for (j = 0; j < TD_RAST_CONTRL_N; j++)
			rast_before[j] = rast_arrays.contrl[j];

		/* 8. the call under measurement */
		ret = td_abi_vdi(rast_arrays.contrl, rast_arrays.intin,
		                 rast_arrays.ptsin, rast_arrays.intout,
		                 rast_arrays.ptsout);

		/* 9. the record.  Input arrays and both MFDBs go in as they stand
		 *    AFTER the call: a raster op may modify neither, and dumping
		 *    them afterwards is what makes a violation visible -- except
		 *    for the destination MFDB's fd_stand, which vr_trnfm is
		 *    required to overwrite. */
		mfdb_words(&rast_src, src_words);
		mfdb_words(&rast_dst, dst_words);
		td_raster_record_write(rec, c->line, (uint16_t)c->opcode, ret,
		                       &rast_arrays, rast_before,
		                       src_words, dst_words);
	}

	return 0;
}
