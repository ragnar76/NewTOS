# DESK52 — `do_fopen` ohne Laufwerksprobe und ohne `STDEEPPA`

Status: **OFFEN** · Prioritaet: **P3** · Typ: Fehlende EmuTOS-Funktion — Nebenfund aus [DESK50](DESK50.md)

Abhaengigkeiten: [DESK50](DESK50.md) (`do_wopen`/`do_diropen`, `w_obid`)

## Sachverhalt

EmuTOS prueft in `do_fopen` das Laufwerk ueber `set_default_path` und schliesst bei Fehler mit `fun_close`; ein zu tiefer Pfad gibt den Alarm `STDEEPPA` (`references/emutos/desk/desksupp.c:1097-1108`). Beides fehlt bei uns.

Der Befund stammt aus der DESK50-Arbeit ([Statusdatei](DESK50-STATUS-DONE.MD),
Abschnitt *Offen*) und ist **nicht nachgeprueft**. Erster Schritt: die
EmuTOS-Stelle lesen und unseren Code dagegenhalten, bevor etwas geaendert wird.

## Arbeit

1. EmuTOS-Stelle lesen, Spec in `docs/api/desktop/` pruefen bzw. ergaenzen.
2. Abweichung belegen (Quelle + QEMU-Lauf) oder als konform schliessen.
3. 1:1 angleichen (ADR-022), ohne Umgehung.

Abnahme: Verhalten wie EmuTOS, Beleg unter `measurements/`, Basislinien gehalten.
