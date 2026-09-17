/*
 * evnt_timer_words.c -- pin the word order mt_evnt_timer() hands to the AES.
 *
 * The AES takes the evnt_timer() millisecond count as two 16-bit words:
 * int_in[0] is the LOW word (T_LOCOUNT), int_in[1] the HIGH word (T_HICOUNT)
 * -- EmuTOS aes/gemsuper.c:142-143 `ev_timer(MAKE_ULONG(T_HICOUNT, T_LOCOUNT))`
 * with include/aesdefs.h:190-191; NewTOS reads them the same way
 * (tos-api/src/aes/event.rs, evnt_timer).
 *
 * Pristine gemlib (a_evnt_timer.c:27-32) produced those words by punning the
 * `unsigned long` argument through an `unsigned short *`, which yields lo/hi
 * only where `long` is 32 bits and big-endian (m68k). On AArch64 LP64 the
 * halves arrive swapped: evnt_timer(150) asked the AES for 150 << 16 ms
 * (TASK/H03, the TeraDesk quit hang). sdk/gemlib/overlay/a_evnt_timer.c
 * derives the words arithmetically instead.
 *
 * This test is the regression guard for that overlay. It is a HOST program:
 * under -D__NEWTOS__ the binding's AES_TRAP() is a call to the C symbol
 * aes(AESPB *) (sdk/gemlib/overlay/gem_aesP.h), so the test defines aes()
 * itself, captures the parameter block, and checks the words the binding
 * wrote -- no trap, no kernel. The host shares the target's LP64 `long` and
 * little-endian byte order, which is what the punning depended on; the test
 * refuses to certify from any other host.
 *
 * Build & run (from the repository root, after build-gemlib.sh has applied
 * the overlay to the extracted tree):
 *   cc -D__NEWTOS__ -I sdk/gemlib/gemlib-0.44.0 -I sdk/libc/mintlib/include \
 *      -o evnt_timer_words sdk/gemlib/tests/evnt_timer_words.c \
 *      sdk/gemlib/gemlib-0.44.0/a_evnt_timer.c
 *   ./evnt_timer_words       # exit 0 = words match the AES contract
 *
 * Compiled against the PRISTINE a_evnt_timer.c from the tarball instead, it
 * fails on every case with a non-zero low word (that is the H03 defect).
 */

#include <stdio.h>
#include <mt_gem.h>

/* What the fake trap saw on the last call. */
static short seen_opcode, seen_n_intin, seen_n_intout;
static short seen_intin[2];
static int   calls;

/* The AES trap primitive the binding links to under __NEWTOS__
 * (sdk/gemlib/overlay/_gc_aes.c on the target). Here: capture and answer. */
void aes(AESPB *pb)
{
	calls++;
	seen_opcode   = pb->control[0];
	seen_n_intin  = pb->control[1];
	seen_n_intout = pb->control[2];
	seen_intin[0] = pb->intin[0];
	seen_intin[1] = pb->intin[1];
	pb->intout[0] = 1;			/* evnt_timer's reserved result */
}

static int failures;

static void check(const char *what, long got, long want)
{
	if (got != want) {
		printf("    FAIL %-10s got 0x%04lx want 0x%04lx\n", what, got & 0xffff, want & 0xffff);
		failures++;
	}
}

/* interval -> the two words the AES must receive: lo = bits 0..15,
 * hi = bits 16..31. The AES interval is a 32-bit ULONG; bits above 31 have
 * no representation on the wire and must not leak into either word. */
static void check_interval(unsigned long interval, unsigned short want_lo, unsigned short want_hi)
{
	short global_aes[15] = { 0 };
	short ret;
	int before = calls;

	printf("  evnt_timer(%lu = 0x%lx)\n", interval, interval);
	ret = mt_evnt_timer(interval, global_aes);

	check("calls",    calls - before, 1);
	check("opcode",   seen_opcode,    24);
	check("n_intin",  seen_n_intin,   2);
	check("n_intout", seen_n_intout,  1);
	check("intin[0]", (unsigned short)seen_intin[0], want_lo);	/* T_LOCOUNT */
	check("intin[1]", (unsigned short)seen_intin[1], want_hi);	/* T_HICOUNT */
	check("return",   ret,            1);
}

int main(void)
{
#if !defined(__BYTE_ORDER__) || (__BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__)
	printf("evnt_timer_words: host is not little-endian; this test certifies the "
	       "AArch64 (little-endian, LP64) build and cannot run here\n");
	return 2;
#endif
	printf("sizeof(unsigned long)=%zu sizeof(short)=%zu\n",
	       sizeof(unsigned long), sizeof(short));
	if (sizeof(unsigned long) != 8) {
		printf("evnt_timer_words: host long is not 64-bit; this test certifies "
		       "the LP64 build and cannot run here\n");
		return 2;
	}

	check_interval(150UL,          150,    0);		/* TeraDesk's wait(150), TASK/H03 */
	check_interval(0UL,            0,      0);		/* return-at-once */
	check_interval(0x10000UL,      0,      1);		/* exactly one high-word unit */
	check_interval(0x12345678UL,   0x5678, 0x1234);	/* both words populated */
	check_interval(0xFFFFFFFFUL,   0xFFFF, 0xFFFF);	/* saturated 32-bit ULONG */
	check_interval(0x100000007UL,  7,      0);		/* bit 32 has no word to land in */

	if (failures) {
		printf("evnt_timer_words: %d FAILURE(S) -- mt_evnt_timer does not hand "
		       "the AES lo/hi words in T_LOCOUNT/T_HICOUNT order\n", failures);
		return 1;
	}
	printf("evnt_timer_words: OK -- mt_evnt_timer hands int_in[0]=low, int_in[1]=high\n");
	return 0;
}
