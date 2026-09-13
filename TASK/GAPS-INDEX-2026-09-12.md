# Index — alle offenen Luecken nach dem Durchgang vom 2026-09-12

Ein Tag, sechs Reparaturen, vier Audits. Was dabei **gefunden und nicht
repariert** wurde, steht hier, damit es keinem Kontextfenster zum Opfer faellt.

## Quellberichte

| Bericht | Inhalt |
|---|---|
| [docs/analysis/audit-applinit-rsrc-load-2026-09-11.md](../docs/analysis/audit-applinit-rsrc-load-2026-09-11.md) | `appl_init` + Resource-Ladepfad, Befunde B1–B11 |
| [docs/analysis/aes-r3-wind-state-einordnung-2026-09-12.md](../docs/analysis/aes-r3-wind-state-einordnung-2026-09-12.md) | die 12 R3-Abweichungen, Befunde A/B/C |
| [docs/api/aes/_full-function-inventory.md](../docs/api/aes/_full-function-inventory.md) | Unterfunktionsinventar, 176 Werte, Befunde A-1…A-16 |
| [docs/analysis/full-regression-2026-09-12.md](../docs/analysis/full-regression-2026-09-12.md) | Gegenmessung aller neun Falltabellen |
| [docs/analysis/vdi-r2-improvement-traced-2026-09-12.md](../docs/analysis/vdi-r2-improvement-traced-2026-09-12.md) | die unerklaerte `vdi-r2`-Verbesserung, Befunde F-1…F-3 |

## Erledigt an diesem Tag

B1 (`rsrc_load`-Pfadaufloesung) · B2 (Rasterausrichtung, Werkzeug + Kernel) ·
[A16](A16-STATUS-DONE.MD) · [A22](A22-STATUS-DONE.MD) ·
[A24](A24-STATUS-DONE.MD) · [A33](A33-STATUS-DONE.MD) ·
[A34](A34-STATUS-DONE.MD) · ILP32-Ladesperre · FIX-05/P2.
Dazu [A32](A32-STATUS-UNFINISHED.MD) auf 3 von 4 Abnahmepunkten.

## Offen — neue Tickets

### Speicherfehler und Regel-2-Verstoesse zuerst

| Ticket | Luecke | Prio |
|---|---|---|
| [A37](A37.md) | `te_txtlen`-Ueberlauf — Kernel repariert, **Werkzeugentscheidung offen** | P1 |
| [A38](A38.md) | zu kurz gelesenes Blob wird ueber den Puffer hinaus relokiert (SIGBUS) | P1 |
| [A45](A45.md) | `appl_tplay`/`appl_trecord` sind **Stubs** — Regel-2-Verstoss | P1 |

### Belegte Defekte

| Ticket | Luecke | Prio |
|---|---|---|
| [V16](V16.md) | GEM-Trap gibt bei unbekanntem Selektor `0` — **`vq_gdos()` luegt** | P1 |
| [A35](A35.md) | `rsrc_load` fuellt `ap_ptree`/`ap_rscmem`/`ap_rsclen` nie | P1 |
| [A36](A36.md) | nur *referenzierte* Substrukturen relokiert | P1 |
| [A42](A42.md) | `wind_get(WF_NAME/WF_INFO)` antwortet, wo EmuTOS abweist | P1 |
| [A28](A28.md) | keine Zustandsruecksetzung an der Startgrenze (= B8) | P1 |
| [A29](A29.md) | Programmende loescht den Deskriptor des Desktops (= B7) | P1 |
| [A43](A43.md) | `menu_settings` mit ungueltigem `flag` schreibt | P2 |
| [A44](A44.md) | `graf_mouse` mit ungueltigem Modus setzt den Cursor nicht | P2 |
| [A46](A46.md) | `objc_draw` zeichnet keine nicelines | P2 |
| [A47](A47.md) | `objc_order` haengt ein Alleinkind falsch aus | P2 |
| [A51](A51.md) | `save_windows` umgeht `wm_delete` | P2 |
| [A21](A21.md) | `RCAP` = 16 verwirft still — **neu dringlicher**, weil die Reihenfolge die Stueckzahl aendert (A22) | P2 |

### Abweichungen, die eine Entscheidung brauchen

| Ticket | Frage |
|---|---|
| [A50](A50.md) | `rsrc_gaddr`/`saddr`-Raender: unsere Fassung ist die robustere — angleichen hiesse eine Indexpruefung entfernen |
| [A39](A39.md) | Resourcezustand als Kernel-Global statt am `AESGLOBAL` — gross, erst planen |
| [A32](A32.md) Punkt 4 | `DROP_SHADOW_SIZE` 2 → 4 nur zusammen mit Rahmenstaerke 2 (`s = 2·th`) |

### Funktionsluecken

| Ticket | Luecke |
|---|---|
| [A41](A41.md) | CICON-Resourcen vom Werkzeug abgewiesen, obwohl das Profil sie kann |
| [A48](A48.md) | `shel_write(SHW_RESCHNG)` wechselt die Aufloesung nicht |
| [A49](A49.md) | `kind`-Bit `HOTCLOSE` fehlt |
| [A40](A40.md) | `ob_fix` im Werkzeug festverdrahtet statt zur Ladezeit |
| [A52](A52.md) | `bump_z` ohne Ueberlaufschutz |

### Dokumentation

| Ticket | Inhalt |
|---|---|
| [A53](A53.md) | fuenf nachweislich falsche Stellen, das zweite Versionsgatter `4b0d9518`, und vier **ungeprueft** erteilte Spec-Freiheiten |

## Offene Hardwareabnahmen

* [A32](A32-STATUS-UNFINISHED.MD) Punkt 4 (haengt an der Rahmenstaerke-Frage)
* [A25](A25-STATUS-UNFINISHED.MD) — `appl_init`-Wiederaufruf
* [A37](A37.md) — Tippen in ein Dateiselektor-Feld und einen TeraDesk-Dialog
* [A44](A44.md), [A46](A46.md), [A49](A49.md) — nur sichtbar bzw. nur per Klick

## Unentschieden gelassen, ausdruecklich

* **`scrp_clear`** sieht nach einem **EmuTOS-Fehler** aus: der Loeschpfad wird
  als `<dir>` + `'S'` + Dateiname gebaut (`aes/gemsclib.c:96`) und loescht damit
  vermutlich nichts. Nicht gefahren — loescht Dateien, gehoert nicht in eine
  Messtabelle. Kein Ticket, weil unklar ist, ob wir etwas tun sollen.
* Ob die Handlereihenfolge je sichtbare **Flaeche verliert** statt sie nur anders
  zu schneiden (A22): nachgerechnet **nein** — beide Reihen ergeben 17165 Pixel.
  Die Kapazitaetsfrage bleibt aber ([A21](A21.md)).
* `wind_create`s 8192 `kind`-Kombinationen: Bits einzeln belegt, Kombinatorik
  nicht gefahren.
* `vdi-r1` und `vdi-r3` hatten **nie** einen NewTOS-Vorstand. Seit dem
  2026-09-12 gibt es einen; ab jetzt ist die Regressionsfrage dort beantwortbar.

## Naechster Schritt laut Anwender

Falltabelle ueber **alle** AES-Funktionen und Unterfunktionen bauen (176 Werte,
Inventar steht), dann Lauf in Hatari, dann mit dem aktuellen Kernel.
Abschnittsaufbau muss Blockierendes und Zerstoerendes trennen — Einzelheiten im
Inventar.
