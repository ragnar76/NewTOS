# AES 1:1-Divergenz-Audit — PAKET P1 (appl / pd / dispatcher)

> Auditor (Opus), 2026-07-22. Rein statisch (kein build/QEMU — 5 parallele
> Auditoren, target/-Contention). Quelle-gegen-Quelle: EmuTOS @ `references/emutos`
> vs. NewTOS `tos-api/src/aes/{appl,pd,mod}.rs`. Soll-Referenz gegengeprüft:
> `docs/api/aes/{application,_dispatcher,_state}.md`, `_discrepancies.md` D-047/D-048.
> Leitprinzip: 1:1 EmuTOS; Default bei Abweichung = "match it".

---

## Teil 1 — Coverage-Tabelle

| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|-----------|------------------|-----------|--------|------|
| `ap_init` / `APPL_INIT` arm | `gemaplib.c:54-63` + `gemsuper.c:90-100` | `appl::appl_init` `appl.rs:244-273` | impl | **no** (ap_id-Nummerierung; LP64-Layout ohne EmuTOS-Grundwahrheit) |
| `ap_rdwr` | `gemaplib.c:69-89` | eingebettet in `appl_read`/`appl_write` + `MsgQueue` (`pd.rs:96-133`) | partial | **no** (16-Byte-Cap; kein Blockieren; Quick-Path-Bedingung fehlt) |
| `APPL_READ` arm | `gemsuper.c:101-105` | `appl::appl_read` `appl.rs:351-380` | impl | **no** (leer→0 statt block; setzt AP_MESAG) |
| `APPL_WRITE` arm | `gemsuper.c:101-105` | `appl::appl_write` `appl.rs:317-341` | impl | **no** (Trunkierung 16B; full→0) |
| `ap_find` / `APPL_FIND` | `gemaplib.c:95-108` + `gemsuper.c:106-107` | `appl::appl_find` `appl.rs:285-302` | **STUB-artig** | **no** (p_name nie gesetzt → immer −1) |
| `ap_tplay` / `APPL_TPLAY` | `gemaplib.c:119-184` + `gemsuper.c:109-110` | `appl::appl_tplay` `appl.rs:395-397` | partial | **no** (Return 0 statt 1) |
| `ap_trecd` / `APPL_TRECORD` | `gemaplib.c:189-246` + `gemsuper.c:112-113` | `appl::appl_trecord` `appl.rs:387-389` | partial | **no** (kein Recorder; kein Block) |
| `APPL_YIELD` arm | `gemsuper.c:115-119` (`#if CONF_WITH_PCGEM`, default=1) | `appl::appl_yield` `appl.rs:307-309` | impl | **no** (Return 0 statt 1) |
| `ap_exit` / `APPL_EXIT` | `gemaplib.c:251-261` + `gemsuper.c:120-123` | `appl::appl_exit` `appl.rs:276-281` | partial | **no** (kein mn_cleanup/wait_for_accs/Drain; wischt ganzes pd) |
| `appl_search` (18), `appl_getinfo` (130) | in EmuTOS abwesend → `default`→−1 (`gemsuper.c:406-417`) | `dispatch _ =>` `mod.rs:251-260` | impl (korrekte Abwesenheit) | **yes** |
| `crysbind` + `xif` (Marshalling/Return) | `gemsuper.c:77-448` | `mod::dispatch` + `Aespb` `mod.rs:59-262` | impl | **no** (intout unbedingt; kein ALNOFUNC-Alert; kein post-dsptch) |
| `super` Trap-Entry | `gemsuper.c:454-466` | `exceptions.rs:341` → `mod::dispatch` `mod.rs:166` | impl | **no** (kein cx=201/dsptch) |
| `fpdnm` / `pd_index` / `getpd` / `p_nameit` / `pstart` | `gempd.c:32-131` | `pd::{open,by_pid,find_by_name,close}` `pd.rs` | partial | **no** (Name nie gesetzt; Lazy-Alloc statt Init-Zeit) |
| geminit AES-Init-Sequenz | `geminit.c:753-903` | statische pd-Tabelle + Lazy-`open` (`pd.rs:176-224`) | partial | **no** (kein pid/Name/queue-Init zur Prozess-Startzeit) |

Nicht-appl-Opcodes (event/menu/object/form/graf/scrp/fsel/wind/rsrc/shel) sind in
`mod::dispatch` `mod.rs:180-250` präsent; ihr Inhalt gehört zu anderen Paketen.
Opcode-Range 10..127 + `default`→−1 ist strukturell korrekt abgebildet.

