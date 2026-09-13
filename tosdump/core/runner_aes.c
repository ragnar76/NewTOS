/*
 * runner_aes.c -- poison, lay the case, call, record: the AES half
 * (spec sections 4 and 7).  See runner_aes.h for why there is no reset and
 * no "before" snapshot.
 */
#include <string.h>

#include "abi.h"
#include "runner_aes.h"

/*
 * The arrays one case is measured with.  Statics, like the VDI runner's:
 * the program runs on a TT with TOS 3.06 and on NewTOS alike, and neither
 * gets an allocator.
 */
static struct td_aes_arrays run_arrays;

/*
 * The addr_in / addr_out arrays the parameter block actually points at.
 *
 * These cannot be run_arrays.addr_in: the record's address slots are 32 bit
 * by format (spec section 3), while the parameter block's are the target's
 * pointer width -- four bytes on the m68k, eight on AArch64.  So the call
 * gets real pointer-width arrays and the record gets the low 32 bits of
 * each, through the same expression on both targets.  A table that names no
 * buffer never writes a pointer here at all, so the only thing that reaches
 * the record is the poison -- whose low 32 bits are the same NUMBER on both
 * targets, which is what keeps the two builds comparable.  See poison_ptrs
 * for why that is a number and not a byte pattern, and run_planted_buf below
 * for what a slot records once a case HAS pointed it at a buffer.
 */
static void *run_addr_in[TD_AES_ADDRIN_N];
static void *run_addr_out[TD_AES_ADDROUT_N];

/*
 * The buffers a case may point a call at (tosdump.h, TD_AES_BUF_N).
 *
 * Statics, like everything else here, and aligned for the widest thing a
 * target may read out of one: the union's `align` member is the machine's
 * own widest scalar, which is what a compiler aligns the whole array to.  A
 * char array would be byte-aligned, and a later case handing one of these to
 * a call that reads WORDs out of it would fault on a 68000.
 *
 * Laid fresh before EVERY call the runner issues, not once per case: a
 * buffer the previous call wrote into would carry that call's result into
 * the next one, and the point of the poison rule is that nothing does.
 */
static union {
	unsigned long align;
	char          bytes[TD_AES_BUF_BYTES];
} run_bufs[TD_AES_BUF_N];

/*
 * Which buffer, if any, each addr_in slot was pointed at for the call that
 * just ran -- and what was planted there, so the record can tell "the slot
 * still holds the buffer" from "the call wrote something else into it".
 * -1 is "not planted", which is every slot of every seven column table.
 */
static int16_t run_planted_buf[TD_AES_ADDRIN_N];
static void   *run_planted_ptr[TD_AES_ADDRIN_N];

static void lay_buffers(void)
{
	int i;

	for (i = 0; i < TD_AES_BUF_N; i++)
		td_aes_buf_fill(run_bufs[i].bytes, sizeof run_bufs[i].bytes, i);
}

/*
 * Put the buffers a line named where the line said (cases.h).
 *
 * An addr_in plant is a plain pointer store.  An int_in plant is written
 * with memcpy at the TARGET's own pointer width -- four bytes on the m68k,
 * eight on AArch64 -- because that is what the classic GEM binding does
 * (gemlib a_wind_set_str.c:25) and what both AESs read back: EmuTOS as
 * *(char **)&int_in[2] (gemwmlib.c:1661), NewTOS as an unaligned load of the
 * caller's pointer width (tos-api/src/aes/mod.rs, intin_ptr).  One
 * expression, correct on both, exactly as the raster runner packs an MFDB
 * pointer into CONTRL.  memcpy and not a cast: &int_in[w] is only WORD
 * aligned.
 */
static void plant_buffers(const struct td_aes_plant *plant, uint16_t count)
{
	uint16_t i;
	int j;

	for (j = 0; j < TD_AES_ADDRIN_N; j++) {
		run_planted_buf[j] = -1;
		run_planted_ptr[j] = 0;
	}

	for (i = 0; i < count; i++) {
		void *p = (void *)run_bufs[plant[i].buffer].bytes;

		if (plant[i].kind == TD_AES_PLANT_ADDRIN) {
			run_addr_in[plant[i].where]     = p;
			run_planted_buf[plant[i].where] = plant[i].buffer;
			run_planted_ptr[plant[i].where] = p;
		} else {
			memcpy(&run_arrays.int_in[plant[i].where], &p, sizeof p);
		}
	}
}

