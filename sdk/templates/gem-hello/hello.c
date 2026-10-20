/*
 * hello.c -- NewTOS SDK template: a minimal GEM application (.NAP).
 *
 * One window with a line of text, the usual message loop, an alert box to
 * confirm quitting, and a clean exit (window closed and deleted, virtual
 * workstation closed, appl_exit). Copy the directory, rename, extend.
 *
 * Build:   make                (Makefile + sdk/mk/newtos.mk)
 *     or:  newtos-cc -nap -o HELLO.NAP hello.c
 *
 * Quit with the window's close box, Ctrl+Q or Esc.
 *
 * Two LP64 details (docs/sdk/03-first-programs.md, step 3): a window title is
 * set with wind_set_str(), not wind_set() with a split pointer; and
 * evnt_multi()'s timer argument is an unsigned long (pass 0L).
 */
#include <gem.h>

#define WIN_KIND (NAME | CLOSER | MOVER)

static short vdi_handle;               /* VDI virtual workstation */
static short win = -1;                 /* window handle, -1 = none */
static char  win_title[] = "GEM Hello";

/* Draw the part of the work area that lies inside `clip`. */
static void draw_work(const GRECT *clip)
{
	GRECT work;
	short pxy[4];

	wind_get_grect(win, WF_WORKXYWH, &work);
	pxy[0] = clip->g_x;
	pxy[1] = clip->g_y;
	pxy[2] = clip->g_x + clip->g_w - 1;
	pxy[3] = clip->g_y + clip->g_h - 1;
	vs_clip(vdi_handle, 1, pxy);

	vsf_interior(vdi_handle, FIS_SOLID);
	vsf_color(vdi_handle, G_WHITE);
	vr_recfl(vdi_handle, pxy);

	vst_color(vdi_handle, G_BLACK);
	v_gtext(vdi_handle, work.g_x + 16, work.g_y + 24, "Hello from a NewTOS GEM application");
	v_gtext(vdi_handle, work.g_x + 16, work.g_y + 48, "Close box, Ctrl+Q or Esc quits.");

	vs_clip(vdi_handle, 0, pxy);
}

/* WM_REDRAW: walk the window's rectangle list (the parts not covered by
 * other windows) and draw each one that intersects the dirty area. */
static void redraw(const GRECT *dirty)
{
	GRECT r;

	wind_update(BEG_UPDATE);
	graf_mouse(M_OFF, 0L);
	wind_get_grect(win, WF_FIRSTXYWH, &r);
	while (r.g_w > 0 && r.g_h > 0) {
		if (rc_intersect(dirty, &r))
			draw_work(&r);
		wind_get_grect(win, WF_NEXTXYWH, &r);
	}
	graf_mouse(M_ON, 0L);
	wind_update(END_UPDATE);
}

static int confirm_quit(void)
{
	return form_alert(1, "[2][Quit GEM Hello?][Quit|Cancel]") == 1;
}

int main(void)
{
	short work_in[11] = { 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2 };
	short work_out[57];
	short wchar, hchar, wbox, hbox;
	short msg[8];
	short mx, my, mb, ks, key, clicks;
	GRECT desk, outer;
	int done = 0;

	if (appl_init() < 0)
		return 1;

	vdi_handle = graf_handle(&wchar, &hchar, &wbox, &hbox);
	v_opnvwk(work_in, &vdi_handle, work_out);
	if (vdi_handle <= 0) {
		form_alert(1, "[3][GEM Hello: no VDI workstation.][Cancel]");
		appl_exit();
		return 1;
	}

	wind_get_grect(0, WF_WORKXYWH, &desk);
	win = wind_create_grect(WIN_KIND, &desk);
	if (win < 0) {
		form_alert(1, "[3][GEM Hello: no window left.][Cancel]");
		v_clsvwk(vdi_handle);
		appl_exit();
		return 1;
	}
	wind_set_str(win, WF_NAME, win_title);
	outer.g_x = desk.g_x + 2 * wbox;
	outer.g_y = desk.g_y + 2 * hbox;
	outer.g_w = 40 * wchar;
	outer.g_h = 8 * hchar;
	wind_open_grect(win, &outer);
	graf_mouse(ARROW, 0L);

	while (!done) {
		short ev = evnt_multi(MU_MESAG | MU_KEYBD,
		                      0, 0, 0,
		                      0, 0, 0, 0, 0,
		                      0, 0, 0, 0, 0,
		                      msg, 0L,
		                      &mx, &my, &mb, &ks, &key, &clicks);

		if (ev & MU_MESAG) {
			switch (msg[0]) {
			case WM_REDRAW:
				if (msg[3] == win)
					redraw((const GRECT *)&msg[4]);
				break;
			case WM_TOPPED:
				if (msg[3] == win)
					wind_set(win, WF_TOP, 0, 0, 0, 0);
				break;
			case WM_MOVED:
				if (msg[3] == win)
					wind_set_grect(win, WF_CURRXYWH, (const GRECT *)&msg[4]);
				break;
			case WM_CLOSED:
				if (msg[3] == win && confirm_quit())
					done = 1;
				break;
			case AP_TERM:
				done = 1;
				break;
			}
		}
		if (ev & MU_KEYBD) {
			short ascii = key & 0xff;
			if ((ascii == 0x11 || ascii == 0x1b) && confirm_quit())   /* Ctrl+Q, Esc */
				done = 1;
		}
	}

	wind_close(win);
	wind_delete(win);
	v_clsvwk(vdi_handle);
	appl_exit();
	return 0;
}