---

## Teil 2 — Divergenz-Findings

### AUD-P1-001: `appl_find` — AESPD-Name wird NIE gesetzt → findet keinen echten Namen
- Severity: **HIGH**
- EmuTOS: `gempd.c:77-86` `p_nameit()` füllt `p->p_name` aus dem Programmnamen
  (aufgerufen in `pstart`, `gempd.c:119`); `fpdnm` (`gempd.c:38-57`) vergleicht via
  `strncmp(pname, p->p_name, 8)`.
- NewTOS: `pd.rs:167-173` `Aespd::EMPTY.name = [b' ';8]`; `pd::open` (`pd.rs:207-224`)
  setzt `name` **nie**; `find_by_name` (`pd.rs:255-262`) vergleicht gegen diesen
  nie-gefüllten Namen; `appl_find` (`appl.rs:285-302`). Bestätigt: crateweit gibt es
  keinen Schreibzugriff auf `Aespd.name` (nur der Vergleich `pd.rs:259` + der lokale
  Puffer `appl.rs:291`).
- Divergenz: `find_by_name` kann ausschließlich einen Namen aus 8 Leerzeichen
  matchen. `appl_find("SCRENMGR")` o. jeder reale App-/ACC-Name → **immer −1**.
- 1:1-Fix: bei `pd::open` (bzw. beim Prozess-Start) den 8-Zeichen-space-gepaddeten,
  extensionslosen Programmnamen wie `p_nameit` in `Aespd.name` schreiben.
- Runtime-verify: yes (aber statisch als sicher belegt: kein name-Setter existiert).

### AUD-P1-002: `appl_yield` — liefert 0, EmuTOS liefert 1 (TRUE)
- Severity: **MEDIUM**
- EmuTOS: `gemsuper.c:85` `ret = TRUE;`; `gemsuper.c:115-119` `APPL_YIELD` (unter
  `#if CONF_WITH_PCGEM`, Default `= 1` per `config.h:1319-1320`) ruft nur `dsptch()`
  und `break` — `ret` bleibt `TRUE`, also `RET_CODE = 1`. Bestätigt vom Soll:
  `application.md:229` ("return: TRUE (1)").
- NewTOS: `appl.rs:307-309` `pb.set_ret(0)`.
- Divergenz: Rückgabe 0 statt 1 (zusätzlich fehlt die kooperative `dsptch()`-Abgabe —
  akzeptabel mangels AES-Scheduler).
- 1:1-Fix: `set_ret(1)`.
- Runtime-verify: no.

### AUD-P1-003: `appl_tplay` — liefert 0, EmuTOS liefert 1 (TRUE)
- Severity: **MEDIUM**
- EmuTOS: `gemsuper.c:109-110` ruft `ap_tplay(...)` (void), `ret` bleibt `TRUE=1`.
  Soll: `application.md:273,316`.
- NewTOS: `appl.rs:395-397` `pb.set_ret(0)`.
- Divergenz: Rückgabe 0 statt 1. (Der fehlende Playback ist akzeptable HW/Input-Absenz,
  aber der Rückgabewert ist Contract.)
- 1:1-Fix: `set_ret(1)` (bis der Input-Tape existiert). Alternativ die vom Soll
  (`application.md:294-296`) erlaubte honest-unsupported-Variante `-1` — aber **nicht 0**.
- Runtime-verify: no.

### AUD-P1-004: `ap_id`-Nummerierung — Primär-App bekommt ≥1, EmuTOS gibt 0; pid-1-Kollision Desktop↔erste EL0-App
- Severity: **MEDIUM** (latenter BLOCKER bei Multi-Kontext)
- EmuTOS: `gemsuper.c:94` `ap_id = rlr->p_pid`; `geminit.c:875-877` setzt Prozess 0
  (die "dos appl" = Desktop/Shell/gestartete App) auf `p_pid = 0`; SCRENMGR/ctlmgr
  bekommt pid 1 (`geminit.c:888` → `getpd` `gempd.c:60-70`, `curpid++`); ACCs 2+.
  Soll: `_state.md:31-32,283` ("system 0/1, apps 2+").
- NewTOS: `appl.rs:203-211` `current_pid()` = `slot_for(bp)+1` bzw. 1 im Boot/EL1-Kontext;
  `appl_init` nutzt es (`appl.rs:245`, `pd::open(pid)`).
