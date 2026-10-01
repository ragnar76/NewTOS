/*
 * quittest.c -- H03 probe: replay TeraDesk's start-up + regular-quit AES/VDI/
 * GEMDOS call sequence (sdk/apps/teradesk/main.c, Single-TOS build) as a
 * `#Z`-autorun `.NAP`, logging every step over Cconws (-> serial + fbcon), so
 * QEMU raspi4b can tell whether the kernel comes back to `desk/` after
 * `Pterm(0)` or where exactly it stops.
 *
 * MODE (compile-time, -DMODE=n):
 *   1  full TeraDesk QUITQUIT path  (main.c:1943-1955, 1972-1992, 2124)
 *   2  same, but ALSO shel_write(SHW_SHUTDOWN, 2, 2, NULL, NULL) before
 *      appl_exit (the QUITSHUT path minus lobo(), main.c:2048)
 *   3  bare: appl_init -> evnt_timer -> appl_exit -> Pterm(0)
 *
 * Built exactly like sdk/apps/gemtest (LP64 gemlib + mintlib).
 */

#include <mint/osbind.h>
#include <gem.h>
#ifndef NULL
#define NULL ((void*)0)
#endif

#ifndef MODE
#define MODE 1
#endif

static void say(const char *s) { Cconws(s); }

static void say_num(const char *pre, long v)
{
	char t[24]; int n = 0; unsigned long u;
	Cconws(pre);
	if (v < 0) { Cconws("-"); u = (unsigned long)(-v); } else u = (unsigned long)v;
	if (u == 0) t[n++] = '0';
	while (u) { t[n++] = (char)('0' + (u % 10)); u /= 10; }
	while (n) { char c[2]; c[0] = t[--n]; c[1] = 0; Cconws(c); }
	Cconws("\r\n");
}

static void mkobj(OBJECT *o, short type, unsigned short flags, long spec,
                  short x, short y, short w, short h,
                  short next, short head, short tail)
{
	o->ob_next = next; o->ob_head = head; o->ob_tail = tail;
	o->ob_type = (unsigned short)type; o->ob_flags = flags; o->ob_state = OS_NORMAL;
	o->ob_spec.index = spec;
	o->ob_x = x; o->ob_y = y; o->ob_width = w; o->ob_height = h;
}

static OBJECT g_desk[1];
static OBJECT g_menu[6];

