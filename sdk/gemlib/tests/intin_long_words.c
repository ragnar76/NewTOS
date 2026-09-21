/*
 * intin_long_words.c -- pin the WORDs the *_intin_long / *_intout_long
 * accessors (and the WF_SCREEN read of mt_wind_get_ptr) exchange with the
 * VDI and the AES.
 *
 * The contract (docs/COMPATIBILITY.md, T04; docs/api/aes/_dispatcher.md,
 * docs/api/vdi/_dispatcher.md): a LONG inside int_in/int_out/ptsin/ptsout
 * is 32 bits over two WORDs, low word first, stored and loaded natively in
 * place at &array[n] -- EmuTOS's `*(LONG *)&int_out[1]` (aes/gemwmlib.c:
 * 1564) and `MAKE_ULONG(int_in[1], int_in[0])` (aes/gemsuper.c:143,
 * include/aesdefs.h:190-191) on a little-endian machine. A pointer packed
 * into a WORD array occupies the caller's own pointer width (A81).
 *
 * Pristine gemlib (gem_vdiP.h:182-185, gem_aesP.h:89-99) implements the
 * accessors as `*(long *)(array + n)`: on AArch64 LP64 an EIGHT-byte access
 * at a two-byte-aligned slot. A store spills the upper half over the next
 * two WORDs -- past the end of an exactly sized local array in
 * v_create_ctab (short vdi_intin[4], vdi_intin_long(2)), vst_track_offset
 * (same), v_value2color (short vdi_intin[2], vdi_intin_long(0)) and
 * vst_arbpt32 (short vdi_intout[2], vdi_intout_long(0) = 0), where at -O2
 * it overwrote the binding's own control[0] (the VDI received opcode 0) or
 * the caller's frame -- and a load takes the two WORDs after the LONG as its
 * upper half (every *_intout_long reader). mt_wind_get_ptr read WF_SCREEN,
 * two 32-bit LONGs (address, length), as one 8-byte pointer: address |
 * length << 32. sdk/gemlib/overlay/gem_vdiP.h and gem_aesP.h make the
 * accessors 32-bit (gem_wire_long); sdk/gemlib/overlay/a_wind_get_ptr.c
 * reads WF_SCREEN as the LONG it is.
 *
 * This test is the regression guard for those overlays, in the shape of
 * evnt_timer_words.c: a HOST program that defines vdi() and aes() itself,
 * captures the parameter block the binding built, checks the slots the
 * binding DECLARES (control[0] = the opcode, control[3] = nintin, the
 * [lo, hi] words) and answers through the same block the way the NewTOS
 * kernel does (32-bit stores, two WORDs each). The bindings' local arrays
 * are exactly sized, so "no bytes beyond the declared arrays" is checked by
 * AddressSanitizer: build with -fsanitize=address and the pristine headers
 * report stack-buffer-overflow in the four bindings, the overlay headers
 * report nothing. The host shares the target's LP64 `long` and little-endian
 * byte order, which is what the punning depended on; the test refuses to
 * certify from any other host.
 *
 * Build & run (from the repository root, after build-gemlib.sh has applied
 * the overlay to the extracted tree):
 *   cc -D__NEWTOS__ -fsanitize=address -O1 \
 *      -I sdk/gemlib/gemlib-0.44.0 -I sdk/libc/mintlib/include \
 *      -o intin_long_words sdk/gemlib/tests/intin_long_words.c \
 *      sdk/gemlib/gemlib-0.44.0/n_vst_arbpt32.c \
 *      sdk/gemlib/gemlib-0.44.0/n_v_create_ctab.c \
 *      sdk/gemlib/gemlib-0.44.0/n_vst_track_offset.c \
 *      sdk/gemlib/gemlib-0.44.0/n_v_value2color.c \
 *      sdk/gemlib/gemlib-0.44.0/a_wind_get_ptr.c \
 *      sdk/gemlib/gemlib-0.44.0/vdi_dummy.c
 *   ./intin_long_words       # exit 0 = words match the contract, ASan silent
 *
 * Negative control -- the PRISTINE accessors and the pristine
 * a_wind_get_ptr.c from the tarball (their C-call trap branch is selected by
 * -D__NO_INLINE__ instead of -D__NEWTOS__; mt_gem.h stays the tree's):
 *   tar xf sdk/gemlib/gemlib-0.44.0.tar.xz -C /tmp/pristine
 *   mkdir /tmp/pristine/hdr && cp /tmp/pristine/gemlib-0.44.0/gem_{aes,vdi}P.h /tmp/pristine/hdr/
 *   cc -D__NO_INLINE__ -fsanitize=address -O1 \
 *      -I /tmp/pristine/hdr -I sdk/gemlib/gemlib-0.44.0 -I sdk/libc/mintlib/include \
 *      -o intin_long_words_pristine sdk/gemlib/tests/intin_long_words.c \
 *      sdk/gemlib/gemlib-0.44.0/n_vst_arbpt32.c ... (same list, with
 *      /tmp/pristine/gemlib-0.44.0/a_wind_get_ptr.c instead of the tree's)
 *   ./intin_long_words_pristine
 * aborts in the first binding with an ASan stack-buffer-overflow (WRITE of
 * size 8 in vst_arbpt32); with ASAN_OPTIONS=halt_on_error=0 it runs on and
 * fails the WF_SCREEN case (address | length << 32) and the vst_arbpt32
 * return (upper half taken from the two WORDs after the LONG).
 */

