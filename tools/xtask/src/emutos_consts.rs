//! `xtask emutos-consts` — the desktop's hand-declared integer constants
//! against EmuTOS' `#define`s of the same name (DESK43 U5a, guard G3 of
//! `docs/analysis/desk43-u5-u10-review-2026-10-14.md` §4).
//!
//! # Why
//!
//! The desktop declares ~430 `const NAME: <int> = <literal>;` by hand, copied
//! from EmuTOS headers. A wrong copy is invisible to every other check: it
//! compiles, it touches no `tos_api::` symbol, and the drift probe does not
//! see it — `WM_VSLID = 25` (`docs/STATUS.md`, 2026-09-01) was one, and the
//! review's prototype found `BLKDEVNUM = 16` against EmuTOS' 26. This check
//! compares every such constant whose name EmuTOS also defines.
//!
//! # Snapshot, not the live checkout
//!
//! `references/emutos/` is gitignored (`references/README.md`), so a check
//! that read it could not run in CI or on a fresh clone. The EmuTOS side is
//! therefore a **checked-in snapshot** ([`SNAPSHOT`]): every integer-literal
//! `#define` of the file set [`EMUTOS_GLOBS`], with its `file:line`, taken from
//! one EmuTOS commit. `cargo xtask emutos-consts --update` regenerates it from
//! a local `references/emutos/` (and records that commit); the host test
//! [`tests::desk_consts_match_emutos_snapshot`] runs on the snapshot alone.
//!
//! # Deviations
//!
//! A mismatch is either a bug or a decision. Every mismatch must have an entry
//! in [`DEVIATIONS`] (`name`, our value, `intended`/`defect`, why); an entry
//! whose mismatch has gone (fixed, renamed) is **stale** and fails the check
//! too, so the list cannot rot.
//!
//! # Shared sets
//!
//! The AES/object constants the desktop uses (`G_*`, `ob_flags`/`ob_state`,
//! `WF_*`, `WM_*`, `MU_*`, …) are no longer copied into the desktop: since
//! DESK43 U5b they have one home, [`SHARED_SETS`] (`tos-types/src/aes.rs`;
//! since U6b also the trap bindings in `programs/newtos-gem`), which the AES,
//! the desktop and native programs import. The check scans
//! those files together with the desktop, so moving a constant out of the
//! desktop into the shared set does not take it out of the comparison.
//!
//! # What it does not see
//!
//! Constants without a same-named EmuTOS `#define` (renamed, NewTOS-only,
//! `enum` members in EmuTOS) — they are counted as "no counterpart", not
//! checked. Non-literal constants (`A | B`, `1 << 3`, casts) are not parsed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The snapshot of EmuTOS' integer `#define`s, relative to the workspace root.
pub const SNAPSHOT: &str = "tools/xtask/emutos-defines.snapshot";
/// The documented mismatches, relative to the workspace root.
pub const DEVIATIONS: &str = "tools/xtask/emutos-consts-deviations.txt";
/// The shared constant sets the desktop imports instead of declaring (DESK43
/// U5b), relative to the workspace root; scanned together with the desktop.
/// Since DESK43 U6b that includes the trap bindings' constants: the opcodes
/// (`APPL_INIT`, `POLYLINE`, …), modes and colours the desktop's bindings
/// declared moved with them to `programs/newtos-gem`, and the GEMDOS function
/// numbers to `tos-types/src/gemdos.rs`.
pub const SHARED_SETS: &[&str] = &[
    "tos-types/src/aes.rs",
    "tos-types/src/gemdos.rs",
    "programs/newtos-gem/src/aes.rs",
    "programs/newtos-gem/src/gemdos.rs",
    "programs/newtos-gem/src/vdi.rs",
    "programs/newtos-gem/src/xbios.rs",
];
/// The EmuTOS files the snapshot covers (directory, file suffix), relative to
/// `references/emutos/`. Every header the desktop's citations name lives here
/// (`include/sysconf.h`, `desk/desk_rsc.h`, `aes/*.h`, …) plus the `#define`s
/// local to `desk/*.c`.
pub const EMUTOS_GLOBS: &[(&str, &str)] =
    &[("include", ".h"), ("desk", ".h"), ("aes", ".h"), ("desk", ".c")];

/// One `const NAME: <int> = <literal>;` in the desktop source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustConst {
    pub file: String,
    pub line: usize,
    pub name: String,
    pub value: i64,
}

