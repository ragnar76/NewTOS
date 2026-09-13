# Audit 2026-09-11 — `appl_init` und der Resource-Ladepfad gegen EmuTOS

Prüfer: Auditor (unabhängig). Antwortsprache Deutsch, Bezeichner im Original.

## 0. Geprüfte Stände

| | |
|---|---|
| EmuTOS (gepinnt) | `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df` (2026-04-14, „Update GitHub Actions to use Node 24"), selbst nachgeprüft mit `git -C references/emutos log -1` |
| NewTOS | Branch `desk-rebuild`, HEAD `70caee212078b11ffa0a70afa1279bdd82532959` |
| Uncommittete Fremdstränge im Baum | `tos-api/src/aes/window.rs`, `tos-api/src/aes/pd.rs`, `kernel/src/{lib.rs,bootlog.rs,drivers/usb/**,hal/uart.rs}`, diverse `docs/`, `TASK/` — **unangetastet gelassen**; kein `git checkout/clean/stash/restore/reset` benutzt |
| Zielprofil | Falcon030/TOS4, EmuTOS `make 512`; `CONF_WITH_COLOUR_ICONS` ist dort **1** (`include/config.h:1266-1267`), `AES_VERSION 0x0340` (`config.h:1345-1347`) |
| Getestete Kernel | Release `005590fb…` (selbst gebaut), Dev `3c6d7a4a36ea…` (selbst gebaut — **derselbe Hash, den `TASK/HANDOVER-2026-09-11.md` §2 als den abstürzenden nennt**), sowie der ausgelieferte `tosdump/deploy-pi4/kernel8.img` `2fbf8462…` vom 2026-09-09 |

Hosttestlauf `cargo run -p xtask -- test` vor und nach dem Audit: **436 passed, 0 failed**.

---

## 1. Die zwei Störungen sind zwei verschiedene Fehler

Die Übergabe behandelt „QED findet seine Resource nicht" und „TeraDesk paniert"
als eine Spur mit gemeinsamer Ursache („das Laden der Resource schlägt fehl").
Das ist **falsch**, und beides ist reproduziert:

* **QED** scheitert an `rsrc_load`, weil es einen **Pfad** statt eines nackten
  Dateinamens übergibt (Befund B1).
* **TeraDesk** lädt `desktop.rsc` über `rsrc_load` **erfolgreich** — es übergibt
  einen nackten Namen. Seine Panik kommt aus `icons.rsc`, das TeraDesk mit
  seinem **eigenen** Lader einliest (`sdk/apps/teradesk/rsc_load.c`
  `xrsrc_load`), und aus ungerade ausgerichteten Rasterzeigern darin
  (Befund B2). `rsrc_load` ist an TeraDesks Panik **nicht beteiligt**.

---

## 2. Befunde, nach Schwere

### B1 — `rsrc_load` kann einen Dateinamen mit Pfad grundsätzlich nicht auflösen — **CONFIRMED, echter Fehler**

**EmuTOS.** `rs_load` (`aes/gemrslib.c:826-850`):

```c
strcpy(tmprsfname,rsfname);          /* char tmprsfname[MAXPATHLEN]  (:46) */
if (!sh_find(tmprsfname)) return FALSE;
dosrc = dos_open(tmprsfname,0);
```

`sh_find` → `findfile` (`aes/gemshlib.c:406-472`) sucht in vier Stufen:
(1) `rlr->p_appdir` + **Namensanteil** (`sh_name`), (2) **`pspec` unverändert**
(aktuelles Verzeichnis; damit wird jeder absolute oder relative Pfad gefunden),
(3) `\<name>` auf dem **aktuellen Laufwerk**, (4) der AES-`PATH=`.

**NewTOS.** `rsrc_load` (`tos-api/src/aes/resource.rs:892-974`):

```rust
let mut name = [0u8; 16];                  // :899
while i < name.len() - 1 { … }             // :904  → nach 15 Zeichen abgeschnitten
for prefix in [app_dir, b"C:\\GEMSYS\\RSC\\", b"C:\\"] { … }   // :937
```

Drei Abweichungen auf einmal:

1. **Der Name wird hart auf 15 Zeichen gekürzt.** `C:\APPS\QED\qed.rsc` (19 B)
   wird zu `C:\APPS\QED\qed`.
2. **Die Stufe „Spezifikation unverändert" fehlt ganz.** Es wird *immer* ein
   Präfix davorgesetzt; ein übergebener Pfad kann deshalb nie zutreffen
   (`C:\APPS\QED\` + `C:\APPS\QED\qed` …).
3. `sh_find` wird nicht benutzt, obwohl es im Baum existiert und weitgehend
   getreu ist (`tos-api/src/aes/shell.rs:779-832`); statt Stufe (3) „Wurzel des
   **aktuellen** Laufwerks" steht fest `C:\`, und `C:\GEMSYS\RSC\` ist eine
   Erfindung ohne EmuTOS-Vorbild.

**Beobachtbarkeit / Fehlerfall.** QED ruft dreimal mit einem **qualifizierten**
Pfad (`sdk/apps/qed/src/rsc.c:50-72`):

```c
strcpy(rsc_path, gl_appdir); strcat(rsc_path, "qed.rsc"); rsc_init = rsrc_load(rsc_path);
… path_from_env("QED", …) …
strcpy(rsc_path, "qed.rsc"); shel_find(rsc_path); rsc_init = rsrc_load(rsc_path);
```

Auch der dritte Versuch scheitert, weil unser `shel_find` den Namen **findet**
und dabei zum vollen Pfad umschreibt (`shell.rs:800`), den `rsrc_load` dann nicht
mehr auflösen kann. Auf EmuTOS greift in genau diesem Fall `findfile`-Stufe (2).

**Reproduziert (QEMU `raspi4b`, 640×480, `#Z`-Autostart, Kernel `005590fb…`):**

* `C:\APPS\QED\QED.NAP` → Alarm „Resource nicht gefunden! / Resource file not
  found!" (`/Users/ingouhlemann/.nt-audit/run3.png`) — das Bild, das der Anwender
  meldet.
* **A/B, nur die Pfadform geändert:** dieselbe `QED.NAP` und dieselbe `qed.rsc`
  nach `C:\` kopiert, Autostartzeile `#Z 01 QED.NAP@`. Dann ist `APP_DIR` leer,
  `gl_appdir` leer, `rsrc_load` bekommt den **nackten** Namen `qed.rsc` — und QED
  startet mit voller Menüleiste (`/Users/ingouhlemann/.nt-audit/run4.png`).

Damit ist die Ursache eingekreist auf die Form des übergebenen Namens, nicht auf
Dateisystem, Kernelstand oder Karte.

**Folge-Abweichungen im selben Pfad (beide latent):**

* Der zusammengesetzte Pfadpuffer ist `[u8; 64]` und wird **stillschweigend**
  gekürzt (`resource.rs:941-948`); EmuTOS rechnet mit `MAXPATHLEN`.
* `APP_DIR_CAP = 64` (`resource.rs:840`) mit stillem `min` in `set_app_dir`
  (`:862`).

---

### B2 — Ungerade ausgerichtete Rasterzeiger → Kernelpanik in `blit_plane` — **CONFIRMED, echter Fehler (zwei Stellen)**

**Die Panik.** `tos-api/src/aes/object.rs:3006`:

```rust
let bits = unsafe { core::slice::from_raw_parts(addr as *const u16, total) };
```

`addr` ist `ib_pmask` / `ib_pdata` (`object.rs:2927-2928`) bzw. `bi_pdata`
(`:2893`) direkt aus der Resource. `addr == 0` ist geprüft (`:3000`), die
Ausrichtung nicht. Rusts `ub_checks` schlagen in einem **Debugbau** zu; im
Releasebau ist die Prüfung wegkompiliert und der unausgerichtete Zugriff läuft
auf AArch64 stillschweigend durch (formal UB).

**Die Datenursache.** `tools/mkrsc.py` legt Rasterdaten ohne Ausrichtung ab:
`intern_str` und `intern_bytes` hängen beide unmittelbar an denselben
`data`-Puffer an (`tools/mkrsc.py:200-223` für `convert`, `:385-408` für
`convert_rshdr`). Ein Icon-Text beliebiger Länge kann deshalb das nächste Raster
auf eine **ungerade** Adresse schieben. In einer klassischen Atari-`.rsc` kann
das nicht vorkommen (m68k-WORD-Zugriff).

Gemessen an den ausgelieferten Dateien:

| Datei | ungerade Raster |
|---|---|
| `sdk/apps/teradesk/build/dist/icons.rsc` (native-RSHDR) | **18 von 38** ICONBLK (u. a. `ib[1] "HARD DISC"`, `ib[2] "CD-ROM"`, `ib[11] "UP"`) |
| `tos-api/src/aes/gem_fsel.nrsc` (kerneleigene GEM.RSC) | **3 von 3** BITBLK (`bi_pdata` = 3321, 3449, 3577) |
| `sdk/apps/qed/build/qed.rsc`, `…/desktop.rsc` | 0 |

**Reproduziert und kausal belegt (QEMU `raspi4b`, `#Z 01 C:\APPS\TERADESK\DESKTOP.NAP@`):**

| Kernel | `icons.rsc` | Ergebnis |
|---|---|---|
| Dev `3c6d7a4a…` (= der Kernel aus der Übergabe) | ausgeliefert (18 ungerade) | **`[PANIC] … slice::from_raw_parts requires the pointer to be aligned … at tos-api/src/aes/object.rs:3006:25`** — Wort für Wort die gemeldete Panik |
| Dev `3c6d7a4a…`, **bitgleich** | dieselbe Datei, nur mit wortausgerichteten Rastern neu erzeugt | **keine Panik**, TeraDesk zeichnet Menüleiste + Laufwerks-, Drucker- und Papierkorbsymbol |
| Release `005590fb…` | ausgeliefert | keine Panik (Prüfung wegkompiliert) |
| Ausgeliefert `2fbf8462…` (2026-09-09) | ausgeliefert | keine Panik |

Weil zwischen Zeile 1 und Zeile 2 **nur die Datei** verändert wurde und das
Kernelabbild identisch blieb, ist damit belegt: **keine Quelltextänderung im
Baum — insbesondere nicht `window.rs` — verursacht die Panik.** Der Unterschied
„gestern lief es" erklärt sich aus dem Bauprofil (Release ohne `ub_checks` gegen
Dev mit); das Skript `tools/build-*`/`cargo xtask build` baut ohne `--release`
den Debugbau (Projektnotiz *flash-release-builds-to-real-HW*).

**Zwei getrennte Fehler, beide echt:**

* **B2a (Werkzeug):** `mkrsc.py` verletzt die Wortausrichtung der Raster in
  beiden Ausgabeformaten. Trifft auch die kerneleigene `gem_fsel.nrsc`.
* **B2b (Kernel):** `blit_plane` darf einen unausgerichteten Zeiger nicht in
  `from_raw_parts` geben. EmuTOS hat an dieser Stelle keinen vergleichbaren
  Abbruch; auf dem Pi4 ist der unausgerichtete 16-Bit-Zugriff hardwareseitig
  erlaubt. Dass Release und Debug bei gleicher Eingabe unterschiedlich enden,
  ist für sich schon ein Mangel — der Releasebau begeht UB.

Artefakte: `/Users/ingouhlemann/.nt-audit/{run5.log,run6.log,run6.png,run7.log}`,
Erzeugerskript `/Users/ingouhlemann/.nt-audit/mkrsc_aligned.py` (Prüfstand, kein
Produktcode).

---

### B3 — `rsrc_load` füllt `ap_ptree`, `ap_rscmem`, `ap_rsclen` nie — **CONFIRMED, echter Fehler**

EmuTOS belegt beim Laden drei `AESGLOBAL`-Felder:
`rs_global->ap_rscmem = rs_hdr; rs_global->ap_rsclen = rslsize;`
(`aes/gemrslib.c:787-788`) und `rs_global->ap_ptree = (OBJECT **)ptreebase;`
(`:597`, in `fix_trindex`). `rs_sglobe` (`:675-679`) liest sie bei **jedem**
`rs_free`/`rs_gaddr`/`rs_saddr` aus dem übergebenen `pglobal` zurück.

NewTOS: `pb.global_ptr()` hat im ganzen Baum **genau einen** Aufrufer,
`tos-api/src/aes/appl.rs:268` (`appl_init`). `rsrc_load`
(`resource.rs:892-974`) fasst den `AESGLOBAL` nicht an.

**Beobachtbar.** TeraDesk liest die Felder direkt:
`icon.c:2707-2708` (`OBJECT **svtree = _AESrscfile; void *svrshdr = _AESrscmem;`
= `aes_global[5]` / `aes_global[7]`), `icon.c:2692`
(`if ((void *)obj->ob_spec.index == _AESrscmem)`) und `rsc_load.c:417-423,
485-486, 504-508`. Nach unserem erfolgreichen `rsrc_load` stehen dort 0.
Dass TeraDesk aktuell trotzdem läuft, liegt nur daran, dass es die Felder für
seinen eigenen Lader selbst setzt — es ist kein Freispruch.

Bestätigt Befund **A1** aus `docs/analysis/applinit-lifecycle-2026-09-11.md`.

---

### B4 — Nur *referenzierte* TEDINFO/ICONBLK/BITBLK werden relokiert — **CONFIRMED, echter Fehler**

EmuTOS relokiert die drei Felder **arrayweise** über die Kopfzähler
(`aes/gemrslib.c:798-805`):

```c
fix_tedinfo_std();                       /* rsh_nted */
ibcnt = rs_hdr->rsh_nib;
fix_nptrs(ibcnt, R_IBPMASK); fix_nptrs(ibcnt, R_IBPDATA); fix_nptrs(ibcnt, R_IBPTEXT);
fix_nptrs(rs_hdr->rsh_nbb, R_BIPDATA);
fix_nptrs(rs_hdr->rsh_nstring, R_FRSTR);
fix_nptrs(rs_hdr->rsh_nimages, R_FRIMG);
```

NewTOS relokiert eine Unterstruktur nur, wenn ein OBJECT sie zeigt
(`resource.rs:653-678` LP64, `:509-538` ILP32), mit Entdopplung über
`substruct_seen`. Ein Satz, den kein Objekt referenziert, behält rohe
Dateioffsets.

**Getrieben (Hosttest, rot):** auf der echten `gem_fsel.nrsc`
(3 BITBLK, kein `G_IMAGE`-Objekt) liefert `rsrc_gaddr(R_BITBLK, 0)` einen Satz
mit `bi_pdata = 3321` statt `Basis + 3321`:

```
thread 'every_bitblk_pdata_is_relocated' panicked …:
bi_pdata = 3321 is still a raw file offset (blob base 4348376000);
EmuTOS fix_nptrs(nbb, R_BIPDATA) would have made it base+offset
```

**Fehlerfall:** eine Anwendung, die ein freies Bild über
`rsrc_gaddr(R_FRIMG/R_IMAGEDATA/R_BITBLK, i)` holt und zeichnet, bekommt einen
wilden Zeiger (kleine Ganzzahl) — bei uns zusätzlich sofort die Panik aus B2b.
Dasselbe gilt für ein unreferenziertes TEDINFO (`te_ptext` als roher Offset).

---

### B5 — `fix_tedinfo_std` fehlt: `te_txtlen` wird nicht neu berechnet, der Textpuffer ist aber verkürzt — **CONFIRMED, echter Fehler mit Überlauf**

EmuTOS (`aes/gemrslib.c:655-669`):

```c
if (fix_ptr(R_TEPTEXT, ii)) ted->te_txtlen = strlen(ted->te_ptext) + 1;
if (fix_ptr(R_TEPTMPLT, ii)) ted->te_tmplen = strlen(ted->te_ptmplt) + 1;
```

NewTOS hat dazu **nichts**; der Wert aus der Datei bleibt stehen. Für sich wäre
das noch eine stille Abweichung — gefährlich wird es durch das Zusammenspiel mit
`mkrsc.py`: `intern_str` kopiert nur bis zum ersten NUL
(`tools/mkrsc.py:205-207`), während eine klassische `.rsc` hinter `te_ptext`
volle `te_txtlen` reservierte Bytes stehen hat. Der umgewandelte Puffer ist also
kürzer als `te_txtlen`.

`objc_edit` begrenzt seine Schreibvorgänge genau an `te_txtlen`
(`tos-api/src/aes/object.rs:1457`, `ins_char(text, idx, ic, txtlen)` `:1556`).

Gemessen an den ausgelieferten Dateien:

| Datei | TEDINFOs mit `te_txtlen` > Puffer | schlimmster Fall |
|---|---|---|
| `tos-api/src/aes/gem_fsel.nrsc` | **11 von 13** | `ted[0]`: Puffer 1 B, `te_txtlen` 39 → 38 B Überlauf, direkt in die Schablone `"____…"` desselben Feldes |
| `sdk/apps/teradesk/build/dist/desktop.rsc` | **6 von 104** | `ted[23]`: Puffer 1 B, `te_txtlen` 34 |
| `sdk/apps/qed/build/qed.rsc` | 0 | — |

**Getrieben (Hosttest, rot):**

```
thread 'tedinfo_txtlen_is_recomputed_from_the_text' panicked …:
te_txtlen was not recomputed; objc_edit will write te_txtlen bytes into a
strlen+1-byte buffer   left: 39  right: 1
```

**Fehlerfall:** Tippen in ein editierbares Feld eines geladenen `.RSC`
überschreibt die im Blob folgenden Zeichenketten → verfälschte Beschriftungen,
im ungünstigen Fall ein nicht mehr NUL-terminierter Text. Der AES-Dateiselektor
selbst ist **nicht** betroffen, weil `fsel::set_ted`
(`tos-api/src/aes/fsel.rs:246-271`) `te_ptext` auf eigene Kernelpuffer umbiegt
und `te_txtlen = cap` setzt — das ist eine Umgehung, keine Behebung.

---

### B6 — Ein zu kurz gelesenes Blob wird angenommen und über seinen Puffer hinaus relokiert — **CONFIRMED, echter Fehler**

EmuTOS liest die ganze Datei und bricht bei Kurzlesung ab
(`aes/gemrslib.c:775-783`): `dos_alloc_anyram(rslsize)` … `if (dos_read(fd,
rslsize, rs_hdr) != rslsize) return FALSE;`

NewTOS liest in einen festen 64-KiB-Static (`RSC_CAP`, `resource.rs:802`) über
`bdos::fs::read_file`, das **stillschweigend auf `out.len()` kürzt**
(`tos-api/src/bdos/fs/mod.rs:142`, `let want = (entry.size as usize).min(out.len())`).
Geprüft wird danach nur `len < size_of::<NrscHeader>()` (`resource.rs:955`) —
nie, ob `len == h.total_size`. `relocate_blob_core` läuft anschließend über
`h.n_objects` Sätze und **schreibt** (`o.ob_spec += tb`) über das Pufferende
hinaus in den nachfolgenden Kernel-`.bss`.

**Getrieben (Hosttest):** derselbe Test auf einem halbierten `gem_fsel.nrsc`
endet nicht mit „abgelehnt", sondern mit

```
process didn't exit successfully: … (signal: 10, SIGBUS: access to undefined memory)
```

d. h. die Relokation greift tatsächlich außerhalb. Auslösbar über die
dokumentierte API mit jeder `.RSC` > 64 KiB (TeraDesks `cicons.rsc` ist klassisch
63 788 B, die native Fassung wäre größer).

Bestätigt und verschärft Befund **A8** der Vorarbeit: es ist keine bloß „latente"
Kappung, sondern ein Schreibzugriff außerhalb.

---

### B7 — Desktop und das erste gestartete Programm teilen sich eine AES-Prozesskennung — **CONFIRMED, echter Fehler**

`appl::current_pid()` (`tos-api/src/aes/appl.rs:191-199`) liefert im
Kernelkontext (Desktop, `RUN == null`) **1**, und für GEMDOS-Slot 0
`slot + 1` = **1**. `aes::pid_of_slot(0)` (`aes/mod.rs:506-508`) bestätigt es.

Folge: `pd::register(1, path)` beim Start (`launch.rs:102`) benennt den
**Deskriptor des Desktops** um; das `appl_init` des Kindes bekommt über
`pd::open(1)` (`pd.rs:295-311`) denselben Deskriptor samt Flags und
Nachrichtenschlange; und `close_client_if_open(1)` nach dem `Pterm`
(`launch.rs:141`) **löscht den Deskriptor des Desktops**.

Bei EmuTOS bekommt jeder Prozess seine eigene `AESPD` mit eigener `p_pid`
(`aes/gempd.c`, `pstart`/`getpd`); zwei lebende AES-Klienten teilen nie eine
Kennung.

**Getrieben (Hosttest, rot):**

```
thread 'the_desktop_and_the_first_launched_app_get_different_aes_ids' panicked …:
the in-kernel desktop and the app in process slot 0 share one AES process
descriptor   left: 1  right: 1
```

Im hier reproduzierten `#Z`-Autostartlauf **nicht** wirksam, weil der eingebaute
Desktop dann gar nicht läuft. Auf dem Klickweg (Desktop → Doppelklick → App)
ist er wirksam — genau dort meldet der Anwender ebenfalls Absturz.

Verwandt, aber nicht identisch mit A5 der Vorarbeit (die beschreibt die
Deskriptorvererbung, nicht die Kennungskollision).

---

### B8 — An der Startgrenze wird gar nichts zurückgesetzt — **CONFIRMED, echter Fehler**

EmuTOS setzt unmittelbar vor `dos_exec` alle Prozessflags zurück
(`aes/gemshlib.c:633-636`):

```c
sh_show(fname); p_nameit(rlr, fname); p_setappdir(rlr, D.s_cmd);
rlr->p_flags = 0;
```

NewTOS' `p_nameit_and_appdir` (`kernel/src/shell_loop.rs:355-357`) führt **nur**
`p_setappdir` aus; `rlr->p_flags = 0` ist schriftlich zurückgestellt. Seit dem
A25-Fix (`pd.rs:309`, `slot.flags |= AP_OPEN` statt `flags = AP_OPEN;
queue.clear()`) setzt auch `appl_init` nichts mehr zurück. Zusammen mit B7
(gemeinsame Kennung) erbt das Kind Flags und Nachrichtenschlange des Vorgängers.

Bestätigt **A3** der Vorarbeit und den Befund B1 von
`TASK/A25-AUDIT-BEANSTANDET.MD`. `tos-api/src/aes/pd.rs` ist Fremdstrang — nicht
angefasst.

---

### B9 — Der Resourcezustand ist ein Kernel-Global statt am `AESGLOBAL` des Aufrufers — **CONFIRMED, echter Fehler**

EmuTOS' `rs_sglobe` (`aes/gemrslib.c:675-679`) holt `rs_hdr` bei **jedem**
Aufruf aus dem übergebenen `pglobal`; jede Anwendung hat ihre eigene Resource.
NewTOS hält einen einzigen `static RSC` (`resource.rs:263-289`), ein einziges
`USER_RSC_RAW` (`:832`) und ein einziges `APP_DIR` (`:848`).

Beobachtbar: ein zweiter `rsrc_load` ersetzt den ersten für alle Klienten
(Desktop + Kind); `rsrc_free` gibt danach den zuletzt geladenen Block frei;
`USER_RSC_RAW` wird beim zweiten Laden ohne Freigabe überschrieben
(`:1049`). Genau dieses Muster umgeht TeraDesk mit einem eigenen Lader.
Bestätigt **A2**.

---

### B10 — Die Koordinatenumrechnung (`ob_fix`) ist im Werkzeug festverdrahtet statt zur Ladezeit — **CONFIRMED, latente Abweichung**

EmuTOS führt `rs_fixit` → `fix_objects` → `rs_obfix` **beim Laden** aus
(`gemrslib.c:816-819, 606-637, 89-99`) und rechnet dabei mit den **lebenden**
`gl_wchar` / `gl_hchar` / `gl_width` (`fix_chpos`, `:53-83`, inklusive
`cpos == 80 → gl_width`).

NewTOS rechnet in `tools/mkrsc.py:125-139` um, mit
`WCHAR, HCHAR, SCREEN_W = 8, 16, 640` (`:85`), und `rsrc_load` rechnet
bewusst nicht nach (Modulkommentar `resource.rs:6-11`).

Fehlerfall: auf einer 1920×1080-Workstation liefert ein Objekt mit
`ob_width`-Zeichenzahl 80 weiterhin 640 px statt der Bildschirmbreite; jede
Dialogkoordinate bleibt am 8×16-Raster hängen, unabhängig vom aktiven
Systemzeichensatz. Für das 640×480-Profil unauffällig, deshalb *latent* — aber
über die API beobachtbar und kein hinnehmbarer Unterschied.

---

### B11 — CICON-Resourcen werden vom Werkzeug abgewiesen, obwohl das Zielprofil sie kann — **CONFIRMED, Funktionslücke**

`CONF_WITH_COLOUR_ICONS` ist im 512er-Profil **an** (`include/config.h:1266-1267`);
`rs_readit` ruft `fix_cicons()` (`gemrslib.c:795-797`), und `fix_objects`
setzt `obj->ob_spec = (LONG)ciconblkptr[obj->ob_spec]` (`:623-626`).
`tools/mkrsc.py:179-180` bricht dagegen mit
`sys.exit("new-format (CICONBLK) .rsc not supported")` ab, und der
Relokationskern behandelt `G_CICON` als „nichts zu tun" (`resource.rs:652`).
TeraDesks `cicons.rsc` wird deshalb bewusst nicht ausgeliefert
(`tools/make-sd-image.sh:274-276`). Die Zeichenseite (`object.rs:2539-2555`)
zeichnet den Monoteil eines CICONBLK korrekt — es fehlt der Ladepfad.

---

### B12 — Nebenbefunde im selben Pfad (kein Reparaturzwang, aber zu benennen)

| | EmuTOS | NewTOS | Bewertung |
|---|---|---|---|
| `rsrc_gaddr` „nicht gefunden" | `get_addr` liefert für `R_TREE`/Arrays immer eine berechnete Adresse, `-1L` nur bei unbekanntem Typ (`gemrslib.c:112-176`), `ret = (*rsaddr != -1)` | `addr_out[0] = 0`, `ret = 0` bei fehlender Tabelle **oder** Indexüberlauf (`resource.rs:1130-1143, 1281-1288`) | bewusste, sicherere Abweichung — aber im **Rückgabewert** beobachtbar; im Code als solche dokumentiert |
| `rsrc_gaddr(R_STRING, i)` bei NULL-Slot | `*rsaddr = NULL`, `ret = 1` | `ret = 0` (`deref_slot` `:1173-1175`) | Abweichung im Rückgabewert |
| `MAX_TREES = 64` (`resource.rs:155`) | unbegrenzt (`ap_ptree[rsindex]`) | Baum ≥ 64 nicht adressierbar | latent |
| `appl_init` kann `-1` liefern | `ap_init` hat keinen Fehlerpfad (`gemaplib.c:54-63`) | `-1` bei voller 8er-Tabelle (`appl.rs:246-250`) — und schreibt dann **keins** der sechs `AESGLOBAL`-Felder, während EmuTOS sie *vor* `ap_init()` setzt (`gemsuper.c:92-98`) | echte, wenn auch schwer erreichbare Abweichung; bestätigt **A6** |
| `appl_exit` | `ap_exit` löscht nur `AP_OPEN`, der `AESPD` bleibt samt `p_name` (`gemaplib.c`) | `pd::close` nullt den ganzen Deskriptor (`pd.rs:434-440`) | `appl_find(name)` danach `-1` statt Treffer — Nebenbefund |
| `rsrc_free` Rückgabe | `1`, wenn `ap_rscmem != NULL`; Doppel-`free` → `0` (`gemrslib.c:685-708`) | `1`, wenn `obj_base != 0`; Doppel-`free` → `0` (`resource.rs:1081,1109`) | gleichwertig, kein Befund |
| `ap_version`, `ap_count`, `ap_private`, `ap_planes`, `ap_3resv` | `gemsuper.c:92-98` | `appl.rs:252-259` | geprüft, deckungsgleich; durch `tos-api/tests/aes_appl_init.rs` abgedeckt |

---

## 3. Was ich ausgeschlossen habe

* **`window.rs`, `pd.rs` oder irgendeine andere Quelltextänderung als Ursache
  der TeraDesk-Panik.** Ausschlussargument: die Panik verschwindet, wenn **nur**
  `icons.rsc` wortausgerichtet neu erzeugt wird — bei **bitgleichem**
  Kernelabbild (B2). Damit ist die Übergabeaussage „`window.rs` bleibt der
  zeitlich nächstliegende Kandidat" gegenstandslos.
* **`rsrc_load` als Ursache der TeraDesk-Panik.** TeraDesk lädt `desktop.rsc`
  mit nacktem Namen erfolgreich (belegt: mit ausgerichteter `icons.rsc` zeichnet
  es Menü **und** Symbole, `run6.png`); seine Symbole kommen aus `icons.rsc`
  über den **eigenen** Lader (`icon.c:2736`, `rsc_load.c:444-476`).
* **Die Aussage „TeraDesk arbeitet mit einem leeren oder halben Objektbaum
  weiter"** (Übergabe §1). Widerlegt: der Baum ist vollständig.
* **Die Slot-/`HELLO.NCP`-Argumentation** aus der Vorarbeit als Erklärung der
  Panik — sie ist in meinem Abbild vorhanden und die Panik hängt nachweislich
  nicht daran (Zeile 2 der Tabelle in B2 hat dieselbe `HELLO.NCP`).
* **Das Dateisystem / die Karte als Ursache von B1.** Dieselbe Datei wird
  gefunden, sobald der Name ohne Pfad ankommt (A/B in B1).

---

## 4. Was unentschieden bleibt

1. **Der ILP32-Relokationszweig** (`relocate_blob_core_ilp32`, `resource.rs:479-560`)
   ist nicht getrieben worden: seit 2026-09-04 gibt es keinen ILP32-Bau mehr.
   Ungeprüft, nicht „in Ordnung".
2. **Der EL0-Abbildungspfad** `load_into_user` (`resource.rs:990-1051`) ist nur
   auf dem glücklichen Weg beobachtet (QED/TeraDesk laden). `Mxalloc`-Fehler,
   erschöpftes Fenster, zweites `rsrc_load` im selben Prozess und `rsrc_free`
   aus dem Kernelkontext heraus sind **nicht** getrieben.
3. **Ob der Anwender auf echter Pi-4 dieselbe Panik sieht** — mein Nachweis ist
   QEMU; das Bauprofil erklärt den Unterschied, aber die Karte des Anwenders hat
   anderen Inhalt (4 Partitionen). Hardwareabnahme offen.
4. **Der Klickweg Desktop → TeraDesk** (B7/B8 werden erst dort wirksam) konnte
   ohne Eingabegerät in QEMU nicht gefahren werden. Der Anwender meldet auch dort
   einen Absturz; ob es derselbe ist, ist **unbelegt** — ich konstruiere dafür
   keine Kausalkette.
5. **Warum EmuTOS `te_txtlen` überhaupt auf `strlen+1` herabsetzt** (B5) — die
   Wirkung ist belegt, die Absicht des DR-GEM-Erbes nicht recherchiert. Für den
   Befund unerheblich: entscheidend ist, dass unser Puffer kürzer ist als der
   Wert, mit dem wir schreiben.

---

## 5. Was ich ausgeführt habe

| Lauf | Was | Ergebnis |
|---|---|---|
| Hosttests | `cargo run -p xtask -- test`, vor und nach dem Audit | 436 passed, 0 failed |
| Hosttests (neu, rot) | `tos-api/tests/audit_resource_2026_09_11.rs`, 4 Fälle, einzeln mit `--test-threads=1` | **4 von 4 rot** (B4, B5, B6, B7), Ausgaben oben zitiert |
| QEMU 1–2 | Release `005590fb…`, `#Z` TeraDesk | läuft, Symbole da (`run2.png`) |
| QEMU 3 | Release, `#Z C:\APPS\QED\QED.NAP` | „Resource nicht gefunden" (`run3.png`) — Anwendermeldung reproduziert |
| QEMU 4 | Release, `#Z QED.NAP` (Wurzel, nackter Name) | QED startet (`run4.png`) — A/B für B1 |
| QEMU 5 | **Dev `3c6d7a4a…`**, `#Z` TeraDesk, ausgelieferte `icons.rsc` | **Panik `object.rs:3006`** — Anwendermeldung wörtlich reproduziert |
| QEMU 6 | Dev `3c6d7a4a…`, nur `icons.rsc` wortausgerichtet | keine Panik, Desktop vollständig (`run6.png`) — Kausalnachweis für B2 |
| QEMU 7 | Ausgeliefertes `2fbf8462…` (09-09), ausgelieferte `icons.rsc` | keine Panik |
| Dateiprüfungen | Kopf-/Strukturdumps von `desktop.rsc`, `icons.rsc` (klassisch **und** konvertiert), `qed.rsc`, `gem_fsel.nrsc`, `references/emutos/aes/gem.rsc` | Zahlen in B2/B5 |

Nach jedem Lauf `pgrep -f qemu-system-aarch64` geprüft — **kein QEMU-Prozess
blieb stehen**. Keine Hardware angefasst, nichts auf die Karten des Anwenders
geschrieben, nichts ausgeliefert. Prüfstand liegt unter
`/Users/ingouhlemann/.nt-audit/` (eigenes Kurzpfadverzeichnis wegen der
macOS-Längengrenze für QMP-Sockets).

---

## 6. Aufräumen

* Die vier roten Hosttests standen während des Laufs als
  `tos-api/tests/audit_resource_2026_09_11.rs` im Baum und sind **von Hand
  wieder entfernt** worden, damit die Suite grün bleibt; die Datei liegt
  unverändert unter `/Users/ingouhlemann/.nt-audit/audit_resource_2026_09_11.rs`
  und im Anhang unten, zum Wiedereinsetzen als Regressionstest nach der
  Reparatur.
* **Kein Produktcode geändert.** `git status --porcelain` ist vor und nach dem
  Audit zeichengleich — bis auf dieses Dokument.
* Kein `git checkout`, `clean`, `stash`, `restore` oder `reset` ausgeführt.
* `kernel8.img` im Wurzelverzeichnis ist ein Bauartefakt (`.gitignore:8`) und
  steht wieder auf dem Dev-Stand `3c6d7a4a…`, also auf dem Stand vom Beginn.

---

## Anhang — die vier roten Tests

```rust
//! AUDIT 2026-09-11 — red tests for the `appl_init` / `rsrc_*` audit.
//! Each test asserts the EmuTOS-documented behaviour (cited per test).
//! Run with `--test-threads=1`: the AES resource descriptor is one global.
use tos_api::bdos::process::Abi;

const R_TEDINFO: i16 = 2;
const R_BITBLK: i16 = 4;

const FSEL: &[u8] = include_bytes!("../src/aes/gem_fsel.nrsc");

fn blob(src: &[u8]) -> (Vec<u64>, *mut u8, u64) {
    let mut v = vec![0u64; src.len().div_ceil(8) + 1];
    let p = v.as_mut_ptr() as *mut u8;
    unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), p, src.len()) };
    let base = p as u64;
    (v, p, base)
}

fn gaddr(rtype: i16, index: i16) -> (i16, u64) {
    let mut control = [112i16, 2, 1, 0, 0];
    let mut global = [0i16; 16];
    let mut intin = [rtype, index, 0, 0];
    let mut intout = [0i16; 8];
    let mut addrin = [0u64; 2];
    let mut addrout = [0u64; 2];
    let raw = [
        control.as_mut_ptr() as u64,
        global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64,
        addrout.as_mut_ptr() as u64,
    ];
    unsafe { tos_api::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64) };
    (intout[0], addrout[0])
}

fn cstrlen(p: u64) -> usize {
    let mut n = 0usize;
    unsafe { while *(p as *const u8).add(n) != 0 { n += 1; } }
    n
}

/// EmuTOS aes/gemrslib.c:803 — fix_nptrs(rsh_nbb, R_BIPDATA).
#[test]
fn every_bitblk_pdata_is_relocated() {
    let (_v, p, base) = blob(FSEL);
    assert!(unsafe { tos_api::aes::resource::relocate_and_register(p) });
    let (ret, addr) = gaddr(R_BITBLK, 0);
    assert_eq!(ret, 1, "rsrc_gaddr(R_BITBLK, 0) must resolve");
    let pdata = unsafe { (addr as *const u64).read_unaligned() };
    assert!(pdata > base,
        "bi_pdata = {pdata} is still a raw file offset (blob base {base})");
}

/// EmuTOS aes/gemrslib.c:660-668 — fix_tedinfo_std.
#[test]
fn tedinfo_txtlen_is_recomputed_from_the_text() {
    let (_v, p, _base) = blob(FSEL);
    assert!(unsafe { tos_api::aes::resource::relocate_and_register(p) });
    let (ret, addr) = gaddr(R_TEDINFO, 0);
    assert_eq!(ret, 1);
    let te_ptext = unsafe { (addr as *const u64).read_unaligned() };
    let te_txtlen = unsafe { ((addr + 36) as *const i16).read_unaligned() };
    assert_eq!(te_txtlen as usize, cstrlen(te_ptext) + 1,
        "te_txtlen was not recomputed");
}

/// EmuTOS aes/gemrslib.c:782-783 — a short read fails the load.
#[test]
fn a_truncated_blob_is_rejected() {
    let cut = FSEL.len() / 2;
    let (_v, p, _base) = blob(&FSEL[..cut]);
    assert!(!unsafe { tos_api::aes::resource::relocate_and_register(p) },
        "a blob shorter than its declared total_size was accepted");
}

/// EmuTOS aes/gempd.c — every process gets its own p_pid.
#[test]
fn the_desktop_and_the_first_launched_app_get_different_aes_ids() {
    let mut control = [10i16, 0, 1, 0, 0];
    let mut global = [0i16; 16];
    let mut intin = [0i16; 4];
    let mut intout = [0i16; 8];
    let mut addrin = [0u64; 2];
    let mut addrout = [0u64; 2];
    let raw = [
        control.as_mut_ptr() as u64,
        global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64,
        addrout.as_mut_ptr() as u64,
    ];
    unsafe { tos_api::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64) };
    assert_ne!(intout[0], tos_api::aes::pid_of_slot(0),
        "the in-kernel desktop and the app in process slot 0 share one AES pd");
}
```
