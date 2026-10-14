//! DESK43 U6b — host checks of the program build coupling and of X03
//! (`docs/analysis/desk43-u5-u10-review-2026-10-14.md` §3.2 U6b, §6 X03 row).
//!
//! * Every native Rust program in `programs/` is built by `build-programs`
//!   and takes the board feature it passes (the X03 pass-through: a program
//!   without the feature would fail the cargo build, a program missing from
//!   [`PROGRAMS`] would silently not be built).
//! * The version and machine strings the desktop's About box shows are
//!   deliberate second copies of the boot screen's (`kernel/src/initinfo.rs`,
//!   `programs/desktop/src/inf.rs`, X03): the desktop must not name a kernel
//!   symbol, so the copies are kept equal by this test instead.
//! * DESK43 U8b: the desktop is the library crate `programs/desktop`. It is
//!   not a program (no `staticlib`, not in [`PROGRAMS`]) but takes the same
//!   board features, which the kernel passes through together with
//!   `selftest-desk`; and its dependency graph is the boundary guard G1 of
//!   the review (§4.2): `newtos-gem` + `tos-types`, nothing else, all the way
//!   down.

use std::fs;
use std::path::{Path, PathBuf};

use crate::{emutos_consts, kernel_embed, program_build_command, Board, PROGRAMS};

fn root() -> PathBuf {
    crate::workspace_root()
}

/// The `members = [...]` of `programs/Cargo.toml`.
fn program_members(root: &Path) -> Vec<String> {
    let text = fs::read_to_string(root.join("programs/Cargo.toml")).expect("programs/Cargo.toml");
    let start = text.find("members").expect("programs/Cargo.toml: no members");
    let open = start + text[start..].find('[').expect("members: no [");
    let close = open + text[open..].find(']').expect("members: no ]");
    text[open + 1..close]
        .split(',')
        .map(|m| m.trim().trim_matches('"').to_string())
        .filter(|m| !m.is_empty())
        .collect()
}

/// The feature names declared in the `[features]` table of a manifest.
fn features(manifest: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_features = false;
    for line in manifest.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_features = l == "[features]";
            continue;
        }
        if in_features && !l.starts_with('#') {
            if let Some((name, _)) = l.split_once('=') {
                out.push(name.trim().to_string());
            }
        }
    }
    out
}

#[test]
fn every_program_is_built_and_takes_both_board_features() {
    let root = root();
    let members = program_members(&root);
    assert!(!members.is_empty(), "programs/Cargo.toml lists no members");
    let mut programs = 0;
    for m in &members {
        let manifest = fs::read_to_string(root.join("programs").join(m).join("Cargo.toml"))
            .unwrap_or_else(|e| panic!("programs/{m}/Cargo.toml: {e}"));
        let is_program = manifest
            .lines()
            .any(|l| l.trim().starts_with("crate-type") && l.contains("\"staticlib\""));
        if !is_program {
            continue;
        }
        programs += 1;
        assert!(
            PROGRAMS.iter().any(|(k, _)| k == m),
            "programs/{m} is a program (staticlib) but not in xtask PROGRAMS: build-programs would skip it"
        );
        let feats = features(&manifest);
        for b in Board::ALL {
            assert!(
                feats.iter().any(|f| f == b.feature()),
                "programs/{m}/Cargo.toml does not declare `{}` (X03 pass-through)",
                b.feature()
            );
        }
    }
    for (k, _) in PROGRAMS {
        assert!(members.iter().any(|m| m == k), "xtask PROGRAMS names `{k}`, which is not a programs/ member");
    }
    assert_eq!(programs, PROGRAMS.len(), "PROGRAMS and the staticlib members of programs/ differ");
}

