# KEYOWNER — Quittung (FERTIG in QEMU): Tasten an den Maus-/Tastatur-Eigentümer; Ssystem 2020

Datum 2026-10-08 · Basis `desk-rebuild` `e3400de9` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-08-keyowner](../measurements/measurement-2026-10-08-keyowner/RUNINFO.TXT)
· Ticket: [KEYOWNER](KEYOWNER.md) · Vorgänger: [SYSINFO02](SYSINFO02-STATUS-DONE.MD) (offener Punkt „Esc")

## Ursache

`evnt_multi` (`MU_KEYBD`) und `evnt_keybd` lasen den einen IKBD-Tastenring für
jeden AES-Client, der gerade fragte. Während SYSACC sein Aufklappfeld unter
`wind_update(BEG_MCTRL)` verfolgt und in `evnt_multi` schläft, lief EmuDESK
und nahm Esc/Return/Pfeile weg. EmuTOS gibt jede Taste dem Maus-Eigentümer:
`gl_mowner` = „current mouse/keybd owner" (`aes/geminput.c:52`), `kchange` →
`post_keybd(gl_mowner->p_cda, ch)` (`:288-308`), jede Wartestelle liest nur
ihre eigene Schlange (`gemevlib.c:173-182`, `akbin` `geminput.c:486-493`).
Ein `gl_kowner` gibt es in diesem EmuTOS nicht.

## Änderung (Spec zuerst)

* `docs/api/aes/event.md` — neuer Abschnitt „Who gets a key — the mouse
  owner (`gl_mowner`)": die vier Wechselstellen (`w_setactive`, `fm_own`/
  `BEG_MCTRL`, `chkown`, `mchange`) mit Datei:Zeile, der beobachtbare Vertrag,
  die benannten NewTOS-Abweichungen.
* `tos-api/src/aes/input.rs` — die Identitätshälfte von `gl_mowner`/`gl_cowner`
  neben dem bisherigen `ctl_pd`-Flag; `Mown`; `owns_keys`; Anwendungskette =
  ein Eigentümer, jedes Accessory ein eigener; `w_setactive` behält den
  Eigentümer, solange `ml_ocnt > 0` (ctlmgr ruht in der Klammer).
* `tos-api/src/aes/window.rs` — `ml_pmown` mit Identität, `top_owner`,
  `desk_owner`, `mctrl_held`; `acc.rs` `is_process`.
* `tos-api/src/aes/event.rs` — Taste nur an den Eigentümer; `evnt_keybd` mit
  der Kopfzeile des Control Managers.
* **Ssystem 2020** (`docs/api/gemdos/154-ssystem.md` zuerst): Eingabemodus,
  0 = Timer-Pumpe, 1 = `input=aes`; `Host::input_mode`, Kernel
  `input_gate::mode`; der „frei"-Test rückt auf 2021. `sdk/examples/sysacc.c`
  zeigt „Mouse while busy" in „Running now".

## Prüfung

* Host-Tests: `tos-api/tests/aes_key_owner.rs` (6) + Ssystem-2020-Test — rot
  gegen HEAD (0/6; Sysinfo-Test kompiliert dort nicht), grün mit der Änderung.
  Gesamt 1268/0/1 (Basis 1261/0/1).
* QEMU raspi4b, HEAD- gegen neue Kernel, 23 Läufe je Satz: Aufklappfeld Esc/
  Pfeile/Return wirkt (vorher blieb es offen); alle anderen Tastaturpfade
  (EmuDESK-Kürzel, Ctrl-I-Tippen, Umbenennen, auch mit Accessory; TeraDesk;
  QED Tippen+Speichern+Öffnen; LDGDEMO; zView Esc-Vollbild; DOOM über kbdvec;
  EmuCON; APITEST 106/0/5) byte-gleich bis auf den beabsichtigten Wert „Mouse
  while busy" und benannte Zeitartefakte (Mauszeiger mitten im Park-Sweep,
  DOOM-Demo-Bilder). Host-Checks PASS, fsck sauber, 0 Ausnahmen in 46 Logs.
* Statisch: Release-Builds 64/94, clippy 76/125 (beide Boards, = Basis),
  Drift 31, SDK-Host-Tests 19/0/0.

## Paket (nicht kopiert)

`bin/kernel8.img` (sha256 `6314c800…`), `bin/SYSACC.NAC` (`812ddd57…`),
`bin/LIESMICH.TXT` — im Laufverzeichnis.

## Offen

* Echte Pi 4 (USB-Tastatur) nicht gelaufen — HW-Abnahme beim Anwender.
* Benannte Abweichungen (Spec): Tasten werden beim Lesen zugeordnet, Tasten
  unter `ctl_pd` werden nicht verschluckt, `w_setactive` läuft bei jedem
  Wartedurchlauf; die Maus-Quick-Checks (`own_mouse`) bleiben beim
  Einzelanwendungs-Flag.
* `docs/STATUS.md` nicht nachgetragen (Lead/Commit).
