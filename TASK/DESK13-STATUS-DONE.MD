# DESK13 — Quittung: **DONE** (Hardwareabnahme offen)

Datum: 2026-09-23 · Zweig `desk-rebuild`, Basis HEAD `f4b8ff5`
Die Aenderungen liegen **uncommitted** im gemeinsamen Arbeitsbaum. Nichts
geflasht, nichts ausgeworfen, kein Lauf an der echten Pi, nichts eingecheckt,
kein `git checkout/restore/clean/stash/reset` am gemeinsamen Baum.

---

## Was tatsaechlich getan wurde

### 1. Kopier- und Ueberschreibruecksprache — gebaut

`kernel/src/desk/dir.rs`

* **`ADCPYDEL`** steht in `dir_op`, wie in EmuTOS
  (`references/emutos/desk/deskdir.c:911-918,951-965,1051-1056,1075-1080`).
  Er geht fuer jede Operation ausser `OP_COUNT` hoch (EmuTOS' `tree = NULL`
  fuer `OP_COUNT` und die `if (tree)` danach), traegt den Titel
  `COPY`/`MOVE`/`DELETE FILE(S)` (`desk_rsc.c:2472-2474`), zeigt die Datei-
  und Ordnerzahl des vorangegangenen Zaehldurchlaufs in zwei
  rechtsbuendigen `G_FBOXTEXT`-Feldern und zaehlt sie waehrend des Laufs
  herunter (`inf_numset` + `draw_fld`, `:592,676,1053`). Ob er auf `OK`
  *wartet*, entscheidet `confirm` (`:906,939-945`): `g_cdelepref` beim
  Loeschen, `g_ccopypref` beim Kopieren, beide ODER-verknuepft beim
  Verschieben/Umbenennen.
* **`ADCPALER`** steht in `output_fname` (`deskdir.c:290-397`), aufgerufen aus
  `d_dofcopy` nach dem Oeffnen der Quelle und vor dem Anlegen des Ziels
  (`:417-424`). `do_namecon` (`:93-130`) zeichnet ihn ueber den schon
  stehenden Kopierkasten (`ml_havebox` → `draw_dial` statt `start_dialog`),
  laesst `form_do` laufen, nimmt ihn mit `form_dial(FMD_FINISH)` wieder weg
  und zeichnet den Kopierkasten darunter neu. `Stop` bricht ganz ab, `Skip`
  ueberspringt diese Datei, `OK` kopiert; der Zielname ist aenderbar und eine
  Aenderung startet die Pruefschleife von vorn (`:362-370`).