#[test]
fn build_programs_passes_the_board_feature_and_writes_the_board_directory() {
    let root = root();
    for b in Board::ALL {
        for (krate, file) in PROGRAMS {
            let c = program_build_command(&root, b, krate, file);
            let args: Vec<String> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
            let i = args.iter().position(|a| a == "--features").expect("no --features");
            assert_eq!(args.get(i + 1).map(String::as_str), Some(b.feature()), "{args:?}");
            assert_eq!(args.get(i - 1).map(String::as_str), Some("--"), "cargo args after `--`: {args:?}");
            let out = kernel_embed::programs_dir(&root, b.name()).join(file);
            assert!(args.contains(&out.to_string_lossy().into_owned()), "{args:?}");
            assert!(args[0].ends_with("programs/build.sh"), "{args:?}");
            assert_eq!(args[1], *krate);
        }
    }
}

/// The first string literal after `const <name>: &str =` in `text`, if any
/// (the literal may start on the next line). Only `\\` and `\"` escapes are
/// expected in these strings.
fn const_str(text: &str, name: &str) -> Option<String> {
    let key = format!("const {name}: &str =");
    let at = text.find(&key)? + key.len();
    string_literal(&text[at..])
}

/// The first `"…"` in `s`, unescaped.
fn string_literal(s: &str) -> Option<String> {
    let open = s.find('"')?;
    let mut out = String::new();
    let mut chars = s[open + 1..].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(chars.next()?),
            '"' => return Some(out),
            c => out.push(c),
        }
    }
    None
}

/// The desktop's per-board `MACHINE_NAME`: the literal of the `const
/// MACHINE_NAME` that follows `#[cfg(feature = "<feature>")]`.
fn desk_machine_name(text: &str, feature: &str) -> Option<String> {
    let key = format!("#[cfg(feature = \"{feature}\")]");
    let mut rest = text;
    while let Some(at) = rest.find(&key) {
        let after = &rest[at + key.len()..];
        if after.trim_start().starts_with("const MACHINE_NAME: &str =") {
            return const_str(after, "MACHINE_NAME");
        }
        rest = after;
    }
    None
}

/// The boot screen's machine name for `board`: the `"<board>" => "…"` arm of
/// `initinfo::machine_name`.
fn boot_machine_name(text: &str, board: &str) -> Option<String> {
    let f = text.find("fn machine_name()")?;
    let body = &text[f..];
    let key = format!("\"{board}\" =>");
    let at = body.find(&key)? + key.len();
    string_literal(&body[at..])
}

/// Every `.rs` text under `dir` (recursively), concatenated with its path.
fn sources(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap_or_else(|e| panic!("{}: {e}", d.display())) {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let t = fs::read_to_string(&p).unwrap();
                out.push((p, t));
            }
        }
    }
    out
}

/// Review §6 (X03 row): the desktop's version, notice and per-board machine
/// strings equal the boot screen's. Found by name in the desktop source
/// (the `desktop` crate since U8b, `kernel/src/desk/` before — the rule of
/// `emutos_consts::desktop_src`).
#[test]
fn desktop_and_boot_screen_strings_agree() {
    let root = root();
    let boot = fs::read_to_string(root.join("kernel/src/initinfo.rs")).expect("kernel/src/initinfo.rs");
    let desk_dir = emutos_consts::desktop_src(&root).expect("desktop source");
    let desk = sources(&desk_dir);

    for name in ["VERSION_STRING", "INDEPENDENCE_NOTICE"] {
        let b = const_str(&boot, name).unwrap_or_else(|| panic!("initinfo.rs: no const {name}"));
        let found: Vec<(String, String)> = desk
            .iter()
            .filter_map(|(p, t)| const_str(t, name).map(|v| (p.display().to_string(), v)))
            .collect();
        assert_eq!(found.len(), 1, "desktop: exactly one const {name} expected, found {found:?}");
        assert_eq!(found[0].1, b, "{name}: desktop {} vs boot screen kernel/src/initinfo.rs", found[0].0);
    }

    for board in Board::ALL {
        let b = boot_machine_name(&boot, board.name())
            .unwrap_or_else(|| panic!("initinfo.rs machine_name(): no arm for \"{}\"", board.name()));
        let found: Vec<(String, String)> = desk
            .iter()
            .filter_map(|(p, t)| desk_machine_name(t, board.feature()).map(|v| (p.display().to_string(), v)))
            .collect();
        assert_eq!(found.len(), 1, "desktop: one MACHINE_NAME for {} expected, found {found:?}", board.feature());
        assert_eq!(found[0].1, b, "machine line for {}: desktop {} vs boot screen", board.name(), found[0].0);
    }
}

