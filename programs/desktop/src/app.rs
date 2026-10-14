//! `desktop::app` — icon-cell / desktop-grid metrics (`app_start`) and the
//! desktop pseudo-window icon tree (`app_blddesk`).
//!
//! Reimplements the icon-geometry / grid-metric portion of EmuTOS
//! `deskapp.c` per [`docs/api/desktop/deskapp.md`] and `_rebuild-plan.md` §5.
//! Two related but **distinct** pairs of numbers are derived:
//!
//! * `g_wicon` × `g_hicon` — the size of **one icon cell** (bitmap + its
//!   fixed-width label box). This is the D1 fix: the cell width is the
//!   **fixed** [`icons::ib_wtext`] label box **plus `2 * IB_XTEXT`**, never
//!   shrunk to the actual name length.
//! * `g_icw` × `g_ich` — the **desktop's** grid pitch, computed by fitting as
//!   many `g_wicon + MIN_WINT`-wide columns into `g_desk` as possible, then
//!   **redistributing the desktop's exact width/height evenly** so spacing is
//!   symmetric (distinct from the window pitch in [`super::win::win_view`],
//!   which uses a plain fixed gap).

use tos_types::aes::{IconBlk, G_ICON, NIL};

use super::global::{
    gl_hchar, gl_hschar, gl_wchar, gl_wschar, min_wint, DeskState, Point, AF_ISEXEC, DROOT,
    MIN_HINT,
};
use super::icons::{
    build_iconblk, ib_wtext, icon_kind, IconKind, IB_HICON, IB_WICON, ICON_LABEL_SLACK,
    MAX_ICONTEXT_WIDTH,
};

use super::model::{AType, LEN_LABEL};