* **`More`** ist EmuTOS' `more`-Tristatus (`:232-236,296-298`) als
  Aufzaehlung, mit einer vierten Auspraegung `Failed` fuer den E/A-Fehler,
  den NewTOS sammelt statt zu erfragen (`:396-414`; die Abweichung steht seit
  Task #9 im Modulkopf).
* **Entfernt:** die beiden NewTOS-Ersatzalerts in `fun_del` und
  `fun_file2desk` samt `confirm_deletes()`. Beide zitierten `deskdir.c:906` —
  die Stelle, die jetzt implementiert ist. Eine Loeschung wird wieder genau
  einmal bestaetigt. Der `wants_to_delete_files`-Vorbau des
  Schreibtisch-Symbols (`STRMVDEL`, `deskfun.c:1433`) ist eine andere Frage
  und bleibt.
* **Die drei Felder sind zurueck in `inf::inf_pref`** — sechs Gruppen in
  `ADSETPRE`s Reihenfolge und unter seinen Objektnummern
  (`desk_rsc.h:164-184`). DESK02s Behelf (Vorbelegung ueber `rendered()`)
  faellt weg, weil `DATEFORM_IDT`/`TIMEFORM_IDT` wieder einen Knopf haben:
  Vorbelegung und Rueckgabe lesen den **rohen** Wert, wie EmuTOS
  (`deskinf.c:754-778,797-814`).
* `CSave`/`DeskState` tragen `cs_confcpy`/`g_ccopypref` und
  `cs_confovwr`/`g_covwrpref`; `#E`-Bits `INF_E1_CONFCPY` (0x08) und
  `INF_E2_ALLOWOVW` (0x10, **invertiert** — `deskapp.c:98`).

### 2. `Supexec` — Befund, kein Code

XBIOS 0x26 bleibt ungebaut. Begruendung mit Quelle:

* EmuTOS ruft `Supexec((LONG)get_idt_cookie)` (`desksupp.c:219`) nicht, weil
  der Keks Privileg braucht, sondern weil die Dose auf m68k nur ueber den
  Zeiger bei `$5A0` im Supervisor-Speicher erreichbar ist. Das ist ein
  m68k-Mechanismus; `docs/COMPATIBILITY.md` stellt Mechanismen frei.
* `docs/api/xbios/26-supexec.md`: „NewTOS status: **decision deferred**
  (needs ADR-Supexec)" — AArch64 hat keinen Privilegwechsel im Prozess; die
  Entscheidung (EL1-Rueckruf / Pseudo-Supervisor / Mustererkennung) ist
  offen. Sie fuer *einen* Keksleser zu treffen waere ein eigener Auftrag.
* `docs/api/bios/_cookie-jar.md` §Status 2026-09-14 benennt den Ersatz
  selbst: „There is no `Supexec` in the kernel … the low-memory area an app
  wants is already in its own address space", und ADR-012 macht FreeMiNT
  `Ssystem` zum Zugriffsweg (`docs/api/gemdos/154-ssystem.md`).

Der `Default`-Knopf braucht also keinen anderen Weg als den, den der Desktop
ohnehin nimmt: neue Bindung `dosbind::dos_getcookie` =
`Ssystem(S_GETCOOKIE, tag, 0)` ueber `svc #1`. Das dritte Argument ist 0,
damit der Wert in `x0` zurueckkommt — die Zeigerform schreibt mit einem
unprivilegierten `sttr` und faellt auf einer Kernadresse; die Folge (ein Keks
mit gesetztem Bit 31 ist nicht vom Fehlerwert zu unterscheiden) steht an der
Bindung. `G.g_idt` (`deskbind.h:183`) wird in `do_wredraw` aufgefrischt, wo
EmuTOS es auffrischt, und zusaetzlich einmal beim Desktopstart, weil NewTOS
die Textzeile beim **Bau der Ansicht** formatiert und nicht beim Zeichnen.

### 3. + 4. Datumstrenner und `_IDT` — eine Entscheidung

Der `sep`-Parameter von `fmt_date` ist geloescht. EmuTOS faengt bei
`DATEFORM_SEP` (`'/'`, `deskbind.h:64`) an und ueberschreibt ihn nur im
`DATEFORM_IDT`-Zweig aus dem Keks (`deskinf.c:129,141-143`); beide
Aufrufstellen uebergeben nichts. Die Regel steht weiter an genau einer Stelle
(`model::DateFmt::rendered` / `TimeFmt::rendered`), liefert jetzt aber
Reihenfolge **und** Trenner und deckt alle vier `_IDT`-Ordnungen ab
(`DateOrder`, `biosdefs.h:168-171`, EmuTOS' zweites `switch`
`deskinf.c:154-176`); `push_dec02` gibt den drei Feldern `%02d`-Semantik, weil
EmuTOS' `sprintf` eine vierstellige Jahreszahl nicht abschneidet.

Mit `_IDT = 0x112F` (24H, DMY, `/`) und EmuTOS' Voreinstellung ohne `.INF`
(`INF_E2_DEFAULT` traegt beide `IDT`-Bits) druckt die Textansicht `23/09/26`
und der Show-Info-Kasten `23/09/2026`. Vorher: `09-23-26` und `09/23/2026` —
zwei Antworten auf dieselbe Frage.

### 5. `check_alt_letter_key` — Behauptung widerlegt

Der Vermerk im Quelltext — und seine Kopien in `TASK/DESK11-STATUS-DONE.MD`,
`docs/api/desktop/_hw-test-2026-09-17.md` und (schon korrekt, aber ohne Beleg)
`_hw-test-checklist.md` — sagte, die Tastaturschicht falte
Alt zu AltGr und liefere nie ASCII 0 fuer einen Buchstaben. Sie tut das
Gegenteil:

* `kernel/src/keymap.rs` `to_ascii` nullt ein alphabetisches Ergebnis,
  sobald `alt` gesetzt ist;
* `ikbd::to_ascii` macht daraus `None`, `ikbd::decode_key` legt das Wort als
  `scancode << 8 | 0` in die Schlange;
* beide Quellen setzen das Alt-Bit: die IKBD-Strecke ueber `SC_ALT = 0x38`,
  der USB-HID-Bericht ueber `modifiers()`, das **LAlt und RAlt** auf `0x38`
  abbildet;
* der AES reicht das ganze Wort unveraendert als `EV_KRET` durch
  (`tos-api/src/aes/event.rs:598`).

Gefahren, nicht behauptet: `Alt`+`C` in QEMU raspi4b oeffnet `C:\` —
`screendumps/b-altc.png`. Alle vier Stellen sind richtiggestellt bzw. mit dem
Beleg versehen.

---

## Ein Befund beim Hinsehen

Die drei Knoepfe des `ADCPALER` **ueberlappten sich um 23 px**: der erste Wurf
hatte die 90-px-Breite des Zwei-Knopf-Kastens uebernommen und den mittleren
zentriert (24..114 / 91..181 / 158..248 in einem 272 px breiten Kasten). Im
ersten Messlauf sichtbar. EmuTOS setzt drei 10 Zeichen breite Knoepfe in den
Spalten 2, 14 und 26 eines 38 Zeichen breiten Kastens
(`desk_rsc.c:1263-1279`); die Kastenbreite kommt jetzt aus der Knopfreihe.
Nachgemessen: `CAOK=(184,287) CASKIP=(280,287) CASTOP=(376,287)`.

---

## Beruehrte Dateien

| Datei | Was |
|---|---|
| `kernel/src/desk/dir.rs` | `ADCPYDEL` + `ADCPALER`, `More`, `OpCtx`, `output_fname`, `do_namecon`, `item_exists`, `fmt_str`/`unfmt_str`, `inf_numset`; die zwei Ersatzalerts entfernt |
| `kernel/src/desk/inf.rs` | `inf_pref` mit allen sechs Gruppen; `fmt_date`/`fmt_time` ohne `sep`, mit `idt`; `push_dec02`; `#E`-Bits `CONFCPY`/`ALLOWOVW`; `mk`/`inf_gindex` `pub(super)` |
| `kernel/src/desk/model.rs` | `_IDT`-Bitlayout, `DateOrder`, `DATEFORM_SEP`, `rendered(idt)` fuer beide Formate |
| `kernel/src/desk/global.rs` | `g_ccopypref`, `g_covwrpref`, `g_idt`; `CSave::cs_confcpy`/`cs_confovwr` |
| `kernel/src/desk/dosbind.rs` | `dos_getcookie` (`Ssystem` 0x154) + die Begruendung |
| `kernel/src/desk/redraw.rs` | `refresh_idt` in `do_wredraw`, wie `desksupp.c:219` |
| `kernel/src/desk/mod.rs` | `refresh_idt` einmal beim Desktopstart |
| `kernel/src/desk/win.rs` | `format_fnode` reicht `idt` durch, kein Bindestrich mehr |
| `kernel/src/desk/main.rs` | `check_alt_letter_key`: der falsche Vermerk ersetzt |
| `tos-api/tests/gemdos_ssystem_idt_cookie.rs` | **neu** — `_IDT` durch den echten GEMDOS-Dispatcher |
| `tos-api/tests/aes_desk_dialog_exit_buttons.rs` | **neu** — die Zustandsablesung beider Kaesten |
| `docs/api/desktop/deskdir.md` | NewTOS-Notizen: beide Kaesten, `More`, benannte Abweichungen |
| `docs/api/desktop/deskinf.md` | NewTOS-Notizen: die drei Felder, `Ssystem` statt `Supexec`, der Trenner |
| `docs/api/desktop/gembind.md` | `dos_getcookie` in der Bindungstabelle + warum GEMDOS und nicht XBIOS |
| `docs/api/desktop/_hw-test-2026-09-17.md` | HW-Punkt 42; der falsche Alt-Vermerk richtiggestellt |
| `docs/api/desktop/_hw-test-checklist.md` | B12 mit dem QEMU-Beleg |
| `docs/STATUS.md` | Phase-14-Zeile + datierter Eintrag in *Recent Decisions* |
| `TASK/DESK11-STATUS-DONE.MD` | Berichtigungskasten zum widerlegten Alt-Vermerk |
| `TASK/DESK13.md` | Status + `## Abschluss` |

---

## Pruefung, die wirklich lief

| Pruefung | Ergebnis |
|---|---|
| `cargo run -p xtask -- test` | **938 passed / 0 failed / 1 ignored** (Basis 934 + 4 neue) |
| `cargo xtask build --board rpi4` (frisch nach `cargo clean -p kernel -p tos-api`) | `tos-api 64 / kernel 94` — **Delta 0** |
| `cargo xtask build --board qemu-virt` (frisch) | `tos-api 64 / kernel 94` — **Delta 0** |
| `cargo clippy --features board-rpi4` | `80 / 125` — **Delta 0**, 0 Treffer in `kernel/src/desk/` |
| `cargo clippy --features board-qemu-virt` | `80 / 125` — **Delta 0**, 0 Treffer in `kernel/src/desk/` |
| QEMU raspi4b, 7 Laeufe, 24 Screendumps | 0 `[EXC]`/`[PANIC]` in allen sieben Protokollen |
| `fsck_msdos -n` auf allen sieben Kartenabbildern | sauber |

### Was am Brett gefahren wurde

| Lauf | Was | Beleg |
|---|---|---|
| a | `Alt`+`C` oeffnet `C:\`; Textansicht zeigt `23/09/26 18:55` | `b-altc.png`, `c-text.png` |
| b | `Set preferences` mit allen sechs Gruppen; `Cancel` rueckstandsfrei | `b-prefs.png`, `b-closed.png` |
| c | Kopierkasten `COPY FILE(S)` (1 Datei, 0 Ordner): `Cancel` → nichts kopiert, `OK` → `C:\ONE.TXT` entsteht | `c-copybox.png`, `c-cancelled.png`, `c-copied.png` + Abbild |
| d | Namenskonflikt: `Skip` → Zieldatei bleibt 21 Byte, `OK` → 51 Byte (ueberschrieben) | `d-conflict.png`, `d-skipped.png`, `d-conflict2.png`, `d-overwritten.png` + Abbild |
| e | Show info zeigt `Date: 23/09/2026  18:55` — derselbe Trenner wie die Textansicht | `e-showinfo.png` |
| f | `File → Delete...` geht jetzt durch denselben Kasten (`DELETE FILE(S)`): `Cancel` → Datei bleibt, `OK` → Datei weg | `f-delbox.png`, `f-cancelled.png`, `f-deleted.png` + Abbild |
| p | Probelauf fuer die Symbolpositionen | `p-subdir.png` |

Der Nachweis „kopiert / nicht kopiert" haengt **nicht** am Bild: das
Kartenabbild wird nach dem Lauf mit `method/lsimg.py` gelesen (reiner
FAT16-Leser, mountet nichts). Ausserdem im Serienprotokoll:
`output_fname(...) -> Skip` bzw. `-> Ok`.

Belege: `measurements/measurement-2026-09-23-desk13/` (`RUNINFO.TXT`, `method/`,
`screendumps/`, `serial/`, `manifest.sha256`; keine `*.img`).

---

## Was offen bleibt

* **Pi-Sichtabnahme**: HW-Punkt 42 in
  `docs/api/desktop/_hw-test-2026-09-17.md` — 1920x1080-Optik beider Kaesten,
  ein zweites Laufwerk als Kopierziel, eine Mehrfachauswahl fuer `Stop`, ein
  getippter neuer Zielname, und `Alt`+Buchstabe auf echter Tastatur.
* **Nicht gefahren, weil der Injektor nicht in Felder tippt:** den Zielnamen
  im Namenskonflikt aendern und mit `OK` bestaetigen (EmuTOS'
  „von-vorn"-Schleife, `deskdir.c:362-370`). Der Code ist da, der Beweis
  fehlt.
* **`desk::dir::selftest`** faehrt den Bestaetigungskasten jetzt mit. Er haengt
  in keinem Bootpfad (Notiz *unwired-selftests-rot-silently*), also ist nichts
  kaputt; wer ihn verdrahtet, braucht den AES oben.

## Neue Tickets, die aus diesem Auftrag fallen

1. **Ordner-Namenskonflikt.** `output_path`/`get_new_name`
   (`deskdir.c:700-779`) fragen nach einem neuen **Ordner**namen; NewTOS
   mischt weiter in einen bestehenden Ordner hinein.
2. **`STDELDIS`.** Die Zusatzfrage beim Loeschen einer ganzen Platte
   (`deskdir.c:971-975`) haengt an `icontype == AT_ISDISK`, das `dir_op` bei
   uns nicht bekommt.