#include <stdio.h>
#include <string.h>
#include <mt_gem.h>
#include <mt_gemx.h>

/* ---------------------------------------------------------------- VDI --- */

/* What the fake VDI trap saw on the last call. Only the declared counts are
 * copied out of the caller's arrays: contrl[0..6], nintin (contrl[3]) intin
 * words, nptsin (contrl[1]) vertices. */
static short seen_control[7];
static short seen_intin[8];
static short seen_nintin;
static int   vdi_calls;

/* The VDI trap primitive the binding links to under __NEWTOS__
 * (sdk/gemlib/overlay/_gc_vdi.c on the target) and under __NO_INLINE__
 * (pristine gem_vdiP.h, "go the old way"). Here: capture and answer the
 * way the NewTOS VDI would -- 32-bit LONGs as two WORDs, low word first. */
void vdi(VDIPB *pb)
{
	int i;
	short op  = pb->control[0];
	short sub = pb->control[5];

	vdi_calls++;
	for (i = 0; i < 7; i++)
		seen_control[i] = pb->control[i];
	seen_nintin = pb->control[3];
	memset(seen_intin, 0, sizeof seen_intin);
	for (i = 0; i < seen_nintin && i < 8; i++)
		seen_intin[i] = pb->intin[i];

	if (op == 246) {					/* vst_arbpt32: 2 intout, 4 ptsout */
		static int neg;
		pb->control[2] = 2;
		pb->control[4] = 2;
		pb->ptsout[0] = 8; pb->ptsout[1] = 16; pb->ptsout[2] = 9; pb->ptsout[3] = 17;
		if (!neg++) {					/* 0x56789ABC */
			pb->intout[0] = (short)0x9ABC;
			pb->intout[1] = (short)0x5678;
		} else {						/* -2: 0xFFFFFFFE */
			pb->intout[0] = (short)0xFFFE;
			pb->intout[1] = (short)0xFFFF;
		}
	} else if (op == 206 && sub == 8) {	/* v_create_ctab: a pointer back */
		void *p = (void *)0x0000000101234560UL;
		pb->control[2] = 0;
		pb->control[4] = (short)(sizeof(void *) / sizeof(short));
		memcpy(&pb->intout[0], &p, sizeof p);
	} else if (op == 237) {				/* vst_track_offset: 2 intout */
		pb->control[2] = 0;
		pb->control[4] = 2;
		pb->intout[0] = 7;
		pb->intout[1] = 9;
	} else if (op == 204 && sub == 1) {	/* v_value2color: LONG + COLOR_ENTRY */
		pb->control[2] = 0;
		pb->control[4] = 6;
		pb->intout[0] = (short)0x0002;	/* 0x00010002 */
		pb->intout[1] = (short)0x0001;
		pb->intout[2] = 0;				/* reserved */
		pb->intout[3] = 100;			/* red */
		pb->intout[4] = 200;			/* green */
		pb->intout[5] = 300;			/* blue */
	}
}

/* ---------------------------------------------------------------- AES --- */

static short seen_aes_control[5];
static short seen_aes_intin[2];
static int   aes_calls;

/* The AES trap primitive the binding links to (sdk/gemlib/overlay/_gc_aes.c
 * on the target; pristine gem_aesP.h under __NO_INLINE__). Here: answer
 * wind_get(0, WF_SCREEN) exactly as the NewTOS AES does
 * (tos-api/src/aes/mod.rs Aespb::set_intout_long, A57): TRUE, the buffer
 * address as one 32-bit LONG in int_out[1..2], its length as one 32-bit
 * LONG in int_out[3..4], nothing past OUT_LEN 5. The WF_NEWDESK case
 * answers a pointer at the caller's width (the branch the overlay leaves
 * as it was). */
