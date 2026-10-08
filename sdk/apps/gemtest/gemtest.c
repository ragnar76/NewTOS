/*
 * gemtest.c -- NewTOS GEM (AES/VDI) diagnostic (native EL0 `.NAP`).
 *
 * The AES/VDI counterpart to the console APITEST (sdk/apps/apitest/apitest.c).
 * It systematically calls AES and VDI functions through gemlib's LP64 bindings
 * (<gem.h> -> mt_gem.h, the SAME headers/lib TeraDesk uses), checks each
 * return/output against the documented value, and writes a durable trace to
 * C:\GEMTEST.LOG with the commit-per-line pattern copied from apitest.c
 * (Fopen(rw) -> Fseek(SEEK_END) -> Fwrite -> Fclose PER LINE), so a hang/crash
 * on real HW leaves a valid N-line log on the SD card.
 *
 * Purpose: TeraDesk (LP64 GEM app) renders with broken layout / no menu / no
 * cursor on real HW. This app reproduces the SAME LP64 gemlib ABI path with a
 * minimal, fully-logged call sequence to localise WHICH AES/VDI call our TOS
 * mis-handles.
 *
 * Output policy (two phases):
 *   - Phase A (steps 1..4, appl_init..text metrics): each line is TEE'd to the
 *     framebuffer console (Cconws) AND C:\GEMTEST.LOG. The console trace stays
 *     readable while the low-level round-trips run.
 *   - Phase B (steps 5..10, the VISIBLE draws + menu + cursor + alert): console
 *     tee is switched OFF (g_screen = 0) so Cconws no longer scrolls over the
 *     graphics; every line still goes durably to the log. The screen is cleared
 *     white first (a full-screen v_bar) so the box/text/menu/cursor render on a
 *     clean background for a photo.
 *
 * Each probe logs a BEFORE line, performs the call, then an AFTER line with the
 * result and an [OK]/[FAIL got=.. exp=..] verdict. A hang/crash leaves the last
 * unpaired BEFORE line as the exact culprit. Draw calls (no single return
 * value) log the control/coordinate values they were driven with so the Mac
 * reader can eyeball them against the photo.
 *
 * Deliberately LOW extra dependency: only <mint/osbind.h> (GEMDOS trap macros
 * for the log + Pterm) and <gem.h> (the gemlib AES/VDI bindings under test),
 * plus hand-rolled string/format helpers -- no printf, no stdio.
 */

#include <mint/osbind.h>   /* GEMDOS traps: Fopen/Fseek/Fwrite/Fclose/Pterm/Cconws */
#include <gem.h>           /* gemlib AES + VDI bindings (LP64), the surface under test */

static const char LOGPATH[] = "C:\\GEMTEST.LOG";

/* 1 once C:\GEMTEST.LOG has been created; enables the commit-per-line tee. */
static int g_log_ok = 0;
/* 1 while console teeing is on (Phase A); 0 once the visible draws own the
 * screen (Phase B) so Cconws does not scroll over the graphics. */
static int g_screen = 1;

/* ------------------------------------------------------------------ *
 *  tiny freestanding string/format helpers (no libc)                  *
 * ------------------------------------------------------------------ */
static unsigned slen(const char *s)
{
	unsigned n = 0;
	while (s[n]) n++;
	return n;
}

/* Durable file tee: append one string to C:\GEMTEST.LOG and commit the
 * directory entry (Fclose) before returning. Copied verbatim from apitest.c:
 * GEMDOS has no append mode, so Fopen read/write then Fseek to the end. Any
 * error at any step -> silently skip the file half for this line. */
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

/* Emit a NUL-terminated string: durably to the log always, and to the
 * framebuffer console (Cconws) only while g_screen (Phase A). */
static void emit(const char *s)
{
	if (g_screen) Cconws(s);                /* -> host.console_write_byte */
	log_commit(s);
}

/* Line builder (identical shape to apitest.c's). */
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

