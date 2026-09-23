# DESK27 — `docs/api/xbios/26-supexec.md` widerspricht sich

Status: **OFFEN** · Prioritaet: **P4** · Typ: Dokumentationsfehler — Nebenbefund DESK13

Abhaengigkeiten: —

## Sachverhalt

In `docs/api/xbios/26-supexec.md` sagt der Kopf richtig, der Dispatcher gebe
die Funktionsnummer zurueck; der Abschnitt *Pi / BCM2711 binding* behauptet
weiter `EINVFN`. Eine der beiden Aussagen ist falsch, und die Spezifikation
ist damit als Beleg unbrauchbar.

DESK13 hat die Stelle nur gelesen (um zu entscheiden, dass der `Default`-Knopf
den Keks ueber `Ssystem` liest statt ueber `Supexec`) und bewusst nicht
angefasst.

## Arbeit

Gegen `tos-api/src/xbios/mod.rs` pruefen, welches der beiden Verhalten der
Dispatcher wirklich zeigt, und die andere Stelle richtigstellen.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
