# A25 — appl_init: Nachrichten und Flags gehen beim Wiederaufruf verloren

Status: **IMPLEMENTIERT — HARDWAREABNAHME OFFEN** · Priorität: **P1** · Typ: Bestätigter Zustandsfehler

Abhängigkeiten: Keine

> **Diese Datei ist eine Rekonstruktion.** Am 2026-09-10 um 08:21 habe ich
> (Audit-Strang A13–A15) die ursprüngliche `A25.md` versehentlich überschrieben,
> weil ich die Nummer A25 für einen eigenen Befund vergeben habe, ohne den
> parallel laufenden Strang zu bemerken. Mein Befund liegt jetzt unter
> [A27](A27.md). Der Inhalt hier ist aus dem unversehrten Ticket
> [A25-STATUS-UNFINISHED.MD](A25-STATUS-UNFINISHED.MD), dem Arbeitsbaum-Diff und
> der EmuTOS-Quelle nachgebaut — **er ist nicht das Original**. Wer den
> ursprünglichen Text hat, ersetze diese Datei; Quellbelege und
> Verlaufsbegründungen des Originals können hier fehlen.

## Der Fehler

`pd::open` (`tos-api/src/aes/pd.rs`) setzte bei einem `appl_init` auf eine
**bestehende** PID den Zustand des Slots zurück:

```rust
slot.flags = AP_OPEN;   // überschreibt bestehende Flags
slot.queue.clear();     // verwirft wartende Nachrichten
```

EmuTOS' `ap_init` fasst dagegen nur ein einziges Bit an und lässt alles andere
stehen — `rlr->p_flags |= AP_OPEN;` (`references/emutos/aes/gemaplib.c:54-63`).
Eine vor dem `appl_init` eingetroffene GEM-Nachricht ging bei uns verloren.

## Der Fix

`slot.flags |= AP_OPEN`, kein `queue.clear()`; neue Slots werden weiterhin
getrennt leer initialisiert. Damit bleiben Queue, Name und übrige Flags eines
bestehenden Slots erhalten.

## Nachweise

- Neuer Dispatcher-Test `tos-api/tests/aes_appl_init.rs` — vor dem Fix **rot**:
  eine vor `appl_init` wartende Nachricht ging verloren.
- Unit-Test `init_tests` in `pd.rs`: Flags, Queue, Name und voller Pool,
  einschließlich des Falls, dass eine bestehende PID keinen freien Slot braucht.
- Integrationstest für TT- und Full-HD-Profil grün.
- `cargo run -p xtask -- test`: **429 bestanden**, 0 fehlgeschlagen, ein
  bestehender Doc-Test ignoriert.
- `cargo build -p tos-api --target aarch64-unknown-none-softfloat`: Exit 0.
- Spezifikation `docs/api/aes/application.md` aktualisiert.

## Stand

Implementierung abgeschlossen und hostverifiziert, **nicht** deployed, **kein**
Commit. Offen ist allein die reale Gegenprobe auf Hardware.

`UNFINISHED` im Ticket heißt hier: Hardwareabnahme fehlt, **nicht**
Implementierung fehlt.

## Abgrenzung

Der Fix berührt A13 und A14 nicht.

## Einstiegspunkte

- [tos-api/src/aes/pd.rs](../tos-api/src/aes/pd.rs)
- [tos-api/tests/aes_appl_init.rs](../tos-api/tests/aes_appl_init.rs)
- [docs/api/aes/application.md](../docs/api/aes/application.md)

## Arbeitsregel

[Gemeinsame Regeln und Messgrenzen](README.md) gelten. Bestehende Tests und Entscheidungen zuerst prüfen; nur fehlende Prüfungen ergänzen. Ein Messunterschied ist ohne Einordnung kein Fehlerauftrag. Abschluss hier mit Commit bzw. Artefakt, ausgeführten Prüfungen und verbleibender Hardwareabnahme festhalten.