/* Append an [OK] / [FAIL got=.. exp=..] verdict comparing got vs exp. */
static void b_verdict(long got, long exp)
{
	if (got == exp) {
		b_str("  [OK]");
	} else {
		b_str("  [FAIL got=");
		b_dec(got);
		b_str(" exp=");
		b_dec(exp);
		b_str("]");
	}
}

/* Append an [OK] / [FAIL got=..] verdict for a ">= threshold" expectation. */
static void b_verdict_ge(long got, long thresh)
{
	if (got >= thresh) {
		b_str("  [OK]");
	} else {
		b_str("  [FAIL got=");
		b_dec(got);
		b_str(" exp>=");
		b_dec(thresh);
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
 *  OBJECT-tree construction (LP64 gemlib OBJECT, 32 bytes)            *
 * ------------------------------------------------------------------ */

/* Fill one OBJECT. `spec` carries the raw ob_spec long: for box types a
 * (thickness<<16)|colour-word (EmuTOS gr_crack), for G_STRING/G_BUTTON/G_TITLE
 * a char* cast to long. */
static void mkobj(OBJECT *o, short type, unsigned short flags, long spec,
                  short x, short y, short w, short h,
                  short next, short head, short tail)
{
	o->ob_next  = next;
	o->ob_head  = head;
	o->ob_tail  = tail;
	o->ob_type  = (unsigned short)type;
	o->ob_flags = flags;
	o->ob_state = OS_NORMAL;
	o->ob_spec.index = spec;
	o->ob_x = x;
	o->ob_y = y;
	o->ob_width  = w;
	o->ob_height = h;
}

/* The tiny dialog tree for step 6 (root G_BOX + G_STRING + G_BUTTON). */
static OBJECT g_dialog[3];
/* The minimal menu tree for step 7 (ROOT/THEBAR/THEACTIVE + 2 titles +
 * empty THEMENUS), mirroring EmuTOS's menu OBJECT layout so our menu_bar
 * draws the bar subtree exactly as it does for a resource-loaded menu. */
static OBJECT g_menu[6];

/* Screen extents from v_opnvwk work_out (max x, max y). */
static short g_maxx = 639, g_maxy = 479;

int main(void)
{
	short   handle = 0;          /* VDI virtual-workstation handle */
	short   ap_id;
	short   wchar = 0, hchar = 0, wbox = 0, hbox = 0;
	short   phys;                /* physical AES handle from graf_handle */
	short   work_in[11];
	short   work_out[57];
	int     i;

	/* Pre-log console banner (log not created yet -> emit() is screen-only). */
	emit("\r\n=== GEMTEST START (pre-log) ===\r\n");

	/* Create/truncate the log, then switch to commit-per-line. */
	{
		long fd = Fcreate(LOGPATH, 0);
		if (fd >= 0) {
			(void)Fclose((short)fd);
			g_log_ok = 1;
		}
	}
	emit("=== GEMTEST v1 START ===\r\n");
	emit("NewTOS EL0 GEM (AES/VDI) diagnostic (C:\\APPS\\GEMTEST\\TESTGEM.NAP)\r\n");
	emit("trace -> HDMI fbcon (Phase A) + C:\\GEMTEST.LOG (commit-per-line)\r\n");
	b_reset();
	if (g_log_ok)
		b_str("G00   C:\\GEMTEST.LOG created; commit-per-line ACTIVE");
	else
		b_str("G00   Fcreate C:\\GEMTEST.LOG FAILED -> SCREEN-ONLY");
	b_nl();
	b_emit();

	/* Log the compiled OBJECT layout so the Mac reader can confirm the LP64
	 * ABI the app was built with (the prime ABI-mismatch suspect). */
	{
		OBJECT probe;
		unsigned long base = (unsigned long)&probe;
		unsigned long off_spec = (unsigned long)&probe.ob_spec - base;
		unsigned long off_x    = (unsigned long)&probe.ob_x    - base;
		b_reset();
		b_str("G00   sizeof(OBJECT)=");
		b_dec((long)sizeof(OBJECT));
		b_str(" [exp 32]  off(ob_spec)=");
		b_dec((long)off_spec);
		b_str(" [exp 16]  off(ob_x)=");
		b_dec((long)off_x);
		b_str(" [exp 24]");
		b_nl();
		b_emit();
	}

	/* ============================================================ *
	 * Step 1 -- appl_init -> ap_id (expect >= 0).                   *
	 * ============================================================ */
	before("G01 appl_init()");
	ap_id = appl_init();
	b_reset();
	b_str("G01   appl_init ap_id=");
	b_dec(ap_id);
	b_verdict_ge(ap_id, 0);
	b_nl();
	b_emit();

	/* ============================================================ *
	 * Step 2 -- VDI workstation: graf_handle + v_opnvwk.           *
	 * ============================================================ */
	before("G02 graf_handle(&wchar,&hchar,&wbox,&hbox)");
	phys = graf_handle(&wchar, &hchar, &wbox, &hbox);
	b_reset();
	b_str("G02   phys_handle=");
	b_dec(phys);
	b_str(" wchar=");
	b_dec(wchar);
	b_str(" hchar=");
	b_dec(hchar);
	b_str(" wbox=");
	b_dec(wbox);
	b_str(" hbox=");
	b_dec(hbox);
	b_verdict_ge(phys, 1);
	b_nl();
	b_emit();

	before("G03 v_opnvwk(work_in,&handle,work_out)");
	for (i = 0; i < 10; i++) work_in[i] = 1;
	work_in[10] = 2;              /* RC coordinates */
	handle = phys;               /* input: physical handle; output: vwk handle */
	for (i = 0; i < 57; i++) work_out[i] = 0;
	v_opnvwk(work_in, &handle, work_out);
	g_maxx = work_out[0];
	g_maxy = work_out[1];
	b_reset();
	b_str("G03   vwk handle=");
	b_dec(handle);
	b_str(" work_out[0]=maxx=");
	b_dec(work_out[0]);
	b_str(" work_out[1]=maxy=");
	b_dec(work_out[1]);
	b_verdict_ge(handle, 1);
	b_nl();
	b_emit();
	/* Dump the rest of work_out (the full VDI device attributes) -- TeraDesk's
	 * trace showed activity here, so log every field. */
	for (i = 0; i < 57; i += 8) {
		int j;
		b_reset();
		b_str("G03   work_out[");
		b_dec(i);
		b_str("..]=");
		for (j = i; j < i + 8 && j < 57; j++) {
			b_dec(work_out[j]);
			b_ch(' ');
		}
		b_nl();
		b_emit();
	}

	/* ============================================================ *
	 * Step 3 -- wind_get(0, WF_WORKXYWH) -> desktop work area.     *
	 * ============================================================ */
	before("G04 wind_get(0, WF_WORKXYWH, &x,&y,&w,&h)");
	{
		short wx = 0, wy = 0, ww = 0, wh = 0, rc;
		rc = wind_get(0, WF_WORKXYWH, &wx, &wy, &ww, &wh);
		b_reset();
		b_str("G04   rc=");
		b_dec(rc);
		b_str(" work x=");
		b_dec(wx);
		b_str(" y=");
		b_dec(wy);
		b_str(" w=");
		b_dec(ww);
		b_str(" h=");
		b_dec(wh);
		b_str("  [expect full screen]");
		b_verdict_ge(rc, 1);
		b_nl();
		b_emit();
	}

	/* ============================================================ *
	 * Step 4 -- VDI text metrics (vst_font/point/height/alignment/  *
	 *           effects, vqt_extent). TeraDesk crashed around       *
	 *           vst_alignment/vst_point -- exercise each, log each. *
	 * ============================================================ */
	before("G05 vst_font(handle, 1)");
	{
		short r = vst_font(handle, 1);
		b_reset();
		b_str("G05   vst_font ret=");
		b_dec(r);
		b_nl();
		b_emit();
	}

	before("G06 vst_point(handle, 10, &cw,&ch,&cellw,&cellh)");
	{
		short cw = 0, ch = 0, cellw = 0, cellh = 0, r;
		r = vst_point(handle, 10, &cw, &ch, &cellw, &cellh);
		b_reset();
		b_str("G06   vst_point ret=");
		b_dec(r);
		b_str(" charw=");
		b_dec(cw);
		b_str(" charh=");
		b_dec(ch);
		b_str(" cellw=");
		b_dec(cellw);
		b_str(" cellh=");
		b_dec(cellh);
		b_nl();
		b_emit();
	}

	before("G07 vst_height(handle, 8, &cw,&ch,&cellw,&cellh)");
	{
		short cw = 0, ch = 0, cellw = 0, cellh = 0;
		vst_height(handle, 8, &cw, &ch, &cellw, &cellh);  /* returns void */
		b_reset();
		b_str("G07   vst_height charw=");
		b_dec(cw);
		b_str(" charh=");
		b_dec(ch);
		b_str(" cellw=");
		b_dec(cellw);
		b_str(" cellh=");
		b_dec(cellh);
		b_nl();
		b_emit();
	}

	before("G08 vst_alignment(handle, TA_LEFT, TA_TOP, &hout,&vout)");
	{
		short hout = -1, vout = -1;
		vst_alignment(handle, TA_LEFT, TA_TOP, &hout, &vout);
		b_reset();
		b_str("G08   vst_alignment hout=");
		b_dec(hout);
		b_str(" vout=");
		b_dec(vout);
		b_str("  [in TA_LEFT=0 TA_TOP=5]");
		b_nl();
		b_emit();
	}

	before("G09 vst_effects(handle, TXT_NORMAL)");
	{
		short r = vst_effects(handle, TXT_NORMAL);
		b_reset();
		b_str("G09   vst_effects ret=");
		b_dec(r);
		b_nl();
		b_emit();
	}

	before("G10 vqt_extent(handle, \"GEMTEST\", extent[8])");
	{
		short ext[8];
		for (i = 0; i < 8; i++) ext[i] = 0;
		vqt_extent(handle, "GEMTEST", ext);
		b_reset();
		b_str("G10   vqt_extent =");
		for (i = 0; i < 8; i++) { b_ch(' '); b_dec(ext[i]); }
		b_str("  [pt0..pt3 x/y of the string box]");
		b_nl();
		b_emit();
	}

	/* ============================================================ *
	 * PHASE B -- visible draws own the screen. Stop console tee,    *
	 * clear to white (full-screen v_bar), then draw.               *
	 * ============================================================ */
	emit("G--   entering Phase B (visible draws); console tee OFF\r\n");
	g_screen = 0;
	{
		short full[4];
		full[0] = 0; full[1] = 0; full[2] = g_maxx; full[3] = g_maxy;
		vsf_color(handle, 0);        /* pen 0 = white background */
		v_bar(handle, full);         /* clear the screen for a clean photo */
	}

	/* ============================================================ *
	 * Step 5 -- VDI draw: filled box + label text + a line.        *
	 * ============================================================ */
	before("G11 vsf_color(2)+v_bar filled box @ (60,120,260,220)");
	{
		short pxy[4];
		pxy[0] = 60;  pxy[1] = 120;
		pxy[2] = 260; pxy[3] = 220;
		vsf_color(handle, 2);        /* fill colour index 2 (visible) */
		v_bar(handle, pxy);
		b_reset();
		b_str("G11   v_bar x1=60 y1=120 x2=260 y2=220 fillcol=2  [filled rect visible]");
		b_nl();
		b_emit();
	}

	before("G12 v_gtext(handle, 70, 130, \"GEMTEST VDI v_bar+v_gtext\")");
	{
		vsl_color(handle, 1);        /* black for the following line too */
		v_gtext(handle, 70, 130, "GEMTEST VDI v_bar+v_gtext");
		b_reset();
		b_str("G12   v_gtext @ (70,130) drawn  [text visible in/near the box]");
		b_nl();
		b_emit();
	}

	before("G13 vsl_type(LDASHED)+v_pline line (60,250)-(300,250)");
	{
		short line[4];
		line[0] = 60;  line[1] = 250;
		line[2] = 300; line[3] = 250;
		vsl_type(handle, LDASHED);
		v_pline(handle, 2, line);
		b_reset();
		b_str("G13   v_pline (60,250)-(300,250) style=LDASHED  [dashed line visible]");
		b_nl();
		b_emit();
	}

	/* ============================================================ *
	 * Step 6 -- AES objects: build a tiny tree and objc_draw it.   *
	 *           The prime ABI-mismatch suspect: if the box/string/  *
	 *           button render at the wrong coordinates, the kernel  *
	 *           is misreading the LP64 OBJECT layout.               *
	 * ============================================================ */
	before("G14 build dialog tree (G_BOX/G_STRING/G_BUTTON) + objc_draw");
	{
		/* root G_BOX: (thickness<<16)|colour-word = 0x00021100
		 *   thickness 2, border colour 1 (black), inside pattern 0 (hollow). */
		mkobj(&g_dialog[0], G_BOX, OF_NONE, 0x00021100L,
		      380, 120, 220, 110,      /* x,y,w,h (screen coords) */
		      -1, 1, 2);               /* next=-1 head=1 tail=2 */
		mkobj(&g_dialog[1], G_STRING, OF_NONE,
		      (long)(unsigned long)"OBJECT TREE (objc_draw)",
		      12, 14, 0, 16,           /* child coords relative to the box */
		      2, -1, -1);              /* next=2 head/tail=-1 */
		mkobj(&g_dialog[2], G_BUTTON, (unsigned short)(OF_SELECTABLE | OF_EXIT | OF_LASTOB),
		      (long)(unsigned long)"  OK  ",
		      80, 70, 64, 24,
		      0, -1, -1);              /* next=0(parent) head/tail=-1, LASTOB */
		objc_draw(g_dialog, ROOT, 8,
		          380, 120, 220, 110); /* clip = the box rect */
		b_reset();
		b_str("G14   tree@");
		b_hex((unsigned long)g_dialog, 8);
		b_str(" root ob_x=");
		b_dec(g_dialog[0].ob_x);
		b_str(" ob_y=");
		b_dec(g_dialog[0].ob_y);
		b_str(" ob_w=");
		b_dec(g_dialog[0].ob_width);
		b_str(" btn ob_x=");
		b_dec(g_dialog[2].ob_x);
		b_str(" ob_y=");
		b_dec(g_dialog[2].ob_y);
		b_nl();
		b_emit();
		b_reset();
		b_str("G14   [eyeball: black-bordered box @ (380,120) 220x110, string at top, OK button below]");
		b_nl();
		b_emit();
	}

	/* ============================================================ *
	 * Step 7 -- menu_bar: build a minimal menu tree, install it.   *
	 *           TeraDesk shows NO menu -- this directly tests the   *
	 *           menu_bar bar-subtree draw + return value.           *
	 * ============================================================ */
	before("G15 build minimal menu tree + menu_bar(tree, 1)");
	{
		short sw = (short)(g_maxx + 1);
		short sh = (short)(g_maxy + 1);
		short tw = (short)(6 * (wchar > 0 ? wchar : 8)); /* " Desk "/" File " */
		short r;
		/* 0 ROOT (THESCREEN): G_IBOX, head=THEBAR(1), tail=THEMENUS(5). */
		mkobj(&g_menu[0], G_IBOX, OF_NONE, 0L, 0, 0, sw, sh, -1, 1, 5);
		/* 1 THEBAR: G_BOX, next=THEMENUS, head=THEACTIVE(2), tail=2. */
		mkobj(&g_menu[1], G_BOX, OF_NONE, 0L, 0, 0, sw, 19, 5, 2, 2);
		/* 2 THEACTIVE: G_IBOX holding the titles, next=THEBAR, head=3, tail=4. */
		mkobj(&g_menu[2], G_IBOX, OF_NONE, 0L, 0, 0, sw, 19, 1, 3, 4);
		/* 3 title " Desk ": G_TITLE, next=4. */
		mkobj(&g_menu[3], G_TITLE, OF_NONE, (long)(unsigned long)" Desk ",
		      0, 0, tw, 19, 4, -1, -1);
		/* 4 title " File ": G_TITLE, LASTOB, next=THEACTIVE(2). */
		mkobj(&g_menu[4], G_TITLE, OF_LASTOB, (long)(unsigned long)" File ",
		      tw, 0, tw, 19, 2, -1, -1);
		/* 5 THEMENUS: empty G_IBOX container, next=ROOT, no children. */
		mkobj(&g_menu[5], G_IBOX, OF_NONE, 0L, 0, 0, sw, sh, 0, -1, -1);
		r = menu_bar(g_menu, 1);
		b_reset();
		b_str("G15   menu_bar(tree, 1) ret=");
		b_dec(r);
		b_str(" tree@");
		b_hex((unsigned long)g_menu, 8);
		b_verdict_ge(r, 1);
		b_nl();
		b_emit();
		b_reset();
		b_str("G15   [eyeball: menu bar strip with titles ' Desk ' ' File ' at top]");
		b_nl();
		b_emit();
	}

	/* ============================================================ *
	 * Step 8 -- graf_mouse: ARROW then M_ON. TeraDesk shows NO      *
	 *           cursor -- this tests the mouse-cursor path.        *
	 * ============================================================ */
	before("G16 graf_mouse(ARROW, 0)");
	{
		short r = graf_mouse(ARROW, 0L);
		b_reset();
		b_str("G16   graf_mouse(ARROW) ret=");
		b_dec(r);
		b_verdict_ge(r, 1);
		b_nl();
		b_emit();
	}
	before("G17 graf_mouse(M_ON, 0)");
	{
		short r = graf_mouse(M_ON, 0L);
		b_reset();
		b_str("G17   graf_mouse(M_ON) ret=");
		b_dec(r);
		b_str("  [eyeball: arrow cursor now visible]");
		b_verdict_ge(r, 1);
		b_nl();
		b_emit();
	}

	/* ============================================================ *
	 * Step 9 -- form_alert (modal). Stays on screen until a click. *
	 * ============================================================ */
	before("G18 form_alert(1, \"[1][GEMTEST:|AES/VDI done][ OK ]\")");
	{
		short r = form_alert(1, "[1][GEMTEST:|AES/VDI done][ OK ]");
		b_reset();
		b_str("G18   form_alert ret=");
		b_dec(r);
		b_verdict(r, 1);       /* single button -> exit button 1 */
		b_nl();
		b_emit();
	}

	/* ============================================================ *
	 * Step 10 -- close down, DONE, then block for a photo.         *
	 * ============================================================ */
	before("G19 v_clsvwk(handle)");
	v_clsvwk(handle);
	emit("G19   v_clsvwk done\r\n");

	before("G20 appl_exit()");
	{
		short r = appl_exit();
		b_reset();
		b_str("G20   appl_exit ret=");
		b_dec(r);
		b_nl();
		b_emit();
	}

	emit("=== GEMTEST DONE ===\r\n");

	/* Block so the screen stays for a photo. Wait for one left-button click,
	 * then a key, so either input dismisses it on real HW. */
	{
		short mx = 0, my = 0, bstate = 0, kstate = 0;
		(void)evnt_button(1, 1, 1, &mx, &my, &bstate, &kstate);
		(void)evnt_keybd();
	}

	Pterm(0);                    /* GEMDOS 0x4C -- does not return */
	return 0;                    /* unreachable */
}
