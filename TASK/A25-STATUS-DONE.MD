# A25 — STATUS: DONE (2026-10-17, QEMU) — `appl_init`-Wiederaufruf erhaelt die Nachricht

**Auftrag:** Letzter offener Punkt: der Wiederaufruffall in einem echten Programm (der Fix in `tos-api/src/aes/pd.rs` war hostgetestet).

**Lauf:** `sdk/examples/a25reinit.c` (GEM-`.NAP`, per `#Z`-Autorun): `appl_init` → 16-Byte-`appl_write` an die eigene PID → `appl_init` → `appl_read` → Vergleich → `appl_exit`; Ergebnis nach Serial und `C:\A25REINI.LOG`. Frischer rpi4-Release-Kern des Baums `db5880a8`, QEMU raspi4b, memprot on / off / strict.

**Ergebnis:** in allen drei Modi `ap_id` beide Male 0, `appl_write` rc=1, `appl_read` rc=1, 0 abweichende Woerter (16 Byte exakt), `RESULT PASS`. Kontrolle ohne Wiederaufruf ebenfalls PASS. Belege: `measurements/measurement-2026-10-17-a25/` (RUNINFO.TXT, serial/, logs/), Commit `752107b9`.

**Grenzen (ehrlich):** kein Lauf auf der Pi/TT; kein Lauf gegen einen Kern vor dem Fix (rot→gruen belegt der Hosttest); `AP_MESAG`/`AP_OPEN` sind aus einem Programm nicht beobachtbar (nur Unit-Test). Beobachtung am Rand: ein `evnt_multi(MU_MESAG|MU_TIMER)` sah die selbst gesendete Nachricht auch **ohne** Wiederaufruf nicht (Timer-Ereignis), `appl_read` dagegen schon; unabhaengig von A25, nicht untersucht (RUNINFO „HISTORY“).
