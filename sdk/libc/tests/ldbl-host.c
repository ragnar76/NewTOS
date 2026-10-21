/*
 * ldbl-host.c -- executor of the long double host test (TASK/SDK01 J).
 *
 *   ldbl-host <program.elf>  < requests  > results
 *
 * <program.elf> is ldbl-elf.c linked by ldbl-host.sh. This program loads it
 * (the loader of sdk/compiler-rt/tests/rt-host.c: PT_LOAD segments,
 * R_AARCH64_RELATIVE relocations, text R-X; see there for the 16 KiB data page
 * and the x18 rebuild), finds `ldbl_table' and calls the target's libc/libm
 * machine code on the AArch64 host CPU. It computes nothing itself: every
 * request line names a table entry and its arguments, the reply line carries
 * the raw result (binary128 as two 64-bit hex words, strings verbatim). The
 * oracle that judges the replies is ldbl-oracle.py (exact rational arithmetic
 * in Python), which also writes the requests.
 *
 * Request: fields separated by TAB, first field = table name. Arguments:
 *   q  binary128 as "<hi>:<lo>" (hex, 64 bits each), d/f = hex bit pattern,
 *   i  decimal integer, s  string (no TAB/newline).
 * Reply: one line per request, fields separated by TAB.
 */
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <libkern/OSCacheControl.h>
#else
static void sys_icache_invalidate(void *p, size_t n) { __builtin___clear_cache((char *)p, (char *)p + n); }
#endif

#if !defined(__aarch64__)
int main(void) { puts("SKIP: not an AArch64 host"); return 77; }
#else

typedef uint64_t q_t __attribute__((vector_size(16)));   /* binary128 carrier, [0] = low */

/* ---------------------------------------------------------------- ELF ---- */
/* The loader of sdk/compiler-rt/tests/rt-host.c, looking up `ldbl_table'. */
typedef struct { unsigned char ident[16]; uint16_t type, machine; uint32_t version;
	uint64_t entry, phoff, shoff; uint32_t flags; uint16_t ehsize, phentsize, phnum,
	shentsize, shnum, shstrndx; } Ehdr;
typedef struct { uint32_t type, flags; uint64_t offset, vaddr, paddr, filesz, memsz, align; } Phdr;
typedef struct { uint32_t name, type; uint64_t flags, addr, offset, size; uint32_t link, info;
	uint64_t addralign, entsize; } Shdr;
typedef struct { uint32_t name; unsigned char info, other; uint16_t shndx; uint64_t value, size; } Sym;
typedef struct { uint64_t offset, info; int64_t addend; } Rela;

