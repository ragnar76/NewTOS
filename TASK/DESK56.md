# DESK56 — Zwei Wurfpfade in `dir.rs` verwerfen die Rueckgabe von `do_aopen_drop`

Status: **ERLEDIGT** 2026-10-17 ([Quittung](DESK56-STATUS-DONE.MD)) · Prioritaet: **P3** · Typ: Abweichung von EmuTOS — Nebenfund aus [DESK50](DESK50.md)

Abhaengigkeiten: [DESK50](DESK50.md) (`do_wopen`/`do_diropen`, `w_obid`)

## Sachverhalt

In EmuTOS setzt der Rueckgabewert von `do_aopen` beim Ablegen auf ein Programmsymbol `exit_desktop` (`references/emutos/desk/deskfun.c:1034`, `:1249`). Zwei Pfade in `kernel/src/desk/dir.rs` verwerfen ihn. Wirkung pruefen: verlaesst der Desktop nach einem Drop-Start korrekt seine Schleife?

Der Befund stammt aus der DESK50-Arbeit ([Statusdatei](DESK50-STATUS-DONE.MD),
Abschnitt *Offen*) und ist **nicht nachgeprueft**. Erster Schritt: die
EmuTOS-Stelle lesen und unseren Code dagegenhalten, bevor etwas geaendert wird.

## Arbeit

1. EmuTOS-Stelle lesen, Spec in `docs/api/desktop/` pruefen bzw. ergaenzen.
2. Abweichung belegen (Quelle + QEMU-Lauf) oder als konform schliessen.
3. 1:1 angleichen (ADR-022), ohne Umgehung.

Abnahme: Verhalten wie EmuTOS, Beleg unter `measurements/`, Basislinien gehalten.