/// `(table, name)` for every dependency of a manifest: inline `name = …`,
/// dotted `name.workspace = true` and `[dependencies.name]` headers, in every
/// `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` and
/// `[target.….dependencies]` table.
fn dependencies(manifest: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut table: Option<String> = None;
    for line in manifest.lines() {
        let l = line.trim();
        if l.starts_with('#') || l.is_empty() {
            continue;
        }
        if l.starts_with('[') {
            let h = l.trim_matches(|c| c == '[' || c == ']').trim().to_string();
            let is_dep_table = |h: &str| {
                h == "dependencies"
                    || h == "dev-dependencies"
                    || h == "build-dependencies"
                    || h.ends_with(".dependencies")
                    || h.ends_with(".dev-dependencies")
                    || h.ends_with(".build-dependencies")
            };
            table = None;
            if is_dep_table(&h) {
                table = Some(h);
            } else if let Some((t, name)) = h.rsplit_once('.') {
                if is_dep_table(t) {
                    out.push((t.to_string(), name.to_string()));
                }
            }
            continue;
        }
        if let Some(t) = &table {
            let name: String =
                l.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
            if !name.is_empty() {
                out.push((t.clone(), name));
            }
        }
    }
    out
}

/// The manifest of the path dependency `dep` of the package at `dir`.
fn path_dependency(dir: &Path, manifest: &str, dep: &str) -> PathBuf {
    let line = manifest
        .lines()
        .find(|l| l.trim_start().starts_with(&format!("{dep} ")) || l.trim_start().starts_with(&format!("{dep}=")))
        .unwrap_or_else(|| panic!("{}: no line for dependency {dep}", dir.display()));
    let at = line.find("path").unwrap_or_else(|| panic!("{dep} is not a path dependency: {line}"));
    let p = string_literal(&line[at..]).expect("path literal");
    dir.join(p).join("Cargo.toml")
}

/// Guard G1 (review §4.2, DESK43 U8b): the desktop crate depends on
/// `newtos-gem` and `tos-types` only — so a `tos_api::` or kernel path in it
/// is a compiler error — and so do they, transitively: `newtos-gem` on
/// `tos-types` alone, `tos-types` on nothing. `newtos-gem` comes without its
/// `rt` (the kernel links the desktop and has its own allocator and panic
/// handler). Mirrors group D of `tools/desk-drift-probe.sh`.
#[test]
fn desktop_depends_only_on_newtos_gem_and_tos_types() {
    let root = root();
    let desk_dir = root.join("programs/desktop");
    let desk = fs::read_to_string(desk_dir.join("Cargo.toml")).expect("programs/desktop/Cargo.toml");
    let deps = dependencies(&desk);
    let mut names: Vec<&str> = deps.iter().map(|(_, n)| n.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["newtos-gem", "tos-types"], "programs/desktop/Cargo.toml dependencies: {deps:?}");
    assert!(
        deps.iter().all(|(t, _)| t == "dependencies"),
        "programs/desktop/Cargo.toml: dependencies outside [dependencies]: {deps:?}"
    );
    let gem_line = desk.lines().find(|l| l.trim_start().starts_with("newtos-gem")).unwrap();
    assert!(gem_line.contains("default-features = false"), "desktop must link newtos-gem without `rt`: {gem_line}");

    let gem_toml = path_dependency(&desk_dir, &desk, "newtos-gem");
    let gem = fs::read_to_string(&gem_toml).unwrap_or_else(|e| panic!("{}: {e}", gem_toml.display()));
    let gem_deps: Vec<String> = dependencies(&gem).into_iter().map(|(_, n)| n).collect();
    assert_eq!(gem_deps, ["tos-types"], "{}: {gem_deps:?}", gem_toml.display());

    let types_toml = path_dependency(&desk_dir, &desk, "tos-types");
    let types = fs::read_to_string(&types_toml).unwrap_or_else(|e| panic!("{}: {e}", types_toml.display()));
    assert_eq!(dependencies(&types), [], "{}: tos-types must stay dependency-free", types_toml.display());
}

