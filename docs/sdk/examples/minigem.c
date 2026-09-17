/*
 * minigem.c -- the smallest useful NewTOS GEM application (.NAP).
 *
 * Derived from sdk/gemlib/tests/gemtest.c (appl_init / graf_handle /
 * v_opnvwk / v_clsvwk / appl_exit) and sdk/apps/gemtest/gemtest.c
 * (wind_get WF_WORKXYWH), plus one window and an evnt_multi loop.
 *
 * Build: see docs/sdk/03-first-programs.md (same flags as
 * sdk/apps/gemtest/build-gemtest.sh).
 */
#include <gem.h>

static short vh;                       /* VDI virtual workstation handle */

static void draw_window(short wh)
{
	short x, y, w, h;
	short pxy[4];

	wind_get(wh, WF_WORKXYWH, &x, &y, &w, &h);
	pxy[0] = x; pxy[1] = y; pxy[2] = x + w - 1; pxy[3] = y + h - 1;
	vs_clip(vh, 1, pxy);
	vsf_color(vh, 0);                  /* white */
	vsf_interior(vh, 1);               /* solid */
	v_bar(vh, pxy);
	v_gtext(vh, x + 8, y + 24, "Hello from a NewTOS .NAP");
	vs_clip(vh, 0, pxy);
}

int main(void)
{
	short work_in[11] = { 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2 };
	short work_out[57];
	short wcell, hcell, wbox, hbox;
	short dx, dy, dw, dh;
	short wh, done = 0;
	short msg[8];
	short mx, my, mb, ks, kr, br;

	if (appl_init() < 0)
		return 1;

	vh = graf_handle(&wcell, &hcell, &wbox, &hbox);
	v_opnvwk(work_in, &vh, work_out);

	wind_get(0, WF_WORKXYWH, &dx, &dy, &dw, &dh);
	wh = wind_create(NAME | CLOSER | MOVER, dx, dy, dw, dh);
	if (wh > 0) {
		wind_set_str(wh, WF_NAME, "MINIGEM");
		wind_open(wh, dx + 32, dy + 32, 320, 160);
		draw_window(wh);
	}

	graf_mouse(ARROW, 0L);

	while (wh > 0 && !done) {
		short ev = evnt_multi(MU_MESAG | MU_KEYBD,
		                      0, 0, 0,
		                      0, 0, 0, 0, 0,
		                      0, 0, 0, 0, 0,
		                      msg, 0L,
		                      &mx, &my, &mb, &ks, &kr, &br);
		if (ev & MU_MESAG) {
			switch (msg[0]) {
			case WM_REDRAW:
				if (msg[3] == wh) {
					wind_update(BEG_UPDATE);
					draw_window(wh);
					wind_update(END_UPDATE);
				}
				break;
			case WM_MOVED:
				wind_set(wh, WF_CURRXYWH, msg[4], msg[5], msg[6], msg[7]);
				break;
			case WM_CLOSED:
				done = 1;
				break;
			}
		}
		if (ev & MU_KEYBD)
			done = 1;
	}

	if (wh > 0) {
		wind_close(wh);
		wind_delete(wh);
	}
	v_clsvwk(vh);
	appl_exit();
	return 0;
}
