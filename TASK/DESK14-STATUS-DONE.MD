# DESK14 — Quittung: **DONE** (Punkte 1-3; Punkt 4 zurueckgestellt, Hardwareabnahme offen)

Datum: 2026-09-23 · Zweig `desk-rebuild`, Basis HEAD `ce30fa2`
Die Aenderungen liegen **uncommitted** im gemeinsamen Arbeitsbaum. Nichts
geflasht, nichts ausgeworfen, kein Lauf an der echten Pi, nichts eingecheckt,
kein `git checkout/restore/clean/stash/reset` am gemeinsamen Baum.

---

## Was tatsaechlich getan wurde

### 1. Dateiauswahl (`fsel_exinput`) — gebaut

`kernel/src/desk/inf.rs`, `kernel/src/desk/mod.rs`

* **`inf::app_read_inf()`** ist neu und ist EmuTOS' `app_read_inf`
  (`references/emutos/desk/deskapp.c:1491-1546`): Vorgabe setzen, fragen,
  laden, pruefen, melden, `TRUE`/`FALSE` zurueckgeben. Der Fragteil ist
  `:1499-1509` Zeile fuer Zeile —
  `strcpy(path, INF_FILE_WILD); strcpy(fname, INF_FILE_NAME+3);
   fsel_exinput(path, fname, &button, desktop_str_addr(STRDINF));
   if ((rc == 0) || (button == 0)) return FALSE;
   strcpy(filename_start(path), fname);`
  mit `INF_FILE_WILD` = `C:\*.INF` (`sysconf.h:45`), `INF_FILE_NAME+3` =
  `NEWDESK.INF` und `STRDINF` = `Select .INF file` (`desk_rsc.c:2488`).
* **`mod::fsel_exinput`** ist die fehlende AES-Bindung: Opcode 91,
  `AES_CTRL_CODE(FSEL_EXINPUT, 0, 2, 3)` — null int-ins, zwei int-outs
  (`RET_CODE`, `FS_BUTTON`), drei addr-ins (`FS_IPATH`, `FS_ISEL`,
  `FS_TITLE`), genau wie `desk/gembind.c:702-712`. Sie folgt der Form der
  bestehenden Wrapper in derselben Datei. `docs/api/desktop/gembind.md`
  fuehrte `fsel_exinput` bis jetzt unter „noch nicht verdrahtet"; das ist
  erledigt.
* **`read_file(path)`** ist `util/gemdos.c:148-162`s `dos_load_file` fuer
  einen beliebigen Pfad und gibt den GEMDOS-Fehler des `Fopen` durch —
  genau die eine Unterscheidung, die `app_read_inf` braucht.
  `read_inf_file()` (Bootpfad, Selftests) ist jetzt `read_file(INF_PATH).ok()`
  und verhaelt sich unveraendert.

### 2. Signaturpruefung `#R`/`#E` — gebaut

`kernel/src/desk/inf.rs`

* **`is_inf_signature`** verlangt `'#'` **und** `'R'` oder `'E'`
  (`deskapp.c:1523-1525`). Vorher pruefte der Lesepfad nur das `'#'`.
* **Der Bootpfad bleibt, wie er ist.** `load_and_apply` testet weiter nur
  `'#'` (`deskapp.c:833`, `if (inf_data[0] != '#') build_inf(...)`). Das ist
  kein Versehen und steht jetzt als Begruendung an der Stelle: die beiden
  Regeln sind beide EmuTOS', und sie sind nicht austauschbar — beim Start
  liegt dort, was `shel_get` geliefert hat, beim Lesen eine Datei, die der
  Benutzer gerade im Auswaehler angeklickt hat.
* **`apply_inf_bytes`** ist der gemeinsame Rumpf beider Pfade (parsen,
  anwenden, in `SESSION` legen), damit es nur eine Anwendung gibt.

### 3. `STFILENF` / `STINVINF` — gebaut

`kernel/src/desk/inf.rs`

* `Fopen` → `EFILNF` (`-33`) ist EmuTOS' einziger Sonderfall
  (`deskapp.c:1536-1538`): `STFILENF` = `[1][%s not found.][  OK  ]`
  (`desk_rsc.c:2548`), Name eingesetzt wie `fun_alert_merge`
  (`deskfun.c:88`).
* Jeder andere Fehler, eine leere Datei und eine falsche Signatur landen in
  EmuTOS' `rc = 0L`-Zweig (`:1540-1542`): `STINVINF` =
  `[1][Illegal .INF file format.][  OK  ]` (`desk_rsc.c:2560`).
* Beides ueber `dir::fun_alert`, also ueber den AES-`form_alert` — kein
  eigener Kasten.

### 4. Punkt 4 — nicht gebaut, wie entschieden

`kernel/src/desk/main.rs::do_read_inf` traegt den Verweis, den das Ticket
verlangt, und sonst nichts: EmuTOS' `restart_desktop = TRUE; done = TRUE;`
(`deskmain.c:919-925`) und der Abbau in `:2136-2148`, daneben der Grund,
warum NewTOS das nicht kann (`global::init` ist idempotent, kein
`win_start()` beim Wiedereintritt, `wm_init()` wuerde die AES-Fenstertabelle
leeren, die Bruecke `cnx_put(); app_save(FALSE)` → `shel_get`,
`deskmain.c:2152-2153`, fehlt) und der Zeiger auf den Umzug in den
Benutzerbereich. **Es wurde nichts um sein Fehlen herumgebaut.**

---

## Geaenderte Dateien