- Divergenz: (a) Primär-App-`ap_id` = 1 statt 0. (b) Boot/EL1-Kontext (bp=null→1) und
  erste EL0-App (slot 0→1) mappen **beide auf pid 1** → `pd::open` reused denselben
  Deskriptor (`pd.rs:211-214`), `appl_exit` wischt den geteilten pd (`pd.rs:268-274`),
  Nachrichten/Window-Ownership kollidieren.
- 1:1-Fix: pid-Vergabe an EmuTOS ausrichten (Primär-Prozess = 0), getrennte Räume für
  Desktop und EL0-App, keine Kollision auf 1.
- Runtime-verify: yes (Desktop-tatsächliche-pid & Reihenfolge nicht statisch belegbar).

### AUD-P1-005: `appl_write` — Nachricht auf 16 Byte trunkiert; EmuTOS bis 128 Byte
- Severity: **MEDIUM**
- EmuTOS: `gemsuper.c:103-104` `ap_rdwr(MU_SDMSG, ..., AP_LENGTH, AP_PBUFF)`;
  `gemqueue.c:36,39` `doq`: `n = m->qpb_cnt` Bytes werden per `memcpy` einqueued
  (Queue `QUEUE_SIZE = 128`, `struct.h:56`); Nachrichten variabler Länge via `om[2]`
  (`gemqueue.c:69`).
- NewTOS: `appl.rs:319,330` `words = (len_bytes/2).min(MSG_WORDS=8)` → hart 16 Byte;
  Queue ist fest `[[i16;8]; …]` (`pd.rs:53-55`).
- Divergenz: `appl_write` mit `len > 16` verliert alle Bytes jenseits 16; `len < 16`
  wird auf 16 gepaddet statt exakt `len` einzuqueuen.
- 1:1-Fix: variable Byte-Länge (bis QUEUE_SIZE) im Queue-Modell abbilden.
- Runtime-verify: yes.

### AUD-P1-006: `appl_write` — Queue-voll liefert 0; EmuTOS blockiert (liefert nie 0)
- Severity: **MEDIUM**
- EmuTOS: `gemqueue.c:97` `qready = (qpb_cnt <= QUEUE_SIZE - p_qindex)`; bei `!qready`
  `evinsert`/Block (`gemqueue.c:120-124`) — der Sender wird geparkt, `appl_write`
  gibt bei Erfolg `1` zurück.
- NewTOS: `appl.rs:340` gibt `push()` zurück; `push` liefert `false`→0 bei 8 vollen
  Slots (`pd.rs:112-114`).
- Divergenz: `appl_write` kann 0 liefern (EmuTOS nie). Eine App, die den Rückgabewert
  als Erfolg/Fehler auswertet, missverhält sich.
- 1:1-Fix: Sender-Blockierung sobald AES-Scheduler existiert; bis dahin dokumentierte
  Abweichung (Queue-voll laut `pd.rs:88-95` als unerreichbar begründet — nicht bewiesen).
- Runtime-verify: yes.

### AUD-P1-007: `appl_read` — leere Queue liefert 0; EmuTOS blockiert bis Nachricht
- Severity: **MEDIUM**
- EmuTOS: `gemsuper.c:103` `ap_rdwr(MU_MESAG,...)`; `gemqueue.c:99,120-124` blockiert
  wenn `p_qindex == 0` (kein Return 0).
- NewTOS: `appl.rs:366-369` liefert 0 bei leerer Queue.
- Divergenz: Nicht-blockierend. Akzeptable Bring-up-Abweichung (kein Scheduler,
  `appl.rs:347-350` dokumentiert), aber Verhalten weicht sichtbar ab.
- 1:1-Fix: Blockieren, sobald Scheduler existiert.
- Runtime-verify: yes.

### AUD-P1-008: `appl_read` — setzt AP_MESAG am pd; EmuTOS tut das nicht
- Severity: **LOW-MEDIUM**
- EmuTOS: `APPL_READ`-Arm (`gemsuper.c:101-105`) verändert `p_flags` nicht; `AP_MESAG`
  wird nur in `EVNT_MESAG`/`EVNT_MULTI` gesetzt (`gemsuper.c:137,148`).
- NewTOS: `appl.rs:378` `src.flags |= pd::AP_MESAG`.
- Divergenz: zusätzlicher Seiteneffekt. `AP_MESAG` steuert `wait_for_accs`
  (`geminit.c:599-628`) — bei DA-Betrieb semantisch falsch; single-app dormant.
