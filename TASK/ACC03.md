# ACC03 — Accessory-Prozessmodell: paralleles `evnt_multi`, `wait_for_accs`

Status: **OFFEN** · Prioritaet: **P2** · Typ: Funktionsluecke / Kern

Abhaengigkeiten: [ACC01](ACC01.md), [ACC02](ACC02.md), [A91](A91.md) (Inline-Kontrollmanager)

## Sachverhalt

Ein Accessory ist in EmuTOS ein eigener `AESPD`, der neben dem Desktop und
der laufenden Anwendung in `evnt_multi` parkt (`aes/geminit.c:654-745`);
nach `dsptch()` wartet der Start mit `wait_for_accs(AP_MESAG)` (`:588-630`,
Timeout 500 Ticks), bis jedes Accessory seine Initialisierung abgeschlossen
hat. NewTOS faehrt den Kontrollmanager **inline** in `evnt_multi`
(`tos-api/src/aes/event.rs`, A91): waehrend eines Menue-Tracks laeuft kein
anderer Prozess. Der Sleep/Wake-Unterbau (`sched.rs`, `wait_mesg`, A75)
traegt aber bereits mehrere wartende Klienten.

## Arbeit

1. Klaeren, was der kooperative Kern fuer Accessories mindestens braucht:
   mehrere Prozesse in `evnt_multi`, Weckung per `post_msg`, Umschaltung im
   Trap-Dispatcher (`Option<Switch>`-Pfad, A75). Belegen, welche Teile
   fehlen (Zeitgeber je Prozess? `MU_TIMER` waehrend fremder Tracks, A91).
2. `wait_for_accs`-Aequivalent nach dem Start.
3. Entscheiden, ob der Inline-Kontrollmanager dafuer fallen muss (A91) oder
   ob Accessories damit leben koennen (sie bekommen waehrend eines Tracks
   keine Ereignisse — EmuTOS schon).

## Abnahme

Zwei Test-Accessories + Desktop laufen gleichzeitig; Hosttest mit zwei
wartenden Klienten; QEMU: beide melden sich, Desktop bedienbar.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
