#include <stdio.h>
#include <stdlib.h>
#include "variants.h"

static void oracle(const uint8_t *src, int n, const uint32_t *val, uint16_t *dst, long ps, long gs)
{
	uint32_t m[16]; int g, k, p;
	for (g = 0; g < n / 16; g++) {
		for (k = 0; k < 16; k++) m[k] = val[src[k]];
		src += 16;
		for (p = 0; p < 8; p++) {
			uint32_t w = 0;
			for (k = 0; k < 16; k++) w = (w << 1) | ((m[k] >> p) & 1u);
			dst[g * gs + p * ps] = (uint16_t)w;
		}
	}
}
int main(void)
{
	static uint32_t val[256]; static uint8_t src[320];
	static uint16_t a[320 * 8], b[320 * 8];
	int it, i, v; long ps, gs;
	void (*prep[6])(const uint32_t *) = { h_prepare, p_prepare, m_prepare, h_prepare, h_prepare, h_prepare };
	void (*row[6])(const uint8_t *, int, uint16_t *, long, long) = { h_row, p_row, m_row, v2_row, v3_row, v4_row };
	srand(1);
	for (it = 0; it < 200; it++) {
		for (i = 0; i < 256; i++) val[i] = rand() & 0xff;
		for (i = 0; i < 320; i++) src[i] = rand() & 0xff;
		for (v = 0; v < 6; v++) for (int lay = 0; lay < 2; lay++) {
			ps = lay ? 20 : 1; gs = lay ? 1 : 8;
			memset(a, 0, sizeof a); memset(b, 0, sizeof b);
			oracle(src, 320, val, a, ps, gs);
			prep[v](val); row[v](src, 320, b, ps, gs);
			if (memcmp(a, b, sizeof a)) { printf("variant %d layout %d FAIL it %d\n", v, lay, it); return 1; }
		}
	}
	printf("all ok\n");
	return 0;
}
