//! `desk::app` — icon-cell / desktop-grid metrics (`app_start`) and the
//! desktop pseudo-window icon tree (`app_blddesk`).
//!
//! Reimplements the icon-geometry / grid-metric portion of EmuTOS
//! `deskapp.c` per [`docs/api/desktop/deskapp.md`] and `_rebuild-plan.md` §5.
//! Two related but **distinct** pairs of numbers are derived:
//!
//! * `g_wicon` × `g_hicon` — the size of **one icon cell** (bitmap + its
//!   fixed-width label box). This is the D1 fix: the cell width is the
//!   **fixed** [`icons::IB_WTEXT`] label box **plus `2 * IB_XTEXT`**, never
//!   shrunk to the actual name length.
//! * `g_icw` × `g_ich` — the **desktop's** grid pitch, computed by fitting as
//!   many `g_wicon + MIN_WINT`-wide columns into `g_desk` as possible, then
//!   **redistributing the desktop's exact width/height evenly** so spacing is
//!   symmetric (distinct from the window pitch in [`super::win::win_view`],
//!   which uses a plain fixed gap).

use tos_api::aes::object::IconBlk;
use tos_api::vdi::font::{SMALL_CELL_H, SMALL_CELL_W};

use super::global::{min_wint, DeskState, DROOT, G_ICON, MIN_HINT, NIL};
use super::icons::{
    build_iconblk, icon_kind, IconKind, IB_HICON, IB_WTEXT, ICON_LABEL_SLACK, MAX_ICONTEXT_WIDTH,
};
use super::model::{AType, LEN_LABEL};

impl DeskState {
    /// `app_start` (icon-metric portion, `deskapp.c:810-817`). Requires
    /// `g_desk` to already reflect the current resolution's work-area rect
    /// (set by [`DeskState::new`]).
    ///
    /// ```text
    /// g_wicon = MAX_ICONTEXT_WIDTH * gl_wschar + 2*ib_xtext (fixed label box!)
    /// g_hicon = ib_hicon + gl_hschar + 2
    /// g_icw   = g_desk.w evenly redistributed over the columns that fit
    /// g_ich   = g_desk.h evenly redistributed over the rows that fit
    /// ```
    ///
    /// `char_w`/`char_h` here are the **small** AES font
    /// ([`SMALL_CELL_W`]/[`SMALL_CELL_H`] = 6×6, EmuTOS `gl_wschar`/
    /// `gl_hschar`), not the normal 8×16 one: `gr_gicon` draws an icon's
    /// label and its overlay character in the small font (`gemgraf.c:802,806`),
    /// so the label box must be measured in the same cell the renderer
    /// advances by — otherwise a 12-character 8.3 name does not fit its box.
    pub fn app_start(&mut self) {
        // Icon cell: fixed 12-char label box + 2*IB_XTEXT (the D1 root fix).
        // `IB_WTEXT + 2*IB_XTEXT` is algebraically `MAX_ICONTEXT_WIDTH*gl_wschar
        // + 2*ib_xtext`, the deskapp.c:810 formula, made explicit here.
        debug_assert_eq!(IB_WTEXT, MAX_ICONTEXT_WIDTH * SMALL_CELL_W);
        self.g_wicon = IB_WTEXT + 2 * super::icons::IB_XTEXT;
        self.g_hicon = IB_HICON + SMALL_CELL_H + ICON_LABEL_SLACK;

        // Desktop grid pitch: even redistribution over the fitting columns/rows
        // (deskapp.c:813-817). Integer division floors the count, then the
        // pitch tiles the desktop exactly (remainder spread across every
        // column/row as a slightly wider pitch — "symmetrical spacing").
        let screen_h = self.g_desk.y + self.g_desk.h;
        let mw = min_wint(screen_h);
        let xcnt = (self.g_desk.w / (self.g_wicon + mw)).max(1);
        self.g_icw = self.g_desk.w / xcnt;
        let ycnt = (self.g_desk.h / (self.g_hicon + MIN_HINT)).max(1);
        self.g_ich = self.g_desk.h / ycnt;
    }

