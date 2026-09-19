/*
 * aesglobal_fields.c -- pin where TeraDesk's build reads and writes ap_ptree
 * and ap_rscmem in the AESGLOBAL.
 *
 * The TOS AESGLOBAL (global[]) is 15 words, 2-byte aligned; ap_ptree is the
 * 4-byte pointer at byte 10 (global[5..6]) and ap_rscmem the one at byte 14
 * (global[7..8]) -- EmuTOS aes/gemrslib.h:15-27 struct aesglobal, spec
 * docs/api/aes/application.md "AESGLOBAL". C has no 32-bit pointer lvalue on
 * LP64, so sdk/gemlib/overlay/gem.h:101-125 reads the two fields as 4 bytes at
 * those offsets (_AESrscfile / _AESrscmem) and stores them through the setters
 * _AESrscfile_set() / _AESrscmem_set().
 *
 * TeraDesk upstream bypasses those accessors: library/xdialog/library.h:55-83
 * re-defines the two macros over a naturally aligned `AES_GLOBAL` struct that
 * on LP64 is 56 bytes long and puts ap_ptree at byte 16 and ap_rscmem at byte
 * 24 (past the end of `short aes_global[16]`), while rsc_load.c stores them
 * as 8-byte pointers at byte 10 and 14. Writer and reader disagreed; measured
 * in QEMU on 2026-09-19 (TASK/A60-STATUS-DONE.MD). The overlay
 * sdk/overlays/teradesk/library/xdialog/library.h removes the redefinition so
 * gemlib's accessors apply, and sdk/overlays/teradesk/{icon.c,rsc_load.c}
 * use the setters.
 *
 * This test is the regression guard for that overlay stack. It is a HOST
 * program that includes TeraDesk's library.h through the same include order
 * as the build's shadow tree (overlay dir before the vendored dir, then gemlib
 * top, then mintlib), and certifies:
 *   - no `AES_GLOBAL` redefinition survives (_AES_GLOBAL_defined is a hard
 *     compile error),
 *   - _AESrscfile_p() / _AESrscmem_p() address byte 10 / 14 of aes_global and
 *     are 4 bytes wide,
 *   - _AESrscfile_set() / _AESrscmem_set() store exactly those 4 bytes and
 *     leave every other byte of the 32-byte array untouched (no more 8-byte
 *     store over ap_rsclen/ap_planes),
 *   - _AESrscfile / _AESrscmem read the stored pointer back.
 * The host shares the target's LP64 pointer width and little-endian byte
 * order, which is all the layout depends on; the test refuses to certify
 * from any other host.
 *
 * Build & run (from the repository root; build-gemlib.sh must have applied
 * the overlay to the extracted gemlib tree, and the vendored TeraDesk source
 * must be present under sdk/apps/teradesk):
 *   cc -D__NEWTOS__ -D_TOS_ONLY -funsigned-char -Wno-macro-redefined \
 *      -I sdk/overlays/teradesk -I sdk/overlays/teradesk/library/xdialog \
 *      -I sdk/apps/teradesk -I sdk/apps/teradesk/library/xdialog \
 *      -I sdk/apps/teradesk/library/utility \
 *      -I sdk/gemlib/gemlib-0.44.0 -I sdk/libc/mintlib/include \
 *      -o aesglobal_fields sdk/gemlib/tests/aesglobal_fields.c
 *   ./aesglobal_fields       # exit 0 = accessors hit the TOS bytes
 *
 * Compiled with `-I sdk/apps/teradesk/library/xdialog` placed BEFORE the
 * overlay directory instead (i.e. against the PRISTINE library.h), it fails at
 * compile time on the _AES_GLOBAL_defined check -- that is the A60 defect.
 */

#include <stdio.h>
#include <string.h>

#include <library.h>	/* TeraDesk's, via the overlay: pulls in gem.h */

#ifdef _AES_GLOBAL_defined
#error "TeraDesk's AES_GLOBAL redefinition of _AESrscfile/_AESrscmem is active (A60): the library.h overlay is not in effect"
#endif

#if !defined(__SIZEOF_POINTER__) || (__SIZEOF_POINTER__ != 8)
#error "this test certifies the LP64 layout; build it on an LP64 host"
#endif
#if !defined(__BYTE_ORDER__) || (__BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__)
#error "this test certifies the little-endian layout; build it on a little-endian host"
#endif