int main(void)
{
	short ap_id, phys, handle = 0;
	short wchar, hchar, wbox, hbox;
	short work_in[11], work_out[57];
	short dx, dy, dw, dh;
	short wh = -1;
	short msg[8];
	int i;
	void *glob = 0;
	long rsc = 0;

	say("\r\n[quittest] === START mode ");
#if MODE == 1
	say("1 (TeraDesk QUITQUIT path) ===\r\n");
#elif MODE == 2
	say("2 (QUITQUIT + shel_write SHW_SHUTDOWN) ===\r\n");
#else
	say("3 (bare appl_init/appl_exit/Pterm) ===\r\n");
#endif

	/* main.c:1817 */
	ap_id = appl_init();
	say_num("[quittest] appl_init -> ", ap_id);
	if (ap_id < 0) { Pterm(-1); }
	say_num("[quittest] _AESversion = ", _AESversion);

#if MODE != 3
	/* main.c:1848 rsrc_load(RSRCNAME) -- desktop.rsc sits beside us */
	rsc = rsrc_load("desktop.rsc");
	say_num("[quittest] rsrc_load(desktop.rsc) -> ", rsc);

	/* init_xdialog: graf_handle + v_opnvwk */
	phys = graf_handle(&wchar, &hchar, &wbox, &hbox);
	say_num("[quittest] graf_handle -> ", phys);
	for (i = 0; i < 10; i++) work_in[i] = 1;
	work_in[10] = 2;
	handle = phys;
	v_opnvwk(work_in, &handle, work_out);
	say_num("[quittest] v_opnvwk -> handle ", handle);

	/* main.c:1874-1875 */
	if (_AESversion >= 0x399) {
		short r = shel_write(SHW_INFRECGN, 1, 0, NULL, NULL);
		say_num("[quittest] shel_write(SHW_INFRECGN,1,0) -> ", r);
		r = menu_register(ap_id, " Quittest");
		say_num("[quittest] menu_register(ap_id) -> ", r);
	}

	/* init(): xw_getwork(NULL,&xd_desk) = wind_get(0, WF_WORKXYWH) */
	wind_get(0, WF_WORKXYWH, &dx, &dy, &dw, &dh);
	say_num("[quittest] wind_get(0,WF_WORKXYWH) w=", dw);

	/* dsk_init -> regen_desktop(desktop): wind_set(0,WF_NEWDESK,tree,0) + dsk_draw */
	mkobj(&g_desk[0], G_BOX, OF_LASTOB, 0x00FF1142L /* thick 0, green fill? -> use colour word */, dx, dy, dw, dh, -1, -1, -1);
	g_desk[0].ob_spec.index = 0x00001172L; /* ipattern 7, interior colour 2 (green), border 1 */
	wind_set_ptr_int(0, WF_NEWDESK, g_desk, 0);
	form_dial(FMD_START, dx, dy, dw, dh, dx, dy, dw, dh);
	form_dial(FMD_FINISH, dx, dy, dw, dh, dx, dy, dw, dh);
	say("[quittest] WF_NEWDESK set + dsk_draw done\r\n");

	/* init(): menu_bar(menu, 1) with a minimal menu tree */
	{
		short sw = (short)(work_out[0] + 1);
		short sh = (short)(work_out[1] + 1);
		short tw = (short)(6 * (wchar > 0 ? wchar : 8));
		short r;
		mkobj(&g_menu[0], G_IBOX, OF_NONE, 0L, 0, 0, sw, sh, -1, 1, 5);
		mkobj(&g_menu[1], G_BOX, OF_NONE, 0L, 0, 0, sw, 19, 5, 2, 2);
		mkobj(&g_menu[2], G_IBOX, OF_NONE, 0L, 0, 0, sw, 19, 1, 3, 4);
		mkobj(&g_menu[3], G_TITLE, OF_NONE, (long)(unsigned long)" Desk ", 0, 0, tw, 19, 4, -1, -1);
		mkobj(&g_menu[4], G_TITLE, OF_LASTOB, (long)(unsigned long)" File ", tw, 0, tw, 19, 2, -1, -1);
		mkobj(&g_menu[5], G_IBOX, OF_NONE, 0L, 0, 0, sw, sh, 0, -1, -1);
		r = menu_bar(g_menu, 1);
		say_num("[quittest] menu_bar(menu,1) -> ", r);
		r = menu_register(-1, "  DESKTOP");
		say_num("[quittest] menu_register(-1) -> ", r);
	}

	/* init(): x_setpath(bslash) */
	say_num("[quittest] Dsetpath(\\) -> ", Dsetpath("\\"));

	/* a window, so wd_del_all has something to close */
	wh = wind_create(NAME | CLOSER | MOVER | SIZER, dx, dy, dw, dh);
	say_num("[quittest] wind_create -> ", wh);
	if (wh > 0) {
		wind_set_str(wh, WF_NAME, " C:\\*.* ");
		say_num("[quittest] wind_open -> ", wind_open(wh, dx + 20, dy + 20, 300, 200));
	}

	/* alloc_global_memory */
	glob = (void *)Malloc(4096);
	say("[quittest] Malloc(global) done\r\n");

	/* arrow_mouse() */
	graf_mouse(ARROW, NULL);

	/* evntloop(): a few blocking evnt_multi rounds like TeraDesk's, timer-bounded */
	for (i = 0; i < 3; i++) {
		short mx, my, mb, ks, kr, br;
		short ev = evnt_multi(MU_KEYBD | MU_BUTTON | MU_MESAG | MU_TIMER,
		                      0x102, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
		                      msg, 700UL, &mx, &my, &mb, &ks, &kr, &br);
		say_num("[quittest] evnt_multi -> ", ev);
	}

	/* ---------------- QUITQUIT: main.c:1943-1955 ---------------- */
	say("[quittest] --- quit path begins ---\r\n");
	/* wd_del_all -> wd_type_close -> xw_close (wind_close) ; later exit_xdialog -> xw_closeall (wind_delete) */
	if (wh > 0) {
		say_num("[quittest] wind_close -> ", wind_close(wh));
	}
	say_num("[quittest] menu_bar(menu,0) -> ", menu_bar(g_menu, 0));
	/* regen_desktop(NULL) */
	wind_set_ptr_int(0, WF_NEWDESK, NULL, 0);
	say("[quittest] wind_set(0,WF_NEWDESK,NULL) done\r\n");
	form_dial(FMD_START, dx, dy, dw, dh, dx, dy, dw, dh);
	form_dial(FMD_FINISH, dx, dy, dw, dh, dx, dy, dw, dh);
	say("[quittest] form_dial START/FINISH done\r\n");
	/* main.c:1972 */
	Mfree(glob);
	say("[quittest] Mfree(global) done\r\n");
	/* main.c:1980 */
	say_num("[quittest] vq_gdos -> ", vq_gdos());
	/* exit_xdialog: xw_closeall -> wind_delete ; v_clsvwk */
	if (wh > 0) {
		say_num("[quittest] wind_delete -> ", wind_delete(wh));
	}
	v_clsvwk(handle);
	say("[quittest] v_clsvwk done\r\n");
	/* main.c:1992 */
	if (rsc) {
		say_num("[quittest] rsrc_free -> ", rsrc_free());
	}
#if MODE == 2
	{
		short q = shel_write(SHW_SHUTDOWN, 2, 2, NULL, NULL);
		say_num("[quittest] shel_write(SHW_SHUTDOWN,2,2) -> ", q);
	}
#endif
#else
	{
		short mx, my, mb, ks, kr, br;
		short ev = evnt_multi(MU_TIMER, 0,0,0, 0,0,0,0,0, 0,0,0,0,0, msg, 700UL,
		                      &mx, &my, &mb, &ks, &kr, &br);
		say_num("[quittest] evnt_multi(timer) -> ", ev);
	}
#endif
	/* main.c:2124 */
	say_num("[quittest] appl_exit -> ", appl_exit());
	say("[quittest] returning 0 -> Pterm(0)\r\n");
	return 0;
}
