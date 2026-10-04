/* scratch: three c2p candidates, 8-plane slice only, for measurement */
#include <stdint.h>
#include <string.h>

/* ---- H: per-position tables (4 positions) + 2 merge passes ---- */
static uint32_t h_tab[4][256];
static void h_prepare(const uint32_t *val)
{
	int v, p, k;
	for (v = 0; v < 256; v++) {
		uint32_t b = val[v] & 0xff, t = 0;
		for (p = 0; p < 8; p++)
			if (b & (1u << p))
				t |= (uint32_t)1 << ((((p & 1) ^ 1) << 4) | (((p >> 2) & 1) << 3) |
						     (((p >> 1) & 1) << 2) | 3);
		for (k = 0; k < 4; k++)
			h_tab[k][v] = t >> k;
	}
}
__attribute__((noinline)) static void h_row(const uint8_t *s, int n, uint16_t *d, long ps, long gs)
{
	const uint32_t *t0 = h_tab[0], *t1 = h_tab[1], *t2 = h_tab[2], *t3 = h_tab[3];
	int g;
	for (g = 0; g < n / 16; g++, s += 16, d += gs) {
		uint32_t x0, x1, x2, x3, t;
		x3 = t0[s[0]] | t1[s[1]] | t2[s[2]] | t3[s[3]];
		x2 = t0[s[4]] | t1[s[5]] | t2[s[6]] | t3[s[7]];
		x1 = t0[s[8]] | t1[s[9]] | t2[s[10]] | t3[s[11]];
		x0 = t0[s[12]] | t1[s[13]] | t2[s[14]] | t3[s[15]];
		t = ((x0 >> 8) ^ x2) & 0x00ff00ffu; x2 ^= t; x0 ^= t << 8;
		t = ((x1 >> 8) ^ x3) & 0x00ff00ffu; x3 ^= t; x1 ^= t << 8;
		t = ((x0 >> 4) ^ x1) & 0x0f0f0f0fu; x1 ^= t; x0 ^= t << 4;
		t = ((x2 >> 4) ^ x3) & 0x0f0f0f0fu; x3 ^= t; x2 ^= t << 4;
		d[0] = (uint16_t)(x0 >> 16); d[ps] = (uint16_t)x0;
		d[2 * ps] = (uint16_t)(x1 >> 16); d[3 * ps] = (uint16_t)x1;
		d[4 * ps] = (uint16_t)(x2 >> 16); d[5 * ps] = (uint16_t)x2;
		d[6 * ps] = (uint16_t)(x3 >> 16); d[7 * ps] = (uint16_t)x3;
	}
}

/* ---- P: pure tables, 16 positions x 4 longs ---- */
static uint32_t p_tab[16][256][4];
static void p_prepare(const uint32_t *val)
{
	int v, p, i;
	for (i = 0; i < 16; i++)
		for (v = 0; v < 256; v++) {
			uint32_t b = val[v] & 0xff;
			memset(p_tab[i][v], 0, 16);
			for (p = 0; p < 8; p++)
				if (b & (1u << p))
					p_tab[i][v][p >> 1] |= (uint32_t)1 << ((((p & 1) ^ 1) << 4) + 15 - i);
		}
}
__attribute__((noinline)) static void p_row(const uint8_t *s, int n, uint16_t *d, long ps, long gs)
{
	int g, i;
	for (g = 0; g < n / 16; g++, s += 16, d += gs) {
		uint32_t x0 = 0, x1 = 0, x2 = 0, x3 = 0;
		for (i = 0; i < 16; i++) {
			const uint32_t *e = p_tab[i][s[i]];
			x0 |= e[0]; x1 |= e[1]; x2 |= e[2]; x3 |= e[3];
		}
		d[0] = (uint16_t)(x0 >> 16); d[ps] = (uint16_t)x0;
		d[2 * ps] = (uint16_t)(x1 >> 16); d[3 * ps] = (uint16_t)x1;
		d[4 * ps] = (uint16_t)(x2 >> 16); d[5 * ps] = (uint16_t)x2;
		d[6 * ps] = (uint16_t)(x3 >> 16); d[7 * ps] = (uint16_t)x3;
	}
}

/* ---- M: classic merge: byte table, gather, 5 merge passes ---- */
static uint8_t m_tab[256];
static void m_prepare(const uint32_t *val)
{
	int v, p;
	for (v = 0; v < 256; v++) {
		uint32_t b = val[v] & 0xff, t = 0;
		for (p = 0; p < 8; p++)
			if (b & (1u << p))
				t |= 1u << ((((p & 1) ^ 1) << 2) | (((p >> 1) & 1) << 1) | ((p >> 2) & 1));
		m_tab[v] = (uint8_t)t;
	}
}
#define G4(a) (((uint32_t)m_tab[s[a]] << 24) | ((uint32_t)m_tab[s[a + 1]] << 16) | \
	       ((uint32_t)m_tab[s[a + 2]] << 8) | m_tab[s[a + 3]])
