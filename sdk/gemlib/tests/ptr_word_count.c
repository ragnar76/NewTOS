/*
 * ptr_word_count.c -- pin the WORD count the bindings that pack a pointer
 * into intin/int_in declare for it.
 *
 * The contract (docs/COMPATIBILITY.md, T04/A81): a pointer packed into a
 * VDI/AES WORD array occupies the caller's own pointer width -- N_PTRINTS =
 * sizeof(void *) / sizeof(short) WORDs (4 on LP64, 2 on the m68k) -- and the
 * declared count (contrl[3] N_INTIN, control[1] IN_LEN) counts those WORDs:
 * the AES copies exactly IN_LEN words into its local buffer (EmuTOS
 * aes/gemsuper.c:436-437; tos-api/src/aes/mod.rs from_raw, A19).
 *
 * Pristine gemlib hands the pointer over byte-correct (`(short *)&ptr` as
 * the array, or aes_intin_ptr) but declares the m68k count of 2:
 * n_v_delete_ctab.c:25 / n_v_delete_itab.c:25 / n_vqt_fontheader.c:54 /
 * u_vqt_fontheader.c:14 (`0,2`), a_wind_get.c:345 (`= 4` for WF_NAME/
 * WF_INFO), a_wind_sget.c:21 (`AES_PARAMS(104,4,...)`). The overlays in
 * sdk/gemlib/overlay/ declare N_PTRINTS resp. 2 + N_PTRINTS.
 *
 * This test is the regression guard for those six overlays, in the shape of
 * evnt_timer_words.c: a HOST program that defines vdi() and aes() itself,
 * captures the parameter block the binding built and checks the declared
 * count and the pointer bytes inside it. The host shares the target's LP64
 * pointer width; the test refuses to certify from any other host.
 *
 * Build & run (from the repository root, after build-gemlib.sh has applied
 * the overlay to the extracted tree):
 *   cc -D__NEWTOS__ -I sdk/gemlib/gemlib-0.44.0 -I sdk/libc/mintlib/include \
 *      -o ptr_word_count sdk/gemlib/tests/ptr_word_count.c \
 *      sdk/gemlib/gemlib-0.44.0/n_v_delete_ctab.c \
 *      sdk/gemlib/gemlib-0.44.0/n_v_delete_itab.c \
 *      sdk/gemlib/gemlib-0.44.0/n_vqt_fontheader.c \
 *      sdk/gemlib/gemlib-0.44.0/u_vqt_fontheader.c \
 *      sdk/gemlib/gemlib-0.44.0/a_wind_get.c \
 *      sdk/gemlib/gemlib-0.44.0/a_wind_sget.c \
 *      sdk/gemlib/gemlib-0.44.0/vdi_array2str.c \
 *      sdk/gemlib/gemlib-0.44.0/vdi_userdef.c \
 *      sdk/gemlib/gemlib-0.44.0/vdi_dummy.c
 *   ./ptr_word_count         # exit 0 = counts match the contract
 *
 * Compiled against the PRISTINE six .c files from the tarball instead (with
 * -D__NO_INLINE__ in place of -D__NEWTOS__ and the pristine gem_{aes,vdi}P.h
 * first on the include path), it fails every count case (2 resp. 4).
 */

#include <stdio.h>
#include <string.h>
#include <mt_gem.h>
#include <mt_gemx.h>

#define PTR_WORDS ((short)(sizeof(void *) / sizeof(short)))

/* What the fake VDI trap saw on the last call. */
static short seen_control[7];
static void *seen_intin_ptr;	/* the sizeof(void *) bytes at &intin[0] */
static int   vdi_calls;

void vdi(VDIPB *pb)
{
	int i;
	vdi_calls++;
	for (i = 0; i < 7; i++)
		seen_control[i] = pb->control[i];
	memcpy(&seen_intin_ptr, pb->intin, sizeof seen_intin_ptr);
	pb->control[2] = 0;
	pb->control[4] = 1;
	pb->intout[0] = 1;
	if (pb->control[0] == 232) {		/* vqt_fontheader: a path name back */
		pb->control[4] = 3;
		pb->intout[0] = 'a'; pb->intout[1] = 'b'; pb->intout[2] = 'c';
	}
}

/* What the fake AES trap saw on the last call. */
static short seen_aes_control[5];
static short seen_aes_intin[2];
static void *seen_aes_intin_ptr;	/* the sizeof(void *) bytes at &int_in[2] */
static int   aes_calls;

void aes(AESPB *pb)
{
	int i;
	aes_calls++;
	for (i = 0; i < 5; i++)
		seen_aes_control[i] = pb->control[i];
	seen_aes_intin[0] = pb->intin[0];
	seen_aes_intin[1] = pb->intin[1];
	memcpy(&seen_aes_intin_ptr, &pb->intin[2], sizeof seen_aes_intin_ptr);
	pb->intout[0] = 1;
}

static int failures;

static void check(const char *what, long got, long want)
{
	if (got != want) {
		printf("    FAIL %-12s got 0x%lx want 0x%lx\n", what, got, want);
		failures++;
	}
}

