# Ressourcenzustand je Anwendung — EmuTOS gegen XaAES, und die Form für NewTOS

Datum: 2026-09-19 · Ticket: [`TASK/A39.md`](../../TASK/A39.md) (P1) · Anwenderentscheid
2026-09-19: *„wie EmuTOS, wobei man mal schauen könnte, wie das bei FreeMiNT gelöst
wurde."* · Nur Dokumentation, kein Produktcode.

Quellenstände: EmuTOS `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(Primärquelle, Track A); FreeMiNT/XaAES `references/freemint` @
`f906416a9387ed68876c738cfc5c30037b294fc0` (sanktionierte Sekundärquelle,
`xaaes/src.km/`). Alle Zeilenangaben beziehen sich auf diese Stände. Aussagen ohne
`Datei:Zeile` sind als **Schluss** gekennzeichnet.

## 0. Auftrag und Abgrenzung

Gefragt ist die Form, in der NewTOS den Zustand einer geladenen Ressource führt, so
dass (a) ein Kindprozess, der `rsrc_load`/`rsrc_free` ruft, den Elternprozess nicht
berührt (H05 F2, gemessen: nach `rsrc_load`/`rsrc_free` des inneren TeraDesk pid 2
liefern alle 36 `rsrc_gaddr` des äußeren TeraDesk pid 1 den Wert 0 —
[`TASK/H05-STATUS-DONE.MD`](../../TASK/H05-STATUS-DONE.MD) § 4), (b) der Zustand wie
bei EmuTOS im `AESGLOBAL` der Anwendung publiziert ist, und (c) eine Anwendung, die
`ap_rscmem` selbst umbiegt, den Kern nicht in einen fremden Blob schickt. Dazu die
zwei offenen Adressfragen aus
[`TASK/A35-STATUS-UNFINISHED.MD`](../../TASK/A35-STATUS-UNFINISHED.MD).

Nicht Gegenstand: das NRSC-Dateiformat, die Relokation, `rsrc_obfix`, der
Dateiselektor.

## 1. Der Befund in einem Satz, und der heutige Zustand

NewTOS hält den Ressourcenzustand in **einem** Kernel-Static je Kern, nicht je
Anwendung: `static RSC: Loaded` (`tos-api/src/aes/resource.rs:375`),
`static USER_RSC_RAW: AtomicI32` (`:1302`), `static APP_DIR` (`:1321`). Jedes
`rsrc_load` überschreibt `RSC` (`relocate_and_register_core`, `:536-600`), jedes
`rsrc_free` nullt es (`:1645-1698`) und gibt den Block `USER_RSC_RAW` frei
(`:1653-1663`). `rsrc_gaddr` (`:1875-1882`) und `rsrc_saddr` (`:1893-1917`) lesen
ausschließlich `RSC`. Das `AESGLOBAL` des Aufrufers wird seit A35 **beschrieben**
(`report_load`, `:1608-1643`: `ap_ptree` u32 @10, `ap_rscmem` u32 @14, `ap_rsclen`
u16 @18), aber nie zurückgelesen (Modulkopf `:70-76`; Spec
`docs/api/aes/resource-fileselector.md` § `rsrc_load`, Absatz „`rs_sglobe` is
deliberately not adopted" in der Fassung bis 2026-09-19 — mit diesem Dokument
ersetzt).

Der AES-Prozessdeskriptor `Aespd` (`tos-api/src/aes/pd.rs:242-249`) hält `pid`,
`flags`, `name`, `queue` — **keinen** Ressourcenzustand. Der Aufrufer eines
AES-Traps ist über `current_pid()` bekannt (`tos-api/src/aes/appl.rs:166-175`:
GEMDOS-Slot + 1, bei leerem `RUN` die 1).

## 2. Vergleich: wo der Zustand liegt

| Frage | EmuTOS (Primärquelle) | XaAES (Sekundärquelle) | NewTOS heute |
|---|---|---|---|
| **Wo liegt der Ressourcenzustand?** | Ausschließlich im `AESGLOBAL` der Anwendung: `ap_ptree`, `ap_rscmem`, `ap_rsclen` (`aes/gemrslib.h:20-23`). Die beiden Statics `rs_hdr`/`rs_global` (`aes/gemrslib.c:44-45`) sind nur der Arbeitszeiger **eines** Aufrufs: `rs_sglobe(pglobal)` setzt sie bei jedem `rs_free`/`rs_gaddr`/`rs_saddr`/`rs_fixit` neu aus dem übergebenen `pglobal` (`:675-679`, Aufrufe `:690`, `:717`, `:732`, `:818`). Der `AESPD` (`aes/struct.h:119-155`) enthält **kein** Ressourcenfeld. | Im Client-Deskriptor `struct xa_client`: `globl_ptr` (Zeiger auf das `global[]` des `appl_init`, `xa_types.h:2185`), `rsrc` (zuletzt geladener `RSHDR *`, `:2188`), `trees` (`:2189`), `rsct` (Zähler, `:2190`) und die verkettete Liste `resources` (`:2222`) aus `struct xa_rscs` (`:954-964`), je Eintrag mit eigenem `globl`-Zeiger — Kommentar `:960`: *„Need a global per resource, otherwise rsrc_gaddr would be ambiguous."* Zusätzlich wird das `global[]` der App beschrieben (§ „`global[5..9]`"). | Ein Kernel-Static `RSC` für alle (`resource.rs:375`); das `AESGLOBAL` nur als Meldung. |
| **Wie viele Ressourcen je Anwendung?** | So viele, wie die Anwendung `AESGLOBAL`-Arrays hat: `rs_load` gibt eine vorhandene Ladung **nicht** frei, sondern überschreibt nur die drei Felder des übergebenen `pglobal` (`rs_readit`, `:786-788`; `fix_trindex`, `:597`). Mit **einem** Array ist zu jedem Zeitpunkt eine Ressource *benannt*; weitere geladene Blobs bleiben GEMDOS-Speicher der Anwendung (§ „Speicherbesitz"). | Beliebig viele, als Liste: `list_resource` hängt jede Ladung **vorn** an `client->resources` (`xa_rsrc.c:878-918`, `:895-900`), `rsct++` (`:1050`). Kommentar in `XA_rsrc_load` (`:1594-1602`): *„GEM AES doesn't auto-free an existing RSC either"* — deshalb die Liste statt Auto-Free. | Genau eine je Kern; die zweite Ladung ersetzt die erste für alle. |
| **Was schreibt `appl_init` in `global[5..9]`?** | Nichts. Der `APPL_INIT`-Arm schreibt `ap_version`, `ap_count`, `ap_id`, `ap_private`, `ap_planes`, `ap_3resv` (`aes/gemsuper.c:92-98`). | `ptree = NULL`, `rshdr = NULL`, `lmem = 0` (`xa_appl.c:468-470`), dazu `version = 0x0410`, `count = -1`, `nplanes`, `res1/res2 = 0`, `c_max_h`, `bvhard = 4` (`:464-475`). Merkt sich `client->globl_ptr = globl` (`:386`, `:407`). | Wie EmuTOS: nichts (`appl.rs:139-162`, `write`). |
| **Was schreibt `rsrc_load`?** | `ap_rscmem = rs_hdr` (Blobbasis), `ap_rsclen = rslsize` (`gemrslib.c:786-788`), `ap_ptree = ptreebase` = `rs_hdr + rsh_trindex`, das **im Blob** liegende, an Ort und Stelle relozierte Baumfeld (`fix_trindex`, `:591-603`). | `Rsrc_setglobal(h, gl)` (`xa_rsrc.c:1245-1268`): `gl->rshdr = h`, `gl->ptree = h + rsh_trindex` (das im Blob relozierte Feld, `fix_trees` `:736-760`, Slotfix `:759`), `gl->lmem = rsh_rssize`. Geschrieben in `client->globl_ptr` **und**, falls verschieden, in das `pb->global` des Aufrufs (`:1618-1634`). Dazu `client->rsrc`/`client->trees` (`:1006`, `:1614-1616`). | Wie EmuTOS, u32/u32/u16 (`report_load`, `resource.rs:1608-1643`), nur wenn Basis und Feld in 32 Bit passen. |
| **Breite der Felder** | m68k: `LONG`/Zeiger 4 Byte an Byte 10 und 14, `UWORD` an 18 (`gemrslib.h:15-27`; `AP_PTREE = ULONG_AT(&global[5])`, `AP_1RESV = ULONG_AT(&global[7])`, `AP_2RESV0 = global[9]`, `include/aesdefs.h:151-154`). | m68k-only, identische Ablage: `struct aes_global` mit `ptree` (Wort 5,6), `rshdr` (7,8), `lmem` (9) (`xa_aes.h:148-160`). | TOS-Ablage auf jeder ABI (Entscheid 2026-09-13, `docs/api/aes/application.md` § `AESGLOBAL`; Test `tos-api/tests/aes_global_layout.rs`). |
| **Wie findet `rsrc_gaddr` den Baum?** | `rs_sglobe(pglobal)` → `rs_hdr = pglobal->ap_rscmem` (`:677-678`); `get_addr` rechnet alle Typen als `rs_hdr + rsh_<feld> + index * size` (`:100-104`, `:112-176`), **außer** `R_TREE`: `return rs_global->ap_ptree[rsindex]` — direkt aus dem `AESGLOBAL`, nicht aus dem Header (`:124`). Keine Prüfung des Zeigers oder des Index. | **Zuerst** das `global` des Aufrufs: `rsc = pb->global->rshdr; trees = pb->global->ptree` (`xa_rsrc.c:1701-1705`). Sind beide 0: Rückfall auf `client->rsrc`/`client->trees` = zuletzt geladen (`:1709-1714`). Nur `ptree` gesetzt: nur `R_TREE` aus diesem Feld (`:1717-1740`). Danach `client->rsrc = rsc` (`:1751`), Auflösung über den Header (`ResourceTree` `:1445-1456` liest `hdr + rsh_trindex`, also **nicht** `ptree`), zuletzt eine **Bereichsprüfung**: ein Ergebnis außerhalb `[rsc, rsc + rsh_rssize)` (bzw. der erweiterten Länge) löst `ALERT(AL_INVALIDP)` aus und liefert `intout[0] = 0`, `*addr = 0` (`:1839-1855`). | Immer `RSC` (`resource.rs:1875-1882`), unabhängig vom Aufrufer und seinem `AESGLOBAL`. |
| **Was tut `rsrc_free`?** | `rs_sglobe(pglobal)`; ist `ap_rscmem` 0 → Rückgabe 0; sonst CICON-Puffer frei, `dos_free(pglobal->ap_rscmem)` (`:685-708`, `:699`). Gibt **genau den Blob frei, den das `AESGLOBAL` des Aufrufers nennt.** Die drei Felder bleiben stehen (kein Schreibzugriff in `rs_free`). | `FreeResources(client, pb, NULL)` (`:1670`) gibt den **Listenkopf** frei = die zuletzt geladene Ressource (`:1305-1312` mit `rsrc == NULL`, Abbruch über `nx = NULL` `:1412`), nicht die, die `pb->global` nennt (das wird dort nur für Diagnose gelesen, `:1296-1301`). War es die aktive (`client->rsrc == hdr`), rückt der nächste Eintrag nach und `Rsrc_setglobal` schreibt dessen Werte — oder `NULL`/`0` — in `client->globl_ptr` und `pb->global` (`:1347-1362`). `intout[0] = 1`, wenn `client->rsrc` gesetzt war, sonst 0 (`:1666-1676`). | Gibt `USER_RSC_RAW` frei und nullt `RSC` (`:1645-1698`); `AESGLOBAL` bleibt stehen. |
| **Speicherbesitz des Blobs** | `dos_alloc_anyram` = `Mxalloc(n, MX_PREFTTRAM)` (`util/gemdos.c:117-120`, Aufruf `gemrslib.c:775`). Der GEMDOS-Allokator stempelt `m_own = run` (`bdos/iumem.c:134`); `run` ist beim Trap der Anwendung deren PD (`proc_go`, `bdos/proc.c:575`). **Schluss:** der Blob gehört dem GEMDOS-Prozess der Anwendung. | Für Clients `umalloc` (Prozessspeicher; `xa_rsrc.c:983-985`), für die AES selbst `kmalloc`. Freigabe über die Liste (`ufree`, `:1406-1410`). | `mxalloc(…, MX_PREFTTRAM)` mit Besitzstempel des laufenden Prozesses (`load_into_user`, `resource.rs:1498-1503`; `current_owner`, `bdos/mem/traps.rs:92-106`), abgebildet als `Kind::Rsc` ins Prozessfenster (`usermem.rs:106`, `:307`). |
| **Was geschieht beim `appl_exit`?** | Nichts mit der Ressource: `ap_exit` (`aes/gemaplib.c:251-261`) leert die Queue und löscht `AP_OPEN`. | `XA_appl_exit` → `exit_client` (`xa_appl.c:895`, `:655`); dort `client->rsrc = NULL; while (client->resources) FreeResources(client, NULL, NULL);` (`:745-750`) — **alle** Ressourcen **dieses** Clients werden freigegeben, schon beim `appl_exit`. | Nichts (`appl.rs:253-258` → `pd::close`, `pd.rs:466-474`). |
| **Was geschieht beim Prozessende?** | Die AES tut nichts (`sh_ldapp`, `aes/gemshlib.c:650-657`, fasst nur `AP_OPEN`/Queue an). GEMDOS `xterm` → `ixterm` → `free_all_owned(r, &pmd)` (`bdos/proc.c:601-619`, `:136`, `:92-100`) gibt jeden Block mit `m_own == r` frei — also auch den Blob. Das `AESGLOBAL` stirbt mit dem Speicher der Anwendung. | `xaaes_on_exit` (`sys_proc.c:72-82`) → `exit_client(…, pexit = true, …)` → dieselben Zeilen `:745-750`: nur die Ressourcen dieses Clients. | `free_all_owned(bp)` (`bdos/process/pterm.rs:167-176`) gibt den Poolblock nach Besitz frei — aber `RSC`/`USER_RSC_RAW` bleiben stehen; die AES hat im `Pterm`-Pfad keinen Haken außer den Tracern (`pterm.rs:142-152`), `close_client_if_open` ruft nur der Kernel-Launcher (`kernel/src/launch.rs:137`). |
| **App biegt `ap_rscmem` um** | Die AES **folgt**: `rs_sglobe` liest den Wert bei jedem Aufruf (`:677-678`), `get_addr` rechnet mit dem Header, der dort steht (`:100-104`), `R_TREE` liest `ap_ptree[i]` aus dem `AESGLOBAL` (`:124`). Kein Schutz. | **Folgt** (`:1701-1705`), liest den Header an der genannten Adresse (`rshdr(rsc, …)`, `:59-61`) und **prüft das Ergebnis** gegen die Länge aus eben diesem Header (`:1839-1855`). Ein sinnvoller Plausibilitätstest, kein Schutz: der Header selbst wird der Anwendung geglaubt. | Ignoriert den Wert. |

## 3. Der Schiedsrichter: TeraDesk biegt `global[5..8]` selbst um

Die Spec in der Fassung bis 2026-09-19 behauptete, kein bekannter Aufrufer
überschreibe `ap_rscmem` und erwarte, dass `rsrc_gaddr` folgt, und TeraDesks
`xrsrc_*`-Lader halte seine Ressource „ganz außerhalb der kernelseitigen `rsrc_*`"
(derselbe Absatz, ebenso `TASK/A35-STATUS-DONE.MD:41-46`). **Beides ist falsch.**

TeraDesk (`sdk/apps/teradesk/`, unveränderte Anwendungsquelle):

1. `main.c:1847` lädt `desktop.rsc` über die AES (`rsrc_load(RSRCNAME)`); die AES
   belegt `global[5..9]`.
2. `icon.c:2705-2766` `load_icons()`: sichert `svtree = _AESrscfile`, `svrshdr =
   _AESrscmem` (`:2707-2708`) — die Werte, die die AES für `desktop.rsc` geschrieben
   hat. Dann, wenn Farbicons möglich, `rsrc_load("cicons.rsc")` **über die AES**
   (`:2726`); `aes_supports_coloricons()` (`:2669-2696`) fragt anschließend
   `rsrc_gaddr(R_TREE, 0)` und erwartet den **cicons**-Baum (`tree[0].ob_type ==
   G_BOX`, `obj[1].ob_type == G_CICON`) — verlässt sich also darauf, dass die AES
   nach der zweiten Ladung diese benennt; bei Nichtunterstützung `rsrc_free()`
   (`:2731`) — bei EmuTOS `dos_free(pglobal->ap_rscmem)`, also der cicons-Blob.
3. Sonst `xrsrc_load("icons.rsc")` (`:2736`) mit eigenem `Malloc`
   (`rsc_load.c:456`) und eigener Relokation; `xrsrc_fix` **schreibt
   `aes_global[5]` und `[7]`** mit dem eigenen Baumfeld und der eigenen Blobbasis
   (`rsc_load.c:414-431`), `xrsrc_gaddr` liest `aes_global[7]` zurück (`:496-508`),
   `xrsrc_free` gibt `*(void **)&aes_global[7]` per `Mfree` frei (`:480-493`).
4. Danach **stellt TeraDesk `global[5..8]` auf `desktop.rsc` zurück**: `_AESrscfile =
   svtree; _AESrscmem = svrshdr;` (`:2762-2763`, im Fehlerpfad `:2742-2743` mit dem
   Kommentar *„alert_abort will call rsrc_gaddr(), need to reset globals here to
   point to desktop.rsc"*). Alle späteren AES-`rsrc_gaddr(R_STRING, …)`
   (`error.c:41`, `resource.c:694`) sollen wieder in `desktop.rsc` auflösen — was auf
   EmuTOS genau `rs_sglobe` leistet. `free_icons` (`:2769-2785`) tauscht für
   `xrsrc_free` ein und zurück; `main.c:1992` `rsrc_free()` am Ende gibt dann
   `desktop.rsc` frei, ein geladener cicons-Blob bleibt bis `Pterm` (EmuTOS:
   `free_all_owned`).

Damit ist die Erwartung einer echten Anwendung an die AES: **das `AESGLOBAL` wählt
die Ressource, mehrere Blobs je Prozess können gleichzeitig geladen sein, und
`rsrc_free` trifft den benannten.** Genau EmuTOS' `rs_sglobe`.

Dass NewTOS heute mit TeraDesk läuft, hat zwei Gründe, die nichts mit Konformität zu
tun haben: `cicons.rsc` wird absichtlich nicht ausgeliefert
(`docs/sdk/04-porting-anatomy.md:174`; H05 § Offen), so dass Schritt 2 nie zwei
Kernel-Blobs erzeugt; und die Kernel-`rsrc_*` ignorieren das `AESGLOBAL`, so dass
Schritt 3/4 sie nicht stören.

**A60 ist auf LP64 die Vorbedingung für das Zurücklesen.** TeraDesks
`_AESrscfile`/`_AESrscmem` sind in `library/xdialog/library.h:80-83` auf eine natürlich
ausgerichtete `AES_GLOBAL`-Struktur umdefiniert (`:65-77`): auf LP64 liegen
`ap_ptree` auf Byte 16 und `ap_rscmem` auf Byte 24 (Schluss aus der Struktur:
`ap_private` als 8-Byte-Union auf 8, danach zwei 8-Byte-Zeiger), während
`xrsrc_fix` acht Byte auf Byte 10 und acht auf Byte 14 schreibt (`rsc_load.c:417-422`,
`&aes_global[5]`/`[7]`) und der Kernel vier Byte auf 10 und 14 (`report_load`).
Sichern und Zurückstellen in `load_icons` lesen und schreiben also Bytes 16..31 —
**nicht** die Kernelfelder. **Schluss** (aus den Quellen hergeleitet, nicht
gemessen): nach `load_icons` steht auf LP64 in Byte 14..17 die untere Hälfte der
privaten `icons.rsc`-Basis (mit von `svtree` zurückgeschriebenen Bytes 16..17) und
in Byte 10..13 die untere Hälfte des privaten Baumfelds — ein Selektor, der keinen
Kernel-Blob nennt. Jede Form des Zurücklesens (EmuTOS-wörtlich oder eingeschränkt,
§ 4) ließe TeraDesk-LP64 danach **ab dem Start** ohne Strings, nicht erst nach
einem Kind. Auf m68k fallen alle drei Lesarten zusammen (30-Byte-Struktur), dort
ist TeraDesk korrekt. Das ist [`TASK/A60.md`](../../TASK/A60.md) — dessen
Abnahme verlangt einen Messbeleg; die Vorhersage hier ersetzt ihn nicht, sie
benennt nur, warum das Zurücklesen A60 voraussetzt.

## 4. Bewertung: die Form für NewTOS

Vertrag (aus EmuTOS, in XaAES bestätigt) und Realisierung sind zu trennen.

### 4.1 Vertrag — was eine Anwendung beobachtet

1. **Der Zustand gehört der Anwendung, nicht dem Kern.** Ein Aufruf wirkt auf die
   Ressource, die das `AESGLOBAL` des Aufrufers nennt (`gemrslib.c:675-679`; XaAES
   `xa_rsrc.c:1701-1705`). Zwei AES-Prozesse mit getrennten `global[]` stören sich
   nicht — EmuTOS hat diesen Fall selbst: jedes Accessory ist ein eigener
   `AESPD` (`load_one_acc` → `pstart`, `aes/geminit.c:254-255`; `gempd.c:110-131`;
   Tabelle `D.g_acc`, `geminit.c:812-814`) und — **Schluss**, da ein eigenständig
   gebautes Programm — trägt sein eigenes `global[]`; `crysbind` erhält `pglobal`
   je Aufruf aus dem AESPB (`gemsuper.c:441`).
2. **`rsrc_load` publiziert** `ap_ptree`/`ap_rscmem`/`ap_rsclen` (`:786-788`, `:597`)
   und gibt eine frühere Ladung nicht frei. **`rsrc_free`** gibt den benannten Blob
   frei, liefert 0 bei leerem Selektor (`:685-708`) und lässt die Felder stehen.
   **`rsrc_gaddr`/`rsrc_saddr`** lösen im benannten Blob auf (`:715-742`).
3. **`appl_exit` fasst die Ressource nicht an** (`gemaplib.c:251-261`). Beim
   **Prozessende** stirbt der Blob mit dem Speicher des Prozesses
   (`proc.c:136`, `:92-100`); andere Prozesse bleiben unberührt. XaAES gibt schon
   beim `appl_exit` frei (`xa_appl.c:745-750`) — Abweichung gegenüber TOS, **nicht
   übernommen** (Default EmuTOS; beobachtbar nur für eine App, die nach `appl_exit`
   noch `rsrc_gaddr` ruft).
4. **Feldbreiten:** TOS-Ablage, 4-Byte-User-VAs (`application.md` § `AESGLOBAL`).

### 4.2 Realisierung — je AES-Prozess geführt, ohne fremden Blob

EmuTOS braucht keinen Deskriptor, weil sein Zustand *der Speicher der Anwendung
ist* und der Kern denselben Adressraum liest. NewTOS liest Anwendungsspeicher nur
über den Kernel-Alias des Prozessfensters (`Slot { target, access }`,
`resource.rs:1713-1716`; `usermem::translate`, `usermem.rs:545`) und muss die
Stride-Information der ABI halten (`Loaded`, `:311-372`). Der Deskriptor ist also
nötig — aber **je Prozess**, und das `AESGLOBAL` entscheidet, **welcher Eintrag**
gilt:

| Regel | Beleg / Herkunft |
|---|---|
| **(R1) Je AES-Prozess eine Liste geladener Ressourcen**, Schlüssel der Aufrufer-pid (`current_pid()`, = GEMDOS-Slot + 1, `appl.rs:166-175`); je Eintrag der heutige `Loaded`-Satz plus der Poolschlüssel (`USER_RSC_RAW` je Eintrag statt global). Mehrere Einträge je Prozess, keine Auto-Freigabe bei erneuter Ladung. | EmuTOS: eine Ladung je `pglobal`, alte Blobs bleiben (`:786-788`); XaAES: `client->resources` (`xa_types.h:2222`, `xa_rsrc.c:895-900`). TeraDesk § 3 Schritt 2 braucht zwei Blobs gleichzeitig. Eine Obergrenze je Prozess ist Internum; beim Erreichen liefert `rsrc_load` 0 wie bei EmuTOS' `dos_alloc_anyram`-Fehlschlag (`:775-776`). |
| **(R2) `rsrc_load` schreibt die drei Felder** wie heute (`report_load`) und hängt den Eintrag an die Liste des Aufrufers. | `gemrslib.c:786-788`, `:597`; XaAES `Rsrc_setglobal` (`:1245-1268`). |
| **(R3) Auswahl bei `rsrc_free`/`rsrc_gaddr`/`rsrc_saddr`: das u32 `ap_rscmem` (Byte 14) des Aufrufer-`AESGLOBAL` wird gegen die `blob_target`-Basen der **eigenen** Liste verglichen** (Prozessfenster < 32 MiB, `USER_TPA_MAX_VA`, also in 32 Bit exakt — `resource.rs:1619-1620`). Trifft es einen Eintrag, gilt dieser. Nennt es keinen eigenen Blob (0, fremde Adresse, Blob eines anderen Prozesses): **„nichts geladen"** — `rsrc_free` 0, `rsrc_gaddr` `addr_out = 0`/`int_out = 0`, `rsrc_saddr` 0. Ein `global`-Nullzeiger im AESPB zählt als leerer Selektor. | Auswahl: EmuTOS `rs_sglobe` (`:677-678`). Einschränkung auf eigene Blobs: XaAES' Gedanke, das Ergebnis gegen den Blob zu prüfen (`:1839-1855`), auf den *Selektor* vorverlegt, weil NewTOS keinen Header parsen darf, den es nicht selbst geladen hat. Null-`global`: EmuTOS undefiniert (Dereferenz von `NULL->ap_rscmem`), XaAES fällt auf `client->rsrc` zurück (`:1709-1714`) — hier 0, weil ein Rückfall auf „zuletzt geladen" bei einem *gesetzten*, aber fremden Wert das A60-Bild verschleiern würde (§ 3). |
| **(R4) `R_TREE`** wird aus dem Baumfeld des gewählten Eintrags beantwortet — ab NRSC v7/v8 wörtlich `ap_ptree[i]` desselben Blobs (`relocate_and_register_core`, `resource.rs:576-590`; alle drei ausgelieferten Blobs sind v7, Kopfbytes geprüft). | EmuTOS `:124` liest `ap_ptree[i]` aus dem `AESGLOBAL`; das ist bei v7/v8 wertgleich. **Rest-Differenz, benannt:** eine App, die *nur* `ap_ptree` auf ein privates Feld umbiegt, wird bei EmuTOS bedient, bei XaAES nur wenn `rshdr == NULL` (`:1717-1740`), bei NewTOS nicht. Kein bekannter Fall (TeraDesk biegt beide um und stellt beide zurück). |
| **(R5) `rsrc_free`** gibt den gewählten Eintrag frei (Poolblock + Fenstereintrag), entfernt ihn aus der Liste, lässt das `AESGLOBAL` stehen. | EmuTOS `:699` (`dos_free(pglobal->ap_rscmem)`), keine Feldschreibung in `rs_free`. XaAES gibt den Listenkopf frei und schreibt das `global` um (`:1305-1362`) — **nicht übernommen** (Default EmuTOS; für TeraDesk § 3 Schritt 2 wäre der Listenkopf zufällig dasselbe). |
| **(R6) `appl_exit`** fasst die Liste nicht an. **`Pterm`** des Prozesses gibt seine Blöcke frei (heute schon: `free_all_owned`, `pterm.rs:167-176`) **und muss die Einträge dieses Prozesses aus der AES-Liste nehmen** — im selben Schritt, mit einem AES-Haken neben den Tracern (`pterm.rs:142-152`). Nur die eigene pid. | EmuTOS: `ap_exit` `gemaplib.c:251-261`; `ixterm` `proc.c:136`. Warum der Haken nötig ist: `pd::open` **wiederverwendet** den Deskriptor einer pid samt Inhalt (`pd.rs:283-301`, A29), und ein per EL0-`Pexec` gestartetes Kind durchläuft `close_client_if_open` nicht (`launch.rs:137` ist der einzige Aufrufer; H05 § Offen). Ohne Haken erbte der nächste Prozess in demselben Slot (dieselbe pid, dasselbe Fenster, dieselben VAs) Einträge mit Kernel-Aliassen auf freigegebenen Poolspeicher. |
| **(R7) `rsrc_load` ohne `appl_init`** muss funktionieren; die Liste darf nicht an `AP_OPEN` hängen. | EmuTOS prüft in `crysbind` nichts (`gemsuper.c:363-365`); XaAES erzwingt einen Client (`handler.c:517-548`). |

Was (R3) gegenüber EmuTOS *nicht* leistet — und was der Anwender wissen soll: eine
Anwendung, die `ap_rscmem` auf einen **selbst gebauten** Blob setzt und die AES
danach darin auflösen lassen will, wird bei EmuTOS bedient (mit einem
m68k-`RSHDR`) und bei NewTOS mit 0 beschieden. Im Baum gibt es diesen Fall nicht:
TeraDesk fragt seinen privaten Blob ausschließlich über `xrsrc_gaddr`
(`icon.c:2750`) und stellt vor jedem AES-Aufruf `desktop.rsc` zurück. Wollte man
ihn bedienen, müsste der Kern einen Anwendungs-Header über `usermem::translate`
parsen und allen Offsets darin trauen — das ist die Stelle, an der XaAES auch nur
nachträglich prüft. Nicht empfohlen; Default EmuTOS gilt für alles, was **durch die
AES geladen** wurde.

### 4.3 Reihenfolge mit A60 — die Entscheidung, die offen ist

(R1), (R2), (R5), (R6), (R7) genügen für H05 F2: Kind lädt und gibt frei, Eltern
unberührt, weil Schlüssel und Liste je pid sind. Sie sind unabhängig von A60.

(R3)/(R4), das Zurücklesen, ist auf LP64 **erst nach A60** für TeraDesk tragbar
(§ 3). Bis dahin gibt es zwei ehrliche Zustände:

* **Variante A (empfohlen, EmuTOS-exakt in zwei Schritten):** zuerst A60 (Overlay
  unter `sdk/overlays/teradesk/`, das `AES_GLOBAL` bzw. die `&aes_global[5]/[7]`-
  Stores auf die 4-Byte-TOS-Felder legt), dann A39 vollständig mit (R3)/(R4).
  Zwischenzeitlich bleibt A39 ein P1 ohne Fix — oder man liefert **Stufe 1** (R1,
  R2, R5, R6, R7 mit „Auswahl = zuletzt geladener Eintrag des Aufrufers") sofort
  und **Stufe 2** (R3/R4) nach A60. Stufe 1 ist für jede Anwendung, die ihr
  `AESGLOBAL` nicht selbst beschreibt, von EmuTOS ununterscheidbar (bei ihr nennt
  `ap_rscmem` ohnehin die letzte Ladung, `:786-788`); der Unterschied ist auf
  TeraDesks `load_icons` begrenzt, das auf LP64 wegen A60 die Felder gar nicht
  korrekt zurückstellen kann, und auf den nicht ausgelieferten cicons-Pfad. Die
  Spec muss Stufe 1 als Zwischenstand ausweisen, nicht als Vertrag.
* **Variante B (nicht empfohlen):** (R3) mit Rückfall auf „zuletzt geladen", wenn
  der Selektor keinen eigenen Blob nennt. Hält TeraDesk-LP64 ohne A60 am Laufen —
  und verdeckt A60 dauerhaft; weder EmuTOS noch XaAES tun das bei gesetztem Wert.

## 5. Antworten auf die A35-Fragen

### (1) Woher kommt das `ap_ptree`-Feld?

**Aus dem Blob — bei beiden Referenzen, und bei NewTOS seit A35.** EmuTOS' `ap_ptree`
ist `rs_hdr + rsh_trindex`, das im Blob liegende Feld, dessen Slots `fix_trindex`
an Ort und Stelle zu Zeigern macht (`gemrslib.c:591-603`). XaAES ebenso: `gl->ptree
= rsc_start(h, rsh_trindex)` (`xa_rsrc.c:1252-1262`), Slots durch `fix_trees`
reloziert (`:759`), Kommentar an `ResourceTree`: *„fixing up the pointer array is
now done in Loadresources, to make it usable via global[5]"* (`:1442-1443`). Kein
erzeugtes, separat verwaltetes Feld — weder im Client-Speicher noch im Kern.

NewTOS' Frage war, dass der NRSC-Baumindex ein `[u32]` von Objektindizes war. Das
hat A35 (Quittung [`TASK/A35-STATUS-DONE.MD`](../../TASK/A35-STATUS-DONE.MD))
bereits gelöst: ab NRSC v7 (LP64) / v8 (ILP32) ist `off_trindex` ein Feld von
Wurzel-Byteversätzen in **Aufruferbreite**, das der Lader wie `frstr`/`frimg` an
Ort und Stelle reloziert (`resource.rs:238-247` `trindex_stride`, `:576-590`), und
`report_load` publiziert seine User-VA. `ap_ptree[i]` und `rsrc_gaddr(R_TREE, i)`
sind dieselbe Adresse (`tos-api/tests/aes_rsrc_global_fields.rs`). Alle drei
ausgelieferten Blobs sind v7 (`gem_fsel.nrsc`, `qed.rsc`, `desktop.rsc`; Kopfbytes
`4e52 5343 0700 0000`). Eigentümer ist der Blob, Freigabe mit ihm (`rsrc_free` bzw.
`Pterm`), ein v1–v6-Blob publiziert nichts (`:1599-1603`) — der einzige lebende
v1-Fall ist der handgebaute Blob in `kernel/src/selftest_objc.rs::run_rsrc`, ohne
Aufrufer.

**Folge für A39:** kein neues Feld nötig; (R4) beantwortet `R_TREE` aus dem Feld des
gewählten Blobs.

### (2) Der EL1-Pfad

**Es gibt keinen EL1-Aufrufer von `rsrc_load` — heute erneut geprüft.**
`/usr/bin/grep -rn "rsrc_load\|opcode 110\|op 110" kernel/src` trifft nur
`kernel/src/dbg_qedtrace.rs:521-525,820-821` (Opcode-Namen des Tracers),
`kernel/src/bench.rs:73` und `kernel/src/launch.rs:95` (Kommentare) sowie
`kernel/src/selftest_objc.rs`. Dort setzen nur `run_rsrc_file` (`:1006`, `control[0]
= 110` `:1020`) und `run_rsrc_gem` (`:1044`, `:1057`) den Opcode 110 ab; beide haben
keinen Aufrufer. `run_rsrc` (`:921`) registriert per `relocate_and_register`
(`:965`) und wird nur aus `kernel/src/aes_demo.rs:52` (`showcase`) erreicht, das
außer der Moduldeklaration `kernel/src/lib.rs:60` keinen Aufrufer hat. `kernel/src/desk/`
enthält kein `rsrc`. Die Aussage der Spec (`resource-fileselector.md:266-274`,
Messung 2026-09-13) ist am 2026-09-19 unverändert wahr.

**Regel:** Der Deskriptor lebt je AES-Prozess; das `AESGLOBAL` der Anwendung trägt
die 4-Byte-User-VAs ihres Prozessfensters. Ein EL1-Aufrufer existiert nicht. Träte
einer auf, hätte er keinen Prozess (`current_pid()` → 1, `RUN` leer) und keine in
32 Bit darstellbare Blobadresse (`RSC_BUF` in der Kernel-High-Half,
`A35-STATUS-DONE.MD` § „Der EL1-Pfad"); er bekäme wie heute **0** in den drei
Feldern (EmuTOS' Lesart „nichts geladen", `gemrslib.c:685`) und seine Ladung landete
in der Liste der pid 1 — die sich der Kernel-Bootkontext mit dem ersten
gestarteten Programm in Slot 0 teilt (`pid_of_slot`, `aes/mod.rs:645-647`; A29).
Das ist mit (R3) folgerichtig: ein EL1-Aufrufer, dessen `AESGLOBAL` 0 trägt, hätte
keinen Selektor und würde „nichts geladen" beschieden — es sei denn, Stufe 1
(§ 4.3) gilt, dann „zuletzt geladen der pid 1". Beides bindet keinen realen
Aufrufer.

## 6. Was der Developer bauen soll

Reihenfolge und Zuschnitt nach § 4.3 entscheidet der Anwender; die Liste gilt für
den Endzustand (Variante A, beide Stufen).

1. **Per-Prozess-Liste statt der drei Statics.** `RSC`, `USER_RSC_RAW` werden zu
   Einträgen einer Liste je AES-pid (Ort: `Aespd` oder eine parallele Tabelle mit
   demselben Schlüssel — Internum). `APP_DIR` ist ein eigener Punkt (H05 § Offen:
   EL0-`Pexec`-Kinder bekommen kein `set_app_dir`), hier nicht enthalten.
   *Akzeptanz:* Hosttest mit zwei Prozessen (Slots über
   `bdos::process::allocate_slot` `process.rs:787`, Wechsel über `set_current`
   `:653`): pid 1 `rsrc_load` + `rsrc_gaddr(R_TREE, 0)` = A; pid 2 `rsrc_load` +
   `rsrc_free`; pid 1 `rsrc_gaddr(R_TREE, 0)` liefert weiterhin 1 und **dieselbe**
   Adresse A; pid 1 `rsrc_free` liefert 1 (H05 § 5 Test b).
2. **`rsrc_load` publiziert und listet.** `report_load` unverändert; der Eintrag
   trägt `blob_target`, Poolschlüssel, Slot. Keine Auto-Freigabe einer früheren
   Ladung des Prozesses; Obergrenze erreicht → 0.
   *Akzeptanz:* zwei `rsrc_load` derselben pid ohne `rsrc_free` — beide Blobs
   bleiben über `rsrc_gaddr` erreichbar, sobald der Selektor den jeweiligen nennt
   (Stufe 2) bzw. der zweite gilt (Stufe 1); `ap_rscmem` nennt nach der zweiten
   Ladung den zweiten Blob (`gemrslib.c:786-788`).
3. **Auswahl über `ap_rscmem` (Stufe 2, nach A60).** `rsrc_free`/`rsrc_gaddr`/
   `rsrc_saddr` lesen u32 @14 des Aufrufer-`AESGLOBAL` und wählen den Eintrag der
   eigenen Liste mit dieser `blob_target`; kein Treffer → „nichts geladen".
   *Akzeptanz:* (a) Selektor auf den ersten von zwei Blobs zurückgestellt →
   `rsrc_gaddr(R_STRING, i)` löst im ersten auf, `rsrc_free` gibt den ersten frei,
   der zweite bleibt; (b) Selektor = Blobadresse von pid 2, Aufrufer pid 1 → 0;
   (c) Selektor = beliebige fremde Adresse → 0, kein Kernelzugriff darauf;
   (d) `global == NULL` im AESPB → 0.
4. **`ap_ptree`-Feld erreichbar und in Aufruferbreite.** Bestehende Tests
   (`aes_rsrc_global_fields.rs`: `ap_ptree[i] == rsrc_gaddr(R_TREE, i)`) bleiben
   grün; für einen ILP32-Aufrufer ein Fall mit 4-Byte-Slots (`trindex_stride`).
5. **`rsrc_free` trifft nur den gewählten Eintrag**, lässt `AESGLOBAL` stehen,
   liefert 0 bei leerem/fremdem Selektor; Doppelfreigabe → 0.
6. **Prozessende gibt nur die eigene Ressource frei.** AES-Haken in
   `terminate_inner` (`pterm.rs:142-152`, neben `trace::flush_on_exit`), der die
   Einträge der sterbenden pid entfernt (die Poolblöcke räumt `free_all_owned`
   bereits). `appl_exit` fasst die Liste nicht an.
   *Akzeptanz:* pid 2 lädt, ruft **kein** `rsrc_free`, endet per `Pterm`; pid 1
   `rsrc_gaddr` unverändert; pid 2s Eintrag ist weg (ein neues Programm mit pid 2
   sieht „nichts geladen"); `pd::open(2)` erbt keinen Ressourceneintrag.
7. **`rsrc_load` ohne `appl_init`** funktioniert (R7). *Akzeptanz:* Hosttest mit
   einer pid ohne `pd::open`.
8. **Kommentar- und Spec-Kopien angleichen** (Regel „eine Begründung steht selten
   nur einmal"): `tos-api/src/aes/resource.rs:70-76` (Modulkopf, „read-back half is
   deliberately not adopted"), `:1571-1573` (`report_load`-Doku), `:1698-1702`
   (`rsrc_free`-Kommentar). Die Spec ist mit diesem Dokument geändert
   (`docs/api/aes/resource-fileselector.md` § `rsrc_load`).
9. **H05-Injektorsequenz mit QED als Kind** (QEMU `raspi4b`, Methode
   `measurement-2026-09-18-h05/method/`, `.NAP` aus TeraDesk starten und beenden):
   Infozeile („… bytes used in … items") und Datei-Icons nach der Rückkehr wie vor
   dem Start; kein `rsrc_gaddr` von pid 1 mit 0 im Trace. QED ruft `rsrc_load`
   (cflib `appinit`, H05 § Verifikation), also greift A39 hier — anders als bei
   `MINIGEM.NAP`, das nie lädt.
10. **Nach A60 (Stufe 2):** TeraDesk-LP64 `load_icons`-Sequenz auf QEMU: nach dem
    Start sind die Strings (`rsrc_gaddr(R_STRING)`) der Infozeile korrekt; mit
    testweise ausgeliefertem `cicons.rsc` bleibt `desktop.rsc` nach dem
    Zurückstellen die aktive Ressource.

## 7. Offene Fragen, die der Anwender entscheiden muss

1. **Reihenfolge A60 → A39 (§ 4.3).** Vollständig erst nach A60 (Variante A ohne
   Zwischenstand), oder Stufe 1 jetzt und Stufe 2 nach A60 (Variante A mit
   Zwischenstand), oder Variante B (Rückfall, nicht empfohlen). Vorschlag:
   Variante A mit Zwischenstand, weil A39 P1/demo-relevant ist und Stufe 1 für
   jede Anwendung ohne eigenes `AESGLOBAL`-Schreiben EmuTOS-gleich ist.
2. **Fremder Blob (§ 4.2, letzter Absatz).** „Nichts geladen" für einen Selektor,
   der keinen von der AES geladenen Blob dieses Prozesses nennt — oder soll der
   Kern wie EmuTOS einen anwendungseigenen Header parsen? Vorschlag: „nichts
   geladen"; kein bekannter Aufrufer, und XaAES prüft an dieser Stelle selbst.

Alles andere (kein Freigeben bei `appl_exit`, `AESGLOBAL` bleibt bei `rsrc_free`
stehen, `R_TREE` aus dem Blobfeld) folgt EmuTOS und braucht keine Entscheidung.

## 8. Out-of-scope observations

* `docs/api/aes/_state.md:274-276` sagt „one resource per application" — als
  EmuTOS-Beschreibung haltbar (ein `AESGLOBAL` nennt eine), aber § 3 zeigt zwei
  gleichzeitig geladene je Prozess; ein Halbsatz dort wäre angebracht.
* `docs/STATUS.md:400` (Eintrag 2026-09-14) und `TASK/A35-STATUS-DONE.MD:41` tragen
  die widerlegte Begründung als historisches Protokoll; nicht umgeschrieben.
* Ein per EL0-`Pexec` gestartetes Kind bekommt weder `pd::register` noch
  `set_app_dir` noch `close_client_if_open` (H05 § Offen, A28/A62-Nähe); `APP_DIR`
  ist derselbe Static-Fehler wie `RSC`, nur für `sh_find`.
* `cicons.rsc` wird nicht ausgeliefert; sobald es das wird, braucht TeraDesk (§ 3
  Schritt 2) Stufe 2 dieses Vorschlags, sonst löst `rsrc_gaddr(R_STRING)` im
  cicons-Blob auf.
* XaAES' `rsrc_free` gibt den Listenkopf frei, nicht den benannten Blob
  (`xa_rsrc.c:1305-1312`) — eine XaAES-Eigenheit, für TOS-Konformität irrelevant.
* `TASK/A35-STATUS-UNFINISHED.MD` ist durch `A35-STATUS-DONE.MD` überholt (Frage 1
  dort ist gelöst); der Lead-Auftrag zitierte noch die alte Fassung.