static const void *load(const char *path, const char *symbol)
{
	int fd = open(path, O_RDONLY);
	struct stat st;
	if (fd < 0 || fstat(fd, &st) != 0) { perror(path); exit(2); }
	unsigned char *f = malloc(st.st_size);
	if (read(fd, f, st.st_size) != st.st_size) { perror("read"); exit(2); }
	close(fd);
	const Ehdr *e = (const Ehdr *)f;
	if (memcmp(e->ident, "\177ELF", 4) || e->ident[4] != 2 || e->ident[5] != 1 ||
	    e->machine != 183 || e->type != 3) {
		fprintf(stderr, "%s: not an ELF64 LE AArch64 ET_DYN\n", path); exit(2);
	}
	const Phdr *ph = (const Phdr *)(f + e->phoff);
	uint64_t span = 0;
	for (int i = 0; i < e->phnum; i++)
		if (ph[i].type == 1 && ph[i].vaddr + ph[i].memsz > span)
			span = ph[i].vaddr + ph[i].memsz;
	long pg = sysconf(_SC_PAGESIZE);
	span = (span + pg - 1) & ~(uint64_t)(pg - 1);
	unsigned char *base = mmap(NULL, span, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
	if (base == MAP_FAILED) { perror("mmap"); exit(2); }
	for (int i = 0; i < e->phnum; i++)
		if (ph[i].type == 1)
			memcpy(base + ph[i].vaddr, f + ph[i].offset, ph[i].filesz);
	const Shdr *sh = (const Shdr *)(f + e->shoff);
	const char *shstr = (const char *)f + sh[e->shstrndx].offset;
	uint64_t table = 0;
	for (int i = 0; i < e->shnum; i++) {
		if (sh[i].type == 4) {                               /* SHT_RELA */
			const Rela *r = (const Rela *)(f + sh[i].offset);
			for (uint64_t k = 0; k < sh[i].size / sizeof(Rela); k++) {
				if ((uint32_t)r[k].info != 1027) {           /* R_AARCH64_RELATIVE */
					fprintf(stderr, "%s: relocation type %u in %s\n", path,
						(unsigned)(uint32_t)r[k].info, shstr + sh[i].name);
					exit(2);
				}
				*(uint64_t *)(base + r[k].offset) = (uint64_t)base + r[k].addend;
			}
		} else if (sh[i].type == 2) {                        /* SHT_SYMTAB */
			const Sym *s = (const Sym *)(f + sh[i].offset);
			const char *str = (const char *)f + sh[sh[i].link].offset;
			for (uint64_t k = 0; k < sh[i].size / sizeof(Sym); k++)
				if (!strcmp(str + s[k].name, symbol)) table = s[k].value;
		}
	}
	for (int i = 0; i < e->phnum; i++) {
		if (ph[i].type != 1) continue;
		if (ph[i].flags & 1) {                               /* PF_X */
			uint64_t end = (ph[i].vaddr + ph[i].memsz + pg - 1) & ~(uint64_t)(pg - 1);
			if (mprotect(base, end, PROT_READ | PROT_EXEC) != 0) { perror("mprotect R-X"); exit(2); }
		} else if (ph[i].vaddr & (pg - 1)) {
			fprintf(stderr, "%s: writable segment at 0x%llx is not on a %ld-byte page\n",
				path, (unsigned long long)ph[i].vaddr, pg);
			exit(2);
		}
	}
	sys_icache_invalidate(base, span);
	if (!table) { fprintf(stderr, "%s: no %s symbol\n", path, symbol); exit(2); }
	free(f);
	return base + table;
}

/* ------------------------------------------------------------ the table ---- */
enum { K_FMT = 1, K_FMTD, K_SCAN, K_SCAND, K_STRTOLD, K_STRTOD, K_ECVT, K_ECVT_R, K_GCVT,
       K_Q_Q, K_Q_QQ, K_Q_QQQ, K_Q_QI, K_Q_QL, K_D_DL, K_F_FL, K_Q_QPI, K_I_Q, K_Q_QPQ,
       K_Q_QQPI };
struct ent { const char *name; void *fn; long kind; };
static const struct ent *T;

static const struct ent *find(const char *name)
{
	for (const struct ent *e = T; e->name; e++)
		if (!strcmp(e->name, name)) return e;
	fprintf(stderr, "unknown function %s\n", name);
	exit(2);
}

static q_t pq(const char *s)
{
	char *e;
	q_t q;
	q[1] = strtoull(s, &e, 16);
	if (*e != ':') { fprintf(stderr, "bad binary128 '%s'\n", s); exit(2); }
	q[0] = strtoull(e + 1, &e, 16);
	return q;
}
static void oq(q_t q) { printf("%016llx:%016llx", (unsigned long long)q[1], (unsigned long long)q[0]); }

int main(int argc, char **argv)
{
	if (argc != 2) { fprintf(stderr, "usage: ldbl-host <program.elf>\n"); return 2; }
	T = load(argv[1], "ldbl_table");
	static char line[1 << 16], buf[1 << 16];   /* the subnormal midpoints have ~16500 digits */
	char *f[8];
	while (fgets(line, sizeof line, stdin)) {
		line[strcspn(line, "\n")] = 0;
		int n = 0;
		for (char *p = line; n < 8; ) {
			f[n++] = p;
			p = strchr(p, '\t');
			if (!p) break;
			*p++ = 0;
		}
		const struct ent *e = find(f[0]);
		switch (e->kind) {
		case K_FMT: {
			int rc = ((int (*)(char *, size_t, const char *, q_t))e->fn)(buf, sizeof buf, f[1], pq(f[2]));
			printf("%d\t%s", rc, buf);
			break; }
		case K_FMTD: {
			uint64_t u = strtoull(f[2], 0, 16); double d; memcpy(&d, &u, 8);
			int rc = ((int (*)(char *, size_t, const char *, double))e->fn)(buf, sizeof buf, f[1], d);
			printf("%d\t%s", rc, buf);
			break; }
		case K_SCAN: {
			q_t o = { 0, 0 };
			int rc = ((int (*)(const char *, const char *, q_t *))e->fn)(f[2], f[1], &o);
			printf("%d\t", rc); oq(o);
			break; }
		case K_SCAND: {
			double o = 0; uint64_t u;
			int rc = ((int (*)(const char *, const char *, double *))e->fn)(f[2], f[1], &o);
			memcpy(&u, &o, 8);
			printf("%d\t%016llx", rc, (unsigned long long)u);
			break; }
		case K_STRTOLD: {
			char *end;
			q_t r = ((q_t (*)(const char *, char **))e->fn)(f[1], &end);
			oq(r); printf("\t%ld", (long)(end - f[1]));
			break; }
		case K_STRTOD: {
			char *end; uint64_t u;
			double r = ((double (*)(const char *, char **))e->fn)(f[1], &end);
			memcpy(&u, &r, 8);
			printf("%016llx\t%ld", (unsigned long long)u, (long)(end - f[1]));
			break; }
		case K_ECVT: {
			int dp = -99999, sg = -1;
			char *r = ((char *(*)(q_t, int, int *, int *))e->fn)(pq(f[1]), atoi(f[2]), &dp, &sg);
			printf("%s\t%d\t%d", r, dp, sg);
			break; }
		case K_ECVT_R: {
			int dp = -99999, sg = -1;
			buf[0] = 0;
			int rc = ((int (*)(q_t, int, int *, int *, char *, size_t))e->fn)(pq(f[1]), atoi(f[2]), &dp, &sg, buf, sizeof buf);
			printf("%d\t%s\t%d\t%d", rc, buf, dp, sg);
			break; }
		case K_GCVT: {
			char *r = ((char *(*)(q_t, int, char *))e->fn)(pq(f[1]), atoi(f[2]), buf);
			printf("%s", r == buf ? buf : "<not buf>");
			break; }
		case K_Q_Q: oq(((q_t (*)(q_t))e->fn)(pq(f[1]))); break;
		case K_Q_QQ: oq(((q_t (*)(q_t, q_t))e->fn)(pq(f[1]), pq(f[2]))); break;
		case K_Q_QQQ: oq(((q_t (*)(q_t, q_t, q_t))e->fn)(pq(f[1]), pq(f[2]), pq(f[3]))); break;
		case K_Q_QI: oq(((q_t (*)(q_t, int))e->fn)(pq(f[1]), atoi(f[2]))); break;
		case K_Q_QL: oq(((q_t (*)(q_t, long))e->fn)(pq(f[1]), atol(f[2]))); break;
		case K_D_DL: {
			uint64_t u = strtoull(f[1], 0, 16); double d; memcpy(&d, &u, 8);
			d = ((double (*)(double, long))e->fn)(d, atol(f[2]));
			memcpy(&u, &d, 8); printf("%016llx", (unsigned long long)u);
			break; }
		case K_F_FL: {
			uint32_t u = (uint32_t)strtoul(f[1], 0, 16); float d; memcpy(&d, &u, 4);
			d = ((float (*)(float, long))e->fn)(d, atol(f[2]));
			memcpy(&u, &d, 4); printf("%08x", u);
			break; }
		case K_Q_QPI: {
			int x = -12345;
			q_t r = ((q_t (*)(q_t, int *))e->fn)(pq(f[1]), &x);
			oq(r); printf("\t%d", x);
			break; }
		case K_I_Q: printf("%d", ((int (*)(q_t))e->fn)(pq(f[1]))); break;
		case K_Q_QPQ: {
			q_t ip = { 0, 0 };
			q_t r = ((q_t (*)(q_t, q_t *))e->fn)(pq(f[1]), &ip);
			oq(r); printf("\t"); oq(ip);
			break; }
		case K_Q_QQPI: {
			int x = -12345;
			q_t r = ((q_t (*)(q_t, q_t, int *))e->fn)(pq(f[1]), pq(f[2]), &x);
			oq(r); printf("\t%d", x);
			break; }
		default: fprintf(stderr, "bad kind %ld\n", e->kind); return 2;
		}
		printf("\n");
	}
	return 0;
}
#endif
