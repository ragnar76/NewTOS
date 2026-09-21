/*
 * obspec_bits.c -- pin the bit positions of gemlib's BFOBSPEC bit-field.
 *
 * OBSPEC's bit-field view (BFOBSPEC, mt_gem.h) is how a GEM program reads and
 * writes the packed colour/thickness word of a box object's ob_spec. The GEM
 * layout of that 32-bit word is fixed by the resource format and by the AES
 * Object Manager (EmuTOS gr_crack, aes/gemgraf.c:522-535, and ob_sst,
 * aes/gemobjop.c:54-58):
 *
 *     31..24 character | 23..16 framesize | 15..12 framecol
 *     11..8  textcol   | 7 textmode | 6..4 fillpattern | 3..0 interiorcol
 *
 * Pristine gemlib declares BFOBSPEC as a plain bit-field struct in m68k
 * (big-endian) field order. On a little-endian target a C bit-field is
 * allocated from the least significant end of its storage unit, so that
 * declaration mirrors the whole word: `character` lands in bits 0..7 and
 * `interiorcol` in bits 28..31. sdk/gemlib/overlay/mt_gem.h.in reverses the
 * declaration order for little-endian targets to restore the layout above.
 *
 * This test is the regression guard for that overlay. It is a HOST program
 * (the bit-field rule depends only on endianness, which the host shares with
 * the AArch64 target), and it also refuses to pass on a host whose endianness
 * differs from the target it is meant to certify.
 *
 * Build & run (from the repository root; mt_gem.h pulls <compiler.h> from
 * mintlib's include dir, so that path is required too):
 *   cc -I sdk/gemlib/gemlib-0.44.0 -I sdk/libc/mintlib/include \
 *      -o obspec_bits sdk/gemlib/tests/obspec_bits.c
 *   ./obspec_bits            # exit 0 = layout matches GEM
 *
 * (sdk/gemlib/gemlib-0.44.0/mt_gem.h is generated from mt_gem.h.in by
 * sdk/gemlib/build-gemlib.sh, which applies the overlay first. All the host
 * tests in this directory run together via sdk/run-host-tests.sh.)
 */

#include <stdio.h>
#include <mt_gem.h>

/* The GEM-documented decomposition of an ob_spec colour/thickness word. */
struct gem_fields {
	unsigned long spec;
	unsigned character;
	int          framesize;
	unsigned     framecol;
	unsigned     textcol;
	unsigned     textmode;
	unsigned     fillpattern;
	unsigned     interiorcol;
};

static int failures;

static void check(const char *what, long got, long want)
{
	if (got != want) {
		printf("    FAIL %-12s got %-6ld want %ld\n", what, got, want);
		failures++;
	}
}

static void check_spec(const char *label, unsigned long spec)
{
	OBSPEC o;
	struct gem_fields g;
	signed char fs = (signed char)((spec >> 16) & 0xffUL);

	g.spec        = spec;
	g.character   = (unsigned)((spec >> 24) & 0xffUL);
	g.framesize   = fs;
	g.framecol    = (unsigned)((spec >> 12) & 0x0fUL);
	g.textcol     = (unsigned)((spec >>  8) & 0x0fUL);
	g.textmode    = (unsigned)((spec >>  7) & 0x01UL);
	g.fillpattern = (unsigned)((spec >>  4) & 0x07UL);
	g.interiorcol = (unsigned)( spec        & 0x0fUL);

	o.index = (long)spec;
	printf("  %s ob_spec=0x%08lx\n", label, spec);
	check("character",   (long)o.obspec.character,   (long)g.character);
	check("framesize",   (long)o.obspec.framesize,   (long)g.framesize);
	check("framecol",    (long)o.obspec.framecol,    (long)g.framecol);
	check("textcol",     (long)o.obspec.textcol,     (long)g.textcol);
	check("textmode",    (long)o.obspec.textmode,    (long)g.textmode);
	check("fillpattern", (long)o.obspec.fillpattern, (long)g.fillpattern);
	check("interiorcol", (long)o.obspec.interiorcol, (long)g.interiorcol);
}

/* Writing through the bit-field must land on the same bits, too:
   TeraDesk does exactly this in resource.c:160 and icon.c:2481-2482. */
static void check_write(void)
{
	OBSPEC o;

	o.index = 0;
	o.obspec.fillpattern = 7;
	printf("  write fillpattern=7 -> ob_spec=0x%08lx\n", (unsigned long)o.index);
	check("fillpattern bits", (long)(unsigned long)o.index, 0x70L);

	o.index = 0;
	o.obspec.interiorcol = 3;
	printf("  write interiorcol=3 -> ob_spec=0x%08lx\n", (unsigned long)o.index);
	check("interiorcol bits", (long)(unsigned long)o.index, 0x03L);

	o.index = 0;
	o.obspec.framesize = -2;
	printf("  write framesize=-2  -> ob_spec=0x%08lx\n", (unsigned long)o.index);
	check("framesize bits", (long)(unsigned long)o.index, 0x00fe0000L);
}

int main(void)
{
#if !defined(__BYTE_ORDER__) || !defined(__ORDER_LITTLE_ENDIAN__) || \
    (__BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__)
	printf("obspec_bits: host is not little-endian; this test certifies the "
	       "little-endian (AArch64) arm of BFOBSPEC and cannot do so here.\n");
	return 2;
#else
	printf("sizeof(BFOBSPEC)=%zu sizeof(OBSPEC)=%zu sizeof(long)=%zu\n",
	       sizeof(BFOBSPEC), sizeof(OBSPEC), sizeof(long));
	if (sizeof(BFOBSPEC) != 4) {
		printf("    FAIL sizeof(BFOBSPEC) got %zu want 4\n", sizeof(BFOBSPEC));
		failures++;
	}

	/* TeraDesk desktop.rsc, tree INFOBOX object 1 -- the dragbox "ear" whose
	   framesize/framecol ub_drag reads (xddraw.c:540,546): thickness 2,
	   colour word 0x9180 = border LBLACK, text BLACK, opaque, hollow, white. */
	check_spec("dragbox      ", 0x00029180UL);
	/* Same resource: the INFOBOX root box (thickness 2, colour 0x4100). */
	check_spec("dialog root  ", 0x00024100UL);
	/* A menu drop-down box: thickness -1 (0xff), colour 0x1100. */
	check_spec("menu box     ", 0x00ff1100UL);
	/* All bits set in every field, to catch a field that is too wide/narrow. */
	check_spec("saturated    ", 0xffffffffUL);
	/* Alternating, to catch a swapped adjacent pair. */
	check_spec("alternating  ", 0x5aa5a55aUL);

	check_write();

	if (failures) {
		printf("obspec_bits: %d FAILURE(S) -- BFOBSPEC does not match the GEM "
		       "ob_spec bit layout\n", failures);
		return 1;
	}
	printf("obspec_bits: OK -- BFOBSPEC matches the GEM ob_spec bit layout\n");
	return 0;
#endif
}
