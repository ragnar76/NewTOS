/*
 * apitest.c -- NewTOS console API diagnostic (native EL0, `.NCP`).
 *
 * Runs from C:\AUTO\ (console, BEFORE the AES/desktop). It walks the
 * GEMDOS / BIOS + low-memory-sysvar surface step by step and writes a
 * progress trace to the screen AND a log file, so a real-Pi4 crash/hang
 * localises exactly which svc or memory access broke.
 *
 * Output policy (robustness principle):
 *   - PRIMARY  = the framebuffer console (HDMI). `Cconws` (GEMDOS 0x09) is
 *     wired to `host.console_write_byte`, which renders to BOTH the PL011
 *     serial AND the VT52 framebuffer console (`kernel/src/host_env.rs:17`,
 *     `fbcon::write_byte`). So every trace line appears on the display with
 *     no serial cable.
 *   - SECONDARY = C:\APITEST.LOG, written with a COMMIT-PER-LINE pattern:
 *     Fopen(rw) -> Fseek(SEEK_END) -> Fwrite -> Fclose for EVERY line. On
 *     GEMDOS/FAT the directory entry (file size) is committed on Fclose, NOT
 *     on Fwrite, so closing after each line means a crash after line N leaves
 *     a valid N-line file on the SD card (readable on the Mac). The obvious
 *     "keep one handle open, Fclose at the end" would lose everything on a
 *     mid-run fault -- which is exactly the case this tool exists to catch.
 *   - `emit()` tees to BOTH; `log_commit()` is the durable file half.
 *
 * Each probe emits a BEFORE line, performs the access, then emits an AFTER
 * line with the result. If a probe faults/hangs on real HW, the AFTER line
 * never prints, so the last unpaired BEFORE line on screen (and the last
 * line in the committed log) = the exact failing svc/access. No single
 * failing probe aborts the run: each is independent and unguarded probes
 * only READ (a stale/zero read is the headline result, not a fault).
 *
 * At the end it blocks on Cconin() ("press a key") so the trace stays
 * readable before C:\AUTO returns and the desktop clears the screen.
 *
 * Deliberately LOW dependency: it uses only the <mint/osbind.h> trap macros
 * (header-only inline SVC) plus hand-rolled string/format helpers -- NO
 * printf, NO mintlib malloc, NO stdio. The only libc pull is crt0's
 * __libc_init_heap. This keeps the diagnostic itself from depending on the
 * high-surface machinery it is meant to diagnose.
 */

#include <mint/osbind.h>

/* ------------------------------------------------------------------ *
 *  raw volatile reads (the classic direct-pointer TOS idiom)          *
 * ------------------------------------------------------------------ */
#define RD_UL(a)  (*(volatile unsigned long  *)(unsigned long)(a)) /* 8-byte */
#define RD_U32(a) (*(volatile unsigned int   *)(unsigned long)(a)) /* 4-byte */
#define RD_U16(a) (*(volatile unsigned short *)(unsigned long)(a)) /* 2-byte */

static const char LOGPATH[] = "C:\\APITEST.LOG";

/* 1 once C:\APITEST.LOG has been created; enables the commit-per-line tee. */
static int g_log_ok = 0;

/* ------------------------------------------------------------------ *
 *  tiny freestanding string/format helpers (no libc)                  *
 * ------------------------------------------------------------------ */
static unsigned slen(const char *s)
{
	unsigned n = 0;
	while (s[n]) n++;
	return n;
}

static int mcmp(const char *a, const char *b, unsigned n)
{
	unsigned i;
	for (i = 0; i < n; i++)
		if (a[i] != b[i]) return 1;
	return 0;
}

/* Durable file tee: append one string to C:\APITEST.LOG and commit the
 * directory entry (Fclose) before returning, so a later crash leaves a
 * valid file. Any GEMDOS error at any step -> silently skip the file half
 * for this line (the screen tee still shows it); logging never aborts the
 * run. GEMDOS has no append mode, so we Fopen read/write then Fseek to the
 * end. */
static void log_commit(const char *s)
{
	long fd, pos;
	if (!g_log_ok) return;
	fd = Fopen(LOGPATH, 2);                 /* S_READWRITE */
	if (fd < 0) return;
	pos = Fseek(0, (short)fd, 2);           /* SEEK_END */
	if (pos < 0) { (void)Fclose((short)fd); return; }
	(void)Fwrite((short)fd, (long)slen(s), (long)s);
	(void)Fclose((short)fd);                /* commits the FAT dir entry */
}

/* Emit a NUL-terminated string to the screen (fbcon+serial via Cconws)
 * AND durably to the log file (commit-per-line). */
