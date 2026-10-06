/*
 * vqprobe.c -- diagnostic probe of measurement-2026-10-06-zview-m4 (not
 * product code): what WinDom's ApplInit asks the VDI for
 * (windom-1.21.3/src/appl_init.c:43-46: vq_extnd on the AES handle from
 * graf_handle, mode 0 then 1, app.color = work_out[13], app.nplanes =
 * work_out[4]), and the same on a virtual workstation opened afterwards
 * (appl_init.c:61). Prints to the console (serial).
 */
#include <stdio.h>
#include <mint/osbind.h>
#include <gem.h>

static void dump(const char *what, short h, short *wo)
{
	char b[200];
	sprintf(b, "VQPROBE: %s handle=%d wo[0]=%d wo[1]=%d wo[4]=%d wo[13]=%d wo[39]=%d\r\n",
		what, h, wo[0], wo[1], wo[4], wo[13], wo[39]);
	Cconws(b);
}

int main(void)
{
	short wo[57], wi[11], d, h, vh, i;

	(void)appl_init();
	h = graf_handle(&d, &d, &d, &d);
	for (i = 0; i < 57; i++) wo[i] = -7;
	vq_extnd(h, 0, wo); dump("vq_extnd(aeshdl,0)", h, wo);
	for (i = 0; i < 57; i++) wo[i] = -7;
	vq_extnd(h, 1, wo); dump("vq_extnd(aeshdl,1)", h, wo);
	for (i = 0; i < 10; i++) wi[i] = 1;
	wi[10] = 2;
	vh = h;
	v_opnvwk(wi, &vh, wo); dump("v_opnvwk", vh, wo);
	for (i = 0; i < 57; i++) wo[i] = -7;
	vq_extnd(vh, 1, wo); dump("vq_extnd(vhandle,1)", vh, wo);
	v_clsvwk(vh);
	appl_exit();
	return 0;
}
