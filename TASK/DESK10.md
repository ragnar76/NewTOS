# DESK10 — `desk/` ruft GEMDOS/BIOS/XBIOS als Trap statt als Rust-Funktion

Status: **OFFEN** · Prioritaet: **P3** · Typ: Strukturluecke desk/ (G6 der Lueckenliste)

Abhaengigkeiten: —

## Sachverhalt

`docs/analysis/desk-aes-gaps-2026-09-14.md` § G6: `desk/` ruft GEMDOS
direkt als Rust-Funktion (`dir.rs` 9 Funktionen, `model.rs`
`Fsfirst`/`Fsnext`/`Fsetdta`), nicht ueber den Trap. Damit laeuft der
Desktop an der ABI vorbei, die jede Anwendung benutzt — Fehler in der
Trap-Schicht bleiben unsichtbar (Memory `desktop-bypasses-app-abi-path`).

## Arbeit

Aufrufstellen auf den Trap umstellen (Muster: die AES-Aufrufe in
`desk/mod.rs` gehen bereits ueber `svc`), Verhalten unveraendert;
Regressionsbeleg per Screendump und `xtask test`.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
