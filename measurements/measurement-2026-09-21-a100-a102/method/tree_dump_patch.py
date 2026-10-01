#!/usr/bin/env python3
"""A100-TEMP (scratch copy only): after the desktop registered its menu bar,
print the geometry `build_menu` really built — THEACTIVE's rectangle (what
`menu_bar` takes as gl_ctwait), the File box's ob_spec word and absolute
rectangle, and the first File entry's rectangle — to the serial line, so the
board run ties the kernel tree to the host replica."""
import sys
p = sys.argv[1] + '/kernel/src/desk/menu.rs'; s = open(p).read()
a = "pub fn install_menu_bar() {\n    let tree = build_menu();\n    super::menu_bar(tree as u64, 1);\n}"
assert s.count(a) == 1
b = '''pub fn install_menu_bar() {
    let tree = build_menu();
    super::menu_bar(tree as u64, 1);
    // A100-TEMP: dump the registered geometry.
    unsafe {
        let o = &*MENU.obj.get();
        let fb = 3 + MENUS.len() + 1 + 1 + MENUS[0].entries.len(); // File box index
        let a = &o[2];
        let b = &o[fb];
        let e = &o[fb + 1];
        crate::kprintln!("[a100] THEACTIVE=({},{},{},{}) FILEBOX#{}=({},{},{},{}) spec={:#x} ENTRY0=({},{},{},{}) titles=({},{}),({},{}),({},{}),({},{})",
            a.ob_x, a.ob_y, a.ob_width, a.ob_height, fb, b.ob_x, b.ob_y, b.ob_width, b.ob_height, b.ob_spec,
            e.ob_x, e.ob_y, e.ob_width, e.ob_height,
            o[3].ob_x, o[3].ob_width, o[4].ob_x, o[4].ob_width, o[5].ob_x, o[5].ob_width, o[6].ob_x, o[6].ob_width);
    }
}'''
s = s.replace(a, b)
open(p, 'w').write(s)
print("patched", p)
