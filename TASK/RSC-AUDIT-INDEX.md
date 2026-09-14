# Index — die Befunde des RSC-/appl_init-Audits vom 2026-09-11

Der vollstaendige Bericht mit Herleitung, `Datei:Zeile` auf beiden Seiten,
Messungen und getriebenen Testfaellen ist
[docs/analysis/audit-applinit-rsrc-load-2026-09-11.md](../docs/analysis/audit-applinit-rsrc-load-2026-09-11.md).
**Er ist die Quelle; die Tickets hier sind nur Auftraege.**

| Befund | Kurz | Ticket | Zustand |
|---|---|---|---|
| **B1** | `rsrc_load` loest keinen qualifizierten Pfad auf | — | **ERLEDIGT** 2026-09-11, HW-abgenommen 2026-09-12 |
| **B2** | ungerade Rasterzeiger → Panik in `blit_plane` | — | **ERLEDIGT** 2026-09-11 (Werkzeug + Kernel), HW-abgenommen |
| **B3** | `rsrc_load` fuellt `ap_ptree`/`ap_rscmem`/`ap_rsclen` nie | [A35](A35.md) | ERLEDIGT (2026-09-14) |
| **B4** | nur *referenzierte* TEDINFO/ICONBLK/BITBLK relokiert | [A36](A36.md) | OFFEN |
| **B5** | `fix_tedinfo_std` fehlt → Pufferueberlauf beim Editieren | [A37](A37.md) | OFFEN, **Speicherfehler** |
| **B6** | zu kurz gelesenes Blob wird ueber den Puffer hinaus relokiert | [A38](A38.md) | OFFEN, **Speicherfehler** |
| **B7** | Desktop und erstes Programm teilen eine AES-Kennung | [A29](A29.md) | OFFEN (Ticket bestand schon) |
| **B8** | an der Startgrenze wird gar nichts zurueckgesetzt | [A28](A28.md) | OFFEN (Ticket bestand schon) |
| **B9** | Resourcezustand ist Kernel-Global statt am `AESGLOBAL` | [A39](A39.md) | OFFEN |
| **B10** | `ob_fix` im Werkzeug festverdrahtet statt zur Ladezeit | [A40](A40.md) | OFFEN, latent |
| **B11** | CICON-Resourcen vom Werkzeug abgewiesen | [A41](A41.md) | OFFEN, Funktionsluecke |

**Reihenfolge nach Benutzerentscheid 2026-09-12:** Speicherfehler vor
Konformitaet, also A37 und A38 zuerst.

B7/B8 hatten schon Tickets aus der A25-Gegenpruefung ([A25-AUDIT-BEANSTANDET.MD](A25-AUDIT-BEANSTANDET.MD));
das Audit bestaetigt sie unabhaengig. Nicht doppelt anlegen.