static void emit(const char *s)
{
	Cconws(s);                              /* -> host.console_write_byte */
	log_commit(s);
}

/* Line builder: build a line with formatted values, then flush via emit(). */
static char LB[512];
static int  LP;

static void b_reset(void) { LP = 0; LB[0] = 0; }

static void b_ch(char c)
{
	if (LP < (int)sizeof(LB) - 1) {
		LB[LP++] = c;
		LB[LP] = 0;
	}
}

static void b_str(const char *s) { while (*s) b_ch(*s++); }

static void b_hex(unsigned long v, int mindigits)
{
	static const char H[] = "0123456789ABCDEF";
	char t[20];
	int n = 0;
	if (v == 0) t[n++] = '0';
	while (v) { t[n++] = H[v & 0xFUL]; v >>= 4; }
	while (n < mindigits) t[n++] = '0';
	b_str("0x");
	while (n) b_ch(t[--n]);
}

static void b_dec(long v)
{
	char t[24];
	int n = 0;
	unsigned long u;
	if (v < 0) { b_ch('-'); u = (unsigned long)(-(v)); }
	else       { u = (unsigned long)v; }
	if (u == 0) t[n++] = '0';
	while (u) { t[n++] = (char)('0' + (int)(u % 10UL)); u /= 10UL; }
	while (n) b_ch(t[--n]);
}

static void b_nl(void)   { b_ch('\r'); b_ch('\n'); }
static void b_emit(void) { emit(LB); }

/* Two raw hex digits (no "0x" prefix), for byte dumps. */
static void b_hh(unsigned char v)
{
	static const char H[] = "0123456789ABCDEF";
	b_ch(H[(v >> 4) & 0xF]);
	b_ch(H[v & 0xF]);
}

/* Append a success verdict: rc >= 0 -> [OK]; rc < 0 -> [FAIL rc=..]. Works
 * for Fopen/Fread (handle/count >= 0) and Dsetpath/Dgetpath (E_OK == 0). */
static void b_verdict(long rc)
{
	if (rc >= 0) {
		b_str("  [OK]");
	} else {
		b_str("  [FAIL rc=");
		b_dec(rc);
		b_str("]");
	}
}

/* BEFORE-line for a probe (constant text, ends with " ...\r\n"). */
static void before(const char *s)
{
	b_reset();
	b_str(s);
	b_str(" ...");
	b_nl();
	b_emit();
}


/* ------------------------------------------------------------------ *
 *  S1 stage probes -- helpers                                         *
 * ------------------------------------------------------------------ */

/*
 * Minimal AES binding. The GEM trap is `svc #2` with x0 = selector (200 =
 * AES) and x1 = &AESPB -- see sdk/gemlib/overlay/_gc_aes.c, which documents
 * the same ABI. Hand-rolled here so this diagnostic keeps its "osbind.h and
 * nothing else" dependency profile.
 */
typedef struct {
	short *control;
	short *global;
	short *intin;
	short *intout;
	long  *addrin;
	long  *addrout;
} AESPB;

static short aes_control[6];
static short aes_global[16];
static short aes_intin[16];
static short aes_intout[16];
static long  aes_addrin[4];
static long  aes_addrout[4];

static void aes_trap(void *pb)
{
	register long x0 __asm__("x0") = 200;
	register long x1 __asm__("x1") = (long)pb;
	__asm__ __volatile__("svc\t#2" : "+r"(x0) : "r"(x1) : "memory", "cc");
	(void)x0;
}

/* Issue one AES call; returns int_out[0] (the AES return code). */
static short aes_call(short opcode, short nin, short nout, short naddrin)
{
	AESPB pb;
	aes_control[0] = opcode;
	aes_control[1] = nin;
	aes_control[2] = nout;
	aes_control[3] = naddrin;
	aes_control[4] = 0;
	aes_intout[0] = 0;
	pb.control = aes_control;
	pb.global  = aes_global;
	pb.intin   = aes_intin;
	pb.intout  = aes_intout;
	pb.addrin  = aes_addrin;
	pb.addrout = aes_addrout;
	aes_trap(&pb);
	return aes_intout[0];
}

/*
 * ap_private out of the AESGLOBAL. gemlib's index macro is
 * `*(long *)&aes_global[3]` with no __LP64__ conditional, so for an LP64
 * caller the field is the EIGHT bytes at byte offset 6 -- read byte-wise
 * because that offset is not 8-aligned.
 */
