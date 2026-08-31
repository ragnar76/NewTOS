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
 * each, through the same expression on both targets.  Round 1 is
 * pointer-free (cases/COVERAGE-AES.md): nothing ever writes a pointer here,
 * so the only thing that reaches the record is the poison -- whose low 32
 * bits are the same NUMBER on both targets, which is what keeps the two
 * builds comparable.  See poison_ptrs for why that is a number and not a
 * byte pattern.
 */
static void *run_addr_in[TD_AES_ADDRIN_N];
static void *run_addr_out[TD_AES_ADDROUT_N];

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

int td_run_aes(const struct td_aes_case_table *ct, int16_t *global,
               uint8_t *out, size_t out_bytes)
{
	uint16_t i;
	int32_t ret;
	int j;

	if (out_bytes < (size_t)ct->count * TD_AES_RECORD_SIZE)
		return TD_RUN_AES_ERR_BUFFER;

	for (i = 0; i < ct->count; i++) {
		const struct td_aes_case *c = &ct->cases[i];
		uint8_t *rec = out + (size_t)i * TD_AES_RECORD_SIZE;

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
			for (k = 0; k < (int)p->n_values; k++)
				run_arrays.int_in[k] = p->intin[k];
			aes_call(p->opcode, p->n_intin, p->n_intout, p->n_addrin,
			         p->n_addrout, global);
		}

		/* 2. poison everything (spec section 4) -- except global[],
		 *    which is live AES state and belongs to the application,
		 *    not to the case (runner_aes.h) */
		td_poison_aes(&run_arrays);
		poison_ptrs(run_addr_in, TD_AES_ADDRIN_N);
		poison_ptrs(run_addr_out, TD_AES_ADDROUT_N);

		/* 3. lay the case's int_in values over the poison; whatever the
		 *    case does not fill stays 0xDEAD, which is what makes an
		 *    unexpected write visible -- and what the dispatcher hands
		 *    the call when IN_LEN reaches past the listed values */
		for (j = 0; j < (int)c->n_values; j++)
			run_arrays.int_in[j] = c->intin[j];

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
		 *    the whole pointer. */
		for (j = 0; j < TD_AES_ADDRIN_N; j++)
			run_arrays.addr_in[j] =
				(uint32_t)(unsigned long)run_addr_in[j];
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

	return 0;
}