/// One integer `#define NAME <literal>` in EmuTOS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Define {
    pub name: String,
    pub value: i64,
    /// `dir/file.h:line`, relative to `references/emutos/`.
    pub at: String,
}

/// A documented mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deviation {
    pub name: String,
    pub value: i64,
    /// `intended` (a recorded decision) or `defect` (known wrong, to be fixed).
    pub kind: String,
    pub reason: String,
}

/// The result of comparing the desktop's constants against the snapshot.
#[derive(Debug, Default)]
pub struct Report {
    pub total: usize,
    pub matched: usize,
    /// Desktop constants whose value is none of EmuTOS' values for that name,
    /// with EmuTOS' definitions and the deviation entry (if documented).
    pub mismatches: Vec<(RustConst, Vec<Define>, Option<Deviation>)>,
    pub no_counterpart: Vec<RustConst>,
    /// Matched, but EmuTOS defines the name with more than one value
    /// (conditional compilation) — the match is against one of them.
    pub ambiguous: Vec<RustConst>,
    /// Deviation entries that match no current mismatch.
    pub stale: Vec<Deviation>,
}

impl Report {
    /// Mismatches without a deviation entry.
    pub fn undocumented(&self) -> Vec<&RustConst> {
        self.mismatches
            .iter()
            .filter(|(_, _, d)| d.is_none())
            .map(|(c, _, _)| c)
            .collect()
    }

    /// The check passes when every mismatch is documented and no entry is stale.
    pub fn ok(&self) -> bool {
        self.undocumented().is_empty() && self.stale.is_empty()
    }
}

/// Parse an integer literal: optional `-`, decimal / `0x` hex / `0b` binary,
/// `_` separators, an optional Rust integer-type suffix (`0x10u16`).
pub fn parse_rust_int(s: &str) -> Option<i64> {
    let s = s.trim();
    let (neg, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, s),
    };
    let mut body = body.replace('_', "");
    for suf in ["usize", "isize", "u64", "i64", "u32", "i32", "u16", "i16", "u8", "i8"] {
        // A suffix starts with `u`/`i`, never a hex digit, so `0x7fu8` is safe.
        if let Some(b) = body.strip_suffix(suf) {
            body = b.to_string();
            break;
        }
    }
    let v = if let Some(h) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else if let Some(b) = body.strip_prefix("0b") {
        i64::from_str_radix(b, 2).ok()?
    } else if !body.is_empty() && body.bytes().all(|c| c.is_ascii_digit()) {
        body.parse::<i64>().ok()?
    } else {
        return None;
    };
    Some(if neg { -v } else { v })
}

/// The part of a Rust line before a `//` comment (string literals do not
/// occur on the lines this module parses: integer constants).
fn strip_line_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Every `[pub[(…)]] const NAME: <int type> = <literal>;` in one Rust file.
/// `NAME` must look like a constant (`[A-Z][A-Z0-9_]*`).
pub fn parse_rust_consts(file: &str, text: &str) -> Vec<RustConst> {
    const INT_TYPES: &[&str] =
        &["i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize"];
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let mut s = strip_line_comment(raw).trim();
        if let Some(rest) = s.strip_prefix("pub") {
            let rest = rest.trim_start();
            s = if let Some(r) = rest.strip_prefix('(') {
                match r.find(')') {
                    Some(j) => r[j + 1..].trim_start(),
                    None => continue,
                }
            } else {
                rest
            };
        }
        let Some(rest) = s.strip_prefix("const ") else { continue };
        let Some((name, rest)) = rest.split_once(':') else { continue };
        let name = name.trim();
        let mut chars = name.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_uppercase())
            || !chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        let Some((ty, rest)) = rest.split_once('=') else { continue };
        if !INT_TYPES.contains(&ty.trim()) {
            continue;
        }
        let Some(lit) = rest.trim().strip_suffix(';') else { continue };
        let Some(value) = parse_rust_int(lit) else { continue };
        out.push(RustConst { file: file.to_string(), line: i + 1, name: name.to_string(), value });
    }
    out
}