| Datei | Was |
|---|---|
| `kernel/src/desk/inf.rs` | `read_file`, `apply_inf_bytes`, `is_inf_signature`, `app_read_inf`, `INF_FILE_WILD`/`STRDINF`/`FSEL_*`, `cbuf_str`/`set_cbuf`/`filename_start`; `load_and_apply` bekommt die Begruendung fuer die schwaechere Bootregel |
| `kernel/src/desk/main.rs` | `do_read_inf` ruft `app_read_inf` statt `load_and_apply` und traegt die benannte Abweichung zu Punkt 4 |
| `kernel/src/desk/mod.rs` | `OP_FSEL_EXINPUT` + `fsel_exinput`-Wrapper |
| `docs/api/desktop/deskapp.md` | NewTOS-Notizen zu `app_read_inf()` |
| `docs/api/desktop/deskmain.md` | „Read .INF"-Absatz und die `CONF_WITH_READ_INF`-Zeile der Gating-Tabelle |
| `docs/api/desktop/gembind.md` | `fsel_exinput` ist verdrahtet (Bindungstabelle, Puffergroessen) |
| `docs/api/desktop/_hw-test-2026-09-17.md` | HW-Punkt 41 |
| `docs/STATUS.md` | Phase-14-Zeile + Eintrag unter *Recent Decisions* |

---

## Benannte Abweichungen

1. **Kein Desktop-Neustart** — `main::do_read_inf`, `deskmain.c:919-925` und
   `:2136-2148`. Lead-Entscheidung 2026-09-23 (`TASK/DESK14.md`).
2. **`INF_FILE_WILD` mit festem `C:`** — EmuTOS setzt den Buchstaben zur
   Laufzeit (`*path += G.g_stdrv`, `deskapp.c:1502`), weil auch sein
   `INF_FILE_NAME` so gesetzt wird (`:686`, `:1050`). NewTOS' `INF_PATH` ist
   fest `C:\NEWDESK.INF`, also ist der Auswaehler-Pfad es ebenso; die
   EmuTOS-Invariante „Auswaehler-Laufwerk = INF-Laufwerk" bleibt erhalten.
3. **Puffergroessen** — `FSEL_PATH_LEN` 128 / `FSEL_NAME_LEN` 32 statt
   `MAXPATHLEN` 81 / `LEN_ZFNAME` 13, weil unser AES seine eigenen 96- und
   16-Byte-Felder zurueckschreibt (`tos-api/src/aes/fsel.rs:135-136`).
   Nichts davon ist beobachtbar.
4. **Lesegrenze** — `read_file` liest bis `MAX_INF_BYTES` (64 KiB), EmuTOS bis
   `SIZE_SHELBUF-CPDATA_LEN-1` (`sysconf.h:74`). Die EmuTOS-Grenze ist die des
   Shell-Puffers, in den es die Datei legt; ohne Punkt 4 gibt es den hier
   nicht. Eine `.INF` dieser Laenge haette EmuTOS halb geparst und uns ganz.

---

## Pruefung

* Hostsuite `cargo run -p xtask -- test`: **938 passed / 0 failed / 1 ignored**
  (Basislinie, Delta 0).
* Frischer Bau je Board, `cargo clean -p kernel -p tos-api --target
  aarch64-unknown-none-softfloat` vor **jedem**: `--board rpi4` und
  `--board qemu-virt`, beide **tos-api 64 / kernel 94** (Delta 0).
* `cargo clippy --features board-rpi4` und `--features board-qemu-virt`:
  **80 / 125** auf beiden (Delta 0), **null** Treffer in `kernel/src/desk/`.
* Vier QEMU-raspi4b-Laeufe (640x480, `-display none`, QMP, Injektor),
  22 Screendumps, 0 `[EXC]`/`[PANIC]`, `fsck_msdos -n` auf allen vier
  Kartenabbildern Phase 1/2/3 sauber.
  Belege: `measurement-2026-09-23-desk14/`.

Gefahren wurden alle drei Ausgaenge: eine gueltige `.INF` wird gelesen und
angewendet (roter Hintergrund, Symbole an den Plaetzen der Datei), eine
Datei, die keine `.INF` ist, bringt `STINVINF` (zweimal: `#Z`-Datei und der
21-Byte-Platzhalter der Standardkarte), ein Name ohne Datei bringt
`STFILENF`. Der Auswaehler selbst ist in vier Bildern zu sehen.

**Offen:** Pi-Sichtabnahme, HW-Punkt 41 in
`docs/api/desktop/_hw-test-2026-09-17.md`.

---

## Als eigenes Ticket gemeldet

* **Der Neustartpfad selbst** (Punkt 4) — gehoert zum Umzug des Desktops in
  den Benutzerbereich, nicht hierher.
* **`app_read_inf` schreibt nicht in den Shell-Puffer.** EmuTOS' `shel_get` /
  `shel_put` (`deskapp.c:1532-1533`) legen die gelesene Datei dort ab, wo
  `app_start` sie beim naechsten Start wiederfindet; NewTOS hat weder diese
  beiden AES-Bindungen im Desktop noch den Puffer. Solange Punkt 4 liegt, ist
  das folgenlos — mit dem Neustartpfad wird es die erste Voraussetzung.
* **Kosmetik in `inf.rs`:** der Doc-Kommentar oberhalb von `reset()` ist mit
  dem von `load_and_apply` verschmolzen, sodass `load_and_apply` gar keinen
  traegt. Unberuehrt gelassen (ausserhalb des Auftrags).
* **Die Nummerierung in `_hw-test-2026-09-17.md`** hat schon zwei Abschnitte
  „38". Unberuehrt gelassen; der neue ist 41.