    /// `snap_icon` (`deskins.c:146-184`) — round a raw pixel position to the
    /// nearest desktop grid cell and clamp it to `[0, cols-1]`/`[0, rows-1]`.
    /// Returns the target **grid** cell `(col, row)`.
    ///
    /// **Pure grid-rounding, no collision avoidance** — it never inspects other
    /// icons (`deskins.md`: `snap_icon` "performs no collision check
    /// whatsoever"). The INF-position apply path (`app_start:983-985`) and the
    /// drag path (`move_drvicon`) both call it alone, so colliding INF positions
    /// silently overlap — faithful to EmuTOS. Collision avoidance is the install
    /// path's job (`ins_posdisk`, #20), not here.
    pub fn snap_icon_grid(&self, x: i16, y: i16) -> (i16, i16) {
        let icw = self.g_icw.max(1);
        let ich = self.g_ich.max(1);
        let cols = (self.g_desk.w / icw).max(1);
        let rows = (self.g_desk.h / ich).max(1);
        let col = ((x + icw / 2) / icw).clamp(0, cols - 1);
        let row = ((y + ich / 2) / ich).clamp(0, rows - 1);
        (col, row)
    }

    /// `snap_icon` (`deskins.c:146-184`) in its own units: pixels in, pixels
    /// out, [`DROOT`]-relative. [`DeskState::snap_icon_grid`] does the rounding;
    /// this multiplies the cell back out and adds EmuTOS's spare-pixel term
    /// (`(g_desk.g_w - columns*icw) / columns`, `:159`), which spreads the
    /// remainder of the column fit evenly. EmuTOS additionally adds
    /// `G.g_desk.g_y` to the result (`:183`) because its item coordinates are
    /// screen-absolute; ours are relative to `DROOT`, which *is* the work area,
    /// so that term is already in the parent.
    pub fn snap_icon(&self, x: i16, y: i16) -> (i16, i16) {
        let icw = self.g_icw.max(1);
        let ich = self.g_ich.max(1);
        let cols = (self.g_desk.w / icw).max(1);
        let rows = (self.g_desk.h / ich).max(1);
        let (col, row) = self.snap_icon_grid(x, y);
        (col * icw + (self.g_desk.w - cols * icw) / cols, row * ich + (self.g_desk.h - rows * ich) / rows)
    }

    /// `app_blddesk` (`deskapp.c:1337-1404`) — build the desktop pseudo-window
    /// icon tree under `DROOT`: one `G_ICON` per [`Anode`] of `G.g_ahead`,
    /// at that node's own `a_xspot`/`a_yspot`.
    ///
    /// The list is the source of truth, exactly as in EmuTOS: this function
    /// invents nothing. What the list *contains* is decided once, at
    /// `app_start` time, by [`super::inf::load_and_apply`] (the `#M`/`#T`/`#O`
    /// lines) or [`super::inf::build_desk_icons`] (the drive map, when the file
    /// has none) — and afterwards by `Install devices` / `Install icon` /
    /// `Remove icon` ([`super::ins`]). That is what makes those three menu
    /// entries mean anything: before, this function re-derived the drive icons
    /// from `Drvmap` on every call, so a removed icon came straight back.
    ///
    /// Each icon's object box equals the icon cell (`g_wicon`×`g_hicon`) and its
    /// bitmap is recentred live against `g_wicon` (the `deskapp.c:1391`
    /// `ib_xicon` recentre, done inside [`build_iconblk`]). The node's
    /// `a_obid` is written back (`deskapp.c:1369`) so
    /// [`DeskState::app_afind_by_id`] can map a clicked object to its node.
    pub fn app_blddesk(&mut self) {
        // Reset DROOT's item tree and reposition it to the desktop work rect.
        let d = self.g_desk;
        self.obj_wfree(DROOT, d.x, d.y, d.w, d.h);
        // Refresh DROOT's box `ob_spec` from the persisted background byte
        // (EmuTOS `deskapp.c:1327`); the AES draws this box as the screen
        // background, so a NEWDESK.INF `#Q` change picks up here.
        self.sync_desk_background();

        for i in 0..self.g_ahead.len() {
            let pa = &self.g_ahead[i];
            let (x, y, atype, letter, aicon) = (pa.a_xspot, pa.a_yspot, pa.a_type, pa.a_letter, pa.a_aicon);
            // "choose appropriate icon" (`deskapp.c:1371-1375`): a desktop icon
            // has no data icon, so it is always `a_aicon`. An icon number NewTOS
            // has no raster for (`IG_REMOV`, or a corrupt byte) falls back to
            // the one its type defaults to rather than drawing nothing.
            let kind = icon_kind(aicon).unwrap_or_else(|| default_icon_kind(atype, letter));
            let label = {
                let mut buf = [0u8; LEN_LABEL];
                let src = pa.a_pappl.as_bytes();
                let n = src.len().min(LEN_LABEL - 1);
                buf[..n].copy_from_slice(&src[..n]);
                buf
            };
            match self.place_desk_icon(x, y, kind, atype, letter, &label) {
                Some(obid) => self.g_ahead[i].a_obid = obid,
                // "can't allocate item object" (`deskapp.c:1364-1368`): stop.
                None => {
                    self.g_ahead[i].a_obid = NIL;
                    break;
                }
            }
        }
    }

