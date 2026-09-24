#!/usr/bin/env python3
"""DESK19/DESK20-TEMP (scratch copy only): the serial scaffolding these board
runs need on top of the standard injector + probe patches.

Every dump goes through `tos_api::aes::h02_log` (the a79 serial-only sink) or
`kprintln!` (UART only), so nothing reaches the framebuffer and the
screendumps stay clean.

1) kernel/src/desk/inf.rs    -- `probe_tree()`: walk any dialog tree and print
                                every object's ABSOLUTE rectangle with the
                                point that clicks it.  Works on the "before"
                                trees too (it walks links, it does not assume
                                indices), which is how the `ADDESKCF` and
                                `ADINSAPP` click points are MEASURED rather
                                than guessed in both trees.
2) kernel/src/desk/inf.rs    -- called from `inf_conf` right after
                                `start_dialog`, with the current `g_appdir` /
                                `g_fullpath` (the latter only exists in the
                                "after" tree).
3) kernel/src/desk/insapp.rs -- the same call from `ins_app_dialog`.
4) tos-api/src/aes/form.rs   -- every `form_alert` box + button rectangle with
                                its click point (copied from
                                measurement-2026-09-23-desk24-desk25-desk26),
                                which is where DESK20's `STINVKEY` /
                                `STDUPKEY` click points come from.
5) kernel/src/desk/dir.rs    -- [after only] what `fun_win2win` found at the
                                drop target and what `build_selected_path`
                                built.
6) kernel/src/desk/main.rs   -- [after only] `do_aopen_drop`'s six arguments.
7) kernel/src/desk/insapp.rs -- [after only] `get_funkey`'s input and answer.

Applied to the scratch copy AFTER a79_patch_desk10.py / apply_injector.py /
a78_patch2.py / key_step_patch.py / probe_patch.py, never to the shared tree.
"""
import sys

scr = sys.argv[1]


def rep(path, a, b, optional=False):
    s = open(path).read()
    n = s.count(a)
    if n == 0 and optional:
        print("skipped (anchor absent, 'before' tree):", a[:50].replace("\n", " "))
        return
    assert n == 1, (path, a[:70], n)
    open(path, 'w').write(s.replace(a, b, 1))
    print("patched", path)


INF = scr + '/kernel/src/desk/inf.rs'
INSAPP = scr + '/kernel/src/desk/insapp.rs'
DIR = scr + '/kernel/src/desk/dir.rs'
MAIN = scr + '/kernel/src/desk/main.rs'

# 1) the generic tree dumper
PROBE_TREE = '''
// DESK19-TEMP BEGIN (scratch copy only)
/// Serial dump of a whole dialog tree: every object's absolute rectangle and
/// the point that clicks it. Walks objects 0.. until `LASTOB`, so it makes no
/// assumption about the numbering and works on the "before" trees too.
pub(super) fn probe_tree(tag: &str, tree: u64, labels: &[(i16, &str)]) {
    const LASTOB: u16 = 0x0020;
    for i in 0..64i16 {
        // SAFETY: `tree` is a live OBJECT array; the walk stops at LASTOB.
        let o = unsafe { &*(tree as *const tos_api::aes::object::Object).add(i as usize) };
        let (x, y) = super::objc_offset(tree, i);
        let name = labels.iter().find(|&&(j, _)| j == i).map(|&(_, n)| n).unwrap_or("");
        tos_api::aes::h02_log(format_args!(
            "[d19] {} obj {} {} abs ({},{}) {}x{} ty={} fl={:x} st={:x} -> click ({},{})\\r\\n",
            tag, i, name, x, y, o.ob_width, o.ob_height,
            o.ob_type, o.ob_flags, o.ob_state, x + o.ob_width / 2, y + o.ob_height / 2));
        if o.ob_flags & LASTOB != 0 {
            break;
        }
    }
}
// DESK19-TEMP END
'''

rep(INF, "// ── Desktop-configuration dialog", PROBE_TREE + "\n// ── Desktop-configuration dialog")

# 2) inf_conf: dump the tree + the two switches it edits
# The field itself, not a doc-comment mention of `G.g_fullpath` (HEAD's
# inf.rs has one of those in the "what this dialog does not offer" note).
HAS_FULLPATH = 'pub g_fullpath: bool' in open(scr + '/kernel/src/desk/global.rs').read()
SW = ('global::with(|g| (g.g_appdir, g.g_fullpath)).unwrap_or((true, true))'
      if HAS_FULLPATH else 'global::with(|g| (g.g_appdir, false)).unwrap_or((true, false))')
rep(INF,
    """    start_dialog(tree_ptr);
    let mut exitobj;""",
    """    start_dialog(tree_ptr);
    {
        // DESK19-TEMP
        let (__ad, __fp) = """ + SW + """;
        tos_api::aes::h02_log(format_args!(
            "[d19] ADDESKCF g_appdir={} g_fullpath={} (fullpath field present: """
    + ("yes" if HAS_FULLPATH else "NO") + """)\\r\\n", __ad, __fp));
        probe_tree("ADDESKCF", tree_ptr, &[]);
    }
    let mut exitobj;""")