static void check_vdi(short op, short sub, const void *p, short ret)
{
	check("calls",      vdi_calls,       1);
	check("control[0]", seen_control[0], op);
	check("control[1]", seen_control[1], 0);
	check("control[3]", seen_control[3], PTR_WORDS);	/* N_PTRINTS, not 2 */
	check("control[5]", seen_control[5], sub);
	check("intin ptr",  (long)seen_intin_ptr, (long)p);
	check("return",     ret,             1);
}

int main(void)
{
	/* Stand-ins with a non-zero upper half, so a count that stops at two
	 * WORDs is visibly short of the pointer. */
	COLOR_TAB *ctab = (COLOR_TAB *)0x0000000101770001UL;
	ITAB_REF   itab = (ITAB_REF)0x000000010177E4C0UL;
	char *buffer = (char *)0x0000000101234560UL;
	char pathname[8];
	short global_aes[15] = { 0 };
	short w1 = 0, w2 = 0, w3 = 0, w4 = 0;
	short ret;

	if (sizeof(void *) != 8) {
		printf("ptr_word_count: host pointer is not 8 bytes; this test certifies "
		       "the LP64 build and cannot run here\n");
		return 2;
	}

	printf("  v_delete_ctab(3, %p)\n", (void *)ctab);
	vdi_calls = 0;
	ret = v_delete_ctab(3, ctab);
	check_vdi(206, 9, ctab, ret);

	printf("  v_delete_itab(3, %p)\n", (void *)itab);
	vdi_calls = 0;
	ret = v_delete_itab(3, itab);
	check_vdi(208, 1, itab, ret);

	printf("  vqt_fontheader(3, %p)\n", (void *)buffer);
	vdi_calls = 0;
	memset(pathname, 0, sizeof pathname);
	vqt_fontheader(3, buffer, pathname);
	check_vdi(232, 0, buffer, 1);
	check("pathname", strcmp(pathname, "abc"), 0);

	printf("  udef_vqt_fontheader(3, %p)\n", (void *)buffer);
	vdi_calls = 0;
	memset(pathname, 0, sizeof pathname);
	udef_vqt_fontheader(3, buffer, pathname);
	check_vdi(232, 0, buffer, 1);
	check("pathname", strcmp(pathname, "abc"), 0);

	printf("  wind_get(4, WF_NAME, %p)\n", (void *)buffer);
	aes_calls = 0;
	ret = mt_wind_get(4, WF_NAME, (short *)buffer, &w2, &w3, &w4, global_aes);
	check("calls",      aes_calls,           1);
	check("control[0]", seen_aes_control[0], 104);
	check("control[1]", seen_aes_control[1], 2 + PTR_WORDS);	/* 6, not 4 */
	check("control[2]", seen_aes_control[2], 5);
	check("intin[0]",   seen_aes_intin[0],   4);
	check("intin[1]",   seen_aes_intin[1],   WF_NAME);
	check("intin ptr",  (long)seen_aes_intin_ptr, (long)buffer);
	check("return",     ret,                 1);
	check("W2..W4",     w2 | w3 | w4,        0);		/* WF_NAME leaves them alone */

	printf("  wind_get(4, WF_INFO, %p)\n", (void *)buffer);
	aes_calls = 0;
	ret = mt_wind_get(4, WF_INFO, (short *)buffer, &w2, &w3, &w4, global_aes);
	check("control[1]", seen_aes_control[1], 2 + PTR_WORDS);
	check("intin ptr",  (long)seen_aes_intin_ptr, (long)buffer);

	printf("  wind_get(4, WF_WORKXYWH)\n");
	aes_calls = 0;
	ret = mt_wind_get(4, WF_WORKXYWH, &w1, &w2, &w3, &w4, global_aes);
	check("control[1]", seen_aes_control[1], 2);		/* no pointer: unchanged */

	printf("  wind_sget(5, WF_NAME, %p)\n", (void *)buffer);
	aes_calls = 0;
	ret = mt_wind_sget(5, WF_NAME, buffer, global_aes);
	check("calls",      aes_calls,           1);
	check("control[0]", seen_aes_control[0], 104);
	check("control[1]", seen_aes_control[1], 2 + PTR_WORDS);	/* 6, not 4 */
	check("control[2]", seen_aes_control[2], 1);
	check("intin[0]",   seen_aes_intin[0],   5);
	check("intin ptr",  (long)seen_aes_intin_ptr, (long)buffer);
	check("return",     ret,                 1);

	if (failures) {
		printf("ptr_word_count: %d FAILURE(S) -- a packed pointer is not declared "
		       "at N_PTRINTS WORDs\n", failures);
		return 1;
	}
	printf("ptr_word_count: OK -- v_delete_ctab/_itab, vqt_fontheader (+udef) declare "
	       "N_PTRINTS intin WORDs; wind_get(WF_NAME/WF_INFO) and wind_sget declare 2 + N_PTRINTS\n");
	return 0;
}
