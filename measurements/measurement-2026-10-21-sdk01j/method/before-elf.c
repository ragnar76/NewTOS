/* before-elf.c -- the libc-only part of sdk/libc/tests/ldbl-elf.c (same table layout, kinds and wrappers), for before/after
   on the host: it links against the HEAD libc.a, whose libm lacks the `l' functions ldbl-elf.c references. */
#include <stdio.h>
#include <stdlib.h>
typedef long double tf;
extern char *qecvt(tf, int, int *, int *);
extern char *qgcvt(tf, int, char *);
extern int qfcvt_r(tf, int, int *, int *, char *, size_t);
int ld_snprintf(char *b, size_t n, const char *f, tf v) { return snprintf(b, n, f, v); }
int ld_snprintf_d(char *b, size_t n, const char *f, double v) { return snprintf(b, n, f, v); }
int ld_sscanf(const char *s, const char *f, tf *o) { return sscanf(s, f, o); }
enum { K_FMT = 1, K_FMTD, K_SCAN, K_SCAND, K_STRTOLD, K_STRTOD, K_ECVT, K_ECVT_R, K_GCVT };
struct ent { const char *name; void *fn; long kind; };
const struct ent ldbl_table[] = {
	{ "snprintf", (void *)ld_snprintf, K_FMT }, { "snprintfd", (void *)ld_snprintf_d, K_FMTD },
	{ "sscanf", (void *)ld_sscanf, K_SCAN }, { "strtold", (void *)strtold, K_STRTOLD },
	{ "qecvt", (void *)qecvt, K_ECVT }, { "qfcvt_r", (void *)qfcvt_r, K_ECVT_R }, { "qgcvt", (void *)qgcvt, K_GCVT },
	{ 0, 0, 0 } };
int main(void) { return ldbl_table[0].fn == 0; }
