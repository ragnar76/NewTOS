# A113 — STATUS: DONE (2026-10-17, QEMU) — Accessories melden ap_id 2.. und sind erreichbar

**Auftrag:** ap_id-Vergabe fuer Accessories wie EmuTOS; `appl_find`/`appl_write`/`msg[1]`/`WF_OWNER` daran.

## Weg

1. Ursache belegt (Auditor): `measurements/measurement-2026-10-17-a113-a114/` (`38781765`).
2. Spec aus EmuTOS (`fcfa4a0a`): `docs/api/aes/application.md` pid-Modell,
   `window-graphics.md`, `event.md`, `_state.md`.
3. Fix (`79383cd0`): `reported_pid` = 2 + feste Ladeposition in `ACC_PDS`,
   sonst 0; `identity_of(0)` = Kettenmitglied (Aufrufer, wenn er dazugehoert,
   sonst innerstes lebendes Nicht-Accessory); `identity_of(2+n)` = Accessory n;
   Loch, 1, negativ, `>= totpds` → nichts (`appl_write` 0, NewTOS-Wahl).
   `release_client` streicht ein beendetes Accessory (`forget_process`,
   `clear_name`), damit der naechste Mieter die pid nicht erbt.
4. Unabhaengiger Tester PASS (`34fd08a8`).

## Belege

| | vorher (`dc3388d1`) | nachher |
|---|---|---|
| ap_id ACCX / ACCY / Programm | 0 / 0 / 0 | 2 / 3 / 0 |
| `appl_find` | 0 / 0 | 2 / 3 |
| Programm → Accessory `appl_write` | eigene Queue | zugestellt, Antwort mit `m1` = 2/3 |
| `WF_OWNER` / `msg[1]` WM_REDRAW | 0 | 2 / 3 |

`measurements/measurement-2026-10-17-a113-fix/RUNINFO.TXT`,
`measurements/measurement-2026-10-17-a113-test/RUNINFO.TXT`. Hosttests 1391/0/1
(+6), SDK 20/0/0, 64/94, 76/125, Drift 0, APITEST 109/0/5 in allen
memprot-Modi; Desktop mit/ohne SYSACC, TeraDesk, QED bytegleich.

## Offen

* Echte Pi 4: TeraDesk mit AV-faehigem Accessory; SYSACC oeffnen/schliessen.
* Nicht gelaufen: Accessory mit `Pexec`-Kind (meldet die Accessory-pid —
  Folgerung, `rlr` wechselt bei `dos_exec` nicht), Accessory endet zur Laufzeit
  (nur Hosttest).
* Ausserhalb: `input::same_pd` kennt Pexec-Kinder eines Accessorys nicht als
  eigenen Besitzer; `selftest_sleepwake_switch.rs:71` Kommentar veraltet; A116.