static unsigned long screen_addr, screen_len;
static void *newdesk_tree;

void aes(AESPB *pb)
{
	int i;
	aes_calls++;
	for (i = 0; i < 5; i++)
		seen_aes_control[i] = pb->control[i];
	seen_aes_intin[0] = pb->intin[0];
	seen_aes_intin[1] = pb->intin[1];

	if (pb->control[0] == 104 && pb->intin[1] == WF_SCREEN) {
		unsigned int a = (unsigned int)screen_addr;
		unsigned int l = (unsigned int)screen_len;
		pb->intout[0] = 1;
		memcpy(&pb->intout[1], &a, 4);	/* one native 32-bit store */
		memcpy(&pb->intout[3], &l, 4);	/* one native 32-bit store */
	} else if (pb->control[0] == 104 && pb->intin[1] == WF_NEWDESK) {
		pb->intout[0] = 1;
		memcpy(&pb->intout[1], &newdesk_tree, sizeof newdesk_tree);
	} else {
		pb->intout[0] = 0;
	}
}

/* ------------------------------------------------------------- checks --- */

static int failures;

static void check(const char *what, long got, long want)
{
	if (got != want) {
		printf("    FAIL %-14s got 0x%lx want 0x%lx\n", what, got, want);
		failures++;
	}
}

static void check_w(const char *what, short got, unsigned short want)
{
	check(what, (unsigned short)got, want);
}

