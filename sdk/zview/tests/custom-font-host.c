/*
 * custom-font-host.c -- TASK/ZVIEW01 host test for zView's draw_text()
 * (zview/custom_font.c as the SDK builds it: the overlay
 * sdk/zview/overlay/zview/custom_font.c if present, else the pristine file;
 * the runner passes it as ZVIEW_CUSTOM_FONT_C).
 *
 * draw_text() blits each character as an 8-row, 1-plane MFDB through
 * vrt_cpyfm. On NewTOS an MFDB form is an array of host-order 16-bit WORDs
 * with bit 15 the leftmost pixel (tos-api/src/vdi/backend.rs MonoSrc; the
 * same contract as EmuTOS vrt_cpyfm16, vdi/vdi_raster.c:1241-1243 reading
 * UWORDs with mask 0x8000 >> x). The 68k original stores glyph row r in the
 * high byte of fx_mem[r], the row bytes being my_font_8[] read big-endian
 * (two rows per WORD, row 2k in the high byte). This test checks, for every
 * character 1..255, that the form handed to vrt_cpyfm is exactly that, and
 * for two glyphs against hand-read rows ('Z', 'A'). The pristine file fails
 * on a little-endian host: its byte walk lands every row in bits 7..0 and
 * swaps row pairs, so the 2..8 copied columns are empty -- the browser's
 * missing file names (measurements/measurement-2026-10-07-zview01).
 *
 * Build:  cc -fsigned-char -I sdk/zview/tests -I sdk/zview/z-tools-04805b7/zview \
 *           -DZVIEW_CUSTOM_FONT_C='"<path to custom_font.c>"' sdk/zview/tests/custom-font-host.c
 */
#include <stdio.h>
#include "general.h"
#include ZVIEW_CUSTOM_FONT_C

static unsigned short got[256][8];
static short got_w[256];
static int calls;

void vrt_cpyfm( short handle, short mode, short *pxy, MFDB *src, MFDB *dst, short *color)
{
	const unsigned short *w = src->fd_addr;
	int r;
	(void)handle; (void)mode; (void)dst; (void)color;
	if (calls < 256) {
		for (r = 0; r < 8; r++)
			got[calls][r] = w[r];
		got_w[calls] = pxy[2] - pxy[0] + 1;
	}
	calls++;
}

/* row r of character c as the 68k reads it: byte r of the big-endian
 * serialisation of my_font_8[(c-1)*4 .. +3] */
static unsigned row_be(int c, int r)
{
	unsigned short v = (unsigned short)my_font_8[((c - 1) << 2) + (r >> 1)];
	return (r & 1) ? (v & 0xFFu) : (v >> 8);
}

int main(void)
{
	static const unsigned char z_rows[8] = { 0x7C, 0x04, 0x08, 0x10, 0x20, 0x40, 0x7C, 0x00 };
	static const unsigned char a_rows[8] = { 0x10, 0x28, 0x44, 0x44, 0x7C, 0x44, 0x44, 0x00 };
	char one[2] = { 0, 0 };
	int c, r, bad = 0, lit = 0;

	for (c = 1; c < 256; c++) {
		calls = 0;
		one[0] = (char)c;
		draw_text(1, 0, 0, BLACK, one);
		if (calls != 1) { printf("char %d: %d vrt_cpyfm calls\n", c, calls); bad++; continue; }
		if (got_w[0] != ofwf[c]) { printf("char %d: width %d, want %d\n", c, got_w[0], ofwf[c]); bad++; }
		for (r = 0; r < 8; r++) {
			unsigned want = row_be(c, r) << 8;
			if (got[0][r] != want) {
				if (bad < 20)
					printf("char %d row %d: form word %04x, want %04x\n", c, r, got[0][r], want);
				bad++;
			}
			/* pixels inside the copied columns 0..width-1 */
			if (got[0][r] & (0xFFFFu << (16 - got_w[0])) & 0xFFFFu)
				lit++;
		}
	}
	for (r = 0; r < 8; r++) {
		if (row_be('Z', r) != z_rows[r]) { printf("'Z' row %d: %02x want %02x\n", r, row_be('Z', r), z_rows[r]); bad++; }
		if (row_be('A', r) != a_rows[r]) { printf("'A' row %d: %02x want %02x\n", r, row_be('A', r), a_rows[r]); bad++; }
	}
	printf("custom-font-host: %d mismatches, %d glyph rows with pixels in the copied columns\n", bad, lit);
	return (bad == 0 && lit > 0) ? 0 : 1;
}
