# DESK58 — STATUS: DONE (2026-10-16) — behoben, in QEMU belegt; nicht committet

**Auftrag:** `BLKDEVNUM` im Desktop ist 16 (Zitat `bdosdefs.h`), EmuTOS hat 26 (`include/sysconf.h:43`, A: … Z:) (`TASK/DESK58.md`).
Die Fundstelle steht inzwischen in `programs/desktop/src/main.rs` (`valid_drive`), nicht mehr in `kernel/src/desk/main.rs`.

## Verwendungen

| EmuTOS | NewTOS-Desktop | Grenze vorher |
|---|---|---|
| `deskapp.c:715` Standard-Desktop: Laufwerkssymbole aus der Laufwerksmaske | `inf.rs` `build_desk_icons`, `for drive in 0..26u32` | 26 (Literal) |
| `deskins.c:313/328` `ins_devices` (Symbole entfernen/installieren) | `ins.rs` `const BLKDEVNUM: u32 = 26` | 26 |
| `desksupp.c:1818` `valid_drive` | `main.rs` `valid_drive`, `const BLKDEVNUM: u32 = 16` | **16 — der Defekt** |

`check_alt_letter_key` filtert vorher auf `'A'..'Z'` (`deskmain.c:1147-1152`) und prueft die Maske inline — keine Grenze betroffen.
Rufer von `valid_drive`: `do_dopen` (`desksupp.c:1041`) und die Laufwerks-Info (`inf.rs`, `if (!valid_drive(dr_id)) return 1;`).
Die Laufwerksmaske (Antwort von `Dsetdrv`) liest der Desktop als `Long`; Bits 16–25 kaemen in allen drei Pfaden an.

## Fix

`valid_drive` ruft jetzt `drive_in_map(drive, dos_sdrv(dos_gdrv()))`; `drive_in_map` ist genau `desksupp.c:1818-1819`
(`drv < BLKDEVNUM && map & (1 << drv)`) mit `BLKDEVNUM = 26` und dem Zitat `include/sysconf.h:43`. Alert-Verhalten unveraendert.
Eintrag `BLKDEVNUM 16 defect` aus `tools/xtask/emutos-consts-deviations.txt` entfernt (Hosttest gruen, `stale=0`).

## Sichtbare Wirkung

**Keine — erwartet und gemessen.** NewTOS meldet keine Laufwerke ueber P:: `tos-api/src/bios/drive.rs` `MAX_DRIVES = 16` begrenzt
Partitionstabelle und `drvbits()` (= `Drvmap`, `Dsetdrv`-Antwort, `_drvbits` `$4C2`, Dateiauswahl). Fuer Q:–Z: gibt `valid_drive`
vorher (Bereich) wie nachher (Maskenbit 0) `FALSE` mit demselben Alert. 8 Ansichten + Serie (Desktop, APITEST, TeraDesk, QED),
EL0-Standarddesktop, on/off/strict: **45/45 Dumps byte-gleich** zu `measurement-2026-10-16-desk59`; APITEST 106/0/5, 0 EXC/PANIC.

## Test rot → gruen

Desktop-Crate `no_std`, `test = false` → Desktop-Selbsttest `main::selftest()` (Build mit `selftest-desk`, QEMU), von
`selftest::run_once` nach `act::selftest` gerufen; rein (synthetische Masken, kein Alert, keine Aenderung auf `C:`), weil die
echte Maske Bits ≥ 16 nie traegt. Acht Faelle: C: gesetzt/leer, P: (15), Q: (16), Z: (25), Z: leer, `[` (26) und `@` (−1) mit Maske −1.

| Kernel | Ergebnis |
|---|---|
| `kernel8-kt-red.img` (Test + Aufteilung, `BLKDEVNUM` noch 16) | `[desk-drv] 6 passed, 2 failed` — FAIL genau Q: und Z: |
| `kernel8-kt.img` (mit Fix) | `[desk-drv] 8 passed, 0 failed`; gesamt 64 PASS / 1 FAIL (= DESK59 56/1 + 8), uebrige Zeilen identisch |

## Guards (docs/api/desktop/_rebuild-plan.md §12)

```
  DRIFT-PROBE TOTAL: 4 (before: 4)   DRIFT-PROBE UNSAFE: 20 (baseline 20)
  EMUTOS-CONSTS: total=466 matched=333 mismatched=1 documented=1 undocumented=0 stale=0 no-counterpart=132   (vorher 332/2/2)
  DESK-CITE: fns=479 cited=318 newtos=5 uncited=156            (whole desktop; vorher 477/316/5/156)
  DESK-CITE --since 77363d0f: main.rs valid_drive, drive_in_map, selftest cited; selftest.rs run_once (bestehender NEWTOS-Marker);
                              0 uncited, keine neuen NEWTOS-fns
  Oracle: not required: the desktop's svc calls are unchanged (valid_drive still makes the same Dgetdrv/Dsetdrv pair)
```

Snapshot `tools/xtask/emutos-defines.snapshot` nicht neu erzeugt.

## Zahlen

| | Soll | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1370/0/1 | 1370/0/1 |
| Warnungen rpi4 / qemu-virt | 64/94 | 64/94 beide |
| Clippy Kernel rpi4 / qemu-virt / +selftest-desk | 76/125 | 76/125 alle |
| Programme (build + clippy) | 0 | 0 |
| Drift / unsafe | 4 / 20 | 4 / 20 |
| emutos-consts | gruen ohne Eintrag | gruen, Eintrag entfernt |
| SDK | 20/0/0 | 20/0/0 |

Beleg: `measurements/measurement-2026-10-16-desk58/RUNINFO.TXT`.

## Offen

* Echte Pi 4 (keine HW-spezifische Wirkung erwartet: reine Bereichslogik). `desk=kernel`, qemu-virt-Boot, on-asid/verify nicht gefahren.

## Ausserhalb des Auftrags (je eine Zeile, nicht angefasst)

* Kernelseite: `tos-api/src/bdos/fs/drive.rs` `BLKDEVNUM = 16` begrenzt `Dsetdrv` auf 0..15, Spec `docs/api/gemdos/drive/0e-dsetdrv.md` sagt 26 — eigener Task noetig.
* `tos-api/src/bios/drive.rs` `MAX_DRIVES = 16`: Laufwerke Q:–Z: koennen gar nicht entstehen (Grund, warum DESK58 unsichtbar bleibt).
* `docs/api/bios/07-getbpb.md:89,228` und `09-mediach.md:84` nennen noch `0=A:..15=P:` bzw. `BLKDEVNUM = 16`.
* `inf.rs` `build_desk_icons` nutzt das Literal `0..26u32` statt einer benannten Konstante; Desktop hat jetzt zwei `BLKDEVNUM`-Konstanten (`ins.rs`, `main.rs`), beide 26.
* `tools/inject-arm.py` T16 weiterhin offen (korrigierte Kopie im Messordner verwendet).

## Selbstpruefung

Aufgabe in einem Satz: der Desktop prueft Laufwerksbuchstaben gegen `BLKDEVNUM = 26` wie `desksupp.c:1818` / `sysconf.h:43`,
mit Rot-Gruen-Test und entferntem Abweichungseintrag. Geaendert: `programs/desktop/src/main.rs`, `programs/desktop/src/selftest.rs`,
`tools/xtask/emutos-consts-deviations.txt`; diese Quittung, `TASK/DESK58.md` (Abschluss), das Messverzeichnis. Beide Boards bauen
(64/94, Clippy 76/125). Nicht committet; `docs/STATUS.md` nicht angefasst.
