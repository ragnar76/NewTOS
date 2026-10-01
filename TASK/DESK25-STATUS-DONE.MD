# DESK25 — STATUS: DONE (2026-09-23)

**Auftrag:** Den Symboltyp bis `dir_op` durchreichen (EmuTOS' zweiter
Parameter) und die `STDELDIS`-Zusatzfrage an EmuTOS' Stelle stellen
(`references/emutos/desk/deskdir.c:971-975`).

Basis: Branch `desk-rebuild`, HEAD `50714b7`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset` auf dem gemeinsamen
Baum.

Messung: [`measurements/measurement-2026-09-23-desk24-desk25-desk26/`](../measurements/measurement-2026-09-23-desk24-desk25-desk26/) §3.
Spezifikation: `docs/api/desktop/deskdir.md`, `docs/api/desktop/deskfun.md`.

---

## 1. Die Analyse des Tickets — gegen die Quelle geprueft

**Richtig, und nachgemessen.** Der Vorher-Lauf (`serial/d25-before-serial.log`)
zeigt `dir_op op=Delete icontype="<absent>" src="C:\" n=31` und danach eine
leere Platte: ein `OK` auf dem zaehlenden Kasten genuegte, um `C:` zu raeumen.

**Praezisiert:** EmuTOS bindet die Frage **nicht** an `g_cdelepref`. Sie steht
hinter dem `if (more …)`, das der Kasten gesetzt hat, aber ihre eigene
Bedingung ist nur `op == OP_DELETE && icontype == AT_ISDISK` (`:971`).
*Confirm deletes* aus nimmt also das Warten am zaehlenden Kasten weg, diese
Frage bleibt.

**Praezisiert:** der Parameter ist nicht nur „irgendwie durchzureichen", er
hat pro Aufrufstelle einen von EmuTOS festgelegten Wert — an einer Stelle
sogar `-1`, obwohl die Quelle ein Schreibtischsymbol ist (`fun_file2win`,
`deskfun.c:1113`; dort ist es ein `OP_COPY`, die Frage kann gar nicht
greifen). Die Tabelle steht in `deskdir.md` und ist Stelle fuer Stelle
uebernommen.

---

## 2. Was gebaut wurde

`kernel/src/desk/dir.rs`:

* **`dir_op(op, icontype, …)`** und **`fun_op(op, icontype, …)`** haben EmuTOS'
  zweiten Parameter als `icontype: Option<AType>`. `None` ist EmuTOS' `-1`
  („die Quelle ist ein Fenster, kein Schreibtischsymbol"),
  `Some(AType::Disk)` sein `AT_ISDISK`. `fun_op` reicht ihn an **beide**
  `dir_op`-Durchlaeufe weiter (`deskfun.c:793,798`).
* **`fun_file2desk(dobj, icontype_src, …)`** hat denselben Parameter, weil
  EmuTOS ihn dort hindurchfaedelt (`deskfun.c:1004,1081`).
* **`delete_disk_alert(letter)`** baut `STDELDIS`
  (`[2][Do you want to delete all|the files on drive %c: ?][Yes| No ]`,
  `desk_rsc.c:2499-2500`) mit dem Laufwerksbuchstaben, Voreinstellung `No`;
  `true` ist EmuTOS' `== 1`.
* Die Frage steht in `dir_op` hinter dem `ADCPYDEL`-Kasten und vor der
  Schleife, wie `deskdir.c:967-976`. Ein `No` setzt `ctx.stop`, was genau
  EmuTOS' `more = FALSE` ist.

Werte an den sechs Aufrufstellen: siehe die Tabelle in
`docs/api/desktop/deskdir.md`. Kurz: `None` fuer alles, was aus einem Fenster
kommt; `Some(Disk)` fuer `fun_del_desktop` und `fun_desk2desk`.

---

## 3. Die Nachweise

    vorher   [d25] dir_op op=Delete icontype="<absent>" src="C:\" n=31
             → Wurzel leer                                   d25-before-d-after.png

    nachher  [d25] STDELDIS drive 'C' icontype=Some(Disk) -> yes=false
             [d25] dir_op op=Delete icontype=Some(Disk) ... stop=true
             → Karte unveraendert (57 Dateien wie das Ausgangsabbild)

    nachher  [d25] STDELDIS drive 'C' icontype=Some(Disk) -> yes=true
             → Wurzel leer bis auf die Datentraegerkennung

`d25-no-d-stdeldis.png` zeigt den Kasten ueber dem stehenden
`DELETE FILE(S)`-Kasten, mit `Yes`/`No` und der Vorauswahl auf `No`.

Die drei Rueckfragen des Schreibtischsymbol-Loeschens sind damit EmuTOS'
drei: `STRMVDEL` (`Cancel`/`Remove`/`Delete`), der zaehlende Kasten (an
*Confirm deletes* gebunden), `STDELDIS`.

`[EXC]`/`[PANIC]`: 0 in allen drei Laeufen.

### Bau

    cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
    cargo xtask build --board rpi4        tos-api 64 / kernel 94   (Basis 64/94)
    cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
    cargo xtask build --board qemu-virt   tos-api 64 / kernel 94   (Basis 64/94)

    cargo clippy --features board-rpi4       tos-api 80 / kernel 125 (Basis 80/125)
    cargo clippy --features board-qemu-virt  tos-api 80 / kernel 125 (Basis 80/125)
    Treffer in kernel/src/desk/: 0

    cargo run -p xtask -- test   938 bestanden / 0 fehlgeschlagen / 1 ignoriert

---

## 4. Offen

* Pi-4-Punkt **49** in `docs/api/desktop/_hw-test-2026-09-17.md`: die Frage mit
  *Confirm deletes* **aus**, das Ausbleiben beim Loeschen aus einem Fenster
  heraus, und `Yes` auf einer Wegwerfkarte.

## 5. Nebenbefunde (nicht angefasst)

* **`fsck_msdos` stirbt auf den zwei leergeraeumten Karten** (`rc=133`, keine
  Ausgabe). Reproduziert sich **identisch auf dem HEAD-Bau**, ist also nicht
  DESK25. Ursache: GEMDOS' `Fdelete`/`Ddelete` loeschen den 8.3-Eintrag und
  lassen die davorliegenden Langnamen-Schlitze (`attr == 0x0F`) stehen —
  klassisches TOS kennt keine Langnamen. Die Karte traegt sie, weil macOS
  `.fseventsd`, `.Spotlight-V100` und die `._*`-Begleiter mit LFN-Ketten
  schreibt. Beweis: dieselbe Karte mit den 35 Waisen-Schlitzen von Hand auf
  `0xE5` gesetzt (`method/lfn_orphan_probe.py --fix`) laeuft sauber durch
  Phase 1/2/3, rc=0. Details und die noch offene Teilfrage in
  `measurements/measurement-2026-09-23-desk24-desk25-desk26/serial/fsck.txt` §6.
  **Eigenes Ticket wert, in der GEMDOS-Dateisystemschicht.**
* **Der `AT_ISTRSH`-Vorbau.** EmuTOS stellt `wants_to_delete_files()` voran,
  wenn `icontype_src >= 0` (`deskfun.c:1049-1051`); NewTOS haelt dort weiter
  den eigenen `fun_confirm`-Kasten in `fun_desk2desk`. Die **Zahl** der
  Rueckfragen stimmt jetzt (Vorbau + zaehlender Kasten + `STDELDIS`), der
  Wortlaut und der *Remove*-Zweig nicht.
