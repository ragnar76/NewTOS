#include "general.h"
#include "custom_font.h"
static char rows[8][256]; static int ncol;
void vrt_cpyfm(short h, short mode, short *pxy, MFDB *src, MFDB *dst, short *col)
{
	unsigned short *w = src->fd_addr; int r, c, any = 0;
	(void)h; (void)mode; (void)dst; (void)col;
	for (r = 0; r < 8; r++) {
		for (c = pxy[0]; c <= pxy[2]; c++) {
			int set = (w[r] & (0x8000u >> c)) != 0;  /* NewTOS MonoSrc convention */
			rows[r][ncol + c] = set ? '#' : '.'; any |= set;
		}
	}
	printf("glyph cols %d..%d words:", pxy[0], pxy[2]);
	for (r = 0; r < 8; r++) printf(" %04x", w[r]);
	printf("  -> %s\n", any ? "pixels" : "NO PIXELS");
	ncol += pxy[2] - pxy[0] + 1;
}
int main(void)
{
	int r;
	memset(rows, ' ', sizeof rows);
	draw_text(1, 0, 0, BLACK, "ZVIEW");
	for (r = 0; r < 8; r++) { rows[r][ncol] = 0; printf("|%s|\n", rows[r]); }
	return 0;
}