/// Parse a C `#define` body that is a plain integer: optional parentheses,
/// optional `-`, decimal or `0x` hex, `U`/`L` suffixes, then nothing but a
/// comment.
pub fn parse_c_value(body: &str) -> Option<i64> {
    let mut s = body.trim();
    // Cut a trailing comment.
    if let Some(i) = s.find("/*") {
        s = s[..i].trim_end();
    }
    if let Some(i) = s.find("//") {
        s = s[..i].trim_end();
    }
    if let Some(inner) = s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        s = inner.trim();
    }
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r.trim_start()),
        None => (false, s),
    };
    let s = s.trim_end_matches(['u', 'U', 'l', 'L']);
    let v = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        if h.is_empty() || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        i64::from_str_radix(h, 16).ok()?
    } else if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) {
        s.parse::<i64>().ok()?
    } else {
        return None;
    };
    Some(if neg { -v } else { v })
}

/// Every object-like integer `#define` in one C file (`at` = `relpath:line`).
pub fn parse_c_defines(relpath: &str, text: &str) -> Vec<Define> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let s = line.trim_start();
        let Some(rest) = s.strip_prefix('#') else { continue };
        let Some(rest) = rest.trim_start().strip_prefix("define") else { continue };
        // `#define` must be followed by whitespace, then the name.
        if !rest.starts_with([' ', '\t']) {
            continue;
        }
        let rest = rest.trim_start();
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let (name, body) = rest.split_at(end);
        if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        // Function-like macro (`NAME(`) or no value: skip.
        if !body.starts_with([' ', '\t']) {
            continue;
        }
        if let Some(value) = parse_c_value(body) {
            out.push(Define { name: name.to_string(), value, at: format!("{relpath}:{}", i + 1) });
        }
    }
    out
}

/// Render the snapshot file: a header naming the source, then
/// `name<TAB>value<TAB>file:line`, in file/line order.
pub fn format_snapshot(emutos_commit: &str, defines: &[Define]) -> String {
    let globs: Vec<String> = EMUTOS_GLOBS.iter().map(|(d, s)| format!("{d}/*{s}")).collect();
    let mut s = String::new();
    s.push_str("# EmuTOS integer #define snapshot for `cargo xtask emutos-consts` (DESK43 U5a, guard G3).\n");
    s.push_str("# Generated -- do not edit by hand. Regenerate: cargo xtask emutos-consts --update\n");
    s.push_str(&format!("# source: references/emutos @ {emutos_commit}\n"));
    s.push_str(&format!("# files: {}\n", globs.join(" ")));
    s.push_str("# name<TAB>value<TAB>file:line\n");
    for d in defines {
        s.push_str(&format!("{}\t{}\t{}\n", d.name, d.value, d.at));
    }
    s
}

/// Parse the snapshot file written by [`format_snapshot`].
pub fn parse_snapshot(text: &str) -> Result<Vec<Define>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 3 {
            return Err(format!("snapshot line {}: expected 3 tab-separated fields", i + 1));
        }
        let value = f[1]
            .parse::<i64>()
            .map_err(|e| format!("snapshot line {}: value {:?}: {e}", i + 1, f[1]))?;
        out.push(Define { name: f[0].to_string(), value, at: f[2].to_string() });
    }
    Ok(out)
}

/// The EmuTOS commit recorded in a snapshot header (`# source: … @ <rev>`).
pub fn snapshot_commit(text: &str) -> Option<&str> {
    text.lines()
        .find_map(|l| l.strip_prefix("# source: references/emutos @ "))
        .map(str::trim)
}

/// Parse the deviation list: `name<TAB>value<TAB>kind<TAB>reason`, `#` comments.
pub fn parse_deviations(text: &str) -> Result<Vec<Deviation>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.splitn(4, '\t').collect();
        if f.len() != 4 {
            return Err(format!("deviations line {}: expected 4 tab-separated fields", i + 1));
        }
        let value = parse_rust_int(f[1])
            .ok_or_else(|| format!("deviations line {}: value {:?}", i + 1, f[1]))?;
        if f[2] != "intended" && f[2] != "defect" {
            return Err(format!("deviations line {}: kind must be intended|defect", i + 1));
        }
        out.push(Deviation {
            name: f[0].to_string(),
            value,
            kind: f[2].to_string(),
            reason: f[3].trim().to_string(),
        });
    }
    Ok(out)
}

