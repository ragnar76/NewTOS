//! `pro` — starting a program from the desktop (EmuTOS `desk/deskpro.c`).
//!
//! Two functions, both of them pure AES work: [`pro_exec`] hands the AES
//! shell the program to run next (`shel_write(SHW_EXEC, …)`) with the busy
//! mouse shape around it, and [`pro_run`] builds the Atari command-tail image
//! in front of it. Nothing here runs a program — that is the shell loop's job
//! ([`crate::shell_loop`], EmuTOS `gemshlib.c` `sh_main`/`sh_ldapp`), which
//! reads exactly the state `shel_write` leaves behind.
//!
//! The busy pair [`desk_busy_on`]/[`desk_busy_off`] is EmuTOS
//! `desk/desksupp.c:1840-1852`; it lives here because `pro_exec` is its first
//! caller in NewTOS.

use super::{graf_mouse, shel_write, HGLASS};
use tos_types::aes::ARROW;

/// `SHW_EXEC` (`include/aesdefs.h:619`) — `shel_write`'s "run `pcmd` after the
/// current program terminates" mode.
const SHW_EXEC: i16 = 1;

/// `DEF_CONSOLE` (`include/aesdefs.h:627`) — the magic program name that makes
/// `sh_write` select the built-in console instead of a normal application
/// (`aes/gemshlib.c:172`). NUL-terminated for the trap.
pub(super) const DEF_CONSOLE: &[u8] = b"EMUCON\0";

/// `CMDTAILSIZE` (`aes/gemlib.h:97`) — the size of the command-tail image
/// `shel_write` copies into `ad_stail`. EmuTOS's `pro_run` writes into
/// `G.g_work`, a `MAXPATHLEN` scratch buffer; ours is a local of this size,
/// which is what the AES reads.
pub(super) const CMDTAILSIZE: usize = 128;

/// `desk_busy_on` (`desk/desksupp.c:1840-1843`) — "change mouse to 'busy'
/// indicator": `graf_mouse(HGLASS, NULL)`.
pub(super) fn desk_busy_on() {
    graf_mouse(HGLASS);
}

/// `desk_busy_off` (`desk/desksupp.c:1849-1852`) — "change mouse to 'not busy'
/// indicator": `graf_mouse(ARROW, NULL)`.
pub(super) fn desk_busy_off() {
    graf_mouse(ARROW);
}

/// `set_default_path` (`desk/desksupp.c:1785-1801`) — "routine to change the
/// default drive and directory":
///
/// ```c
/// desk_busy_on();
/// dos_sdrv(path[0]-'A');
/// rc = (WORD)dos_chdir(path);
/// desk_busy_off();
/// return rc;
/// ```
///
/// `path` is a fully-qualified directory ending in a separator, the shape
/// `filename_start(app_path); *p = '\0'` leaves behind (`C:\SUB\`, or `C:\` at
/// a drive root). `path[0]-'A'` is EmuTOS's own drive derivation, taken
/// verbatim — it is why the argument must carry its `X:` prefix.
///
/// Returns the `Dsetpath` result: `E_OK`, or a negative GEMDOS error (`EPTHNF`
/// for a directory that does not exist **and** for a drive that is not
/// mounted — `docs/api/gemdos/directory/3b-dsetpath.md`, which folds
/// `ckdrv`'s failure into `findit`'s). The busy hourglass is up across both
/// calls "because this can involve disk i/o (for example, if the media has
/// changed)" (`:1790-1793`).
pub(super) fn set_default_path(path: &str) -> tos_types::Long {
    desk_busy_on();
    let drive = path.as_bytes().first().copied().unwrap_or(0);
    super::dosbind::dos_sdrv(drive as i16 - b'A' as i16);
    let rc = super::dosbind::dos_chdir(path);
    desk_busy_off();
    rc
}

/// `pro_exec` (`desk/deskpro.c:35-45`):
///
/// ```c
/// desk_busy_on();
/// ret = shel_write(SHW_EXEC, isgraf, isover, pcmd, ptail);
/// if (!ret)
///     desk_busy_off();
/// return ret;
/// ```
///
/// The hourglass stays up on success because the desktop is on its way out —
/// the arrow comes back with the desktop, from `desk_busy_off` at the end of
/// its start-up (`deskmain.c:2071`).
fn pro_exec(isgraf: bool, isover: i16, pcmd: &[u8], ptail: &[u8]) -> bool {
    desk_busy_on();
    let ret = shel_write(SHW_EXEC, i16::from(isgraf), isover, pcmd.as_ptr(), ptail.as_ptr());
    if ret == 0 {
        desk_busy_off();
    }
    ret != 0
}

/// `pro_run` (`desk/deskpro.c:52-64`) — "run a program via `shel_write()`":
///
/// ```c
/// tail[0] = len = strlen(tail+1);
/// tail[len+2] = 0x0D;     /* follows the nul byte, just like Atari TOS */
/// ret = pro_exec(isgraf, 1, cmd, tail);
/// if (wh != -1)
///     do_wopen(FALSE, wh, curr, &G.g_desk);
/// return ret;
/// ```
///
/// `tail` comes in as EmuTOS hands it over: the **text** of the command tail
/// starting at byte 1, byte 0 reserved for the length this function fills in.
///
/// `(wh, curr)` is the *"optionally, deselect the current icon & zoom to
/// desktop size"* step (`deskpro.c:50,61-62`): `wh` is `G.g_cwin` and
/// `curr` the launched icon for every `do_aopen` (`desksupp.c:951,986,1001`),
/// `curr = -1` for a function key (`deskmain.c:1102`), and `wh = -1` for the
/// console launch (`deskmain.c:753`), which skips the step. It runs whatever
/// `pro_exec` answered, exactly as the source does — `ret` is not tested.
/// The body is [`super::main::do_wopen`] with `new_win = FALSE` and the whole
/// desktop as target: a `graf_growbox` from the icon out to `G.g_desk` and a
/// redraw-less deselect of the icon (DESK50, `docs/api/desktop/desksupp-wopen.md`).
/// A *Cancel* in `opn_appl` never gets here (`desksupp.c:986`), which is why
/// the icon stays selected then.
pub(super) fn pro_run(isgraf: bool, cmd: &[u8], tail: &mut [u8; CMDTAILSIZE], wh: i16, curr: i16) -> bool {
    // `tail[0] = len = strlen(tail+1);` — clamped so the length byte, the
    // text, its NUL and the CR all fit the image. EmuTOS has no such bound
    // (`G.g_work` is the same 128 bytes and it writes past on a maximal
    // tail); for every tail that fits — which is every tail a caller can
    // legally pass — the two are identical.
    let text = &tail[1..];
    let len = text.iter().position(|&b| b == 0).unwrap_or(text.len()).min(CMDTAILSIZE - 3);
    tail[0] = len as u8;
    // `tail[len+2] = 0x0D;` — "follows the nul byte, just like Atari TOS".
    tail[len + 2] = 0x0D;
    let ret = pro_exec(isgraf, 1, cmd, tail);
    // `if (wh != -1) do_wopen(FALSE, wh, curr, &G.g_desk);` (`:61-62`).
    if wh != -1 {
        super::main::do_wopen(false, wh, curr, super::g_desk());
    }
    ret
}
