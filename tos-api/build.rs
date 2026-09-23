// build.rs — stamps the build date and revision into the crate.
//
// This is NewTOS's counterpart of EmuTOS's `tools/mkheader.awk`, which
// generates `obj/header.h` with `OS_DATE`, `OS_DOSDATE` and `COPYRIGHT_YEAR`
// (see `references/emutos/obj/header.h`). The OS header's build date is not a
// cosmetic string: `os_date`/`os_dosdate` are part of the OSHEADER that
// programs read, so the value has to come from the build, not from a literal.
//
// The stamp lives in this crate, not in `kernel`, because both need it and a
// build script's environment reaches only its own crate — `kernel` reads it
// back through `tos_api::build::BUILD_DATE`.

use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo = manifest_dir.parent().unwrap_or(&manifest_dir).to_path_buf();

    // A new commit moves `.git/HEAD` or the ref it points at, which is what
    // keeps the stamp from going stale between rebuilds.
    for p in ["HEAD", "refs/heads"] {
        let git_path = repo.join(".git").join(p);
        if git_path.exists() {
            println!("cargo:rerun-if-changed={}", git_path.display());
        }
    }

    // `SOURCE_DATE_EPOCH` wins over the wall clock, so a reproducible build
    // stays reproducible; without it the stamp is when this script last ran.
    let secs = match env::var("SOURCE_DATE_EPOCH").ok().and_then(|v| v.trim().parse::<i64>().ok()) {
        Some(v) => v,
        None => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    };
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    println!("cargo:rustc-env=NEWTOS_BUILD_DATE={y:04}-{m:02}-{d:02}");

    let rev = Command::new("git")
        .args(["-C", repo.to_str().unwrap_or("."), "rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=NEWTOS_BUILD_REV={rev}");
}

/// Days since 1970-01-01 to a civil date — Howard Hinnant's `civil_from_days`
/// (proleptic Gregorian, exact for every date this project will ever stamp).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
