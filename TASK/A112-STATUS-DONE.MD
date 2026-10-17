# A112 — STATUS: DONE (2026-10-17, QEMU) — AES-Weckwege wie EmuTOS

**Auftrag:** `evnt_multi` und die AES-Abgabe nach EmuTOS; Paket F1–F4 (Anwenderfreigabe 2026-10-17).

## Weg

1. Ursache, unabhaengig bestaetigt: `measurements/measurement-2026-10-17-a112/`,
   `measurements/measurement-2026-10-17-a112-audit/` (`ca4914ff`).
2. Spec aus EmuTOS berichtigt (`d21a6b21`): `docs/api/aes/event.md`
   „Wake conditions and dispatch“, `docs/api/aes/application.md`.
3. Fix (`1762e837`): Weckmenge je Prozess (Eingabe, Nachricht, Frist),
   Fristen bei jedem Dispatch, Abgabe nach jedem abgeschlossenen AES-Aufruf,
   `appl_yield` gibt doppelt ab, Sofortpruefungen einmal (`EV_ARMED`),
   `evnt_timer(0)` endet am naechsten Tick. A91/A75 unberuehrt.
4. Unabhaengiger Tester PASS (`3dd45372`).

## Belege

| Fall | HEAD `d21a6b21` | Fix |
|---|---|---|
| f1 Accessory `evnt_multi(MU_TIMER,250)` | steht ab 18 s | 4 Ticks/s |
| f1b Programm `MU_MESAG\|MU_TIMER` | steht nach 2 Rueckgaben | 30 Schleifen mit 1 Hz |
| f2 fremdes `WM_REDRAW` an `evnt_multi(MU_MESAG)` | nie | sofort |
| f3 pollendes Programm + Accessory | 0 Ticks; 250× `evnt_timer(0)` in 0 s | Accessory tickt; 4,98 s |

`measurements/measurement-2026-10-17-a112-fix/RUNINFO.TXT`,
`measurements/measurement-2026-10-17-a112-test/RUNINFO.TXT`. Hosttests 1385/0/1
(+5 `aes_a112_wake_conditions.rs`), SDK 20/0/0, Bau 64/94, Clippy 76/125,
Drift 0, APITEST 109/0/5 in memprot on/off/strict. Desktop mit/ohne
Accessory, TeraDesk, QED pixelgleich zu HEAD (ein Zeiger-Timing-Race).

## Offen

* Echte Pi 4 (Timing, Accessory mit eigenem Timer neben Programm).
* Rest-Abweichung: Auswahl nach Slot-Reihenfolge statt EmuTOS-`rlr`-Liste
  (`gemdisp.c:221-253`, `gempd.c:134-145`) — ein Geweckter laeuft einen
  Aufruf frueher, mehrere in anderer Reihenfolge. Anwenderentscheidung offen.
* Ausserhalb: `input::own_mouse()` prueft nicht den Aufrufer (M1/M2 fuer
  Nicht-Besitzer); A113, A114.

## Pi-Abnahme (2026-10-17)

Kern aus `f2c68a31` (sha256 `bff063c5…`). Anwender auf echter Pi 4: Desktop,
SYSACC, TeraDesk, QED ohne Rueckschritt, Menue-Stillstand unveraendert —
*„soweit alles bestätigt, keine bugs“*. Ein Accessory mit eigenem Timer war
nicht auf der Karte; der Fall selbst ist nur in QEMU belegt.
