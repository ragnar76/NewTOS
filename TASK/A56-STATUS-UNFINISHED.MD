# A56 — Quittung

**Zustand: UNFINISHED** · 2026-09-20 · kein Commit (Untersuchung, kein Fix)

**UNFINISHED heißt hier: die Untersuchung ist fertig, die Angleichung steht
aus** — und sie ist, wie das Ticket verlangt, **nicht isoliert** möglich
(Abschnitt 3). Gemeinsam mit A62 untersucht.

Bericht: [docs/analysis/aes-process-table-and-apid-2026-09-20.md](../docs/analysis/aes-process-table-and-apid-2026-09-20.md)
(Abschnitte 2, 3, 5, 6). Artefakte: [measurement-2026-09-20-a62/](../measurement-2026-09-20-a62/RUNINFO.TXT).
Basis `desk-rebuild` @ `8f052b2`, gepinnte EmuTOS-Quelle `1a7484ab…`.

## 1. Der Wert — aus der Quelle, genauer als im Ticket

EmuTOS vergibt **pid 0** nicht nur dem Desktop und „dem ersten Programm",
sondern der **gesamten Anwendungskette**: Desktop, jede vom Shell-Loop
gestartete Anwendung **und jedes per `Pexec` gestartete Kind** laufen auf
demselben `rlr` (AESPD 0; `getpd` hat nur die Aufrufer `iprocess` und
`load_one_acc`, `aes/gempd.c:60-70`, `geminit.c:226,255`). pid 1 =
`SCRENMGR` (`geminit.c:884-888`), 2+ = nur Accessories. Die live-Tabelle hat
`totpds = num_accs + 2` Einträge — ohne Accessories **zwei**.

NewTOS (`appl.rs:166-174`): Desktop 1, Slot 0 → 1, Slot 1 → 2 … (H05-Trace
`pid=2` für das TeraDesk-Kind; eigene Messung `serial/nest-serial.log`
1…8, dann −1).

## 2. App-Sweep: steht eine portierte Anwendung darauf? **Nein → A56 bleibt P2**

`/usr/bin/grep -rn "ap_id\|gl_apid\|_AESapid\|apid"` über TeraDesk, QED,
cflib, gemlib-Overlay — Tabelle im Bericht, Abschnitt 5a. Kein Vergleich mit
der Konstanten 0; `msg[1] == ap_id` nur in AV-Protokoll-Pfaden (zwei
AES-Klienten gleichzeitig, bei uns unerreichbar); `WF_OWNER != ap_id` nur
unter `mint || geneva` bzw. `aes_version >= 0x399`; QEDs `appl_write(gl_apid)`
an sich selbst funktioniert mit jedem Wert. Latent: gemlib
`mt_gem.h.in:938` schickt `SM_M_SPECIAL` an die Konstante 1 (MagiC, ungenutzt).

## 3. Warum nicht isoliert — die zwei Enden, die das Ticket verlangt

1. **A39 schlüsselt die Ressourcenliste über `caller_pid()` =
   `current_pid()`** (`resource.rs:678`). Bekämen Eltern und Kind beide 0 als
   *Identität*, löschte `on_process_exit(0)` beim `Pterm` des Kindes die
   Einträge des Elternprozesses — H05 käme zurück. Folge: **Wert ≠ Identität.**
2. **H02 hat `ctl_pd` als Flag gebaut.** EmuTOS' `msg[1]` (`ap_sendmsg`,
   `gemwmlib.c:855`) ist 1 für Control-Manager-Nachrichten
   (`WM_MOVED/SIZED/CLOSED/…`, `MN_SELECTED`, `gemctrl.c`) und 0 für
   `WM_REDRAW` aus den eigenen `wind_*`-Aufrufen. NewTOS schreibt überall die
   Leser-pid (`event.rs:116`, `window.rs:1594,3936,4079`). Heute stimmt
   `msg[1]` für Control-Manager-Nachrichten an Slot 0 zufällig (1 = 1) und ist
   für `WM_REDRAW` falsch; ein isoliertes „`ap_id` = 0" kehrte das um. Der
   Absenderwert muss am Poster gestempelt werden (drei Stellen).

## 4. Eingebauter Desktop und Kind

`desk::run` → `appl_init` im Kernel-Kontext → **1**; per Doppelklick
gestartetes Programm → Slot 0 → **1** (derselbe Deskriptor, A29); dessen Kind
per `Pexec` → Slot 1 → **2**. EmuTOS: 0, 0, 0.

## 5. Vorschlag (Bericht, Abschnitt 6) — für die Entscheidung des Anwenders

Erst A62 (Identität = GEMDOS-Slot, kein beobachtbarer Wert ändert sich), dann
A56 (berichteter Wert nach EmuTOS: 0 für die Anwendungskette, `msg[1]` = 1
für Control-Manager-Poster; Ziel-Auflösung von pid 0 → der laufende Klient).
Vorher die Spec korrigieren: `docs/api/aes/application.md:452-453` („2+ =
accessories/applications" — falsch), `:194-197` (lizenziert das −1), `:587`
(„ap_id = Phase-06 process id"). Fall 644 (`aes-r4-subfunctions.tsv`, CRC
`227422B2`) wird erst mit Schritt 2 grün.

## 6. Aufräumen

Kein Produktcode geändert, keine Hardware-/Outward-Aktion, kein Commit;
Details in [A62-STATUS-UNFINISHED.MD](A62-STATUS-UNFINISHED.MD) § 5.