/// X03 for the desktop crate (DESK43 U8b): it declares both board features and
/// `selftest-desk`, and the kernel passes each of its own through
/// (`board-x = ["desktop/board-x"]`), so the desktop is always built for the
/// kernel's board — and the NEWDESK program (U9) gets the same switch from
/// `build-programs`.
#[test]
fn desktop_takes_the_kernel_board_and_selftest_features() {
    let root = root();
    let desk = fs::read_to_string(root.join("programs/desktop/Cargo.toml")).expect("programs/desktop/Cargo.toml");
    let kernel = fs::read_to_string(root.join("kernel/Cargo.toml")).expect("kernel/Cargo.toml");
    let desk_feats = features(&desk);
    let mut switches: Vec<&str> = Board::ALL.iter().map(|b| b.feature()).collect();
    switches.push("selftest-desk");
    for f in switches {
        assert!(desk_feats.iter().any(|d| d == f), "programs/desktop/Cargo.toml does not declare `{f}`");
        let line = kernel
            .lines()
            .find(|l| l.trim_start().starts_with(&format!("{f} =")))
            .unwrap_or_else(|| panic!("kernel/Cargo.toml: no feature `{f}`"));
        assert!(
            line.contains(&format!("\"desktop/{f}\"")),
            "kernel/Cargo.toml feature `{f}` does not enable `desktop/{f}`: {line}"
        );
    }
}

#[test]
fn dependency_scanner_reads_every_table_form() {
    let m = "[package]\nname = \"x\"\n[dependencies]\n# c\na = { path = \"../a\" }\nb.workspace = true\n\
             [dependencies.c]\npath = \"../c\"\n[target.'cfg(x)'.dependencies]\nd = \"1\"\n\
             [dev-dependencies]\ne = \"1\"\n[features]\nf = []\n";
    let d = dependencies(m);
    let names: Vec<&str> = d.iter().map(|(_, n)| n.as_str()).collect();
    assert_eq!(names, ["a", "b", "c", "d", "e"], "{d:?}");
    assert_eq!(dependencies("[package]\nname = \"t\"\n"), []);
}

#[test]
fn string_scanner_reads_multi_line_and_escaped_literals() {
    let t = "pub(crate) const A: &str =\n    \"x \\\"y\\\" z\";\nconst B: &str = \"b\";";
    assert_eq!(const_str(t, "A").as_deref(), Some("x \"y\" z"));
    assert_eq!(const_str(t, "B").as_deref(), Some("b"));
    assert_eq!(const_str(t, "C"), None);
    let d = "#[cfg(feature = \"board-x\")]\nconst MACHINE_NAME: &str = \"X\";\n#[cfg(feature = \"board-y\")]\nconst MACHINE_NAME: &str = \"Y\";";
    assert_eq!(desk_machine_name(d, "board-y").as_deref(), Some("Y"));
    assert_eq!(desk_machine_name(d, "board-z"), None);
    let i = "fn machine_name() -> &'static str {\n    match B {\n        \"x\" => \"Ex\",\n        other => other,\n    }\n}";
    assert_eq!(boot_machine_name(i, "x").as_deref(), Some("Ex"));
    assert_eq!(boot_machine_name(i, "y"), None);
}
