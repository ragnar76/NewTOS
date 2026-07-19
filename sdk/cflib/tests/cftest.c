/*
 * cftest.c -- minimal cflib client linked against crt0 + libc.a + gemlib.a +
 * cflib.a.
 *
 * Purpose (build/link verification for the NewTOS cflib port): exercise the
 * cflib entry points so the linked ELF pulls a real cross-section of the cflib
 * archive AND, transitively, the gemlib AES/VDI bindings and mintlib libc.
 *
 *   init_app("")   -> cflib appinit.c: appl_init(), graf_handle(),
 *                     wind_get_grect(), graf_mouse(), the cookie probes
 *                     (getcookie -> cflib cookie.c), appl_xgetinfo(),
 *                     open_vwork() (cflib vopnvwk.c), vq_extnd(), the font
 *                     setup (height2pts -> cflib vheight.c, vst_*), shel_read(),
 *                     split_filename() (cflib fsnorm.c), path_exists() (cflib
 *                     fexists.c) and init_userdef() (cflib userdef.c).
 *   do_alert()     -> cflib alerts.c -> form_alert() (AES).
 *   exit_app(0)    -> cflib appexitg.c exit_gem() (v_clsvwk + appl_exit) then
 *                     the libc exit() -> crt0 -> Pterm.
 *
 * This is a LINK test: it proves cflib.a resolves cleanly on top of gemlib.a
 * and libc.a with zero undefined symbols and yields a valid AArch64 ET_EXEC.
 * The empty resource-file argument makes init_app() skip rsrc_load(); the alert
 * text is conventional. Real dialog rendering needs the desktop and is run by
 * the lead. exit_app() terminates the process via Pterm (it does not return).
 */
#include "cflib.h"

int main(void)
{
	/* cflib application init (no .rsc): AES appl_init + graf_handle +
	 * open_vwork + system-font metrics + userdef install. */
	init_app("");

	/* cflib alert box (form_alert wrapper). */
	do_alert(1, 0, "[1][cflib on NewTOS|link test][OK]");

	/* cflib teardown: v_clsvwk + appl_exit, then libc exit() -> Pterm. */
	exit_app(0);

	return 0;
}
