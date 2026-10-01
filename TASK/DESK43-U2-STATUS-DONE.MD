# DESK43 / U2 — Der Zustandskanal `app_start`/`app_save` über `shel_get`/`shel_put`

Status: **FERTIG, mit einem Befund gegen den Plan** · 2026-09-26 · Basis `814de2c`
Belege: `measurements/measurement-2026-09-26-desk43-u2/`
Plan: `docs/analysis/desktop-userspace-plan-v2.md` §5 U2
Spec: `docs/api/aes/shell-loop.md` §10 · `docs/api/aes/scrap-shell.md` §2 ·
`docs/api/desktop/deskapp.md` („`app_start()` — full AES/GEMDOS call sequence")

Die Änderung liegt **unversioniert** im Baum (`git status`: vier Produktdateien).

---

## 1. Die drei Zahlen

| | |
|---|---|
| **Driftprobe** | **47** (A 21 · B 21 · C 5) — unverändert gegenüber U1 |
| Bauen, **beide Boards**, je frisch nach `touch kernel/src/lib.rs` | tos-api **64** / kernel **94** Warnungen — Basis 64/94, **Delta 0** |
| clippy, **beide Feature-Sätze** | tos-api **80** / kernel **125** — Basis 80/125, **Delta 0**; in `kernel/src/desk/` **null** clippy-Treffer |
| `cargo run -p xtask -- test` | **956 / 0 / 1** — Basis 956/0/1 |

**Warum die Driftprobe nicht fällt.** Sie zählt `tos_api::`- und `crate::`-Pfade
in `kernel/src/desk/`. U2 *entfernt* nichts davon: es baut den Ersatz, den U3
braucht, um entfernen zu dürfen (Plan §6.3, Maßnahme 8). Die beiden neuen
Bindungen laufen über `Aes::call` → `svc #2` und nennen weder `tos_api::` noch
`crate::`. Eine Einheit, die die Zahl senkt, ist U3/U4 — nicht diese.

**Keine Umgehung stehen gelassen.** Im Gegenteil: eine ist verschwunden. Der
Kommentar in `desktop_relinquish` („`app_save(FALSE)` … hat keine Entsprechung:
NewTOS' `NEWDESK.INF` geht direkt auf die Datei") war eine schriftlich
begründete Abweichung im Sinne von Maßnahme 9 — der Satz ist gestrichen und die
Anweisung `deskmain.c:2155` steht jetzt an ihrer Stelle.

---

## 2. Was geändert wurde

### `kernel/src/desk/mod.rs` — die zwei Bindungen

* `OP_SHEL_GET = 122`, `OP_SHEL_PUT = 123`.
* **`shel_get(&mut [u8]) -> i16`** — `desk/gembind.c:918-923`
  (`AES_CTRL_CODE(SHEL_GET, 1, 1, 1)`), Serverseite `aes/gemshlib.c:203-206`.
* **`shel_put(&[u8]) -> i16`** — `desk/gembind.c:926-931`
  (`AES_CTRL_CODE(SHEL_PUT, 1, 1, 1)`), Serverseite `aes/gemshlib.c:213-216`.

Beide nehmen einen Slice statt Zeiger+Länge (Hausstil, vgl. `fsel_exinput`); die
Länge auf dem Draht ist die Slicelänge, auf `i16::MAX` begrenzt, weil EmuTOS'
`len` ein `WORD` ist. Beide Aufrufer bleiben unter `SIZE_SHELBUF`.

**`SHELLBUF = 4192` geprüft, nicht angenommen:** `tos-api/src/aes/shell.rs:37`
gegen `references/emutos/include/sysconf.h:74`
(`#define SIZE_SHELBUF 4192L`) — gleich.

### `kernel/src/desk/inf.rs` — `app_start`s Reihenfolge und `app_save(todisk)`

Neue Konstanten, jede mit Quelle: `SIZE_SHELBUF` (`sysconf.h:74`),
`CPDATA_LEN = 128` (`deskapp.c:136`, mit dem Kommentar `:131-135` als
Begründung), `MAX_INF_TEXT = SIZE_SHELBUF-CPDATA_LEN-1` (`deskapp.c:688`).

* **`load_and_apply()`** ist jetzt die Erwerbshälfte von `app_start`
  (`deskapp.c:808-841`) in EmuTOS' Reihenfolge:
  `shel_get(buf, SIZE_SHELBUF)` (`:832`) → `inf_data = buf + CPDATA_LEN`
  (`:833`) → nur bei ungültiger Signatur `read_inf_file` (`:836`) → immer noch
  ungültig ⇒ `false`, worauf der Aufrufer `build_inf` fährt (`:839-840`).
  Die Signaturregel (`'#'` allein, nicht `#R`/`#E`) bleibt wie sie war, samt
  ihrer Begründung.
* **`inf_text()`** (neu) schneidet am ersten NUL ab — `infbuf` ist in EmuTOS ein
  C-String und `app_start`s Schleife endet dort (`deskapp.c:845`,
  `while (*pcurr)`). Ohne das läse unser längenbasierter Parser die Reste des
  vorherigen `shel_put` mit.
* **`read_inf_file()`** begrenzt auf `MAX_INF_TEXT` — `deskapp.c:688` liest in
  denselben Puffer, den es jetzt auch bei uns gibt. (Die frühere Begründung, die
  Grenze sei „eine Eigenschaft des Mechanismus, den wir nicht haben", ist damit
  erledigt; `docs/api/desktop/deskapp.md:588-597` beschreibt sie für den
  *Read-.INF*-Pfad, der unverändert `MAX_INF_BYTES` liest.)
* **`app_save(todisk: bool)`** (vorher `save_desktop()`, jetzt EmuTOS' Name),
  `deskapp.c:1106-1299`: `shel_get(outbuf, CPDATA_LEN)` (`:1123`) →
  Text ab `outbuf+CPDATA_LEN` (`:1124`) → `*pcurr = 0` (`:1285`) →
  `shel_put(outbuf, len+1)` (`:1293`) → nur bei `todisk`
  `save_to_disk(inf_data, len-CPDATA_LEN)` (`:1297`). Der Rückgabewert (ob die
  Datei geschrieben wurde) ist NewTOS' Zusatz für den `SAVEITEM`-Alert und
  bleibt; `app_save(false)` gibt immer `true`.
* **`InfData::serialize()` → `serialize_into(&mut Vec<u8>)`**: hängt an EmuTOS'
  `outbuf` an, damit `out.len()` **`pcurr - outbuf`** ist — der Ursprung, von
  dem aus die Überlaufprüfung der `ANODE`-Schleife misst. Diese Prüfung
  (`deskapp.c:1276-1282`) ist neu und jetzt real: sie wirft die Zeile, die
  `SIZE_SHELBUF` reißt, **ganz** weg und bricht ab, genau wie EmuTOS
  zurückspult. Vorher gab es sie nicht, weil nur auf Platte geschrieben wurde.

### `kernel/src/desk/main.rs` — die Anweisung an ihrer Stelle

`desktop_relinquish()` ruft nach `cnx_put()` jetzt `inf::app_save(false)` —
`deskmain.c:2154-2155`, in dieser Reihenfolge.

### `kernel/src/desk/global.rs`

Nur drei Doc-Verweise `[`super::inf::save_desktop`]` → `[`super::inf::app_save`]`.

### Jede berührte `fn` mit/ohne EmuTOS-Zitat (Maßnahme §6.4/3)

Mit Zitat: `shel_get`, `shel_put`, `load_and_apply`, `inf_text`,
`read_inf_file`, `app_save`, `InfData::serialize_into`, `desktop_relinquish`,
`do_save_desktop`.
**Ohne** Zitat — und das ist die vollständige Liste: `inf::selftest` und
`inf::selftest_install`. Beides sind NewTOS-Eigenprüfungen ohne EmuTOS-Vorbild;
berührt wurden dort nur die Aufrufstellen (`save_desktop` → `app_save(true)`,
`serialize` → `serialize_into`).

---

## 3. Die vier Beweise des Plans

**(1) „Der neue Desktop kommt mit denselben Fenstern aus dem Shell-Puffer,
nicht aus der Datei" — in der Form des Plans NICHT erfüllbar, siehe §4. Ersetzt
durch zwei Läufe, die dasselbe härter zeigen.**

*Lesen:* `run3-file` und `run4-buffer` fahren **dasselbe Abbild** mit derselben
`C:\NEWDESK.INF` (`#W … C:\ACC\@`). Einziger Unterschied: bei `run4` liegt im
Shell-Puffer `CP_SHELL_INIT` (`#a000000\r\n`, `aes/geminit.c:97`) + ein INF mit
`#W … C:\AUTO\@`. Ergebnis: `run3` öffnet `C:\ACC\`, `run4` öffnet
**`C:\AUTO\`** (`screendumps/run3-file-desktop.png` vs.
`run4-buffer-desktop.png`). Der Puffer schlägt die Datei — und der Versatz
stimmt, denn das Präfix beginnt mit `#`: bei Offset 0 hätte die Signaturprüfung
auf den Steuerfelddaten angeschlagen.

*Schreiben, auf dem echten Pfad:* `run7-launch` nullt den Puffer
(`buffer wiped: all zero = true`), ruft dann das unveränderte
`main::run_gem_app("C:\ARGVTEST.NAP")` — also `desktop_relinquish` → Programm →
`desktop_resume`. Das Programm läuft (`ARGVTEST: argc 1`) und endet; danach
steht der INF-Text mit `#W 00 00 02 04 26 0C 00 C:\AUTO\*.*@` wieder im Puffer.
Der Desktop kommt vollständig zurück (`run7-after-launch.png`).

**(2) Kaltstart ohne `NEWDESK.INF`** — `run5-noinf` (Datei gelöscht) ist
**byteweise identisch** mit `run6-placeholder`: unveränderte Standardumgebung
(`deskapp.c:839-840`).

**(3) *Save desktop* schreibt weiter korrekt, `fsck_msdos -n` sauber** —
`app_save(TRUE)` schrieb 627 bzw. 635 Byte; `inf/newdesk-after-run4.inf` trägt
die Fensterzeile und **kein** Präfix. `build/fsck-after-app_save.txt`: Phase 1–3
ohne Befund, rc 0.

**(4) Beide Boards bauen; Boot-Screendump ohne Regression** — Zahlen oben; die
Bilder `final-placeholder` gegen `head-placeholder` und `final-file` gegen
`head-file` haben je **0 abweichende Bytes**. Der Referenzkernel wurde in einem
abgetrennten `git worktree` auf `814de2c` gebaut und danach entfernt; der
geteilte Baum wurde nie angefasst.

---

## 4. Der Befund gegen den Plan — Beweispunkt 1 ist EmuTOS-widrig

Der Plan (§2.4, §5 U2) nimmt an, der Shell-Puffer überlebe einen A106-Neustart.
**Er tut es nicht, und er darf es nicht.**

`aes_restart` nullt vor jedem neuen `gem_main` den ganzen `THEGLO`-Bereich —
`jsr _size_theglo` / `_bzero_nobuiltin` auf `_D` (`aes/gemstart.S:86,99-106`),
`size_theglo() = sizeof(THEGLO)` (`aes/geminit.c:149-152`) — und `g_shelbuf`
ist ein `THEGLO`-Mitglied (`aes/gemlib.h:129`). EmuTOS' Desktop findet nach
einem Auflösungswechsel also einen leeren Puffer und liest die **Datei**.

Unser Baum macht das bereits richtig, und zwar begründet: `aes::reinit()` ruft
`shell::reinit()` (`tos-api/src/aes/mod.rs:186`), das `(*S.buf.get()).fill(0)`
ausführt (`tos-api/src/aes/shell.rs:801`) — mit `gemstart.S:101-110` im
Doc-Kommentar. Diese Zeile ist **nicht** angefasst worden.

Der Kanal trägt in EmuTOS über den Tod des *Desktop-Prozesses innerhalb einer
Sitzung* (Anwendungsstart: `deskmain.c:2155` schreibt, der wiedereintretende
`app_start` liest, `:832`) — nicht über den Neustart der Sitzung. Lauf 7 fährt
genau diesen Pfad und ist damit der sachlich richtige Beweispunkt 1.

**Die Vorhersage des Plans** („erster Bruch ist das `CPDATA`-Präfix; zweiter
Kandidat die `#W`-Zeilen aus `cnx_put`") **hat nicht gehalten** — beide Stellen
waren von Anfang an gegen die Quelle gelegt. Das Präfix wird bei 128 respektiert
und unverändert durchgereicht (`prefix preserved = true`); die `#W`-Zeilen
stehen im Text, weil `build_from_state` ohnehin aus `g_cnxsave` liest
(`deskapp.c:1193-1203`). Gebrochen ist stattdessen die **Prämisse von
Beweispunkt 1**.

---

## 5. Eine benannte Abweichung, die U2 nicht kostenlos schließen kann

`app_start`s Bedingung lautet vollständig
`if (!(bootflags & BOOTFLAG_SKIP_AUTO_ACC) && (inf_data[0] != '#'))`
(`deskapp.c:834-835`). Der `bootflags`-Term fehlt bei uns — vor U2 wie nach U2.

EmuTOS' Desktop liest `bootflags` als BIOS-Global im gemeinsamen Adressraum.
Bei uns steht der Wert in `crate::launch::skip_auto_acc()`
(`kernel/src/launch.rs:202`), also in einem Kernelsymbol, das der Desktop auf
dem Weg nach EL0 nicht nennen darf — es wäre eine Zeile mehr in Gruppe B der
Driftprobe, genau die Sorte Umgehung, die diese Kampagne abbaut. Einen Trap, der
das Bit trägt, gibt es nicht.

Nach der Ordnungsregel (Plan §6.3) gehört das damit **nicht** in U2: erst der
Träger, dann die Angleichung. Im Quelltext steht die Abweichung jetzt benannt
und zitiert im Doc-Kommentar von `load_and_apply`, statt unerwähnt zu bleiben.
Wirkung heute: ein Ctrl-Boot liest `NEWDESK.INF` trotzdem.

## 6. Zwei Beobachtungen außerhalb des Auftrags (nicht angefasst)

* **`CP_SHELL_INIT` fehlt serverseitig.** EmuTOS' `run_accs_and_desktop` legt
  einmal pro AES-Boot `sh_put("#a000000\r\n", 9)` in den Puffer
  (`aes/geminit.c:97,685`); unsere AES tut das nicht, das Präfix ist also 128
  Nullbytes. Beobachtbar nur über `shel_get` durch einen fremden Client; für die
  Messung wurde es vom Prüfstand geliefert. Gehört in `tos-api`, nicht in
  `desk/`.
* **`process_inf2`s `#Z`-`isgem`-Fädelung** ist weiterhin offen
  (`docs/api/aes/shell-loop.md`, „Predictions — confirmed/refuted" (b)) und von
  U2 unberührt.

## 7. Übergabe an den Tester

Was ich gefahren habe, steht oben; **nicht** gefahren habe ich:

1. **Einen echten Doppelklick-Start** aus dem Desktop (H03-Injektor). Lauf 7
   ruft `run_gem_app` direkt, also die vollständige Sequenz ohne die Geste
   davor. Erwartung bei einem echten Start von QED/TeraDesk: unverändertes
   Verhalten; die neue Anweisung schreibt nur in den Shell-Puffer.
2. **Den A106-Neustart** (Videodialog in TeraDesk, Klicks). Erwartung nach §4:
   der neue Desktop kommt **aus der Datei**, nicht aus dem Puffer — und das ist
   das *richtige* Ergebnis. Bild wie bisher.
3. **`board-qemu-virt` im Lauf** — dort gibt es kein Blockgerät, also keinen
   Desktop mit `.INF`; nur der Bau ist geprüft.
4. **Echte Pi 4** — gar nicht, auftragsgemäß.

Ein neuer Selbsttest wurde **nicht** verdrahtet. Der vorhandene
`desk::inf::selftest()` hängt weiterhin in keinem Bootpfad
([[unwired-selftests-rot-silently]]); seine Aufrufstellen wurden nur an die
umbenannten Funktionen angepasst und er kompiliert. Ob er verdrahtet werden
soll, ist eine Entscheidung des Leads.

Regressionsflächen dieser Änderung, nach Risiko:
`Options → Save desktop` (`app_save(true)`), der GEM-Anwendungsstart
(`desktop_relinquish`), der Desktop-Kaltstart mit und ohne `NEWDESK.INF`.
