# FS06 — Abschluss

Status: **ERLEDIGT** 2026-10-17 — CLOSE — Praemisse widerlegt.

EmuTOS legt beim Umbenennen im selben Verzeichnis auf einen leeren Namen ebenfalls einen Eintrag an (`bdos/fsdir.c:1076-1083`); `fsdir.c:912-913` ist die Existenzpruefung, keine Namenspruefung. Rest (Umbenennen ueber Verzeichnisgrenzen, EmuTOS `EPTHNF` ueber `ixcreat`, `fsopnclo.c:120-121`) → [FS14](FS14.md).

Beleg: [docs/analysis/open-tickets-review-2026-10-17.md](../docs/analysis/open-tickets-review-2026-10-17.md) (Abschnitt FS06, Commit `1d17eae8`).
