# PROC04 — Remaining GEMDOS handle deviations found during PROC02

Status: **OFFEN** · Prioritaet: **P3** · Typ: Abweichungen von EmuTOS — aus [PROC02](PROC02-STATUS-DONE.MD)

Abhaengigkeiten: —

## Sachverhalt (vom PROC02-Worker gemeldet 2026-10-06, nicht einzeln nachgeprueft)

1. Zeichenfunktionen (`Cconout`, `Cconin`, … ) folgen der Umlenkung ueber
   `p_uft` nicht und gehen immer zur Konsole; EmuTOS leitet sie um
   (`references/emutos/bdos/bdosmain.c:498-568`).
2. `p_curdir`/`p_curdrv` sind in NewTOS global statt je Prozess; deshalb erbt
   `init_pd_files` sie nicht (`bdos/proc.c` `init_pd_files`, zweite Haelfte).
3. `Fread`/`Fwrite` lesen den Handle als u16; ein negativer BIOS-Handle ergibt
   `EIHNDL` statt Geraetezugriff. AUX/PRN (-2/-3) landen auf der Konsole.
4. Die Dateitabelle hat 16 Plaetze, EmuTOS 75 (`OPNFILES`).
5. `Fdelete` auf eine offene Datei macht alle Handles darauf ungueltig
   (alte Abweichung F6); EmuTOS haelt sie am Leben.

## Arbeit

Jeden Punkt gegen die EmuTOS-Quelle bestaetigen, Spec ergaenzen, 1:1 umsetzen,
Hosttests; Abnahme: QEMU-Reihe wie `measurements/measurement-2026-10-06-proc02/`.

## Punkt 2 erledigt (2026-10-18, QEMU, `59c8eb4c`)

Laufwerk und Verzeichnis je PD wie EmuTOS (Teil des PROC03-Pakets,
Anwenderentscheidung 2026-10-18). Belege
`measurements/measurement-2026-10-18-proc04-curdir/` (rot 19 Fehler → gruen)
und `measurements/measurement-2026-10-18-proc04-curdir-test/` (Tester PASS,
`4f4602aa`: Ordnernavigation in NEWDESK/TeraDesk, Start aus Ordner, QED-fsel).
Accessories: eigener PD bis B5 entschieden. Nebenbefunde: 16 statt 26
Laufwerke (`BLKDEVNUM`/`NUMCURDIR`, vgl. FS12), Pfade auf 63 statt 111 Byte.
Punkte 1, 3, 4, 5 offen.

## Re-Validierung Punkte 1, 3, 4, 5 (2026-10-18, Researcher)

Spec `docs/api/gemdos/_dispatcher.md` §5, `_state.md` §2,
`file/41-fdelete.md`, `3f-fread.md`, `40-fwrite.md`, `3e-fclose.md`.

1. **Bestaetigt, praeziser:** zwei Mechanismen — Datei-Umlenkung
   (`bdosmain.c:498-568`, `stdio_typ` `:150-197`, Crawio ausgenommen) und
   BIOS-Geraet ueber `HXFORM(run->p_uft[n])` (`console.c`). NewTOS ignoriert
   `p_uft` in allen 15 Zeichenfunktionen (`tos-api/src/gemdos/console.rs`).
   Betrifft `system()`/`popen()` der mintlib mit Kindern im TOS-Stil.
3. **Ueberholt** — seit FS11 (2026-10-13) EmuTOS-gleich (`file.rs:231-246,393-455`).
4. **Bestaetigt:** EmuTOS `OPNFILES 75`, `NUMHANDLES 81`, systemweit, ohne
   CONF-Gate (`bdos/fs.h:43-45`); NewTOS 16 (`file.rs:30`).
5. **Bestaetigt, praeziser:** EmuTOS `ixdel` (`fsopnclo.c:652-669`): eigene
   offene Datei → `ixclose`, Handle bleibt gueltig, `Fdelete` = `E_OK`,
   spaeteres `Fclose` = `EINTRN`; fremde → `EACCDN`. NewTOS (F6, `00c8137a`)
   macht alle Handles ungueltig. Keine benannte Abweichung — F6 war eine
   Fehllesung. EmuTOS-Gefahr: Schreiben nach dem Loeschen landet in freien
   Clustern → Anwenderentscheidung.

Nebenfund → [FS15](FS15.md) (`CON:`/`AUX:`/`PRN:` bei `Fopen`/`Fcreate`).
Reihenfolge: 4 → 1 → 5 (nach Entscheidung).

## Anwenderentscheidung Punkt 5 (2026-10-18)

Variante (b): `Fdelete` auf eine eigene offene Datei liefert `E_OK`, der
Handle bleibt gueltig (wie `ixdel`/`ixclose`), auf eine fremde `EACCDN`;
spaeteres Schreiben/`Fclose` ueber den Handle fasst die Karte nicht mehr an
(keine freigegebenen Cluster, kein Verzeichnisslot) — benannte Abweichung in
`docs/COMPATIBILITY.md` beim Bau.
