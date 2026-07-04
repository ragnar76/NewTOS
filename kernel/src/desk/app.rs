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
use tos_api::bios::drive::drvbits;
use tos_api::vdi::font::{CELL_H, CELL_W};

use super::global::{min_wint, DeskState, G_ICON, MIN_HINT, DROOT, NIL};
use super::icons::{
    build_iconblk, IconKind, IB_HICON, IB_WTEXT, ICON_LABEL_GAP, MAX_ICONTEXT_WIDTH,
};
use super::model::LEN_LABEL;

impl DeskState {
    /// `app_start` (icon-metric portion, `deskapp.c:810-817`). Requires
    /// `g_desk` to already reflect the current resolution's work-area rect
    /// (set by [`DeskState::new`]).
    ///
    /// ```text
    /// g_wicon = MAX_ICONTEXT_WIDTH * char_w + 2*ib_xtext   (fixed label box!)
    /// g_hicon = ib_hicon + char_h + 2
    /// g_icw   = g_desk.w evenly redistributed over the columns that fit
    /// g_ich   = g_desk.h evenly redistributed over the rows that fit
    /// ```
    ///
    /// `char_w`/`char_h` are NewTOS's fixed 8×16 font ([`CELL_W`]/[`CELL_H`]),
    /// which is exactly the per-glyph width the VDI renderer advances by — so
    /// the metric and the renderer agree (the precondition `_rebuild-plan.md`
    /// §5 requires the Developer to confirm before wiring D1).
    pub fn app_start(&mut self) {
        // Icon cell: fixed 12-char label box + 2*IB_XTEXT (the D1 root fix).
        // `IB_WTEXT + 2*IB_XTEXT` is algebraically `MAX_ICONTEXT_WIDTH*char_w
        // + 2*ib_xtext`, the deskapp.c:810 formula, made explicit here.
        debug_assert_eq!(IB_WTEXT, MAX_ICONTEXT_WIDTH * CELL_W);
        self.g_wicon = IB_WTEXT + 2 * super::icons::IB_XTEXT;
        self.g_hicon = IB_HICON + CELL_H + ICON_LABEL_GAP;

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

    /// `app_blddesk` (`deskapp.c:1337-1404`) — build the desktop pseudo-window
    /// icon tree under `DROOT`: one `G_ICON` per present drive, plus the Trash
    /// and Printer fixtures, placed on the desktop grid (`g_icw`/`g_ich`).
    ///
    /// NewTOS has no `NEWDESK.INF` desktop-icon positions yet (Task #9), so
    /// icons take built-in default grid spots: drives down column 0, Trash and
    /// Printer in the rightmost column. Each icon's object box equals the icon
    /// cell (`g_wicon`×`g_hicon`) and its bitmap is recentred live against
    /// `g_wicon` (the `deskapp.c:1391` `ib_xicon` recentre, done inside
    /// [`build_iconblk`]).
    pub fn app_blddesk(&mut self) {
        // Reset DROOT's item tree and reposition it to the desktop work rect.
        let d = self.g_desk;
        self.obj_wfree(DROOT, d.x, d.y, d.w, d.h);

        let cols = (d.w / self.g_icw).max(1);
        let bits = drvbits();

        // Drives down column 0 (A/B are floppies, C+ hard disks —
        // deskapp.c:721).
        let mut row = 0i16;
        for drv in 0..26u32 {
            if bits & (1 << drv) != 0 {
                let letter = b'A' + drv as u8;
                let (kind, label): (IconKind, &[u8]) = if drv > 1 {
                    (IconKind::Hard, b"Hard disk")
                } else {
                    (IconKind::Floppy, b"Floppy disk")
                };
                if self.place_desk_icon(0, row, kind, letter, label).is_none() {
                    return; // arena exhausted — stop placing (EmuTOS parity)
                }
                row += 1;
            }
        }

        // Trash + Printer fixtures in the rightmost column.
        let rc = cols - 1;
        self.place_desk_icon(rc, 0, IconKind::Trash, 0, b"Trash");
        self.place_desk_icon(rc, 1, IconKind::Printer, 0, b"Printer");
    }

    /// Allocate and populate one desktop `G_ICON` at grid cell `(col, row)`
    /// (pixel origin relative to `DROOT`). Returns its `g_screen[]` index or
    /// [`None`] if the object arena is exhausted.
    fn place_desk_icon(
        &mut self,
        col: i16,
        row: i16,
        kind: IconKind,
        letter: u8,
        label: &[u8],
    ) -> Option<i16> {
        let x = col * self.g_icw;
        let y = row * self.g_ich;
        let g_wicon = self.g_wicon;
        let g_hicon = self.g_hicon;
        let obid = self.obj_ialloc(DROOT, x, y, g_wicon, g_hicon)?;

        // Fill the per-object label + ICONBLK in the (stable-address)
        // ScreenInfo slot, then point the object at them.
        let info = &mut self.g_screeninfo[obid as usize];
        info.fnode = NIL; // ANODE-backed desktop icon — no FNode
        info.atype = match kind {
            IconKind::Hard | IconKind::Floppy => super::model::AType::Disk,
            IconKind::Trash => super::model::AType::Trash,
            IconKind::Printer => super::model::AType::Printer,
            _ => super::model::AType::None,
        };
        let n = label.len().min(LEN_LABEL - 1);
        info.label = [0; LEN_LABEL];
        info.label[..n].copy_from_slice(&label[..n]);
        let ptext = info.label.as_ptr() as u64;
        info.block = build_iconblk(kind, ptext, letter, g_wicon);
        let spec = &info.block as *const IconBlk as u64;

        let o = &mut self.g_screen[obid as usize];
        o.ob_type = G_ICON;
        o.ob_spec = spec;
        Some(obid)
    }
}
