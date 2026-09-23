# A91 — STATUS: DONE (2026-09-23) — dokumentiert, kein Code

**DONE bedeutet hier: die Abweichung ist an beiden vorgesehenen Stellen mit
Quellenbelegen und Aufloesungspfad festgehalten; es wurde kein Produktcode
geaendert.** Bearbeitet 2026-09-23 zusammen mit A104 und A110 (ein Paket),
Branch `desk-rebuild`, Basis HEAD `671b362`, nichts committet.

## Der Befund, an der Quelle

**EmuTOS.** Der Kontrollmanager ist ein **Prozess**:
`gl_mowner = ctl_pd = iprocess("SCRENMGR", ctlmgr)`
(`references/emutos/aes/geminit.c:888`), und `ctlmgr` ist eine eigene
Endlosschleife `w_setactive(); ev_multi(…); wm_update(BEG_UPDATE); …`
(`aes/gemctrl.c:427-486`). Ein Menue-Track laeuft damit im Kontext von
`ctl_pd`: `mn_do`s Zustandsmaschine (`aes/gemmnlib.c:530-690`) blockiert
`ctl_pd`, **nicht** die Anwendung. Die Anwendung bleibt auf den Ready-/Delay-
Listen des AES; ein angefordertes `MU_TIMER` wird von `tchange`
(`aes/gemflag.c:33-64`) gepostet und an sie ausgeliefert, waehrend die Tafel
auf dem Schirm steht.

**NewTOS.** Es gibt keinen AES-Scheduler und keinen `ctl_pd`-Prozess. Der
Kontrollmanager laeuft **inline** im `evnt_multi`-Trap der Anwendung selbst —
`menu_intercept` / `window::intercept` am Kopf der Warteschleife
(`tos-api/src/aes/event.rs`), und `menu::track` (`tos-api/src/aes/menu.rs`)
faehrt die ganze `mn_do`-Zustandsmaschine dort und kehrt erst mit der Auswahl
oder dem Abbruch zurueck. Solange das laeuft, ist `evnt_multi` der Anwendung
nicht zurueckgekehrt: ihr `MU_TIMER` feuert nicht, und kein anderer Arm dieses
Aufrufs wird ausgewertet. Seit A73 bleibt der Track auch im `OUTSIDE_STATE`
offen (Zeiger zwischen Leiste und Kasten; kein Ereignis verlaesst ihn bis zum
Wiedereintritt oder einem Tastenuebergang, `gemmnlib.c:415-443,559-585`) —
„solange das laeuft" ist damit beliebig lang.

Es ist eine Eigenschaft des Inline-Modells, nicht von A73; A73 hat das Fenster
nur lang genug gemacht, um es zu bemerken. Nachrichten, ihre Wortinhalte und
ihre Reihenfolge sind unveraendert. Beobachtbar ist es fuer eine Uhr, eine
Fortschrittsanimation, alles Timer-getriebene: es steht still, solange ein
Menue offen ist, und laeuft danach weiter.

## Wo es steht

* **`docs/api/aes/event.md`**, § `evnt_multi`, direkt nach den A73/A92-Notizen:
  neuer Abschnitt „NewTOS deviation — the application does not run while a
  drop-down is open (A91, 2026-09-23)" mit den obigen Zitaten und dem
  Aufloesungspfad.
* **`docs/COMPATIBILITY.md`**, § „Sanctioned deviations, one by one", im Format
  der vorhandenen Eintraege (datiert, attribuiert, mit Quelle, Wirkung und
  Aufloesungspfad), eingeordnet vor dem „Not sanctioned, still open"-Absatz.

## Der Aufloesungspfad — die Quittung

Der kooperative Kern (Meilenstein 2b, `docs/analysis/aes-audit/00-MASTER.md`)
gibt dem AES echte Prozesse; der Screen Manager wird dann einer, der Track
zieht dorthin um, und die Anwendung wird waehrend eines Tracks weiter
dispatcht — wie in EmuTOS. Der Befund entfaellt damit **von selbst**; es gibt
keinen eigenen Fix zu bauen.

**`TASK/ACC03.md` ist die Stelle, an der das entschieden wird.** Das Ticket
(Accessory-Prozessmodell: paralleles `evnt_multi`, `wait_for_accs`) fuehrt A91
bereits als Abhaengigkeit, beschreibt das Accessory als zweiten `AESPD`, der
neben Desktop und Anwendung in `evnt_multi` parkt (`aes/geminit.c:654-745`),
und hat als dritten Arbeitspunkt woertlich: „Entscheiden, ob der
Inline-Kontrollmanager dafuer fallen muss (A91) oder ob Accessories damit leben
koennen". Mit ACC03 verschwindet der Befund oder wird dort bewusst
weitergetragen — in beiden Faellen ist er dann keine unbenannte Abweichung mehr.

## Pruefung

Kein Produktcode geaendert, also kein eigener Test und keine eigene Messung.
Die Zahlen des Pakets (A104/A110) gelten unveraendert: `cargo run -p xtask --
test` **926/0/1**, beide Boards tos-api **64** / kernel **94**, Clippy
**80**/**125** unveraendert.

## Offen

Nichts. Das Ticket ist als **ERLEDIGT — dokumentiert** geschlossen; die
sachliche Aufloesung gehoert zu ACC03.