    /// `app_afind_by_id` (`deskapp.c:1408-1419`) — the index in `G.g_ahead` of
    /// the node whose `a_obid` is `obid`, or [`None`].
    pub fn app_afind_by_id(&self, obid: i16) -> Option<usize> {
        if obid == NIL {
            return None;
        }
        self.g_ahead.iter().position(|pa| pa.a_obid == obid)
    }

    /// Allocate and populate one desktop `G_ICON` at pixel `(x, y)` relative to
    /// `DROOT`. Returns its `g_screen[]` index or [`None`] if the object arena
    /// is exhausted.
    fn place_desk_icon(
        &mut self,
        x: i16,
        y: i16,
        kind: IconKind,
        atype: AType,
        letter: u8,
        label: &[u8; LEN_LABEL],
    ) -> Option<i16> {
        let g_wicon = self.g_wicon;
        let g_hicon = self.g_hicon;
        let obid = self.obj_ialloc(DROOT, x, y, g_wicon, g_hicon)?;

        // Fill the per-object label + ICONBLK in the (stable-address)
        // ScreenInfo slot, then point the object at them.
        let info = &mut self.g_screeninfo[obid as usize];
        info.fnode = NIL; // ANODE-backed desktop icon — no FNode
        info.atype = atype;
        info.index = super::icons::ig_of(kind) as u16;
        info.label = *label;
        let ptext = info.label.as_ptr() as u64;
        info.block = build_iconblk(kind, ptext, letter, g_wicon);
        // `pic->ib_char |= pa->a_letter;` (`deskapp.c:1399`) — **unconditional**
        // in the source, and it has to be: the Install-icon dialog lets a drive
        // node take any picture, and the drive letter is read back out of
        // `ib_char` by every desktop-icon consumer (`super::dir`'s drop
        // handlers, `super::inf`'s `#M` writer). `build_iconblk` already ORs it
        // in for the two disk rasters, which have a char position; for any
        // other picture this is what keeps the letter from being lost.
        info.block.ib_char |= letter as i16;
        let spec = &info.block as *const IconBlk as u64;

        let o = &mut self.g_screen[obid as usize];
        o.ob_type = G_ICON;
        o.ob_spec = spec;
        Some(obid)
    }
}

/// The icon an `ANODE` of this type falls back to when its `a_aicon` names a
/// picture NewTOS has no raster for — the same choice `install_drive`
/// (`deskins.c:248`) and `build_inf` (`deskapp.c:719`) make for a fresh node:
/// `A:`/`B:` are floppies, every other drive is a hard disk.
pub fn default_icon_kind(atype: AType, letter: u8) -> IconKind {
    match atype {
        AType::Trash => IconKind::Trash,
        AType::Printer => IconKind::Printer,
        _ => {
            if matches!(letter, b'A' | b'B') {
                IconKind::Floppy
            } else {
                IconKind::Hard
            }
        }
    }
}
