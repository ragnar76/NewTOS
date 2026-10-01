# A32 — Abnahme der Rechtecklisten-Reparatur auf echter Hardware

**Zustand: UNFINISHED** · 2026-09-12 · **drei von vier** Abnahmepunkten belegt

UNFINISHED heißt hier: der inhaltliche Kern ist auf echter Hardware abgenommen,
es fehlt ein Gegenmesslauf und die daran hängende Folgeentscheidung.

## Abnahmepunkte aus [A32.md](A32.md)

| # | Punkt | Zustand |
|---|---|---|
| 1 | Frischer Kernel mit FIX-01…06 auf der Karte, Abbild-Hash notiert | **erledigt** — Release `39b293a4c068…`, 783216 B, auf `/Volumes/BOOT` kopiert und per Hash gegengeprüft |
| 2 | Die drei Punkte aus „Was noch fehlt" geprüft und belegt | **erledigt für Punkt 1 und 2**, siehe unten |
| 3 | Pi-Lauf gegen `aes-r2b-rlist.tsv`: dieselben Fälle wie in QEMU | **erledigt** — `measurements/measurement-2026-09-11-pi4/run1-rlist-fixed/`, gefahren am 2026-09-11 |
| 4 | Erst danach `DROP_SHADOW_SIZE` 2 → 4 ([A23](A23.md)) | **offen**, hängt an 3 |

## Was auf echter Hardware belegt ist

**Zwei überlappende Fenster und der interaktive Weg** (Punkte 1 und 2 der Liste
„Was noch fehlt — und nur auf der Pi geht"). Vom Anwender am **2026-09-11** auf
der RPi 4 gefahren, 1920×1080: Fenster überlappen lassen, Fokus zwischen ihnen
wechseln, sie übereinander verschieben — **alles ohne Fehler**, keine
stehengebliebenen Bänder, kein Überzeichnen über den Rahmen. Das ist der
D2/R2-Residue-Fall, das Leitsymptom des ganzen Durchgangs.

Protokolliert in [docs/HW-TEST-LOG.md](../docs/HW-TEST-LOG.md) und als
Checklistenpunkt D2 in
[docs/api/desktop/_hw-test-checklist.md](../docs/api/desktop/_hw-test-checklist.md).

**Erratum zur Protokollführung:** der Test lief am 11.09., wurde damals aber
nicht festgehalten. Die leere Checkbox in der Checkliste hat ihn danach
fälschlich als offen geführt, und dieser Bericht hat das zunächst übernommen.
Der Anwender hat es richtiggestellt. Ein nicht protokollierter Test ist für
jeden späteren Leser ein nicht durchgeführter Test.

**Nicht ausdrücklich bestätigt** ist Punkt 3 der Liste („die bestehenden
Screendumps müssen unverändert sein"). Stufe 6 ändert kein Pixel, nur das
Rechteck in der Nachricht; ein Pixelvergleich gegen `measurements/measurement-2026-09-10-qemu/`
wäre der saubere Beleg und steht aus.

## Abnahmepunkt 3 — erledigt, war nur falsch gelesen

`measurements/measurement-2026-09-11-pi4/run1-rlist-fixed/` enthält den Lauf samt fertigem
Diff (`DIFF-gegen-TT-und-QEMU.txt`), gefahren am **2026-09-11** auf der echten
Pi 4 (1920×1080, 16 Planes, Kernel `3c6d7a4a…` mit FIX-01…06 + A16,
`DROP_SHADOW_SIZE = 2`):

* **Gegen die TT-Referenz:** sechs Fälle bleiben — vier P1-Stapelfälle ohne Soll
  (127, 133, 144, 204) und zwei Geräteparameter (201, 208). Vor der Reparatur
  waren es zehn. Die vier Zerlegungsfälle **166/174/183/213 sind bitgleich zum
  TT**.
* **Gegen den QEMU-Lauf desselben Kernels:** identisch bis auf 201/208, also
  genau die auflösungsabhängigen. Damit ist belegt, dass die QEMU-Messung die
  echte Hardware trifft.

Full HD in QEMU liegt zusätzlich als
`measurements/measurement-2026-09-10-qemu/run2-qemu-fhd-rlist-fixed/` vor.

**Erratum, zweites an einem Tag:** dieses Ticket hat den Punkt zunächst als
offen geführt, weil der Lauf nicht in `A32.md` selbst vermerkt war. Er war
archiviert und diffed, nur nicht am erwarteten Ort notiert. Dieselbe Lehre wie
oben, in die andere Richtung: **erst die Artefakte durchsuchen, dann einen Lauf
verlangen.**

## Was noch fehlt

**Nur noch Abnahmepunkt 4: `DROP_SHADOW_SIZE` von 2 auf 4.**

Das ist kein Hochzählen einer Konstante. [A23](A23.md) hält fest, dass in der
Referenz die **Beziehung** `s = DROP_SHADOW_SIZE = 2 * th(W_BOX) = 2` gilt und
nicht die Zahl 2 — `th = 1` stammt aus `ob_spec 0x00011101`
(`aes/gemwmlib.c:84-86`). Beim Wechsel auf 4 muss dieselbe Gleichheit für das
Pi-Profil hergestellt werden, sonst entsteht genau der von A23 gesuchte Fehler
in klein: eine Fläche, die als belegt gemeldet, aber nicht bemalt wird.

Unser Code hält die Invariante bereits fest (`tos-api/src/aes/window.rs:283-295`:
`SHADOW_TH = DROP_SHADOW_SIZE / 2` plus Compile-Assert auf Geradzahligkeit), die
Rahmenstärke des `W_BOX` muss aber mitgezogen werden. **Offene Frage an den
Anwender**, bevor das jemand anfasst: soll die Fensterrahmenstärke auf dem
Pi-Profil auf 2 gehen (dann folgt `DROP_SHADOW_SIZE = 4` aus der Beziehung), oder
bleibt der Rahmen bei 1 und die 4 wäre eine bewusste Abweichung von der
EmuTOS-Identität?

## Nebenbefund aus demselben Lauf

Die R3-Messung auf echter Hardware bestätigt erneut, dass die Emulation die
Hardware trifft: **38 von 39 Fällen bitgleich** zwischen Pi (1920×1080) und
QEMU `raspi4b` (640×480). Der eine Unterschied ist Fall 603, `int_out[3]/[4]` =
Bildschirmgeometrie. Kein Defekt.