/// `DRAG_BOX_WIDTH` (`deskapp.c:57`): `2 + LEN_ZNODE + 1 + LEN_ZEXT` = 14 —
/// the width, in normal-font characters, of the outline dragged for an item
/// shown as text (`sysconf.h:53-54`).
const DRAG_BOX_WIDTH: i16 = 2 + 8 + 1 + 3;

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
    /// ([`gl_wschar()`]/[`gl_hschar()`] = 6×6, EmuTOS `gl_wschar`/
    /// `gl_hschar`), not the normal 8×16 one: `gr_gicon` draws an icon's
    /// label and its overlay character in the small font (`gemgraf.c:802,806`),
    /// so the label box must be measured in the same cell the renderer
    /// advances by — otherwise a 12-character 8.3 name does not fit its box.
    pub fn app_start(&mut self) {
        // Icon cell: fixed 12-char label box + 2*IB_XTEXT (the D1 root fix).
        // `ib_wtext() + 2*IB_XTEXT` is algebraically `MAX_ICONTEXT_WIDTH*gl_wschar
        // + 2*ib_xtext`, the deskapp.c:810 formula, made explicit here.
        debug_assert_eq!(ib_wtext(), MAX_ICONTEXT_WIDTH * gl_wschar());
        self.g_wicon = ib_wtext() + 2 * super::icons::IB_XTEXT;
        self.g_hicon = IB_HICON + gl_hschar() + ICON_LABEL_SLACK;

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

        self.app_drag_outlines();
    }

    /// The second half of `app_start` (`deskapp.c:988-1011`) — the two drag
    /// outlines `gr_drgplns` draws while the user drags a selection, built from
    /// the icon metrics just computed.
    ///
    /// **Icon outline** (9 vertices, `:989-1005`): the silhouette of one icon
    /// cell — a narrow column the width of the icon *bitmap*, centred over a
    /// full-cell-wide label box. `xcent` is the bitmap's inset,
    /// `(g_wicon - ib_wicon) / 2`, the same recentre `build_iconblk` applies to
    /// `ib_xicon`; the step down happens at `g_hicon - gl_hschar - 2`, i.e.
    /// where the label starts.
    ///
    /// **Text outline** (5 vertices, `:1007-1011`): a plain rectangle
    /// `DRAG_BOX_WIDTH` normal-font characters wide and one row high, for the
    /// `V_TEXT` views.
    ///
    /// Both lists are **relative** to an item's top-left; `gr_plns` adds the
    /// per-object origin `gr_accobs` collected.
    fn app_drag_outlines(&mut self) {
        // `xcent = (G.g_wicon - G.g_iblist[0].ib_wicon) / 2;` (`:991`) — the
        // shipped icon bitmap width, which is `IB_WICON` for every icon in the
        // built-in set (`icons.rs`).
        let xcent = (self.g_wicon - IB_WICON) / 2;
        let step = self.g_hicon - gl_hschar() - 2; // `g_hicon-gl_hschar-2`
        let (w, h) = (self.g_wicon, self.g_hicon);
        self.g_xyicon = [
            Point { x: xcent, y: 0 },
            Point { x: xcent, y: step },
            Point { x: 0, y: step },
            Point { x: 0, y: h },
            Point { x: w, y: h },
            Point { x: w, y: step },
            Point { x: w - xcent, y: step },
            Point { x: w - xcent, y: 0 },
            Point { x: xcent, y: 0 },
        ];

        // `G.g_xytext[1].x = gl_wchar * DRAG_BOX_WIDTH;` etc. (`:1008-1011`) —
        // the other two vertices stay at the `bzero`'d origin, which closes the
        // rectangle.
        let tw = gl_wchar() * DRAG_BOX_WIDTH;
        let th = gl_hchar();
        self.g_xytext = [
            Point { x: 0, y: 0 },
            Point { x: tw, y: 0 },
            Point { x: tw, y: th },
            Point { x: 0, y: th },
            Point { x: 0, y: 0 },
        ];
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
            let (x, y, atype, letter) = (pa.a_xspot, pa.a_yspot, pa.a_type, pa.a_letter);
            // "choose appropriate icon" (`deskapp.c:1373-1378`):
            //
            // ```c
            // if ((pa->a_dicon < 0) || ((pa->a_type == AT_ISFILE) && (pa->a_flags & AF_ISEXEC)))
            //     icon = pa->a_aicon;
            // else
            //     icon = pa->a_dicon;
            // ```
            //
            // A drive/trash/printer node has no data icon (`a_dicon == NIL`),
            // so it always takes the first arm; a shortcut takes the
            // application icon when the file is executable and the document
            // icon otherwise.
            //
            // EmuTOS indexes `G.g_iblist[icon]` straight away, because
            // `app_parse` has already made the number usable — out of range
            // becomes `IG_APPL` (`deskapp.c:359-361`), which [`super::inf`]'s
            // parser reproduces. The one value that still names no picture is
            // `-1` (`FF` on an `#M`/`#T`/`#O` line, which only a hand-written
            // file can carry: `build_inf` puts the icon number there,
            // `deskapp.c:723`); EmuTOS would read `g_iblist[-1]`, so NewTOS
            // takes `app_parse`'s own answer for an unusable number instead.
            let icon = if pa.a_dicon < 0 || (pa.a_type == AType::File && pa.a_flags & AF_ISEXEC != 0)
            {
                pa.a_aicon
            } else {
                pa.a_dicon
            };
            let kind = icon_kind(icon).unwrap_or(IconKind::App);
            // "the label position varies" (`deskapp.c:1392-1396`): a shortcut
            // is labelled with its bare file name (`a_pdata`), everything else
            // with `a_pappl`.
            let label = {
                let mut buf = [0u8; LEN_LABEL];
                let src = match pa.a_type {
                    AType::File | AType::Folder => pa.a_pdata.as_bytes(),
                    _ => pa.a_pappl.as_bytes(),
                };
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

    /// The `(a_type, a_aicon)` of the desktop node `app_afind_by_id` finds —
    /// `i_find`'s desktop arm (`desksupp.c:1746-1749`) reads exactly these two
    /// fields and nothing else.
    pub fn app_afind_id_type(&self, obid: i16) -> Option<(AType, i16)> {
        let i = self.app_afind_by_id(obid)?;
        Some((self.g_ahead[i].a_type, self.g_ahead[i].a_aicon))
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
        // in for the three drive rasters, which have a char position (hard,
        // floppy, removable); for any other picture this is what keeps the
        // letter from being lost.
        info.block.ib_char |= letter as i16;
        let spec = &info.block as *const IconBlk as u64;

        let o = &mut self.g_screen[obid as usize];
        o.ob_type = G_ICON;
        o.ob_spec = spec;
        Some(obid)
    }
}