/*
 * Poison a pointer array with the 0xDEAD pattern (spec section 4).
 *
 * The pattern is built as a VALUE of pointer width, not written as a byte
 * pattern, and that distinction is the whole point: the record carries the
 * low 32 bits of each slot as a number, so what has to agree between the
 * two builds is the number, not the byte order it happens to sit in.  A
 * byte-pattern fill gives 0xDEADDEAD on the big-endian m68k and 0xADDEADDE
 * on little-endian AArch64 -- two builds disagreeing about poison, which
 * would fabricate a difference in every single record.
 *
 * Repeating 0xDEAD to the pointer's width leaves the low 32 bits at
 * 0xDEADDEAD whatever that width is, so both builds record the same thing.
 *
 * The integer it is built in is `unsigned long`, NOT uintptr_t, and that is
 * not a style choice: the NewTOS build compiles against mintlib's headers
 * (-nostdinc -I sdk/libc/mintlib/include, see build-dumpaes-newtos.sh),
 * whose stdint.h is written for the 32-bit m68k, so uintptr_t is FOUR bytes
 * there while a pointer is eight.  unsigned long is the pointer width on
 * both targets -- 4 on m68k-atari-mint, 8 on AArch64 LP64 -- and the
 * typedef below turns that from an assumption into a build error.  (The
 * record's own fields stay fixed-width per spec section 2.1; this is the
 * one place that deliberately wants the machine's pointer width.)
 */
typedef char td_ptr_fits_in_ulong[
	(sizeof(unsigned long) >= sizeof(void *)) ? 1 : -1];

static void poison_ptrs(void **p, int count)
{
	unsigned long v = 0;
	size_t i;
	int j;

	for (i = 0; i < sizeof(unsigned long) / 2; i++)
		v = (v << 16) | (unsigned long)(uint16_t)TD_POISON16;

	for (j = 0; j < count; j++)
		p[j] = (void *)v;
}

/*
 * One call through the ABI backend, with control[] laid from the four
 * counts.  Used for the measured call and for a case's preludes, so the two
 * cannot lay the parameter block differently.
 */
static int32_t aes_call(int16_t opcode, int16_t n_intin, int16_t n_intout,
                        int16_t n_addrin, int16_t n_addrout, int16_t *global)
{
	/* All five slots are inputs in the AES: the dispatcher copies
	 * control[] onto a local buffer (EmuTOS aes/gemsuper.c:435) and never
	 * writes any of it back, so none of them may be left poisoned the way
	 * the VDI runner leaves contrl[2] and contrl[4]. */
	run_arrays.contrl[0] = opcode;
	run_arrays.contrl[1] = n_intin;
	run_arrays.contrl[2] = n_intout;
	run_arrays.contrl[3] = n_addrin;
	run_arrays.contrl[4] = n_addrout;

	return td_abi_aes(run_arrays.contrl, global, run_arrays.int_in,
	                  run_arrays.int_out, run_addr_in, run_addr_out);
}

/* ------------------------------------------------------------------ */
/* Reset (spec section 5)                                              */
/* ------------------------------------------------------------------ */
/*
 * "Before every case, reset the state the call could depend on."  For the
 * VDI that is v_clrwk plus the attribute defaults; for the AES there is no
 * such documented default set (runner_aes.h), and round 1 lives without one.
 *
 * The window manager does have one, and it is the AES' own: wm_new closes
 * and deletes every window with a handle >= 1 and drains the wind_update
 * mouse-control and screen-sync locks (gemwmlib.c:1875-1893).  EmuTOS runs
 * it itself after every graphics-mode program the shell starts
 * (gemshlib.c:676-677), so issuing it inside a run leaves the machine in a
 * state the shell would have produced anyway -- which is what makes it safe
 * to issue unattended.
 *
 * Laid exactly as gemlib lays it: AES_PARAMS(109,0,0,0,0) (a_wind_new.c:25).
 * OUT_LEN 0 matters -- xif copies OUT_LEN words back out of an
 * UNINITIALISED stack buffer (gemsuper.c:430-432, :444-445), so a reset
 * that declared output words would write stack garbage over the caller's
 * array before the case that is about to be measured even starts.
 *
 * Which reset runs comes from the TABLE, not from this file and not from a
 * command line: see cases.h.  The sequence itself lives here, so both
 * targets run the identical one.
 */
static void td_reset_aes(int16_t reset, int16_t *global)
{
	if (reset != TD_AES_RESET_WIND_NEW)
		return;

	/* poisoned like every other call the runner issues, so the reset
	 * cannot carry the previous case's values into the AES.  The buffers
	 * are laid too: the reset names none, but a buffer the previous case
	 * wrote into is state the reset exists to sweep. */
	td_poison_aes(&run_arrays);
	poison_ptrs(run_addr_in, TD_AES_ADDRIN_N);
	poison_ptrs(run_addr_out, TD_AES_ADDROUT_N);
	lay_buffers();
	plant_buffers(NULL, 0);
	aes_call(109, 0, 0, 0, 0, global);
}