static unsigned long ap_private_of_global(void)
{
	const unsigned char *g = (const unsigned char *)aes_global;
	unsigned long v = 0;
	int i;
	for (i = 7; i >= 0; i--)
		v = (v << 8) | (unsigned long)g[6 + i];
	return v;
}

/* The 32-byte native (LP64) GEM OBJECT: ob_spec is 8 bytes at offset 16. */
typedef struct {
	short          ob_next, ob_head, ob_tail;
	unsigned short ob_type, ob_flags, ob_state;
	unsigned long  ob_spec;
	short          ob_x, ob_y, ob_width, ob_height;
} OBJ;

/*
 * A two-object menu bar: ROOT (G_IBOX) + THEBAR (G_BOX, LASTOB). menu_bar
 * resizes THEBAR to the screen width itself and always draws the black
 * separator line under the bar, so a successful redraw is visible as a
 * 1-pixel black rule across the top of the screen.
 */
static OBJ mbar_tree[2] = {
	{ -1,  1,  1, 24 /* G_IBOX */, 0x0000, 0, 0UL,      0, 0, 640, 19 },
	{ -1, -1, -1, 20 /* G_BOX  */, 0x0020 /* LASTOB */, 0, 0x70UL, 0, 0, 640, 19 }
};

/* struct keytbl as XBIOS Keytbl returns it (EmuTOS bios/ikbd.h:57-86). */
typedef struct {
	const unsigned char *norm;
	const unsigned char *shft;
	const unsigned char *caps;
	const unsigned char *altnorm;
	const unsigned char *altshft;
	const unsigned char *altcaps;
	const unsigned char *const *dead;
	short features;
} KEYTBL;

/* ------------------------------------------------------------------ */

