/*
 * wind_set_addrin.c -- pin the slots the pointer-typed wind_set() bindings
 * hand to the AES.
 *
 * mt_wind_set_str() / mt_wind_set_ptr() / mt_wind_set_ptr_int() carry a
 * pointer (a WF_NAME/WF_INFO string, a WF_NEWDESK tree). The NewTOS AES takes
 * that pointer in addr_in[0] whenever control[3] (AIN_LEN) is >= 1, with the
 * companion word (wind_set_ptr_int's W3, WF_NEWDESK's root) in int_in[2]
 * (docs/api/aes/window-graphics.md, "Pointers in `addr_in[]`";
 * tos-api/src/aes/window.rs, wf_str_ptr / set_newdesk).
 *
 * Pristine gemlib (a_wind_set_ptr_int.c:27-28 and siblings) packs the pointer
 * into int_in[2..] instead and writes W3 to int_in[4] -- correct for a 4-byte
 * m68k pointer, but on AArch64 LP64 the pointer is 8 bytes (int_in[2..5]) and
 * int_in[4] is its bits 32..47: a non-zero WF_NEWDESK root corrupted the tree
 * pointer (TASK/A81). sdk/gemlib/overlay/a_wind_set_{str,ptr,ptr_int}.c use
 * the addr_in[0] form instead.
 *
 * This test is the regression guard for those overlays, in the shape of
 * evnt_timer_words.c: a HOST program that defines aes() itself, captures the
 * parameter block the binding built, and checks the slots -- no trap, no
 * kernel. Only the words the binding DECLARES (control[1] = 6, int_in[0..5])
 * are checked: the AES copies exactly IN_LEN words and never reads beyond
 * them (EmuTOS aes/gemsuper.c:435-436; tos-api/src/aes/mod.rs, from_raw).
 *
 * Build & run (from the repository root, after build-gemlib.sh has applied
 * the overlay to the extracted tree):
 *   cc -D__NEWTOS__ -I sdk/gemlib/gemlib-0.44.0 -I sdk/libc/mintlib/include \
 *      -o wind_set_addrin sdk/gemlib/tests/wind_set_addrin.c \
 *      sdk/gemlib/gemlib-0.44.0/a_wind_set_str.c \
 *      sdk/gemlib/gemlib-0.44.0/a_wind_set_ptr.c \
 *      sdk/gemlib/gemlib-0.44.0/a_wind_set_ptr_int.c
 *   ./wind_set_addrin        # exit 0 = slots match the AES contract
 *
 * Compiled against the PRISTINE a_wind_set_*.c from the tarball instead, it
 * fails on every case (control[3] is 0 and addr_in[0] is never written).
 */

#include <stdio.h>
#include <mt_gem.h>

/* What the fake trap saw on the last call. */
static short seen_control[5];
static short seen_intin[6];
static long  seen_addrin0;
static int   calls;

/* The AES trap primitive the binding links to under __NEWTOS__
 * (sdk/gemlib/overlay/_gc_aes.c on the target). Here: capture and answer. */
void aes(AESPB *pb)
{
	int i;
	calls++;
	for (i = 0; i < 5; i++)
		seen_control[i] = pb->control[i];
	for (i = 0; i < 6; i++)
		seen_intin[i] = pb->intin[i];
	seen_addrin0 = pb->addrin[0];
	pb->intout[0] = 1;			/* wind_set's result */
}

static int failures;

static void check(const char *what, long got, long want)
{
	if (got != want) {
		printf("    FAIL %-12s got 0x%lx want 0x%lx\n", what, got, want);
		failures++;
	}
}

/* The block every one of the three bindings must lay: control 105,6,1,1,0;
 * handle/field in int_in[0..1]; W3 in int_in[2]; int_in[3..5] zero; the
 * pointer in addr_in[0]. */
static void check_block(short handle, short field, const void *p, short w3, short ret)
{
	check("calls",      calls,           1);
	check("control[0]", seen_control[0], 105);
	check("control[1]", seen_control[1], 6);
	check("control[2]", seen_control[2], 1);
	check("control[3]", seen_control[3], 1);	/* AIN_LEN 1: the native form */
	check("control[4]", seen_control[4], 0);
	check("intin[0]",   seen_intin[0],   handle);
	check("intin[1]",   seen_intin[1],   field);
	check("intin[2]",   seen_intin[2],   w3);	/* the companion word */
	check("intin[3]",   seen_intin[3],   0);
	check("intin[4]",   seen_intin[4],   0);	/* never W3 (the A81 slot) */
	check("intin[5]",   seen_intin[5],   0);
	check("addrin[0]",  seen_addrin0,    (long)p);
	check("return",     ret,             1);
}

int main(void)
{
	short global_aes[15] = { 0 };
	/* Stand-ins with a non-zero upper half, so a pointer that leaked into
	 * int_in[2..5] would be visible in the words the test checks. */
	const char *title = (const char *)0x0000000101770001UL;
	void *tree = (void *)0x000000010177E4C0UL;	/* TASK/A81.md's tree */
	short ret;

	if (sizeof(void *) != 8) {
		printf("wind_set_addrin: host pointer is not 8 bytes; this test certifies "
		       "the LP64 build and cannot run here\n");
		return 2;
	}

	printf("  wind_set_str(3, WF_NAME, %p)\n", (const void *)title);
	calls = 0;
	ret = mt_wind_set_str(3, WF_NAME, title, global_aes);
	check_block(3, WF_NAME, title, 0, ret);

	printf("  wind_set_ptr(4, WF_INFO, %p)\n", (const void *)title);
	calls = 0;
	ret = mt_wind_set_ptr(4, WF_INFO, (void *)title, global_aes);
	check_block(4, WF_INFO, title, 0, ret);

	printf("  wind_set_ptr_int(0, WF_NEWDESK, %p, 2)\n", tree);
	calls = 0;
	ret = mt_wind_set_ptr_int(0, WF_NEWDESK, tree, 2, global_aes);
	check_block(0, WF_NEWDESK, tree, 2, ret);	/* root 2: the measured A81 case */

	printf("  wind_set_ptr_int(0, WF_NEWDESK, %p, 0)\n", tree);
	calls = 0;
	ret = mt_wind_set_ptr_int(0, WF_NEWDESK, tree, 0, global_aes);
	check_block(0, WF_NEWDESK, tree, 0, ret);	/* TeraDesk, icon.c:2511 */

	if (failures) {
		printf("wind_set_addrin: %d FAILURE(S) -- the pointer-typed wind_set bindings "
		       "do not use addr_in[0] + int_in[2]\n", failures);
		return 1;
	}
	printf("wind_set_addrin: OK -- wind_set_str/_ptr/_ptr_int hand the pointer in "
	       "addr_in[0] (AIN_LEN 1) and the companion word in int_in[2]\n");
	return 0;
}