/// Compare the desktop's constants against EmuTOS' defines.
pub fn compare(consts: &[RustConst], defines: &[Define], deviations: &[Deviation]) -> Report {
    let mut by_name: BTreeMap<&str, Vec<&Define>> = BTreeMap::new();
    for d in defines {
        by_name.entry(d.name.as_str()).or_default().push(d);
    }
    let mut r = Report { total: consts.len(), ..Report::default() };
    let mut used = vec![false; deviations.len()];
    for c in consts {
        let Some(defs) = by_name.get(c.name.as_str()) else {
            r.no_counterpart.push(c.clone());
            continue;
        };
        if defs.iter().any(|d| d.value == c.value) {
            r.matched += 1;
            let mut vals: Vec<i64> = defs.iter().map(|d| d.value).collect();
            vals.sort_unstable();
            vals.dedup();
            if vals.len() > 1 {
                r.ambiguous.push(c.clone());
            }
            continue;
        }
        let dev = deviations
            .iter()
            .position(|d| d.name == c.name && d.value == c.value);
        if let Some(i) = dev {
            used[i] = true;
        }
        r.mismatches.push((
            c.clone(),
            defs.iter().map(|d| (*d).clone()).collect(),
            dev.map(|i| deviations[i].clone()),
        ));
    }
    r.stale = deviations
        .iter()
        .zip(used)
        .filter(|(_, u)| !u)
        .map(|(d, _)| d.clone())
        .collect();
    r
}

/// The desktop source directory: `kernel/src/desk/` while the desktop is a
/// kernel module, else the `src/` of the crate named `desktop` (the U8b end
/// state). Same rule as `tools/desk-drift-probe.sh`; never "nothing".
pub fn desktop_src(root: &Path) -> Result<PathBuf, String> {
    let module = root.join("kernel/src/desk");
    if module.is_dir() {
        return Ok(module);
    }
    let mut found = Vec::new();
    find_cargo_tomls(root, root, 0, &mut found);
    for t in found {
        let text = fs::read_to_string(&t).unwrap_or_default();
        if text.lines().any(|l| {
            let l = l.trim();
            l.starts_with("name") && l.split_once('=').is_some_and(|(_, v)| v.trim() == "\"desktop\"")
        }) {
            return Ok(t.parent().unwrap().join("src"));
        }
    }
    Err("no desktop source: kernel/src/desk/ is gone and no crate is named \"desktop\"".into())
}

fn find_cargo_tomls(root: &Path, dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 3 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if p.is_dir() {
            if matches!(name, "target" | "references" | ".git" | "measurements" | "Applications") {
                continue;
            }
            find_cargo_tomls(root, &p, depth + 1, out);
        } else if name == "Cargo.toml" && p != root.join("Cargo.toml") {
            out.push(p);
        }
    }
}

/// Every `.rs` file below `dir`, sorted.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let rd = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            rust_files(&p, out)?;
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
    Ok(())
}

/// All literal integer constants of the desktop source plus the
/// [`SHARED_SETS`] it imports, `file` relative to `root`.
pub fn desktop_consts(root: &Path) -> Result<Vec<RustConst>, String> {
    let src = desktop_src(root)?;
    let mut files = Vec::new();
    rust_files(&src, &mut files)?;
    if files.is_empty() {
        return Err(format!("{} holds no .rs file", src.display()));
    }
    for s in SHARED_SETS {
        let p = root.join(s);
        if !p.is_file() {
            return Err(format!("{}: shared constant set missing", p.display()));
        }
        files.push(p);
    }
    let mut out = Vec::new();
    for f in files {
        let text = fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let rel = f.strip_prefix(root).unwrap_or(&f).to_string_lossy().into_owned();
        out.extend(parse_rust_consts(&rel, &text));
    }
    Ok(out)
}

/// Load snapshot + deviations + desktop constants and compare.
pub fn check(root: &Path) -> Result<(Report, String), String> {
    let snap = fs::read_to_string(root.join(SNAPSHOT))
        .map_err(|e| format!("{SNAPSHOT}: {e} (regenerate with `cargo xtask emutos-consts --update`)"))?;
    let commit = snapshot_commit(&snap).unwrap_or("?").to_string();
    let defines = parse_snapshot(&snap)?;
    let devs = parse_deviations(
        &fs::read_to_string(root.join(DEVIATIONS)).map_err(|e| format!("{DEVIATIONS}: {e}"))?,
    )?;
    let consts = desktop_consts(root)?;
    Ok((compare(&consts, &defines, &devs), commit))
}