int main(void)
{
#if !defined(__BYTE_ORDER__) || (__BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__)
	printf("intin_long_words: host is not little-endian; this test certifies the "
	       "AArch64 (little-endian, LP64) build and cannot run here\n");
	return 2;
#endif
	if (sizeof(long) != 8 || sizeof(void *) != 8 || sizeof(short) != 2) {
		printf("intin_long_words: host is not LP64; this test certifies the LP64 "
		       "build and cannot run here\n");
		return 2;
	}

	/* --- vst_arbpt32: (short *)&point as intin (T04 class 1), and the
	 *     vdi_intout_long(0) pre-zero + read-back (class 3) on an exactly
	 *     sized short vdi_intout[2]. --- */
	{
		short wc = 0, hc = 0, wcell = 0, hcell = 0;
		long r;
		printf("  vst_arbpt32(3, 0x00123456)\n");
		vdi_calls = 0;
		r = vst_arbpt32(3, 0x00123456L, &wc, &hc, &wcell, &hcell);
		check("calls",      vdi_calls,       1);
		check("control[0]", seen_control[0], 246);		/* not 0: the pre-zero stayed in intout */
		check("control[1]", seen_control[1], 0);
		check("control[3]", seen_control[3], 2);
		check("control[6]", seen_control[6], 3);
		check_w("intin[0]", seen_intin[0],   0x3456);	/* low word first */
		check_w("intin[1]", seen_intin[1],   0x0012);
		check("wchar/hchar", (wc << 8) | hc, (8 << 8) | 16);
		check("wcell/hcell", (wcell << 8) | hcell, (9 << 8) | 17);
		check("return",     r,               0x56789ABCL);	/* 32 bits, not intout[2..3] */

		printf("  vst_arbpt32(3, 0x00123456) -> -2\n");
		r = vst_arbpt32(3, 0x00123456L, &wc, &hc, &wcell, &hcell);
		check("return(-2)", r,               -2L);		/* sign-extended 68k long */
	}

	/* --- v_create_ctab: two LONGs through vdi_intin_long(0) and (2) on an
	 *     exactly sized short vdi_intin[4]; a pointer back at caller width. --- */
	{
		COLOR_TAB *t;
		printf("  v_create_ctab(4, 0x00010002, 0x00030004)\n");
		vdi_calls = 0;
		t = v_create_ctab(4, 0x00010002L, 0x00030004UL);
		check("calls",      vdi_calls,       1);
		check("control[0]", seen_control[0], 206);		/* not 0 */
		check("control[3]", seen_control[3], 4);
		check("control[5]", seen_control[5], 8);
		check("control[6]", seen_control[6], 4);
		check_w("intin[0]", seen_intin[0],   0x0002);	/* color_space lo */
		check_w("intin[1]", seen_intin[1],   0x0001);	/* color_space hi */
		check_w("intin[2]", seen_intin[2],   0x0004);	/* px_format lo -- not the spill of color_space */
		check_w("intin[3]", seen_intin[3],   0x0003);	/* px_format hi */
		check("return",     (long)t,         0x0000000101234560L);
	}

	/* --- vst_track_offset: two WORDs then a LONG at vdi_intin_long(2) on an
	 *     exactly sized short vdi_intin[4]. --- */
	{
		short tracks = 0, pairs = 0;
		printf("  vst_track_offset(5, 0x00123456, 1)\n");
		vdi_calls = 0;
		vst_track_offset(5, 0x00123456L, 1, &tracks, &pairs);
		check("calls",      vdi_calls,       1);
		check("control[0]", seen_control[0], 237);		/* not 0 */
		check("control[3]", seen_control[3], 4);
		check("control[6]", seen_control[6], 5);
		check_w("intin[0]", seen_intin[0],   255);		/* the two WORD stores survived */
		check_w("intin[1]", seen_intin[1],   1);
		check_w("intin[2]", seen_intin[2],   0x3456);
		check_w("intin[3]", seen_intin[3],   0x0012);
		check("tracks/pairs", (tracks << 8) | pairs, (7 << 8) | 9);
	}

	/* --- v_value2color: one LONG at vdi_intin_long(0) on an exactly sized
	 *     short vdi_intin[2] (the pristine 8-byte store ran into the caller's
	 *     frame); the LONG answer read as 32 bits. --- */
	{
		COLOR_ENTRY c;
		long r;
		memset(&c, 0, sizeof c);
		printf("  v_value2color(6, 0x89ABCDEF)\n");
		vdi_calls = 0;
		r = v_value2color(6, 0x89ABCDEFUL, &c);
		check("calls",      vdi_calls,       1);
		check("control[0]", seen_control[0], 204);
		check("control[3]", seen_control[3], 2);
		check("control[5]", seen_control[5], 1);
		check_w("intin[0]", seen_intin[0],   0xCDEF);
		check_w("intin[1]", seen_intin[1],   0x89AB);
		check("return",     r,               0x00010002L);
		check("color.rgb",  (c.rgb.red * 1000L + c.rgb.green) * 1000L + c.rgb.blue, (100L * 1000 + 200) * 1000 + 300);
	}

	/* --- mt_wind_get_ptr(0, WF_SCREEN): the AES writes two 32-bit LONGs;
	 *     the binding must return the FIRST one (the address), not the eight
	 *     bytes at &int_out[1] (address | length << 32). --- */
	{
		short global_aes[15] = { 0 };
		void *v = (void *)-1;
		short ret;

		screen_addr = 0x01000218UL;		/* the reference TT's answer */
		screen_len  = 40960UL;
		printf("  wind_get_ptr(0, WF_SCREEN) -> 0x%lx / %lu\n", screen_addr, screen_len);
		aes_calls = 0;
		ret = mt_wind_get_ptr(0, WF_SCREEN, &v, global_aes);
		check("calls",      aes_calls,           1);
		check("control[0]", seen_aes_control[0], 104);
		check("control[1]", seen_aes_control[1], 2);
		check("control[2]", seen_aes_control[2], 5);
		check("intin[1]",   seen_aes_intin[1],   WF_SCREEN);
		check("return",     ret,                 1);
		check("*v",         (long)v,             (long)screen_addr);

		/* A length with a non-zero high word: the 8-byte read would put it
		 * into bits 32..47 of the "pointer". */
		screen_addr = 0x0000FFF0UL;
		screen_len  = 0x0003F480UL;		/* 259 200 */
		printf("  wind_get_ptr(0, WF_SCREEN) -> 0x%lx / %lu\n", screen_addr, screen_len);
		v = (void *)-1;
		ret = mt_wind_get_ptr(0, WF_SCREEN, &v, global_aes);
		check("return",     ret,                 1);
		check("*v",         (long)v,             (long)screen_addr);	/* not 0x0003F4800000FFF0 */

		/* The pointer-class branch is untouched: WF_NEWDESK at caller width. */
		newdesk_tree = (void *)0x000000010177E4C0UL;
		printf("  wind_get_ptr(0, WF_NEWDESK) -> %p\n", newdesk_tree);
		v = (void *)-1;
		ret = mt_wind_get_ptr(0, WF_NEWDESK, &v, global_aes);
		check("return",     ret,                 1);
		check("*v",         (long)v,             (long)newdesk_tree);
	}

	if (failures) {
		printf("intin_long_words: %d FAILURE(S) -- the *_long accessors / the WF_SCREEN "
		       "read do not exchange 32-bit LONGs as [lo, hi] WORD pairs\n", failures);
		return 1;
	}
	printf("intin_long_words: OK -- *_intin_long/*_intout_long are 32-bit [lo, hi] in place, "
	       "control[0] keeps the opcode, wind_get_ptr(WF_SCREEN) returns the address LONG\n");
	return 0;
}
