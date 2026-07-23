# BIOS-Audit — Dashboard

> Stand: 2026-07-23, Branch `desk-rebuild`. NewTOS-Ist vs. EmuTOS-Spec +
> COMPATIBILITY-Contract. Scope: BIOS Trap #13 (12 Fn) + Boot-Init/Sysvar/
> OSHEADER/Cookie-Jar. XBIOS separat (BB4). HW-Backend-Adaption (EMMC statt
> WD1772/ACSI, USB-HID statt IKBD-ACIA, kein MFP/Shifter) = dokumentierte
> Deviation, KEIN Bug. Vollständiger Katalog: `bios-findings.md`.

## Severity-Übersicht

| Severity | # | Findings |
|---|---|---|
| **BLOCKER** | 1 | B1 Boot-Init-Gap (Low-Memory-Sysvars `$400`–`$5FF` + OSHEADER `$4F2` + Cookie-Jar `$5A0` fehlen für native EL0-Apps) — **Fix OFFEN** (Machbarkeit/Design-Entscheidung nach Audit) |
| **HIGH** | 1 | H1 etv_*-Hooks per Setexc gespeichert, aber nie dispatcht (install → feuert nie); Boot-Default-Sentinels nicht callable |
| **MEDIUM** | 4 | M1 Rwabs ohne etv_critic-Pfad · M2 Getbpb `reserved==0` nicht auf 1 normalisiert · M3 Getbpb FAT16-`>65524`-Reject fehlt + u16-Truncation · M4 Getbpb fehlende Validitäts-Guards (FAT-Count/recsiz/clsizb) |
| **LOW** | 3 | L1 Rwabs physical-mode (RW_NOTRANSLATE) · L2 Bconin conterm-Bit3-Fold · L3 fatrec/datrec bei num_fats≥3 |
| **SPEC/DOC** | 3 | S1 `09-mediach.md` „Pi binding" stale (Code bereits korrekt) · S2 COMPATIBILITY.md §sysvars + ADR-012 durch ADR-019/TeraDesk widerlegt · S3 os_version=0x0404 unverifiziert |

## Kernbild

- **6 echte Verhaltens-Divergenzen** (B1, H1, M1–M4) + 3 LOW + 3 Spec/Doc-Korrekturen.
- **Sauber/korrekt (kein Finding):** Tickcal, Kbshift, Getmpb, Dispatcher-Out-of-Range; Bconstat/Bconin/Bconout/Bcostat/Mediach/Drvmap sind kontrakt-korrekt (ihre Abweichungen = reine HW-Absenz).
- **HW-abstrakt vs. adaptiert:** 9/12 Fn gemischt (ABI abstrakt, Backend RPi4-adaptiert), 3 nahe rein-abstrakt (Getmpb/Setexc/Tickcal). Alle „Acceptable HW-absence"-Punkte = bewusst, kein Bug.
- **Top-Blocker B1:** der Boot-Init-Gap — Fix bewusst offen; der Lösungsweg (ADR-012/„Zwei-Welten" umkehren + Low-Memory je Prozess in EL0-VA mappen + OSHEADER + `$5A0` **vs.** App-Patch) ist die nachgelagerte Lead-Machbarkeits-/Design-Entscheidung.
- **M2–M4 (FS-Härtung):** aus der Quelle bewiesen (PLAUSIBLE), im Normalbetrieb (reale 500-MB-FAT16, reserved=1, <65524 Cluster) nicht getriggert; nur bei malformierten/oversized Images. Gezielter fsck_msdos-/Selftest empfohlen, falls FS-Härtung priorisiert.

## Reihenfolge

BB1 (Boot-Init B1 + H1 + M1 etv_critic — nach der Machbarkeits-Entscheidung) → BB2 (Memory-Modell, STRAM/TTRAM) → BB3 (M2–M4 FS-Härtung + L1–L3) → BB4 (XBIOS). Spec/Doc-Korrekturen S1–S3 begleitend.