/// Regenerate the snapshot from `references/emutos/` (local checkout).
fn update(root: &Path) -> Result<(), String> {
    let em = root.join("references/emutos");
    if !em.is_dir() {
        return Err(format!(
            "{} is missing — the snapshot can only be regenerated with a local EmuTOS checkout (references/README.md)",
            em.display()
        ));
    }
    let commit = std::process::Command::new("git")
        .arg("-C")
        .arg(&em)
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .ok_or("cannot read the EmuTOS commit (git rev-parse in references/emutos)")?;
    let mut defines = Vec::new();
    for (dir, suffix) in EMUTOS_GLOBS {
        let d = em.join(dir);
        let rd = fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        let mut files: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.to_string_lossy().ends_with(suffix))
            .collect();
        files.sort();
        for f in files {
            let bytes = fs::read(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            let text = String::from_utf8_lossy(&bytes);
            let rel = format!("{dir}/{}", f.file_name().unwrap().to_string_lossy());
            defines.extend(parse_c_defines(&rel, &text));
        }
    }
    let out = root.join(SNAPSHOT);
    fs::write(&out, format_snapshot(&commit, &defines)).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("wrote {SNAPSHOT}: {} defines from references/emutos @ {commit}", defines.len());
    Ok(())
}

/// Print the comparison; `verbose` also lists the constants without counterpart.
pub fn print_report(r: &Report, commit: &str, verbose: bool) {
    println!("NewTOS desktop constants vs EmuTOS #defines (snapshot {SNAPSHOT}, EmuTOS {commit})");
    println!();
    println!(
        "literal integer constants: {}  matched: {}  mismatched: {}  no EmuTOS counterpart: {}  (ambiguous matches: {})",
        r.total,
        r.matched,
        r.mismatches.len(),
        r.no_counterpart.len(),
        r.ambiguous.len()
    );
    if !r.mismatches.is_empty() {
        println!();
        println!("mismatches:");
        for (c, defs, dev) in &r.mismatches {
            let em: Vec<String> = defs.iter().map(|d| format!("{} ({})", d.value, d.at)).collect();
            let tag = match dev {
                Some(d) => format!("{} — {}", d.kind, d.reason),
                None => "UNDOCUMENTED".to_string(),
            };
            println!(
                "  {:<14} ours {:>6} at {}:{}  EmuTOS {}  [{tag}]",
                c.name,
                c.value,
                c.file,
                c.line,
                em.join(", ")
            );
        }
    }
    if !r.stale.is_empty() {
        println!();
        println!("STALE deviation entries (no such mismatch any more — remove them from {DEVIATIONS}):");
        for d in &r.stale {
            println!("  {} = {} [{}]", d.name, d.value, d.kind);
        }
    }
    if verbose {
        println!();
        println!("ambiguous (EmuTOS defines several values; ours matches one):");
        for c in &r.ambiguous {
            println!("  {:<24} {:>6} at {}:{}", c.name, c.value, c.file, c.line);
        }
        println!();
        println!("no EmuTOS counterpart (not checked):");
        for c in &r.no_counterpart {
            println!("  {:<24} {:>6} at {}:{}", c.name, c.value, c.file, c.line);
        }
    }
    println!();
    println!(
        "EMUTOS-CONSTS: total={} matched={} mismatched={} documented={} undocumented={} stale={} no-counterpart={}",
        r.total,
        r.matched,
        r.mismatches.len(),
        r.mismatches.len() - r.undocumented().len(),
        r.undocumented().len(),
        r.stale.len(),
        r.no_counterpart.len()
    );
}