- 1:1-Fix: das `|= AP_MESAG` in `appl_read` entfernen.
- Runtime-verify: no.

### AUD-P1-009: Dispatcher — `set_ret` schreibt `intout[0]` unbedingt; EmuTOS nur bei OUT_LEN≥1
- Severity: **LOW-MEDIUM**
- EmuTOS: `xif` (`gemsuper.c:441-445`) — `RET_CODE` ist das **lokale** `int_out[0]`;
  Rückkopie nach `pcrys_blk->intout` **nur wenn `OUT_LEN`** (`control[2]`) ≠ 0.
  Soll ausdrücklich: `_dispatcher.md:99-118,186-195,205` ("A caller must declare
  OUT_LEN ≥ 1 to observe the −1/return").
- NewTOS: `mod.rs:95-103` `set_intout`/`set_ret` schreiben `intout[0]` **immer**;
  genutzt von `dispatch` (`mod.rs:260`) und allen appl-Fns.
- Divergenz: Return/−1 auch bei `OUT_LEN == 0` sichtbar (EmuTOS: nicht). Benign, da
  gemlib `aes_intout` global 7 WORDs groß hält; aber Contract-Detail verletzt.
- 1:1-Fix: `intout`-Rückschreiben an `control[OUT_LEN] >= 1` binden.
- Runtime-verify: no.

### AUD-P1-010: Dispatcher — kein `fm_show(ALNOFUNC)`-Alert für unsupported Opcodes
- Severity: **LOW**
- EmuTOS: `gemsuper.c:411-417` — `default`-Arm zeigt für `opcode != 0` einen
  synchronen Alert `fm_show(ALNOFUNC,&opcode,1)` und setzt `ret = -1`.
- NewTOS: `mod.rs:251-260` `set_ret(-1)` still (Kommentar verweist auf D-047/D-084).
- Divergenz: sichtbarer Alert fehlt. `−1`-Return stimmt; Alert ist per D-047 als
  reference-level eingestuft. Als Divergenz vermerkt, nicht als Bug.
- 1:1-Fix: optional `fm_show(ALNOFUNC)` für `opcode != 0` (reference-level).
- Runtime-verify: no.

### AUD-P1-011: `appl_exit` — DA-Close-Handshake/mn_cleanup/queue-Drain fehlen; wischt ganzes pd statt nur AP_OPEN
- Severity: **LOW**
- EmuTOS: `ap_exit` (`gemaplib.c:251-261`): `wm_update(BEG_UPDATE)`, `mn_cleanup()`,
  `wait_for_accs(AP_ACCLOSE)`, eigene Message-Queue via `ap_rdwr` drainen,
  `wm_update(END_UPDATE)`, `all_run()`, dann **nur** `p_flags &= ~AP_OPEN`.
- NewTOS: `appl.rs:276-281` `pd::close(pid)` → `Aespd::EMPTY` (kompletter Deskriptor
  weg); Return 1.
- Divergenz: mangels DAs/Scheduler weitgehend n/a; Return 1 stimmt. Das komplette
  Wischen (statt AP_OPEN-Bit) verschärft die pid-Kollision aus AUD-P1-004.
- 1:1-Fix: nur AP_OPEN clearen (Deskriptor besteht bis Prozessende); Menü aufräumen.
- Runtime-verify: yes.

### AUD-P1-012: `appl_find` — liest 8 Rohbytes bedingungslos; EmuTOS `strncmp` stoppt an NUL (Over-Read-Risiko)
- Severity: **LOW**
- EmuTOS: `gempd.c:48` `strncmp(pname, p_name, 8)` bricht an einem NUL in `pname` ab.
- NewTOS: `appl.rs:291-298` liest fix 8 Bytes ab `addrin[0]`.
- Divergenz: Übergibt die App einen kürzeren NUL-terminierten Puffer (<8 Byte), liest
  NewTOS über das App-Ende hinaus (potenzieller Fault). Bei der dokumentierten
  8-Zeichen-space-gepaddeten Konvention (`application.md:211-216`) identisch.
- 1:1-Fix: Kopie an NUL/8 abbrechen wie `strncmp`.
- Runtime-verify: no.

### AUD-P1-013: LP64-`AESGLOBAL`-Overlap-Layout hat keine EmuTOS-Grundwahrheit (ILP32 = 1:1 verifiziert)
- Severity: **MEDIUM** (unbewiesen — appl_init ist der erste Call jeder App)
- EmuTOS: `gemrslib.h:15-27` + `aesdefs.h:147-157` = m68k-Layout mit 4-Byte-`LONG`,
  Offsets `global[3/5/7/9/10/11]` = Byte `6/10/14/18/20/22`. NewTOS `write_ilp32`
  (`appl.rs:172-196`) trifft dieses Byte-für-Byte — **verifiziert 1:1**.
- NewTOS: `write_lp64` (`appl.rs:132-161`) verbreitert `ap_private`/`ap_ptree`/
  `ap_rscmem` auf 8 Byte an den überlappenden Offsets 6/10/14 und schreibt in
  aufsteigender Reihenfolge, damit ein "native LP64 gemlib-Reader" korrekt zurückliest.
- Divergenz: EmuTOS ist m68k-only; für LP64 existiert **keine** Grundwahrheit. Die
  Korrektheit hängt daran, ob die portierte gemlib `aes_global` über `ULONG_AT`-Makros
  (ohne `__LP64__`-Konditional, überlappend) oder über ein natürlich ausgerichtetes
  `struct aesglobal` liest — das konnte ich mit den P1-Dateien **nicht** feststellen
  (gemlib-Quelle nicht im Scope). Bricht die App-Sicht auf `ap_id`/`ap_planes`, bricht
  alles Nachfolgende.
- 1:1-Fix: keiner nötig, falls die ADR-020/D-048-Annahme (überlappende Makros) stimmt;
  sonst natürliches LP64-Struct-Layout. Muss gegen eine echte LP64-GEM-App belegt werden.
- Runtime-verify: **yes** (LP64-App: `ap_id == appl_init()` und `ap_planes` prüfen).

### AUD-P1-014: `appl_trecord` — kehrt sofort mit 0 zurück; EmuTOS zeichnet auf (blockiert) und liefert Count
- Severity: **LOW**
- EmuTOS: `gemsuper.c:112-113` `ret = ap_trecd(...)`; `gemaplib.c:189-246` zeichnet in
  0,1-s-Schritten auf und liefert die tatsächliche Ereigniszahl.
- NewTOS: `appl.rs:387-389` `set_ret(0)`.
- Divergenz: kein Recorder (akzeptable Input-Absenz); Return 0 = "0 Records" ist
  vertretbar, aber es findet keine Aufzeichnung/kein Block statt.
- 1:1-Fix: honest-unsupported bleibt zulässig; Count 0 ist ok. Kein Blocker.
- Runtime-verify: no.

---

## Zusammenfassung

Findings nach Severity:
- **BLOCKER:** 0 (kein harter Crash/ABI-Bruch/fehlende Public-Fn statisch belegbar —
  aber siehe HIGH/MEDIUM: eine Public-Fn ist faktisch tot, mehrere Returns falsch).
- **HIGH:** 1 (AUD-P1-001 `appl_find` liefert nie einen echten Namen).
- **MEDIUM:** 6 (AUD-P1-002/003/004/005/006/007 + LP64 AUD-P1-013).
- **LOW / LOW-MEDIUM:** 6 (AUD-P1-008/009/010/011/012/014).

Top-3 (ranghöchste, "match it"-Pflicht):
1. **AUD-P1-001** — `Aespd.name` wird nirgends gesetzt; `appl_find` ist damit eine
   effektive Non-Funktion (immer −1). Public-API-Kontrakt gebrochen.
2. **AUD-P1-004** — `ap_id`-Nummerierung weicht ab (Primär-App 1 statt 0) UND
   Desktop/erste-EL0-App teilen pid 1 → geteilter/gewischter Deskriptor (latente
   Message/Window-Korruption).
3. **AUD-P1-002 + AUD-P1-003** — `appl_yield` und `appl_tplay` liefern 0 statt des
   EmuTOS-Contracts 1; `appl_yield` ist unter dem Default-Build (PCGEM=1, AES 0x0340)
   ausdrücklich präsent.

Korrekt 1:1 (positiv verifiziert): ILP32-`AESGLOBAL`-Fülllayout (`appl.rs:172-196` vs.
`gemrslib.h:15-27`/`aesdefs.h:147-157`), `ap_version = 0x0340` (Default-Build),
`ap_count = 1`, Opcode-Range 10..127 + `default`→−1, Abwesenheit von
`appl_search`/`appl_getinfo` (→−1), `appl_exit`-Return 1, Redraw/Arrowed-Coalescing
für 16-Byte-Nachrichten (`pd.rs:96-118` vs. `gemqueue.c:45-72`).