int main(void)
{
	/* Pre-log screen banner (log not created yet -> emit() is screen-only
	 * because g_log_ok == 0). Proves EL0 + Cconws work even if Fcreate
	 * itself faults. */
	emit("\r\n=== APITEST START (pre-log) ===\r\n");

	/* ---- T01: create/truncate the log, then switch to commit-per-line ---- */
	{
		long fd = Fcreate(LOGPATH, 0);
		if (fd >= 0) {
			(void)Fclose((short)fd);   /* created + truncated + committed */
			g_log_ok = 1;
		}
	}

	/* First COMMITTED line, before any probe: if T02+ faults, this proves on
	 * the Mac that the log path itself works on real HW. */
	emit("=== APITEST v1 START ===\r\n");
	emit("NewTOS EL0 console API diagnostic (C:\\AUTO\\APITEST.NCP)\r\n");
	emit("trace -> HDMI fbcon (Cconws) + C:\\APITEST.LOG (commit-per-line)\r\n");
	b_reset();
	if (g_log_ok)
		b_str("T01   C:\\APITEST.LOG created; commit-per-line ACTIVE");
	else
		b_str("T01   Fcreate C:\\APITEST.LOG FAILED -> SCREEN-ONLY");
	b_nl();
	b_emit();

	/* ---- T02: Sversion -- basic EL0 GEMDOS round-trip ---- */
	before("T02 Sversion() (GEMDOS 0x30)");
	{
		unsigned int sv = (unsigned int)(Sversion()) & 0xFFFFu;
		b_reset();
		b_str("T02   Sversion = ");
		b_hex(sv, 4);
		b_str("  [expect 0x3000]");
		b_nl();
		b_emit();
	}

	/* ---- T03..T07: low-memory sysvars (the #1 suspect: B1 per-process
	 *      page-0 populate visible on real HW?) ---- */

	/* T03: _sysbase @0x4F2 (LP64 8-byte read, the TeraDesk `ldr x8,[x8]`
	 *      idiom) -> OSHEADER VA. */
	before("T03 read _sysbase @0x4F2 (8-byte)");
	{
		unsigned long sb = RD_UL(0x4F2UL);
		b_reset();
		b_str("T03   _sysbase = ");
		b_hex(sb, 8);
		b_str("  [expect 0x000005C0, NOT 0]");
		b_nl();
		b_emit();

		/* T04: OSHEADER.os_version @ sysbase+2 -- only if sysbase looks
		 *      sane (inside the reserved low sysvar region). */
		before("T04 read OSHEADER.os_version @_sysbase+2");
		b_reset();
		if (sb >= 0x400UL && sb < 0x1000UL) {
			unsigned short ov = RD_U16(sb + 2);
			b_str("T04   os_version = ");
			b_hex(ov, 4);
			b_str("  [expect 0x0404]");
		} else {
			b_str("T04   SKIPPED (sysbase out of [0x400,0x1000) -> not populated)");
		}
		b_nl();
		b_emit();
	}

	/* T05: _p_cookies @0x5A0 -> cookie jar VA. */
	before("T05 read _p_cookies @0x5A0 (8-byte)");
	{
		unsigned long pc = RD_UL(0x5A0UL);
		b_reset();
		b_str("T05   _p_cookies = ");
		b_hex(pc, 8);
		b_str("  [expect 0x000005F0]");
		b_nl();
		b_emit();
	}

	/* T06: memvalid @0x420 -- RAM-config validity magic (4-byte). */
	before("T06 read memvalid @0x420 (4-byte)");
	{
		unsigned int mv = RD_U32(0x420UL);
		b_reset();
		b_str("T06   memvalid = ");
		b_hex(mv, 8);
		b_str("  [expect 0x752019F3]");
		b_nl();
		b_emit();
	}

	/* T07: drvbits @0x4C2 -- physical drive bitmap sysvar (4-byte). */
	before("T07 read drvbits @0x4C2 (4-byte)");
	{
		unsigned int db = RD_U32(0x4C2UL);
		b_reset();
		b_str("T07   drvbits = ");
		b_hex(db, 8);
		b_str("  [nonzero: C:=bit2=0x4, ...]");
		b_nl();
		b_emit();
	}

	/* ---- T08: BasePage @ VA 0 (the loader populates it; image loads at
	 *      0x1000 post-B1). p_tbase@8, p_env@0x2C, p_hitpa@4 ---- */
	before("T08 read BasePage @VA0 (p_lowtpa/p_hitpa/p_tbase/p_env)");
	{
		unsigned int lowtpa = RD_U32(0x00UL);
		unsigned int hitpa  = RD_U32(0x04UL);
		unsigned int tbase  = RD_U32(0x08UL);
		unsigned int penv   = RD_U32(0x2CUL);
		b_reset();
		b_str("T08   p_lowtpa=");
		b_hex(lowtpa, 8);
		b_str(" p_hitpa=");
		b_hex(hitpa, 8);
		b_nl();
		b_emit();
		b_reset();
		b_str("T08   p_tbase=");
		b_hex(tbase, 8);
		b_str(" [expect 0x00001000]  p_env=");
		b_hex(penv, 8);
		b_str(" [0=none]");
		b_nl();
		b_emit();
	}

	/* ---- T09: Dgetdrv -- current drive ---- */
	before("T09 Dgetdrv() (GEMDOS 0x19)");
	{
		short drv = (short)Dgetdrv();
		b_reset();
		b_str("T09   current drive = ");
		b_dec(drv);
		b_str("  (");
		b_ch((char)('A' + (drv & 0x1F)));
		b_str(":)");
		b_nl();
		b_emit();
	}

	/* ---- T10: Drvmap -- BIOS drive bitmap ---- */
	before("T10 Drvmap() (BIOS 0x0A)");
	{
		long dm = Drvmap();
		b_reset();
		b_str("T10   Drvmap = ");
		b_hex((unsigned long)dm, 8);
		b_str("  (should match drvbits sysvar T07)");
		b_nl();
		b_emit();
	}

	/* ---- T11: Dgetpath -- current path on the default drive ---- */
	before("T11 Dgetpath(buf,0) (GEMDOS 0x47)");
	{
		static char path[128];
		long r;
		unsigned i;
		for (i = 0; i < sizeof(path); i++) path[i] = 0;
		r = Dgetpath(path, 0);
		b_reset();
		b_str("T11   rc=");
		b_dec(r);
		b_str("  path=\"");
		b_str(path);
		b_str("\"");
		b_nl();
		b_emit();
	}

	/* ---- T12: Dfree(0) -- free/total clusters on the current drive.
	 *      The kernel writes 4 x 32-bit LONGs (drive.rs:68), so use a
	 *      4 x u32 buffer, NOT mintlib's _DISKINFO (4 x LP64 long). ---- */
	before("T12 Dfree(&buf,0) (GEMDOS 0x36)");
	{
		static unsigned int di[4];
		long r;
		di[0] = di[1] = di[2] = di[3] = 0;
		r = Dfree(di, 0);
		b_reset();
		b_str("T12   rc=");
		b_dec(r);
		b_str(" free_cl=");
		b_dec((long)di[0]);
		b_str(" tot_cl=");
		b_dec((long)di[1]);
		b_str(" bps=");
		b_dec((long)di[2]);
		b_str(" spc=");
		b_dec((long)di[3]);
		b_nl();
		b_emit();
	}

	/* ---- T13/T14: directory enumeration (the DTA probe). Names must be
	 *      correct byte strings; a 4-byte shift = the old DTA bug.
	 *
	 *      DTA-SAFETY: do NOT open the log (commit-per-line -> Fopen) between
	 *      Fsfirst and the final Fsnext -- a filesystem op mid-search could
	 *      disturb the active DTA search state. So collect the names into
	 *      local arrays during the loop (no fs op / no emit inside it) and
	 *      emit+commit them AFTER the enumeration finishes. ---- */
	before("T13 Fsetdta+Fsfirst(\"*.*\",0x17)+Fsnext loop (GEMDOS 0x1A/0x4E/0x4F)");
	{
		static _DTA dta;
		static char          names[30][16];
		static unsigned char attrs[30];
		static unsigned int  sizes[30];
		long fsfirst_rc, last_rc;
		int count = 0;
		int i;

		Fsetdta(&dta);
		fsfirst_rc = Fsfirst("*.*", 0x17);
		last_rc = fsfirst_rc;
		while (last_rc == 0 && count < 30) {
			/* copy the DTA fields out -- NO filesystem op inside the search */
			for (i = 0; i < 14; i++) names[count][i] = dta.dta_name[i];
			names[count][14] = 0;
			names[count][15] = 0;
			attrs[count] = (unsigned char)dta.dta_attribute;
			sizes[count] = (unsigned int)dta.dta_size;
			count++;
			last_rc = Fsnext();
		}

		/* search finished -> now it is safe to emit+commit (opens the log) */
		b_reset();
		b_str("T13   Fsfirst rc=");
		b_dec(fsfirst_rc);
		if (fsfirst_rc != 0) b_str("  [no match / empty dir?]");
		b_nl();
		b_emit();

		for (i = 0; i < count; i++) {
			b_reset();
			b_str("T14   [");
			b_dec(i);
			b_str("] name=\"");
			b_str(names[i]);
			b_str("\" attr=");
			b_hex((unsigned long)attrs[i], 2);
			b_str(" size=");
			b_dec((long)sizes[i]);
			b_nl();
			b_emit();
		}
		b_reset();
		b_str("T14   enumerated ");
		b_dec(count);
		b_str(" entr");
		b_str(count == 1 ? "y" : "ies");
		b_str(", last Fsnext rc=");
		b_dec(last_rc);
		b_nl();
		b_emit();
	}

	/* ---- T15/T16/T17: file round-trip ---- */
	{
		static const char TPATH[] = "C:\\APITST.TMP";
		static const char PAYLOAD[] = "APITEST round-trip 0123456789 ABCDEF";
		static char rbuf[64];
		long wh, rh, wr, rd;
		unsigned plen = slen(PAYLOAD);
		unsigned i;

		before("T15 Fcreate C:\\APITST.TMP + Fwrite payload");
		wh = Fcreate(TPATH, 0);
		b_reset();
		if (wh >= 0) {
			wr = Fwrite((short)wh, (long)plen, (long)PAYLOAD);
			(void)Fclose((short)wh);
			b_str("T15   handle=");
			b_dec(wh);
			b_str(" Fwrite=");
			b_dec(wr);
			b_str(" (want ");
			b_dec((long)plen);
			b_str(") + Fclose");
		} else {
			b_str("T15   Fcreate FAILED err=");
			b_dec(wh);
		}
		b_nl();
		b_emit();

		before("T16 Fopen C:\\APITST.TMP + Fread + compare");
		for (i = 0; i < sizeof(rbuf); i++) rbuf[i] = 0;
		rh = Fopen(TPATH, 0);
		b_reset();
		if (rh >= 0) {
			rd = Fread((short)rh, (long)sizeof(rbuf), (long)rbuf);
			(void)Fclose((short)rh);
			b_str("T16   handle=");
			b_dec(rh);
			b_str(" Fread=");
			b_dec(rd);
			b_str("  compare=");
			if (rd == (long)plen && mcmp(rbuf, PAYLOAD, plen) == 0)
				b_str("MATCH");
			else
				b_str("MISMATCH");
		} else {
			b_str("T16   Fopen FAILED err=");
			b_dec(rh);
		}
		b_nl();
		b_emit();

		before("T17 Fdelete C:\\APITST.TMP");
		{
			long dr = Fdelete(TPATH);
			b_reset();
			b_str("T17   Fdelete rc=");
			b_dec(dr);
			b_nl();
			b_emit();
		}
	}

	/* ---- T18/T19/T20: memory (Malloc EL0 VA -> write/read -> Mfree) ---- */
	before("T18 Malloc(1024) (GEMDOS 0x48)");
	{
		long va = Malloc(1024);
		b_reset();
		b_str("T18   Malloc = ");
		b_hex((unsigned long)va, 8);
		b_str("  [expect a mapped EL0 VA > 0x1000]");
		b_nl();
		b_emit();

		if (va > 0) {
			before("T19 write+read a byte at the Malloc VA");
			{
				volatile unsigned char *p = (volatile unsigned char *)(unsigned long)va;
				unsigned char got;
				*p = 0x5A;
				got = *p;
				b_reset();
				b_str("T19   wrote 0x5A, read ");
				b_hex((unsigned long)got, 2);
				b_str(got == 0x5A ? "  MATCH" : "  MISMATCH");
				b_nl();
				b_emit();
			}
			before("T20 Mfree the block (GEMDOS 0x49)");
			{
				long mr = Mfree(va);
				b_reset();
				b_str("T20   Mfree rc=");
				b_dec(mr);
				b_nl();
				b_emit();
			}
		} else {
			before("T19/T20 SKIPPED (Malloc returned no VA)");
			emit("T19/T20   skipped\r\n");
		}
	}

	/* ---- T21/T22: time (raw packed DOS words) ---- */
	before("T21 Tgettime() (GEMDOS 0x2C)");
	{
		unsigned int tt = (unsigned int)(Tgettime()) & 0xFFFFu;
		b_reset();
		b_str("T21   Tgettime raw = ");
		b_hex(tt, 4);
		b_str("  (packed hhhhh mmmmmm sssss)");
		b_nl();
		b_emit();
	}
	before("T22 Tgetdate() (GEMDOS 0x2A)");
	{
		unsigned int td = (unsigned int)(Tgetdate()) & 0xFFFFu;
		b_reset();
		b_str("T22   Tgetdate raw = ");
		b_hex(td, 4);
		b_str("  (packed yyyyyyy mmmm ddddd, y+1980)");
		b_nl();
		b_emit();
	}

	/* ================================================================
	 * T23..T27: QED rsrc_load reproduction (console, no AES). Replicates
	 * exactly the Fopen that QED's rsrc_load performs, so we can reproduce
	 * "RSC not found" on real HW from the AUTO console app, with the same
	 * durable commit-per-line log. Card layout for reference:
	 *   C:\qed.rsc            (root, 41166 bytes)
	 *   C:\APPS\QED\qed.rsc   (subdir copy)
	 *   C:\APPS\QED\QED.NAP   (the app)
	 * ================================================================ */
	{
		static char          savebuf[128];
		static char          nowbuf[128];
		static unsigned char hbuf[16];
		long rc, rc2;
		unsigned i;

		/* T23 -- absolute drive path (the one the stale trace shows
		 *        failing): the exact string rsrc_load("C:\qed.rsc") used. */
		before("T23 Fopen(\"C:\\qed.rsc\",0) -- absolute rsrc_load path");
		rc = Fopen("C:\\qed.rsc", 0);
		b_reset();
		b_str("T23   Fopen rc/handle=");
		b_dec(rc);
		b_verdict(rc);
		b_nl();
		b_emit();
		if (rc >= 0) (void)Fclose((short)rc);

		/* T24 -- bare name from CWD=root (C:\qed.rsc exists -> resolves). */
		before("T24 Fopen(\"qed.rsc\",0) -- bare name from CWD (root)");
		rc = Fopen("qed.rsc", 0);
		b_reset();
		b_str("T24   Fopen rc/handle=");
		b_dec(rc);
		b_verdict(rc);
		b_nl();
		b_emit();
		if (rc >= 0) (void)Fclose((short)rc);

		/* T25 -- relative from the QED subdir (how the desktop launches
		 *        QED: chdir into C:\APPS\QED, then open by bare name). */
		before("T25 chdir C:\\APPS\\QED + relative Fopen (qed.rsc, QED.NAP)");
		/* 25.1 save the current dir first (restored in T27) */
		for (i = 0; i < sizeof(savebuf); i++) savebuf[i] = 0;
		rc = Dgetpath(savebuf, 0);
		b_reset();
		b_str("T25.1 Dgetpath(save,0) rc=");
		b_dec(rc);
		b_str(" cwd=\"");
		b_str(savebuf);
		b_str("\"");
		b_verdict(rc);
		b_nl();
		b_emit();
		/* 25.2 change into the QED subdir */
		rc = Dsetpath("C:\\APPS\\QED");
		b_reset();
		b_str("T25.2 Dsetpath(\"C:\\APPS\\QED\") rc=");
		b_dec(rc);
		b_verdict(rc);
		b_nl();
		b_emit();
		/* 25.3 confirm the CWD actually changed */
		for (i = 0; i < sizeof(nowbuf); i++) nowbuf[i] = 0;
		rc = Dgetpath(nowbuf, 0);
		b_reset();
		b_str("T25.3 Dgetpath(now,0) rc=");
		b_dec(rc);
		b_str(" cwd=\"");
		b_str(nowbuf);
		b_str("\"");
		b_verdict(rc);
		b_nl();
		b_emit();
		/* 25.4 relative RSC open from the subdir */
		rc = Fopen("qed.rsc", 0);
		b_reset();
		b_str("T25.4 Fopen(\"qed.rsc\",0) rel rc/handle=");
		b_dec(rc);
		b_verdict(rc);
		b_nl();
		b_emit();
		if (rc >= 0) (void)Fclose((short)rc);
		/* 25.5 relative NAP open */
		rc2 = Fopen("QED.NAP", 0);
		b_reset();
		b_str("T25.5 Fopen(\"QED.NAP\",0) rel rc/handle=");
		b_dec(rc2);
		b_verdict(rc2);
		b_nl();
		b_emit();
		if (rc2 >= 0) (void)Fclose((short)rc2);

		/* T26 -- actually READ the RSC header (16 bytes) to confirm we can
		 *        read content, not just open. rsh_vrsn is the first WORD. */
		before("T26 Fopen C:\\qed.rsc + Fread 16 bytes (RSC header)");
		rc = Fopen("C:\\qed.rsc", 0);
		b_reset();
		if (rc >= 0) {
			long got;
			for (i = 0; i < sizeof(hbuf); i++) hbuf[i] = 0;
			got = Fread((short)rc, 16, (long)hbuf);
			(void)Fclose((short)rc);
			b_str("T26   Fread=");
			b_dec(got);
			b_str(" hdr=");
			for (i = 0; i < 16; i++) { b_hh(hbuf[i]); b_ch(' '); }
			b_verdict(got);
		} else {
			b_str("T26   Fopen rc=");
			b_dec(rc);
			b_verdict(rc);
		}
		b_nl();
		b_emit();

		/* T27 -- RESTORE the CWD (critical). The GEMDOS current directory is
		 *        GLOBAL in single-tasking TOS, so leaving it at C:\APPS\QED
		 *        would leak into the next AUTO program AND the desktop. If the
		 *        saved path was empty/root, restore to "\". */
		before("T27 restore CWD via Dsetpath(saved)");
		if (savebuf[0] == 0) { savebuf[0] = '\\'; savebuf[1] = 0; }
		rc = Dsetpath(savebuf);
		b_reset();
		b_str("T27   Dsetpath(\"");
		b_str(savebuf);
		b_str("\") rc=");
		b_dec(rc);
		b_verdict(rc);
		b_nl();
		b_emit();
	}

	/* ================================================================ *
	 *  S1 stage: the four AES/BIOS server corrections                  *
	 * ================================================================ */

	/* ---- T28 (S1b): appl_init -> ap_private == rs_trees[DESKTOP][ROOT]
	 *      .ob_spec. 0x1143 (4419) with no #Q line in C:\NEWDESK.INF;
	 *      0x1100 | patcol when the #Q line supplies one (only the LOW
	 *      byte is replaced -- EmuTOS geminit.c:635-641). A real TT
	 *      running EmuTOS measures 0x1143. ---- */
	before("T28 appl_init() -> ap_private (AES opcode 10)");
	{
		short apid;
		unsigned long apriv;
		int i;
		for (i = 0; i < 16; i++) aes_global[i] = 0;
		apid  = aes_call(10, 0, 1, 0);
		apriv = ap_private_of_global();
		b_reset();
		b_str("T28   ap_id = ");
		b_dec(apid);
		b_str("  ap_private = ");
		b_hex(apriv, 4);
		b_str("  [expect 0x1143 without #Q, 0x11xx with]");
		if ((apriv & 0xFFFFFF00UL) == 0x1100UL)
			b_str("  [OK]");
		else
			b_str("  [FAIL: high 24 bits are not 0x1100]");
		b_nl();
		b_emit();
	}

	/* ---- T29 (S1c): the DTA lives in the running process's basepage
	 *      (p_xdta), so a Pexec'd child cannot move the parent's.
	 *      C:\DTACHILD.NCP does Fsetdta(0x2000) and exits. ---- */
	before("T29 Fsetdta(buf); Pexec(C:\\DTACHILD.NCP); Fgetdta()==buf");
	{
		static char dtabuf[44];
		unsigned long want = (unsigned long)(void *)dtabuf;
		unsigned long got0, got1;
		long prc;

		Fsetdta(dtabuf);
		got0 = (unsigned long)Fgetdta();
		b_reset();
		b_str("T29.1 Fsetdta/Fgetdta round-trip: set=");
		b_hex(want, 4);
		b_str(" got=");
		b_hex(got0, 4);
		if (got0 == want) b_str("  [OK]"); else b_str("  [FAIL]");
		b_nl();
		b_emit();

		prc = Pexec(0, "C:\\DTACHILD.NCP", "", 0L);
		b_reset();
		b_str("T29.2 Pexec(0,C:\\DTACHILD.NCP) rc=");
		b_dec(prc);
		b_nl();
		b_emit();

		got1 = (unsigned long)Fgetdta();
		b_reset();
		b_str("T29.3 parent Fgetdta after child = ");
		b_hex(got1, 4);
		if (prc < 0) {
			b_str("  [child did not run -- T29.3 inconclusive]");
		} else if (got1 == want) {
			b_str("  [OK: child did not disturb the parent DTA]");
		} else {
			b_str("  [FAIL: expected ");
			b_hex(want, 4);
			b_str("]");
		}
		b_nl();
		b_emit();
	}

	/* ---- T30 (S1d): Keytbl(-1,-1,-1) returns a struct keytbl the caller
	 *      can DEREFERENCE from EL0 (EmuTOS's desktop does exactly that,
	 *      desk/deskmain.c:1144-1145). shft[0x1E] is the shifted form of
	 *      the 'a' key = 'A' on the US layout. ---- */
	before("T30 k = Keytbl(-1,-1,-1); read k->shft[0x1E] from EL0");
	{
		KEYTBL *k = (KEYTBL *)Keytbl(-1L, -1L, -1L);
		b_reset();
		b_str("T30.1 Keytbl returned ");
		b_hex((unsigned long)(void *)k, 4);
		b_nl();
		b_emit();
		before("T30.2 dereference k->shft / k->norm");
		b_reset();
		if (k == 0) {
			b_str("T30.2 NULL -- no keyboard translator on this board");
		} else {
			unsigned char sh = k->shft[0x1E];
			unsigned char no = k->norm[0x1E];
			b_str("T30.2 norm[0x1E]='");
			b_ch((char)(no ? no : '?'));
			b_str("' shft[0x1E]='");
			b_ch((char)(sh ? sh : '?'));
			b_str("' (");
			b_hex(sh, 2);
			b_str(") features=");
			b_dec(k->features);
			if (sh == 'A') b_str("  [OK]"); else b_str("  [FAIL: expected 'A']");
		}
		b_nl();
		b_emit();
	}

	/* ---- T31 (S1a): menu_bar(NULL, 0) must be a legal call -- EmuTOS's
	 *      mn_bar never touches `tree` on the hide path (gemmnlib.c:
	 *      724-728). Afterwards menu_bar(tree, 1) must draw again; the
	 *      visible proof is the black separator rule at y = 18 across the
	 *      full screen width. ---- */
	before("T31 menu_bar(NULL,0) then menu_bar(tree,1)");
	{
		short r0, r1;
		aes_addrin[0] = 0L;               /* NULL tree */
		aes_intin[0]  = 0;                /* hide */
		r0 = aes_call(30, 1, 1, 1);
		b_reset();
		b_str("T31.1 menu_bar(NULL,0) returned ");
		b_dec(r0);
		if (r0 == 1) b_str("  [OK: survived, reports 1]");
		else b_str("  [FAIL: expected 1]");
		b_nl();
		b_emit();

		aes_addrin[0] = (long)(void *)mbar_tree;
		aes_intin[0]  = 1;                /* show */
		r1 = aes_call(30, 1, 1, 1);
		b_reset();
		b_str("T31.2 menu_bar(tree,1) returned ");
		b_dec(r1);
		b_str("  (look for the black rule at y=18)");
		if (r1 == 1) b_str("  [OK]"); else b_str("  [FAIL: expected 1]");
		b_nl();
		b_emit();
	}

	/* ---- done: the DONE marker is committed per-line like every other line
	 *      (no trailing Fclose to lose). Then pause so the screen stays
	 *      readable before C:\AUTO returns and the desktop clears it. ---- */
	emit("\r\n=== APITEST DONE ===\r\n");
	g_log_ok = 0;                /* stop teeing to file: prompt is screen-only */
	emit("Press any key to continue (then the desktop starts)...\r\n");
	(void)Cconin();              /* blocks; USB-HID/IKBD from AUTO */

	Pterm(0);                    /* GEMDOS 0x4C -- does not return */
	return 0;                    /* unreachable; crt0 would Pterm(0) anyway */
}
