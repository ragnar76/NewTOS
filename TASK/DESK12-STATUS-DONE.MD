# DESK12 — STATUS: DONE (2026-09-23)

## Auftrag

Eine Baukonstante erzeugen (Datum + kurzer Git-Hash, `SOURCE_DATE_EPOCH`
beruecksichtigen), aus derselben Quelle wie Version und Hinweis lesen, danach
die Zeile im About-Kasten und ggf. im Startschirm.

## Was die Quelle daran geaendert hat

EmuTOS' About-Kasten zeigt **kein** Baudatum. `ADDINFO` traegt `DECOPYRT`, ein
*Jahr* aus `COPYRIGHT_YEAR` (`desk/deskmain.c:1892`). Das Baudatum geht allein
in den OS-Kopf: `tools/mkheader.awk` erzeugt `obj/header.h` mit
`#define OS_DATE 0x08292026` und `#define OS_DOSDATE 23837`; der Startschirm
druckt nur `EmuTOS Version %s` (`bios/initinfo.c:321`).

Die echte Luecke war damit `os_date`/`os_dosdate` — beide standen als
1980-01-01-Platzhalter in `tos-api/src/bdos/process/loader.rs` und sind Teil
des OSHEADER, den Programme lesen.

## Aenderungen

| Datei | Inhalt |
|---|---|
| `tos-api/build.rs` (neu) | `NEWTOS_BUILD_DATE`/`NEWTOS_BUILD_REV`; `SOURCE_DATE_EPOCH` vor der Wanduhr; Neuablauf ueber `.git/HEAD` und `refs/heads`; Hinnants `civil_from_days` |
| `tos-api/src/build.rs` (neu) | `BUILD_DATE`/`BUILD_REV`/`BUILD_STAMP`/`BUILD_YEAR`/`BUILD_MONTH`/`BUILD_DAY`, `OS_DATE`, `OS_DOSDATE`; zwei Hosttests |
| `tos-api/src/lib.rs` | `pub mod build;` |
| `tos-api/src/bdos/process/loader.rs` | `OS_DATE`/`OS_DOSDATE` aus `crate::build` statt Platzhalter |
| `kernel/src/initinfo.rs` | `BUILD_STAMP` von `tos_api::build`; Zeile `Built` im Startschirm |
| `kernel/src/desk/inf.rs` | Nur Kommentar: der About-Kasten bleibt ohne Datumszeile, jetzt mit dem Grund aus der Quelle statt „es gibt keine Konstante" |

Die Konstante liegt in `tos-api`, weil beide Kisten sie brauchen und die
Umgebung eines Bauskripts nur die eigene Kiste erreicht.

## Pruefung

* Hostsuite **934 passed / 0 failed / 1 ignored** (Basis 932/0/1 + zwei neue
  Tests in `tos_api::build`).
* Beide Bretter frisch gebaut (`cargo clean -p kernel -p tos-api
  --target aarch64-unknown-none-softfloat` vor jedem): **tos-api 64 /
  kernel 94**, Delta 0. Clippy 80/125, Delta 0.
* QEMU raspi4b, Startschirm-Screendump: Zeile
  `Built     : 2026-09-23 (786cd80a)` (`measurements/measurement-2026-09-23-desk12/`).
* Reproduzierbarkeit: `SOURCE_DATE_EPOCH=1700000000 cargo build -p tos-api`
  gibt `cargo:rustc-env=NEWTOS_BUILD_DATE=2023-11-14` aus.

## Offen

Keine Hardwareabnahme noetig — die Zeile ist Text auf dem Startschirm, den die
Pi genauso rendert; sie faellt bei der naechsten HW-Runde ohnehin ins Auge.