/* gemlib's globals.c defines this in the library; the host test owns it. */
short aes_global[AES_GLOBMAX];

/* The two fields are 4-byte, 2-byte-aligned TOS pointers. */
_Static_assert(sizeof(*_AESrscfile_p()) == 4, "_AESrscfile must be a 4-byte field");
_Static_assert(sizeof(*_AESrscmem_p()) == 4, "_AESrscmem must be a 4-byte field");
_Static_assert(sizeof(_AESrscfile) == sizeof(OBJECT **), "_AESrscfile reads back a pointer");
_Static_assert(sizeof(_AESrscmem) == sizeof(void *), "_AESrscmem reads back a pointer");

static int failures;

static void check(const char *what, long got, long want)
{
	if (got != want) {
		printf("    FAIL %-44s got 0x%lx want 0x%lx\n", what, got, want);
		failures++;
	}
}

static unsigned long field_u32(int byte_off)
{
	unsigned int v;
	memcpy(&v, (const unsigned char *)aes_global + byte_off, 4);
	return v;
}

int main(void)
{
	const unsigned char *base = (const unsigned char *)aes_global;
	unsigned char before[sizeof aes_global], after[sizeof aes_global];
	OBJECT **tree = (OBJECT **)(unsigned long)0x00427028ul;	/* an EL0 user VA */
	void *mem = (void *)(unsigned long)0x00427000ul;
	size_t i;

	printf("aesglobal_fields: sizeof(aes_global) = %zu\n", sizeof aes_global);

	/* 1. the accessors address byte 10 and 14 */
	check("_AESrscfile_p() - aes_global (ap_ptree @10)",
	      (const unsigned char *)_AESrscfile_p() - base, 10);
	check("_AESrscmem_p() - aes_global (ap_rscmem @14)",
	      (const unsigned char *)_AESrscmem_p() - base, 14);

	/* 2. the setters store 4 bytes and nothing else */
	for (i = 0; i < sizeof aes_global; i++)
		((unsigned char *)aes_global)[i] = (unsigned char)(0xA0 + i);
	memcpy(before, aes_global, sizeof before);

	_AESrscfile_set(tree);
	memcpy(after, aes_global, sizeof after);
	check("ap_ptree u32 @10 after _AESrscfile_set", field_u32(10), 0x00427028ul);
	for (i = 0; i < sizeof aes_global; i++) {
		if (i >= 10 && i < 14)
			continue;
		if (after[i] != before[i]) {
			printf("    FAIL _AESrscfile_set touched byte %zu (0x%02x -> 0x%02x)\n",
			       i, before[i], after[i]);
			failures++;
		}
	}

	memcpy(before, aes_global, sizeof before);
	_AESrscmem_set(mem);
	memcpy(after, aes_global, sizeof after);
	check("ap_rscmem u32 @14 after _AESrscmem_set", field_u32(14), 0x00427000ul);
	check("ap_ptree u32 @10 still intact", field_u32(10), 0x00427028ul);
	for (i = 0; i < sizeof aes_global; i++) {
		if (i >= 14 && i < 18)
			continue;
		if (after[i] != before[i]) {
			printf("    FAIL _AESrscmem_set touched byte %zu (0x%02x -> 0x%02x)\n",
			       i, before[i], after[i]);
			failures++;
		}
	}
	/* ap_rsclen (@18) and ap_planes (@20) keep the fill pattern: the 8-byte
	 * store of the pristine build zeroed them. */
	check("ap_rsclen word @18 untouched", (unsigned short)aes_global[9], 0xB3B2);
	check("ap_planes word @20 untouched", (unsigned short)aes_global[10], 0xB5B4);

	/* 3. the readers give the stored pointers back */
	check("_AESrscfile reads back", (long)(unsigned long)_AESrscfile, (long)(unsigned long)tree);
	check("_AESrscmem reads back", (long)(unsigned long)_AESrscmem, (long)(unsigned long)mem);

	if (failures) {
		printf("aesglobal_fields: %d FAILURE(S)\n", failures);
		return 1;
	}
	printf("aesglobal_fields: OK -- ap_ptree/ap_rscmem are 4-byte fields at byte 10/14 through TeraDesk's library.h\n");
	return 0;
}