# 3) ins_app_dialog: dump the tree
rep(INSAPP,
    """    super::inf::start_dialog(tree_ptr);
    let outcome = loop {""",
    """    super::inf::start_dialog(tree_ptr);
    super::inf::probe_tree("ADINSAPP", tree_ptr, &[]); // DESK20-TEMP
    let outcome = loop {""")

# 4) every form_alert: box + buttons with their click points
rep(scr + '/tos-api/src/aes/form.rs',
    """    // Hide the cursor while drawing the alert so its save-under does not""",
    """    {
        // DESK19-TEMP: the alert's geometry, so DESK20's two alert click
        // points are measured and not guessed.
        crate::aes::h02_log(format_args!(
            "[alert] box ({},{}) {}x{} nbut={} defbut={}\\r\\n",
            box_x, box_y, box_w, box_h, numbut, defbut));
        for j in 0..numbut {
            let o = &tree[first_btn + j];
            crate::aes::h02_log(format_args!(
                "[alert]   but {} abs ({},{}) {}x{} -> click ({},{})\\r\\n",
                j + 1, box_x + o.ob_x, box_y + o.ob_y, o.ob_width, o.ob_height,
                box_x + o.ob_x + o.ob_width / 2, box_y + o.ob_y + o.ob_height / 2));
        }
    }
    // Hide the cursor while drawing the alert so its save-under does not""")

# 5) fun_win2win's drop-on-an-application branch (after only)
rep(DIR,
    """    if let Some((dst_name, pda)) = i_find_win(dst_wh, dst_obj) {
        if pda.aicon >= 0 {
            let fullpath = global::with(|g| g.g_fullpath).unwrap_or(true);
            if let Some(destpath) = build_selected_path(&src_dir, &entries, fullpath) {""",
    """    if let Some((dst_name, pda)) = i_find_win(dst_wh, dst_obj) {
        // DESK19-TEMP
        tos_api::aes::h02_log(format_args!(
            "[d19] fun_win2win i_find -> {:?} aicon={} installed={} pappl={:?} appdir={}\\r\\n",
            dst_name.as_str(), pda.aicon, pda.is_installed(), pda.pappl.as_str(), pda.appdir));
        if pda.aicon >= 0 {
            let fullpath = global::with(|g| g.g_fullpath).unwrap_or(true);
            if let Some(destpath) = build_selected_path(&src_dir, &entries, fullpath) {
                tos_api::aes::h02_log(format_args!(
                    "[d19] build_selected_path(src={:?}, first={:?}, fullpath={}) -> {:?}\\r\\n",
                    src_dir.as_str(), entries[0].name_str(), fullpath, destpath.as_str())); // DESK19-TEMP""",
    optional=True)

# 6) do_aopen_drop's arguments (after only)
rep(MAIN,
    """) -> bool {
    // `if (is_installed(pa)) { … AF_APPDIR … pa->a_pappl } else { … G.g_appdir""",
    """) -> bool {
    // DESK19-TEMP
    tos_api::aes::h02_log(format_args!(
        "[d19] do_aopen_drop installed={} pappl={:?} node_appdir={} dst_dir={:?} prog={:?} tail={:?}\\r\\n",
        installed, pappl, node_appdir, dst_dir, prog_name, tail));
    // `if (is_installed(pa)) { … AF_APPDIR … pa->a_pappl } else { … G.g_appdir""",
    optional=True)

# 7) get_funkey's input and answer (after only)
rep(INSAPP,
    """fn get_funkey(fkey: &[u8], app_path: &str, current: u8, installed: bool) -> Option<u8> {""",
    """fn get_funkey(fkey: &[u8], app_path: &str, current: u8, installed: bool) -> Option<u8> {
    let __r = get_funkey_inner(fkey, app_path, current, installed); // DESK20-TEMP
    tos_api::aes::h02_log(format_args!(
        "[d20] get_funkey(fkey={:?}, app={:?}, current={}, installed={}) -> {:?}\\r\\n",
        core::str::from_utf8(&fkey[..fkey.iter().position(|&c| c == 0).unwrap_or(fkey.len())])
            .unwrap_or("?"),
        app_path, current, installed, __r));
    __r
}

fn get_funkey_inner(fkey: &[u8], app_path: &str, current: u8, installed: bool) -> Option<u8> {""",
    optional=True)

# 8) what `install_application` was finally given (after only)
rep(MAIN,
    """            super::inf::install_application(&app_path, &doc_ext, fl, full_path, false, funkey, "");""",
    """            tos_api::aes::h02_log(format_args!(
                "[d20] install_application(app={:?}, ext={:?}, full_path={}, funkey={})\\r\\n",
                app_path.as_str(), doc_ext.as_str(), full_path, funkey)); // DESK20-TEMP
            super::inf::install_application(&app_path, &doc_ext, fl, full_path, false, funkey, "");""",
    optional=True)