/// `xtask emutos-consts [--update] [-v]`.
pub fn cmd(root: &Path, args: &[String]) -> Result<(), String> {
    let mut verbose = false;
    let mut do_update = false;
    for a in args {
        match a.as_str() {
            "-v" | "--verbose" => verbose = true,
            "--update" => do_update = true,
            other => return Err(format!("emutos-consts: unknown argument {other:?}")),
        }
    }
    if do_update {
        update(root)?;
    }
    let (r, commit) = check(root)?;
    print_report(&r, &commit, verbose);
    if r.ok() {
        Ok(())
    } else {
        Err("emutos-consts: undocumented mismatches or stale deviation entries (see above)".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
    }

    #[test]
    fn rust_const_forms() {
        let src = "\
const A: i16 = 14;
pub const B: u16 = 0x00_10; // trailing comment
    pub(super) const C_D: i32 = -1;
const E: usize = 0b101;
const F: u8 = 0x7fu8;
const lower: i16 = 1;
const G: &str = \"x\";
const H: i16 = A | 2;
// const I: i16 = 3;
const J: i16 = 3; const K: i16 = 4;
";
        let v = parse_rust_consts("f.rs", src);
        let got: Vec<(&str, i64, usize)> = v.iter().map(|c| (c.name.as_str(), c.value, c.line)).collect();
        assert_eq!(
            got,
            vec![("A", 14, 1), ("B", 16, 2), ("C_D", -1, 3), ("E", 5, 4), ("F", 0x7f, 5)]
        );
    }

    #[test]
    fn c_define_forms() {
        let src = "\
#define BLKDEVNUM   26                  /* number of block devices */
#  define NEG (-1)
#define HEX 0x8000UL
#define FN(a) ((a)+1)
#define EXPR (1<<3)
#define ALIAS OTHER
#define EMPTY
#define DEOK            16
";
        let v = parse_c_defines("include/x.h", src);
        let got: Vec<(&str, i64, &str)> = v.iter().map(|d| (d.name.as_str(), d.value, d.at.as_str())).collect();
        assert_eq!(
            got,
            vec![
                ("BLKDEVNUM", 26, "include/x.h:1"),
                ("NEG", -1, "include/x.h:2"),
                ("HEX", 0x8000, "include/x.h:3"),
                ("DEOK", 16, "include/x.h:8"),
            ]
        );
    }

    #[test]
    fn snapshot_round_trip() {
        let defs = vec![
            Define { name: "A".into(), value: -1, at: "aes/a.h:3".into() },
            Define { name: "B".into(), value: 26, at: "include/sysconf.h:43".into() },
        ];
        let text = format_snapshot("1a7484ab", &defs);
        assert_eq!(snapshot_commit(&text), Some("1a7484ab"));
        assert_eq!(parse_snapshot(&text).unwrap(), defs);
    }

    #[test]
    fn compare_classifies_and_flags_stale_entries() {
        let c = |n: &str, v: i64| RustConst { file: "f.rs".into(), line: 1, name: n.into(), value: v };
        let d = |n: &str, v: i64| Define { name: n.into(), value: v, at: "x.h:1".into() };
        let consts = vec![c("SAME", 1), c("DIFF", 2), c("UNDOC", 3), c("NEWTOS_ONLY", 4), c("MULTI", 5)];
        let defines = vec![d("SAME", 1), d("DIFF", 9), d("UNDOC", 7), d("MULTI", 5), d("MULTI", 6)];
        let devs = vec![
            Deviation { name: "DIFF".into(), value: 2, kind: "intended".into(), reason: "r".into() },
            Deviation { name: "GONE".into(), value: 1, kind: "defect".into(), reason: "r".into() },
        ];
        let r = compare(&consts, &defines, &devs);
        assert_eq!(r.total, 5);
        assert_eq!(r.matched, 2);
        assert_eq!(r.mismatches.len(), 2);
        assert_eq!(r.no_counterpart.len(), 1);
        assert_eq!(r.ambiguous.len(), 1);
        assert_eq!(r.undocumented().iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["UNDOC"]);
        assert_eq!(r.stale.len(), 1);
        assert_eq!(r.stale[0].name, "GONE");
        assert!(!r.ok());
    }

    /// The guard itself: every desktop constant that EmuTOS also defines has
    /// EmuTOS' value, or its difference is documented in [`DEVIATIONS`] — and
    /// no documented difference has silently disappeared.
    #[test]
    fn desk_consts_match_emutos_snapshot() {
        let (r, commit) = check(&root()).expect("emutos-consts check could not run");
        assert!(r.total > 0, "no desktop constants found — the scan is empty");
        assert!(
            r.ok(),
            "desktop constants vs EmuTOS {commit}: undocumented mismatches {:?}, stale deviation entries {:?} \
             (run `cargo xtask emutos-consts` for the table)",
            r.undocumented().iter().map(|c| format!("{}={} at {}:{}", c.name, c.value, c.file, c.line)).collect::<Vec<_>>(),
            r.stale.iter().map(|d| format!("{}={}", d.name, d.value)).collect::<Vec<_>>()
        );
    }
}
