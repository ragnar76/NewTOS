# DESK20 — STATUS: DONE (2026-09-23)

**Auftrag:** Das Funktionstastenfeld `APFUNKEY` in `ins_app_dialog`
nachziehen, samt Lesen/Schreiben wie in `deskins.c`; danach zeigt der Anzeiger
in `inf_conf` etwas.

Basis: Branch `desk-rebuild`, HEAD `9a85fd8`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset`.

Messung: [`measurement-2026-09-23-desk19-desk20/`](../measurement-2026-09-23-desk19-desk20/).
Spezifikation: `docs/api/desktop/deskins.md` (§ `ins_app`),
`docs/api/desktop/deskinf.md` (§ *Function keys*).

---

## 1. Die Praemisse, gegen die Quelle geprueft

`references/emutos/desk/desk_rsc.c:1059-1063` — `ADINSAPP` Objekt 5:

```c
{ 6, -1, -1, G_FBOXTEXT,                 /*** 5 ***/  /* APFUNKEY */
  EDITABLE | FL3DBAK | 0x4000, NORMAL,
  (LONG) &desk_rs_tedinfo[19],
  3, 6, 23, 1},
```

`desk_rs_tedinfo[19]` = Vorlage `"Install as: F__"`, `te_pvalid` `"9"`,
`te_txtlen` **3** (`desk_rsc.c:130-133`). Ein `te_txtlen` zaehlt die NUL mit,
das Feld traegt also **zwei** Ziffern — genau was F1…F20 braucht.

`deskins.c:474-477` fuellt vor, `:404-444` (`get_funkey`) liest und prueft,
`:520-531` speichert. `FIRST_FUNKEY`/`LAST_FUNKEY` = 1/20 (`deskins.h:16-17`).
Die Praemisse wird **nicht** zurueckgezogen.

## 2. Was gebaut wurde

| Datei | Funktion | EmuTOS |
|---|---|---|
| `kernel/src/desk/insapp.rs` | `APFUNKEY`-Zeile im Baum | `desk_rsc.c:1059-1063` |
| `kernel/src/desk/insapp.rs` | Vorbelegung aus `a_funkey` | `deskins.c:474-477` |
| `kernel/src/desk/insapp.rs` | `atow` | `deskins.c:380-393` |
| `kernel/src/desk/insapp.rs` | `get_funkey` + `STINVKEY`/`STDUPKEY` | `deskins.c:404-444`, `desk_rsc.c:2525-2527` |
| `kernel/src/desk/insapp.rs` | Wiederholung nach `-1` | `deskins.c:520-530` |
| `kernel/src/desk/inf.rs` | `install_application(…, funkey, …)` | `deskins.c:531` |
| `kernel/src/desk/inf.rs` | `Installed { …, funkey }` | `deskins.c:470-503` |
| `kernel/src/desk/inf.rs` | `funkey_owner` / `release_funkey` | `deskins.c:430-441` |
| `kernel/src/desk/main.rs` | `do_install_app` reicht durch | `deskins.c:721-756` |

Die Rundlauf-Haelften: EmuTOS fragt **innerhalb** seiner Schleife
(`fun_alert(1,STDUPKEY)`); NewTOS kann den `SESSION`-Mutex nicht ueber einen
Trap halten, also ist der Rundlauf in Test (`funkey_owner`) und Tat
(`release_funkey`) geteilt. Zwischen den beiden kann sich die Antwort nicht
aendern — der Desktop ist einkernig und modal.

## 3. Gemessen

```
[d20] get_funkey(fkey="7",  app="C:\CMDLINE.NCP", current=0, installed=false) -> Some(7)
[d20] install_application(app="C:\CMDLINE.NCP", ext="txt", full_path=false, funkey=7)
[d20] get_funkey(fkey="99", app="C:\CMDLINE.NCP", current=0, installed=false) -> None
[d20] get_funkey(fkey="7",  app="C:\HELLO2.NCP",  current=0, installed=false) -> Some(7)
```

* `screendumps/d20-funkey-a-insapp.png` — der Dialog mit der neuen Zeile
  `Install as:  F [__]` zwischen *Document type* und *Program type*, wie
  `ADINSAPP` sie hat.
* `screendumps/d20-funkey-d-conf.png` — *Desktop configuration* zeigt
  `F7  C:\CMDLINE.NCP______`. Diese Anzeige war vorher **immer** leer, weil
  keine Anwendung eine Taste tragen konnte.
* `screendumps/d20-invalid-b-alert.png` — `STINVKEY`, „Invalid function key
  value.", ein OK-Knopf.
* `screendumps/d20-invalid-c-retry.png` — danach ist das Feld geleert und der
  Dialog wartet (`deskins.c:522-529`).
* `screendumps/d20-dup-b-alert.png` — `STDUPKEY`, „Function key already
  assigned.|Do you want to reassign it?".
* `screendumps/d20-dup-c-conf.png` — nach OK steht `F7  C:\HELLO2.NCP`, die
  erste Anwendung hat ihn verloren.

**Ein eigener Defekt, waehrend der Messung gefunden und behoben:** das Feld
hatte zuerst `te_txtlen = 2` und nahm damit nur **eine** Ziffer — F10…F20
waeren unerreichbar gewesen. Sichtbar wurde es daran, dass `99` als `"9"`
ankam (`get_funkey -> Some(9)`, also gueltig statt `STINVKEY`). Jetzt
`FKEY_MAX + 1 = 3` wie `desk_rs_tedinfo[19]`; der Beleg ist
`[d20] get_funkey(fkey="99", …) -> None`. Alle Laeufe in
`measurement-2026-09-23-desk19-desk20/` stammen aus dem Stand **nach** dem
Fix.

## 4. Was DESK20 nicht baut

Ein Druck auf F7 startet weiterhin nichts. `process_funkey`
(`deskmain.c:1087-1106`) hat keine Entsprechung; `main.rs::check_function_key`
faellt weiterhin durch. Der Vermerk dort sagt das jetzt so, statt zu
behaupten, es koenne gar keine Anwendung eine Taste tragen (das war bis heute
wahr und ist es nicht mehr). Als Folgeticket gemeldet.

## 5. Benannte Abweichungen

* Die beiden Meldungen stehen woertlich als Ressourcentext im Modul
  (`[  OK  ]` / `[  OK  |Cancel]` wie in `desk_rsc.c:2525-2527`), nicht ueber
  `dir::fun_alert`, dessen Rahmen `[ OK ]` schreibt. Dieser Unterschied im
  Rahmen ist alt und bleibt unangetastet (DESK18s `STDEFDIR` geht weiter
  durch `fun_alert`).
* Der Dialog bleibt eine *faithful subset* von `ADINSAPP`: *Arguments*,
  *Boot status* und *Default dir* fehlen weiter (im Modulkopf benannt). Nur
  das Funktionstastenfeld ist dazugekommen.
* `get_funkey`s „unveraendert"-Abkuerzung (`installed && funkey == current`,
  `deskins.c:415-416`) ist uebernommen: ein von Hand in die `.INF`
  geschriebener Wert ausserhalb 1..20 ueberlebt damit das Bearbeiten der
  anderen Felder. Das ist EmuTOS' Verhalten, nicht unseres.

## 6. Bau + Regression (geteilter Baum)

```
cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
cargo xtask build --board rpi4        tos-api 64 / kernel 94   (Basis 64/94)
cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
cargo xtask build --board qemu-virt   tos-api 64 / kernel 94   (Basis 64/94)

cargo clippy --features board-rpi4       tos-api 80 / kernel 125 (Basis 80/125)
cargo clippy --features board-qemu-virt  tos-api 80 / kernel 125 (Basis 80/125)
Treffer in kernel/src/desk/: 0

cargo run -p xtask -- test   938 passed / 0 failed / 1 ignored
```
