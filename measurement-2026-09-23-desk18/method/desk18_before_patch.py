#!/usr/bin/env python3
"""DESK18 CONTROL build (scratch copy only): take the new current-directory
step back out of the non-installed launch arm, so the same card and the same
action list run on a kernel that behaves exactly as the tree did before this
ticket. Nothing else differs -- this is the "before" half of the A/B."""
import sys
p = sys.argv[1] + '/kernel/src/desk/main.rs'
s = open(p).read()
a = """            let appdir = global::with(|g| g.g_appdir).unwrap_or(true);
            if set_launch_dir(appdir, &path) {
                launch_native(&path, &name, flavor, &[]);
            }
"""
b = """            let _ = global::with(|g| g.g_appdir).unwrap_or(true); // CONTROL: no set_launch_dir
            launch_native(&path, &name, flavor, &[]);
"""
assert s.count(a) == 1
open(p, 'w').write(s.replace(a, b, 1))
print("patched (CONTROL, no set_launch_dir)", p)
