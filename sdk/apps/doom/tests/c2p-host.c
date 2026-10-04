/*
 * c2p-host.c -- host test for DOOM's chunky-to-planar conversion
 * (sdk/apps/doom/doomgeneric_gem.c, section "Chunky to planar"; TASK/PORT06,
 * user decision 2026-10-04: table c2p).
 *
 * The new c2p_prepare/c2p_row are compiled straight from the platform file
 * (DOOM_GEM_C2P_HOST_TEST keeps everything else -- GEM, doomgeneric -- out)
 * and compared word for word against the routines they replaced, kept here
 * as the oracle:
 *   old_c2p_std    the old c2p_row (standard form, vr_trnfm route)
 *   old_c2p_inter  the old EDDI_PLANES loop of build_device_form
 *                  (interleaved planes, EdDI format 0)
 * Both are copied unchanged except that the globals became parameters and
 * that they go on into a last, partial group with 0 for the pixels past n
 * (the old code only ever saw n = 320; the new one defines that tail).
 *
 * Coverage: 1..32 planes (1..8 the planar screens, more the standard form of
 * a truecolour vr_trnfm route), widths 0..49, 63..65, 127..129, 319..321,
 * 335, both layouts, value tables random / random full 32 bit / identity /
 * all ones / zero, pixel data random / 0 / 255 / ramp / alternating. The
 * destination is pre-filled with a sentinel and compared whole, so a word
 * written outside the form fails too; the source is an exact-size heap
 * block, so under AddressSanitizer a read past n fails.
 *
 * Build/run (sdk/run-host-tests.sh does this):
 *   cc -O1 -g -fsanitize=address,undefined -Wall -Wextra \
 *      -o c2p-host sdk/apps/doom/tests/c2p-host.c && ./c2p-host
 * Exit status 0 = all equal.
 */
#define DOOM_GEM_C2P_HOST_TEST 1
#include "../doomgeneric_gem.c"

/* The old c2p_row: `dst` = std_buf + row * wdw. */
static void old_c2p_std(const uint8_t *src, int n, uint16_t *dst,
			long plane_words, const uint32_t *pixval, int nplanes)
{
	uint32_t m[16];
	int g, k, p;

	for (g = 0; g < (n + 15) / 16; g++) {
		for (k = 0; k < 16; k++)
			m[k] = 16 * g + k < n ? pixval[src[k]] : 0;
		src += 16;
		for (p = 0; p < nplanes; p++) {
			uint32_t w = 0;

			for (k = 0; k < 16; k++)
				w = (w << 1) | ((m[k] >> p) & 1u);
			dst[p * plane_words + g] = (uint16_t)w;
		}
	}
}

/* The old EDDI_PLANES row of build_device_form: `dst` = the row. */
static void old_c2p_inter(const uint8_t *src, int n, uint16_t *dst,
			  const uint32_t *devpix, int nplanes)
{
	const uint8_t *s = src;
	uint32_t m[16];
	int g, k, p;

	for (g = 0; g < (n + 15) / 16; g++, s += 16) {
		for (k = 0; k < 16; k++)
			m[k] = 16 * g + k < n ? devpix[s[k]] : 0;
		for (p = 0; p < nplanes; p++) {
			uint32_t w = 0;

			for (k = 0; k < 16; k++)
				w = (w << 1) | ((m[k] >> p) & 1u);
			*dst++ = (uint16_t)w;
		}
	}
}

static uint32_t rng = 12345u;
static uint32_t rnd(void)
{
	rng ^= rng << 13;
	rng ^= rng >> 17;
	rng ^= rng << 5;
	return rng;
}

#define MAXW	335
#define DSTW	((MAXW + 15) / 16 * C2P_MAX_PLANES * 2 + 64)

static int checks, failures;

static void one(int planes, int n, const uint32_t *val, const uint8_t *data,
		int layout)
{
	static uint16_t want[DSTW], got[DSTW];
	uint8_t *src = malloc(n ? (size_t)n : 1);
	long wd = (n + 15) / 16, ps = wd + 3;	/* a plane stride wider than a row */
	int i;

	memcpy(src, data, (size_t)n);
	for (i = 0; i < DSTW; i++)
		want[i] = got[i] = 0xa5a5;
	c2p_prepare(val, planes);
	if (layout == 0) {
		old_c2p_std(src, n, want, ps, val, planes);
		c2p_row(src, n, got, ps, 1);
	} else {
		old_c2p_inter(src, n, want, val, planes);
		c2p_row(src, n, got, 1, planes);
	}
	checks++;
	if (memcmp(want, got, sizeof(want)) != 0) {
		for (i = 0; i < DSTW && want[i] == got[i]; i++)
			;
		if (failures++ < 10)
			printf("FAIL planes %d width %d layout %s: word %d is 0x%04x, "
			       "oracle 0x%04x\n", planes, n,
			       layout ? "interleaved" : "standard", i, got[i],
			       want[i]);
	}
	free(src);
}

int main(void)
{
	static const int widths[] = { 63, 64, 65, 127, 128, 129, 319, 320, 321, 335 };
	static uint32_t val[256];
	static uint8_t data[MAXW];
	int planes, vt, dt, wi, n, i, layout;

	for (planes = 1; planes <= C2P_MAX_PLANES; planes++)
		for (vt = 0; vt < 5; vt++) {
			uint32_t mask = planes == 32 ? 0xffffffffu
					: ((uint32_t)1 << planes) - 1u;

			for (i = 0; i < 256; i++)
				val[i] = vt == 0 ? rnd() & mask :	/* measured-like */
					 vt == 1 ? rnd() :		/* stray high bits */
					 vt == 2 ? (uint32_t)i & mask :
					 vt == 3 ? mask : 0;
			for (dt = 0; dt < 5; dt++) {
				for (i = 0; i < MAXW; i++)
					data[i] = dt == 0 ? (uint8_t)rnd() :
						  dt == 1 ? 0 :
						  dt == 2 ? 255 :
						  dt == 3 ? (uint8_t)i :
						  (i & 1) ? 255 : 0;
				for (layout = 0; layout < 2; layout++) {
					for (n = 0; n <= 49; n++)
						one(planes, n, val, data, layout);
					for (wi = 0; wi < (int)(sizeof(widths) / sizeof(widths[0])); wi++)
						one(planes, widths[wi], val, data, layout);
				}
			}
		}
	printf("c2p-host: %d comparisons, %d differ\n", checks, failures);
	return failures != 0;
}
