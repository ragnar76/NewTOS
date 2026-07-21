# Desktop rendering-core data model — `GLOBES`, `WNODE`, `PNODE`, `FNODE`, `SCREENINFO`

> Phase-00 extraction, Track A Step 4/5 hybrid. Source: EmuTOS @ `1a7484ab`.
> These are **EmuTOS-internal application state** for the desktop, which is
> itself an ordinary AES client program — **not** TOS-API structures that
> cross a trap boundary. Per `docs/COMPATIBILITY.md`: nothing in this file
> is contract *by itself*. What a program can observe is the AES calls the
> desktop makes and the `OBJECT`/`ICONBLK` trees it hands to `objc_draw`
> (both already contract-documented in `docs/api/structures/`); the private
> bookkeeping below (`WNODE`/`FNODE`/`PNODE`/`GLOBES`) is EmuTOS's own
> implementation and free for NewTOS to replace outright — extracted here
> only so the rendering-core *behaviour* (grid metrics, view building,
> redraw) is understood. Byte-offset tables are given only where the exact
> *size* of a sub-range is load-bearing (the `FNODE`/`DTA` mirroring, below);
> otherwise fields are listed in declaration order with semantics, not a
> verified compiled layout (no `sizeof()` was run — these structs never
> cross an ABI boundary, so compiler-chosen padding is EmuTOS's business).

---

## `GLOBES` (`G`) — `desk/deskbind.h:130-245`, instance `desk/deskglob.c:25`

One process-lifetime global struct, referenced everywhere via `deskglob.h`
(`extern GLOBES G;`). Grouped by what this task cares about (window
management / view / icon geometry); the ANODE-management, EMUDESK.INF and
preference fields are listed for completeness but not re-derived (see
`docs/api/desktop.md`).

| Field | Type | Purpose | Written by | Read by |
|---|---|---|---|---|
| `g_wdta` | `DTA` | general-use `DTA`, the target of `Fsetdta()` around every `Fsfirst`/`Fsnext` scan the desktop does | `pn_active()` sets it as the active DTA via `dos_sdta(&G.g_wdta)` before scanning (`deskfpd.c:249`), restores the caller's previous DTA after (`deskfpd.c:295`) — **added in this validation pass**, was previously undocumented in this table (only mentioned in the `PNODE`/`FNODE` section below, via the `memcpy` trick) | `pn_active()`'s `memcpy(&fn->f_attr, &G.g_wdta.d_attrib, 23)` (`deskfpd.c:271`, see the `FNODE` section below) |
| `g_wdesktop` | `WNODE` | the desktop's own pseudo-`WNODE` (handle `DESKWH`=0) | `win_start()` sets `w_flags=WN_DESKTOP` (`deskwin.c:120`) | `win_find(DESKWH)` returns `&G.g_wdesktop` (`deskwin.c:202-203`); `pn_open()` falls back to it when no window is given (`deskfpd.c:80-81`) |
| `g_wlist` | `WNODE*` | malloc'd array of `NUM_WNODES` (7) real window nodes | `win_start()` allocates + chains via `w_next` (`deskwin.c:122-131`) | `win_alloc`/`win_free`/`win_find` walk it |
| `g_wfirst` | `WNODE*` | head of the **z-order** list (front-to-back) of *allocated* window slots — reused as "top window" pointer via `win_top()` | `win_start()` inits `=g_wlist`; `win_top()` re-splices (`deskwin.c:218-240`) | `win_find`, `win_srtall`, `win_bdall`, `win_shwall` iterate `w_next` from here. **Added in this validation pass:** `win_top()` also makes a real AES call, `objc_order(G.g_screen, thewin->w_root, NIL)` (`deskwin.c:222`), to bring the window's `OBJECT` to the front of the tree's own draw/hit-test order — not just a `g_wfirst` list-splice; full narrative is `deskwin.md`'s territory (out of this doc's file scope), catalogued in `docs/api/desktop/gembind.md`'s call-surface inventory |
| `g_wcnt` | `WORD` | count of currently-open windows (0..`NUM_WNODES`) | `win_alloc`/`win_free` (`deskwin.c:170`, `:146`) | `win_alloc` refuses new windows at `NUM_WNODES`; `win_count()` returns it |
| `g_iview` | `WORD` | current view type, `V_ICON`(0)/`V_TEXT`(1) (`deskfpd.h:29-30`) — **global, not per-window**: all windows share one view mode | menu handler (`deskmain.c`, out of scope here); default `START_VIEW`=`V_ICON` set by `win_start()` (`deskwin.c:110`) | `win_view()` switches on it (`deskwin.c:65`); `win_bldview()` switches on it per item (`deskwin.c:450`) |
| `g_iwext`/`g_ihext` | `WORD` | width/height of **one item's own extent** (icon+label bbox, or one text line) — see `deskwin.md` `win_view()` | `win_view()` | `win_bldview()` sizes each `obj_ialloc()` call; `win_ocalc()`'s `wfit`/`hfit` params come from `w/g_iwspc`, `h/g_ihspc` at the call site |
| `g_iwint`/`g_ihint` | `WORD` | **inter-item gap** (min horizontal/vertical spacing before the next item) | `win_view()` | `win_bldview()` insets each item's origin by this from its cell's top-left (`deskwin.c:431-432`) |
| `g_iwspc`/`g_ihspc` | `WORD` | **column/row pitch** = `iwext+iwint` / `ihext+ihint` — the grid step in a *window* | `win_view()` (`deskwin.c:89-90`) | `win_bldview`/`win_ocalc`/`win_blt`/`sob_malloc` (`deskobj.c:70`) |
| `g_isort` | `WORD` | current sort key, `S_NAME`/`S_TYPE`/`S_SIZE`/`S_DATE`/`S_NSRT` (`deskfpd.h:38-43`) | menu handler; default `START_SORT`=`S_NAME` (`win_start`) | `pn_comp()`/`pn_fcomp()` (`deskfpd.c:99-147`) |
| `g_ifit` | `BOOL` | `CONF_WITH_SIZE_TO_FIT` only — "size windows to fit" preference | `win_start()` defaults `TRUE` (`deskwin.c:113`); EMUDESK.INF `E` line (`deskapp.c:946`) | `win_ocalc()` branches virtual-column calc on it |
| `g_icols` | `WORD` | `CONF_WITH_SIZE_TO_FIT` only — max columns that fit in a **full-size** window at the current resolution, via `wind_calc_grect(1,…)` | `win_view()` (`deskwin.c:91-98`) | `win_ocalc()`'s non-size-to-fit branch |
| `g_desk` | `GRECT` | desktop work-area rectangle (screen minus menu bar) | set during desktop init (out of scope, `deskmain.c`) | `win_view()`'s `wind_calc_grect` call; `app_start()`'s `g_icw`/`g_ich` derivation; `do_wredraw(DESKWH,&G.g_desk)` in `app_blddesk` |
| `g_croot` | `WORD` | `g_screen[]` index of the **currently active** root (either `DROOT` or some `pw->w_root`) | `desk_verify()` (`desksupp.c:191,194`) | `do_wopen()` and selection code use it as "current tree" |
| `g_cwin`/`g_wlastsel` | `WORD` | current / last-selected window handle | `desk_verify()` (`desksupp.c:196-197`); `do_wopen()` sets `g_wlastsel` | menu enable/disable logic (out of scope), `hndl_menu` |
| `g_icw`/`g_ich` | `WORD` | **desktop** grid pitch in pixels (evenly divides `g_desk.g_w`/`g_h`) — see `deskapp.md` | `app_start()` (`deskapp.c:813-817`); **correction, this validation pass:** *not* "recomputed on resolution change by `deskrez.c`" as an earlier draft claimed — `deskrez.c` never calls `app_start()` (grepped, zero hits); a confirmed resolution change instead triggers a full desktop restart (`deskmain.c:900-913` → `shel_write(SHW_RESCHNG,…)` → `done=TRUE`), and `app_start()` re-runs as part of that restart, not via an in-place recompute — see `deskapp.md`'s corrected dependency note | `app_start()`'s icon-snap loop (`deskapp.c:983-984`), `app_blddesk` is **not** direct — it uses the already-pixel `a_xspot`/`a_yspot` — and `deskinf.c:1259-1260` (INF save, divides back to grid units) |
| `g_wicon`/`g_hicon` | `WORD` | universal icon-**cell** extent (icon+label bbox), derived once from icon #0 | `app_start()` (`deskapp.c:810-811`) | `win_view()`'s `V_ICON` case (`g_iwext=g_wicon`); `app_blddesk()`'s `obj_ialloc` size and `ib_xicon` recentre (`deskapp.c:1362-1363,1391`) |
| `g_xyicon[9]`/`g_xytext[5]` | `Point[]` | drag-outline polygon points (icon / text drag box) | `app_start()` (`deskapp.c:990-1011`) | drag-rubber-band code (`deskact.c`, out of scope) |
| `g_atext`/`g_alist`/`g_aavail`/`g_ahead` | ANODE management | out of scope (see `docs/api/desktop.md`) | | |
| `g_numiblks`/`g_iblist` | `WORD`/`ICONBLK*` | count + array of all loaded icon blocks (built-in + user), **transformed** (mask/data already `gsx_trans`'d for the current device) | `setup_iconblks()` (`deskapp.c:454-532`), called from `app_rdicon()` (`deskapp.c:662-674`) which `app_start()` calls first | `win_bldview` (`memcpy(ib,&G.g_iblist[i_index],…)`, `deskwin.c:477`), `app_blddesk` (`deskapp.c:1390`), `app_start`'s `g_wicon` derivation reads `g_iblist[0]` |
| `g_cnxsave` | `CSAVE*` | staging area between live preferences and EMUDESK.INF | out of scope | out of scope |
| `g_patcol[3]` | `PATCOL` | `CONF_WITH_BACKGROUNDS` only — desktop/window pattern+colour per plane-count bucket | `app_start()` defaults, EMUDESK.INF `Q` line (`deskapp.c:824-830,952-960`) | `set_background()` (`deskapp.c:1306-1331`) |
| `g_screenfree` | `WORD` | head of the item-object free chain (index into `g_screen[]`, chained via `ob_next`) | `obj_init`/`obj_wfree`/`obj_ialloc` (`deskobj.c`) | same |
| `g_screen` | `OBJECT*` | the **entire** desktop+windows+items `OBJECT` array (see layout below) | `obj_init()` allocates via `sob_malloc()` (`deskobj.c:51-97`) | everywhere; this **is** the AES object tree the desktop hands to `objc_draw`/`objc_find`/etc. |
| `g_screeninfo` | `SCREENINFO*` | parallel array (index-aligned with `g_screen[]`) of per-object auxiliary data | `obj_init()` (same allocation call, `deskobj.c:93-94`) | `win_bldview`, `app_blddesk`, `obj_get_obid` |

### `g_screen[]` index layout (`deskconf.h:18-33`, `desk/deskwin.h:44` for `DROOT`)

```
index 0                 : ROOT  — parent of everything, full-screen G_IBOX
index 1  (DROOT)        : the desktop itself, G_IBOX, parent of desktop icons
index 2..NUM_WNODES+1   : one G_IBOX per possible window (WOBS_START = NUM_WNODES+2 = 9)
index WOBS_START..end   : item objects (desktop icons + in-window icons/text),
                          chained on a free list via ob_next when unused
```

`obj_init()` (`deskobj.c:113-147`) sets this up: objects `0..WOBS_START-1`
get `ob_head=ob_next=ob_tail=NIL`; objects `WOBS_START..num_sobs-1` are
threaded onto the free chain (`G.g_screenfree = WOBS_START`); `ROOT` is
sized to the full screen (`gl_width`×`gl_height`); objects `DROOT..
DROOT+NUM_WNODES` (i.e. `1..NUM_WNODES+1`) are made children of `ROOT` via
`objc_add`.

**Contract note:** the `OBJECT`/`ICONBLK` struct layouts themselves
(`ob_x`/`ob_y`/…, `ib_xicon`/…) are contract (`docs/api/structures/
object.md`, `iconblk.md`). Which *index* the desktop happens to use for
which purpose (`DROOT`=1, `WOBS_START`=9, …) is **internal** — no other
program indexes into the desktop's private `g_screen[]`.

---

## `WNODE` — `desk/deskwin.h:57-80`

Represents one open window (or, via `G.g_wdesktop`, the desktop pseudo-
window). Fields:

| Field | Type | Purpose | Written by |
|---|---|---|---|
| `w_next` | `WNODE*` | z-order chain (front-to-back among *open* windows), also the *free-list* chain among unopened window slots | `win_start()` init chain; `win_top()` re-splices for z-order |
| `w_flags` | `UWORD` | `WN_DESKTOP` (0x0001, marks `G.g_wdesktop`), `WN_REBUILD` (0x8000, "needs rebuilding" — set/tested outside this task's file set) | `win_start()`, `win_alloc()` |
| `w_id` | `WORD` | AES window handle; **`0` = slot not in use** (sentinel checked by `win_shwall`/z-order walks) | `win_alloc()` (`wind_create_grect`), `win_free()` sets back to `0` |
| `w_obid` | `WORD` | "desktop object id" for this window — if negative, its one's-complement is the drive letter that opened it | `win_alloc()` caller passes it in |
| `w_root` | `WORD` | index into `G.g_screen[]` of this window's item-object root (one of the `2..WOBS_START-1` slots) | `win_alloc()` via `obj_walloc()` |
| `w_cvcol`/`w_cvrow` | `WORD` | **current virtual** top-left column/row being displayed (the scroll position) | `win_alloc()` inits `0`; `win_blt()`/`win_ocalc()` adjust |
| `w_pncol`/`w_pnrow` | `WORD` | **physical** column/row count — how many items actually fit in the current window size | `win_alloc()` initial estimate from window rect; `win_ocalc()` recomputes on every rebuild |
| `w_vncol`/`w_vnrow` | `WORD` | **virtual** column/row count — how many would be needed to show *all* files | `win_ocalc()` (`deskwin.c:320-337`) |
| `w_pnode` | `PNODE` | embedded (not a pointer) — this window's file list, see below | `pn_open`/`pn_active`/`pn_sort` |
| `w_name` | `char[LEN_ZPATH+2]` | title-bar text (path, padded with a leading/trailing space) | `win_sname()` (`deskwin.c:846-859`) |
| `w_info` | `char[72]` | info-line text (item count / total size, `sprintf`'d from a resource format string) | `win_sinfo()` (`deskwin.c:867-906`) |

**cv/pn/vn naming, spelled out:** for a window that can show 4 columns at
once (`pncol=4`) but has 10 files needing 10 columns' worth of virtual
space in a single-row layout (`vncol=10` — actually rows/cols depend on
fit mode, see `deskapp.md`/`deskwin.md`), `cvcol` is which of those 10
virtual columns is scrolled to the window's left edge. This is **not** a
pixel scroll offset — it's a grid-cell offset, converted to pixels only
at draw time via `g_iwspc`/`g_ihspc`.

**Contract:** none. `WNODE` never crosses an ABI boundary (no GEMDOS/AES
call takes a `WNODE*`). Free for NewTOS to redesign.

---

## `PNODE` — `desk/deskfpd.h:69-78`

One open directory's scan result, embedded inside its owning `WNODE`
(`w_pnode`) — or, when there is no owning window (e.g. a disk-to-disk
copy with no window open on either side), inside `G.g_wdesktop.w_pnode`
(`deskfpd.c:80-81`).

| Field | Type | Purpose | Written by |
|---|---|---|---|
| `p_attr` | `WORD` | attribute mask passed to `Fsfirst` — always `DISPATTR` = `FA_SUBDIR` (`deskbind.h:43`; i.e. "also match subdirectories", since files always match) | `pn_open()` (`deskfpd.c:86`) |
| `p_spec` | `char[LEN_ZPATH]` | the directory path + wildcard (e.g. `A:\*.*`) | `pn_open()` (`deskfpd.c:85`) |
| `p_fbase` | `FNODE*` | base pointer of the `malloc`'d `FNODE` block (freed as a whole) | `pn_active()` (`deskfpd.c:243`) |
| `p_flist` | `FNODE*` | head of the **sorted, singly-linked** list of matched entries (may reorder `p_fbase`'s array order — `f_next` is the real order) | `pn_active()` builds the raw chain, then `pn_sort()` re-links it (`deskfpd.c:288`) |
| `p_count` | `WORD` | number of `FNODE`s | `pn_active()` (`deskfpd.c:278`) |
| `p_size` | `LONG` | sum of `f_size` over all entries | `pn_active()` (`deskfpd.c:273,279`) |

`pn_active(pn, include_folders)` (`deskfpd.c:227-298`) is the scan: it
frees any previous list, allocates as many `FNODE`s as available memory
allows (`dos_avail_anyram()/sizeof(FNODE)`, **silently truncating** the
directory listing if memory-constrained — `deskfpd.c:220-224` comment,
citing this as intentional Atari-TOS-compatible behaviour, not a bug),
walks `Fsfirst`/`Fsnext` (via `dos_sfirst`/`dos_snext`, GEMDOS), skips `.`/
`..`, and (if `CONF_WITH_FILEMASK`) applies a wildcard filter to *files*
only (folders always pass when `include_folders`). Returns non-zero only
for a `dos_sfirst` error other than "no files found" (e.g. no disk in a
floppy drive) — `EFILNF`/`ENMFIL` are folded to success (empty list).

**Contract:** none (internal to the desktop app). Its *inputs* are the
GEMDOS `Fsfirst`/`Fsnext` contract (already spec'd elsewhere) and the
`DTA` contract (`docs/api/structures/dta.md`) — see the `FNODE` mirroring
note below.

---

## `FNODE` — `desk/deskfpd.h:51-66`

One matched directory entry.

```c
struct _filenode {
    FNODE *f_next;              /* singly-linked list */
    char  f_selected;           /* TRUE if user-selected */
    char  f_attr;               /* NOTE: f_attr..f_name[] must match DTA layout — see below */
    UWORD f_time;
    UWORD f_date;
    LONG  f_size;
    char  f_name[LEN_ZFNAME];   /* LEN_ZFNAME = 13 (sysconf.h:52) */
    WORD  f_seq;                /* sequence within directory (pre-sort order) */
    WORD  f_obid;                /* index into G.g_screen[] while this file is on-screen */
    ANODE *f_pa;                 /* the matching installed-app ANODE, if any */
    BOOL  f_isap;                /* TRUE => f_pa is an application (use a_aicon) */
};
```

### The `f_attr..f_name[]` / `DTA` size-mirroring trick (edge case)

The header comment (`deskfpd.h:56-59`) says these fields "MUST be the
same size & in the same sequence as the corresponding items in the DTA
structure" — and `pn_active()` exploits this literally:

```
memcpy(&fn->f_attr, &G.g_wdta.d_attrib, 23);   /* deskfpd.c:271 */
```

`DTA`'s `d_attrib..d_fname[14]` range is exactly 23 bytes
(`1+2+2+4+14`, per `docs/api/structures/dta.md`). But `FNODE`'s
`f_attr..f_name[13]` range is only `1+2+2+4+13 = 22` bytes — **one byte
short**. The `memcpy` therefore deliberately overruns by one byte into
`f_seq`'s first byte; this is harmless only because the very next line
(`deskfpd.c:272`) unconditionally overwrites `f_seq` with `count++`. This
is a genuine EmuTOS internal quirk (struct-packing reuse for code size,
not a bug given the immediate overwrite) — **not** contract, but flagged
per protocol as "invisible without reading the code." A NewTOS
reimplementation with a differently-sized `f_name` (e.g. a longer name
buffer, no ABI reason to keep it at 13) would need to copy the DTA fields
individually rather than relying on this overlap.

### Other fields

- `f_selected`/`f_next`: selection UI state (`deskact.c`, out of scope)
  and list linkage (`pn_active`, `pn_sort`).
- `f_seq`: pre-sort directory order; used by `S_NSRT` ("no sort") compares
  (`pn_fcomp`, `deskfpd.c:120-122`) and by `win_dispfile()`'s "file number"
  addressing (`CONF_WITH_SEARCH`, out of scope for rendering core).
- `f_obid`: **write-once per view-build.** `win_ocalc()` resets every
  `f_obid` in the list to `NIL` before recomputing the visible window
  (`deskwin.c:308-312`) — it is the desktop's way of saying "not currently
  on screen." `win_bldview()` then sets it to the real `g_screen[]` index
  for exactly the items that end up displayed (`deskwin.c:440`).
- `f_pa`/`f_isap`: filled in by `win_icalc()` (`deskwin.c:380-384`) calling
  `app_afind_by_name()` (out of scope, `deskapp.c`) to look up whether this
  file/folder matches an installed `ANODE` (for icon choice — see
  `deskwin.md` `win_bldview`).

**Contract:** none. `FNODE` never crosses an ABI boundary. Its *source*
data (the `DTA` fields it mirrors) is contract; the `FNODE` struct itself
is a private cache.

---

## `ANODE` (rendering-relevant fields only) — `desk/deskapp.h:100-116`

Full semantics (install dialogs, EMUDESK.INF encoding, function-key
association) are in `docs/api/desktop.md`. For the rendering core, the
fields that matter are:

| Field | Purpose | Rendering-relevant behaviour |
|---|---|---|
| `a_flags` | bit `AF_ISDESK` (0x0004) selects "needs a desktop icon" | `app_blddesk()`'s main loop filters on this bit (`deskapp.c:1360`) |
| `a_xspot`/`a_yspot` | desktop icon position | **Unit changes across the object's lifetime** (see below) |
| `a_obid` | `g_screen[]` index once placed | written by `app_blddesk()` (`deskapp.c:1371`) |
| `a_letter` | drive letter (or 0), OR'd into the icon's `ib_char` | `app_blddesk()` (`deskapp.c:1397`), `win_bldview()` (`deskwin.c:480`) |
| `a_aicon`/`a_dicon` | index into `G.g_iblist[]` for "is application" / "is data" icon | `app_blddesk()` (`deskapp.c:1374-1378`), `win_bldview()` (`deskwin.c:465-467`) |

### `a_xspot`/`a_yspot` unit gotcha (grid units on disk, pixels in memory)

In the on-disk EMUDESK.INF text format, an item's desktop position is
saved as **grid cell coordinates** (`deskinf.c:1259-1260`:
`(pa->a_xspot/G.g_icw)`, `((pa->a_yspot-G.g_desk.g_y)/G.g_ich)`, both
masked to a byte — so max 255 columns/rows). But at runtime, `app_start()`
**overwrites `a_xspot`/`a_yspot` in place with pixel coordinates**:

```
x = pa->a_xspot * G.g_icw;
y = pa->a_yspot * G.g_ich + G.g_desk.g_y;
snap_icon(x, y, &pa->a_xspot, &pa->a_yspot, 0, 0);   /* deskapp.c:983-985 */
```

i.e. the *parsed* (grid-unit) value is read once, multiplied out to
pixels, passed through `snap_icon()` — **resolved** (was flagged "needs
deeper analysis" here): full extraction in [`deskins.md`](deskins.md),
section "`snap_icon`/`ins_posdisk`/`grid_free` — desktop grid placement".
`snap_icon()` only rounds/clamps to the nearest grid cell — it performs
**no collision avoidance** against other already-placed icons, so this
loop can (and does, given colliding INF data) silently overlap two
desktop icons — and the *result* (pixels) is written back into the same
two fields. Every later
read of `a_xspot`/`a_yspot` (`app_blddesk`'s `obj_ialloc` call,
`deskapp.c:1362`) sees **pixels**, not grid units. Only the INF-save path
(`deskinf.c`) converts back. This dual-unit lifecycle is invisible from
the field declaration alone — a pure "read the code" finding.

**Contract:** none (desktop-internal placement bookkeeping). The
EMUDESK.INF *file format* itself (grid-unit encoding, byte-masked to 0-255)
would be a contract only if NewTOS chooses to support reading/writing
original-format `EMUDESK.INF` files for cross-boot-compatibility with a
real TOS install — out of scope for this task; flag for
`docs/REQUIREMENTS.md`/`docs/STATUS.md` if that goal is ever adopted.

---

## `SCREENINFO`/`ICONINFO` — `desk/deskbind.h:89-102`

Parallel array to `G.g_screen[]` (same index), carrying per-object data
that doesn't fit in the fixed-size `OBJECT` struct:

```c
typedef struct { WORD index; ICONBLK block; } ICONINFO;   /* for G_ICON objects */
typedef struct {
    FNODE *fnptr;               /* -> FNODE this object represents, or NULL */
    union { ICONINFO icon; USERBLK udef; } u;   /* icon: G_ICON; udef: G_USERDEF (text) */
} SCREENINFO;
```

- `fnptr`: lets selection code (`deskact.c`, out of scope) map a clicked
  `g_screen[]` object back to its `FNODE` (for files/folders in a window)
  — `NULL` for desktop icons (`ANODE`-backed, no `FNODE`).
- `u.icon.block`: the **actual `ICONBLK`** that `obj->ob_spec` points at
  — i.e. every displayed icon gets its **own copy** (via `memcpy` from
  `G.g_iblist[i_index]`), not a shared pointer, because each instance
  needs its own `ib_ptext` (the specific file/drive name) and `ib_char`
  (drive-letter overlay).
- `u.icon.index`: which `G.g_iblist[]` entry this copy came from (used
  by `act_chkobj()`/`insa_icon()`, out of scope here, per the source
  comment at `deskapp.c:495-497`).
- `u.udef`: `V_TEXT` mode only — a `USERBLK` whose `ub_code` is the
  `dr_code()` draw callback (`deskinf.c:331`, out of scope) and `ub_parm`
  is the `FNODE*` to format.

**Contract:** none — this is exactly the private "extra data per object"
table the desktop needs because `OBJECT`'s own `ob_spec` is only a
pointer; the *pointee* layout (`ICONBLK`, `USERBLK`) is contract, the
*array-of-unions bookkeeping* is not.