int td_run_aes(const struct td_aes_case_table *ct, int16_t *global,
               uint8_t *out, size_t out_bytes)
{
	uint16_t i;
	int32_t ret;
	int j;

	if (out_bytes < (size_t)ct->count * TD_AES_RECORD_SIZE)
		return TD_RUN_AES_ERR_BUFFER;

	/* Does THIS AES have the opcodes the table names?  global[0] is
	 * ap_version as the machine's own appl_init wrote it (gemsuper.c:92),
	 * which is the only thing that can answer; see runner_aes.h for why
	 * the answer is taken here as well as in the dumper's main. */
	if (td_aes_cases_version_refusal(ct, global[0], NULL) != 0)
		return TD_RUN_AES_ERR_VERSION;

	for (i = 0; i < ct->count; i++) {
		const struct td_aes_case *c = &ct->cases[i];
		uint8_t *rec = out + (size_t)i * TD_AES_RECORD_SIZE;

		/* 0. the reset the table declares (spec section 5).  Ahead of
		 *    the preludes, so a prelude establishes state on top of a
		 *    known one rather than on top of the previous case's
		 *    leftovers. */
		td_reset_aes(ct->reset, global);

		/* 1. the case's preludes: calls that establish what this case
		 *    measures, and never recorded.  Each is poisoned and laid
		 *    exactly like a measured call -- a prelude that inherited
		 *    the previous call's arrays would establish something other
		 *    than what the table says. */
		for (j = 0; j < (int)c->prelude_count; j++) {
			const struct td_aes_prelude *p =
				&ct->preludes[c->prelude_first + (uint16_t)j];
			int k;

			td_poison_aes(&run_arrays);
			poison_ptrs(run_addr_in, TD_AES_ADDRIN_N);
			poison_ptrs(run_addr_out, TD_AES_ADDROUT_N);
			lay_buffers();
			for (k = 0; k < (int)p->n_values; k++)
				run_arrays.int_in[k] = p->intin[k];
			/* after the values: a plant and a value never share a WORD
			 * (the loader refuses that), but the order has to be one
			 * fixed one or the two builds could differ */
			plant_buffers(p->plant, p->n_plants);
			aes_call(p->opcode, p->n_intin, p->n_intout, p->n_addrin,
			         p->n_addrout, global);
		}

		/* 2. poison everything (spec section 4) -- except global[],
		 *    which is live AES state and belongs to the application,
		 *    not to the case (runner_aes.h) */
		td_poison_aes(&run_arrays);
		poison_ptrs(run_addr_in, TD_AES_ADDRIN_N);
		poison_ptrs(run_addr_out, TD_AES_ADDROUT_N);
		lay_buffers();

		/* 3. lay the case's int_in values over the poison; whatever the
		 *    case does not fill stays 0xDEAD, which is what makes an
		 *    unexpected write visible -- and what the dispatcher hands
		 *    the call when IN_LEN reaches past the listed values */
		for (j = 0; j < (int)c->n_values; j++)
			run_arrays.int_in[j] = c->intin[j];

		/* 3a. the buffers the case's addr_in column names (cases.h).
		 *     After the values, and after the poison, so what a slot
		 *     holds going in is either a buffer this case asked for or
		 *     the poison. */
		plant_buffers(c->plant, c->n_plants);

		/* 4. the call under measurement */
		ret = aes_call(c->opcode, c->n_intin, c->n_intout, c->n_addrin,
		               c->n_addrout, global);

		/* 5. the live global as it stands after the call.  Copied into
		 *    the frame only to be recorded; the array the call saw is
		 *    the application's own. */
		for (j = 0; j < TD_AES_GLOBAL_N; j++)
			run_arrays.global[j] = global[j];

		/* 6. the address arrays, as the low 32 bits of whatever the
		 *    parameter block holds.  Same expression on both targets;
		 *    see the declaration above for why the record cannot hold
		 *    the whole pointer.
		 *
		 *    A slot this case pointed at a buffer is the one exception:
		 *    its address is different on the two machines by
		 *    construction, so recording it would fabricate a difference
		 *    in every such case.  It records the buffer TOKEN instead --
		 *    as long as the slot still holds what was planted.  If the
		 *    call wrote something else there, that is recorded, and a
		 *    difference is exactly what it should produce.  See
		 *    tosdump.h, TD_AES_ADDR_TOKEN. */
		for (j = 0; j < TD_AES_ADDRIN_N; j++)
			run_arrays.addr_in[j] =
				(run_planted_buf[j] >= 0 &&
				 run_addr_in[j] == run_planted_ptr[j])
					? TD_AES_ADDR_TOKEN(run_planted_buf[j])
					: (uint32_t)(unsigned long)run_addr_in[j];
		for (j = 0; j < TD_AES_ADDROUT_N; j++)
			run_arrays.addr_out[j] =
				(uint32_t)(unsigned long)run_addr_out[j];

		/* 7. the record.  The input arrays go in as they stand AFTER
		 *    the call: the dispatcher is not supposed to touch them
		 *    (gemsuper.c:435-439 copies them onto local buffers), and
		 *    dumping them afterwards is what makes a violation visible */
		td_aes_record_write(rec, c->line, (uint16_t)c->opcode, ret,
		                    &run_arrays);
	}

	/*
	 * One last reset, after the last case and outside the measurement.
	 * The per-case reset runs BEFORE a case, so without this the run would
	 * end holding whatever the last case left -- an open window, a taken
	 * update lock, a raised ml_ocnt.  On the TT the shell would sweep it
	 * (gemshlib.c:668-677); relying on that would make the harness' claim
	 * that it leaves nothing behind true on one machine and unverified on
	 * the other.
	 */
	td_reset_aes(ct->reset, global);

	return 0;
}