#define MRG(a, b, sh, m) t = (((a) >> (sh)) ^ (b)) & (m); (b) ^= t; (a) ^= t << (sh)
__attribute__((noinline)) static void m_row(const uint8_t *s, int n, uint16_t *d, long ps, long gs)
{
	int g;
	for (g = 0; g < n / 16; g++, s += 16, d += gs) {
		uint32_t x0, x1, x2, x3, t;
		x3 = G4(0); x2 = G4(4); x1 = G4(8); x0 = G4(12);
		MRG(x0, x2, 8, 0x00ff00ffu); MRG(x1, x3, 8, 0x00ff00ffu);
		MRG(x0, x1, 4, 0x0f0f0f0fu); MRG(x2, x3, 4, 0x0f0f0f0fu);
		MRG(x0, x2, 1, 0x55555555u); MRG(x1, x3, 1, 0x55555555u);
		MRG(x0, x1, 16, 0x0000ffffu); MRG(x2, x3, 16, 0x0000ffffu);
		MRG(x0, x1, 2, 0x33333333u); MRG(x2, x3, 2, 0x33333333u);
		d[0] = (uint16_t)(x0 >> 16); d[ps] = (uint16_t)x0;
		d[2 * ps] = (uint16_t)(x1 >> 16); d[3 * ps] = (uint16_t)x1;
		d[4 * ps] = (uint16_t)(x2 >> 16); d[5 * ps] = (uint16_t)x2;
		d[6 * ps] = (uint16_t)(x3 >> 16); d[7 * ps] = (uint16_t)x3;
	}
}

/* ---- shared store ---- */
#define ST8(d, ps) do { \
	d[0] = (uint16_t)(x0 >> 16); d[ps] = (uint16_t)x0; \
	d[2 * ps] = (uint16_t)(x1 >> 16); d[3 * ps] = (uint16_t)x1; \
	d[4 * ps] = (uint16_t)(x2 >> 16); d[5 * ps] = (uint16_t)x2; \
	d[6 * ps] = (uint16_t)(x3 >> 16); d[7 * ps] = (uint16_t)x3; } while (0)
#define MG() do { \
	t = ((x0 >> 8) ^ x2) & 0x00ff00ffu; x2 ^= t; x0 ^= t << 8; \
	t = ((x1 >> 8) ^ x3) & 0x00ff00ffu; x3 ^= t; x1 ^= t << 8; \
	t = ((x0 >> 4) ^ x1) & 0x0f0f0f0fu; x1 ^= t; x0 ^= t << 4; \
	t = ((x2 >> 4) ^ x3) & 0x0f0f0f0fu; x3 ^= t; x2 ^= t << 4; } while (0)

/* ---- V2: one 1 KiB table (T_3), accumulate with <<1, single loop ---- */
#define A4(a) ((((((t3[s[a]] << 1) | t3[s[a + 1]]) << 1) | t3[s[a + 2]]) << 1) | t3[s[a + 3]])
__attribute__((noinline)) static void v2_row(const uint8_t *s, int n, uint16_t *d, long ps, long gs)
{
	const uint32_t *t3 = h_tab[3];
	int g;
	for (g = 0; g < n / 16; g++, s += 16, d += gs) {
		uint32_t x0, x1, x2, x3, t;
		x3 = A4(0); x2 = A4(4); x1 = A4(8); x0 = A4(12);
		MG(); ST8(d, ps);
	}
}

/* ---- V3: 4 tables, two phases per 64-pixel chunk ---- */
__attribute__((noinline)) static void v3_row(const uint8_t *s, int n, uint16_t *d, long ps, long gs)
{
	const uint32_t *t0 = h_tab[0], *t1 = h_tab[1], *t2 = h_tab[2], *t3 = h_tab[3];
	uint32_t buf[16];
	int c, i, g, ng;
	for (c = 0; c < n / 16; c += 4) {
		ng = n / 16 - c < 4 ? n / 16 - c : 4;
		for (i = 0; i < 4 * ng; i++, s += 4)
			buf[i] = t0[s[0]] | t1[s[1]] | t2[s[2]] | t3[s[3]];
		for (g = 0; g < ng; g++, d += gs) {
			uint32_t x0, x1, x2, x3, t;
			x3 = buf[4 * g]; x2 = buf[4 * g + 1]; x1 = buf[4 * g + 2]; x0 = buf[4 * g + 3];
			MG(); ST8(d, ps);
		}
	}
}

/* ---- V4: one table + accumulate, two phases ---- */
__attribute__((noinline)) static void v4_row(const uint8_t *s, int n, uint16_t *d, long ps, long gs)
{
	const uint32_t *t3 = h_tab[3];
	uint32_t buf[16];
	int c, i, g, ng;
	for (c = 0; c < n / 16; c += 4) {
		ng = n / 16 - c < 4 ? n / 16 - c : 4;
		for (i = 0; i < 4 * ng; i++, s += 4)
			buf[i] = A4(0);
		for (g = 0; g < ng; g++, d += gs) {
			uint32_t x0, x1, x2, x3, t;
			x3 = buf[4 * g]; x2 = buf[4 * g + 1]; x1 = buf[4 * g + 2]; x0 = buf[4 * g + 3];
			MG(); ST8(d, ps);
		}
	}
}
