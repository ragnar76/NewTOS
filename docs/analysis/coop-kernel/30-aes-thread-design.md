# AES als Kernel-Thread — Design für den synchronen G_USERDEF-Callback (Approach B)

> Stand: 2026-07-26, Branch `desk-rebuild`. Design-Phase-Deliverable vor
> Implementierung. Richtungsentscheidung (Lead+User, siehe Aufgabenstellung):
> **Approach B** (XaAES `PROGDEF_BY_SIGNAL`-Idee: Upcall an den Client, AES
> blockiert/yieldet, Client führt sein Progdef aus und meldet den Rückgabewert
> über einen gemeinsamen Slot, die AES resümiert damit) — **nicht** Approach A
> (`do_callout`/`jsr`-Direktaufruf; auf AArch64 trifft das PXN, unser Crash).
> Dieses Dokument ist **nicht** die Neuverhandlung der Richtung — es entwirft
> den kleinsten AES-Thread, der B trägt.
>
> **Lesekonvention.** Jede *Mechanismus-Aussage* ist mit `datei:zeile` belegt
> (FreeMiNT/XaAES `references/freemint/…` oder NewTOS `kernel/…`/`tos-api/…`).
> Design-Entscheidungen sind **Empfehlung** (meine Wahl, begründet), echte
> Unentschiedenheiten **OFFEN** mit Optionen. Wo eine Quelle nicht belegbar war
> oder ich sie nicht bis zum Ende verfolgt habe, steht das explizit da.
>
> Vorgängerdokumente (in dieser Reihenfolge gelesen): `10-upcall-signal-design.md`
> (Baustein 1: generischer EL0-Upcall + `Psigreturn`, **gelandet**, `ee82ead`),
> `20-sleepwake-substrate-design.md` (Baustein 3: `sleep`/`wake` + S2-Restart +
> Deliver-on-Resume, **gelandet**, `bcf8008`/`59b60d4`/`d33f0fd`),
> `00-effort-comparison.md` (Fahrplan), `emutos-cooperative-model.md`
> (EmuTOS-AES-Scheduler-Referenz).

---

## 1. Ziel, Crash, warum B und nicht A

### 1.1 Der Crash

Ein EL0-GEM-Programm (TeraDesk) registriert einen eigenen Desktop-Hintergrund
via `wind_set(0, WF_NEWDESK, tree)` mit einem `G_USERDEF`-Objekt (`ub_bckbox`).
Der spätere Repaint läuft synchron **innerhalb** eines beliebigen `wind_*`-Traps
(z.B. `wind_open`/`wind_close`/eines Drag-Settle) über `compose()`
(`tos-api/src/aes/window.rs:634-669`) → `paint_bg_clip`
(`window.rs:571-612`, aufgerufen `window.rs:645` und erneut in
`repaint_vacated`/`drag_size`, `window.rs:2245,2247,2253,2255,2283`) →
`render_tree_app` (`tos-api/src/aes/object.rs:888-903`) → `draw_tree` → `just_draw`
(`object.rs:2356-2358`) → `ob_user` (`object.rs:1987-2031`). `ob_user` lädt das
`USERBLK`, baut ein `PARMBLK` und muss `ub_code(PARMBLK*)` aufrufen — eine
Funktion, die in TeraDesks **EL0**-Textsegment liegt.

**Heutiger Stand (Bring-up-Kompromiss, kein Fix):** `ob_user` unterscheidet über
`ObjTree::kernel_built()` (`object.rs:85-91,161-166`) EL1-eigene Bäume
(Desktop-`g_screen`, Alerts, Fileselector — `ObjTree::lp64`, `object.rs:107-124`)
von App-Bäumen (`ObjTree::app_at`, `object.rs:126-149`, explizit für genau diesen
WF_NEWDESK-Fall dokumentiert). Nur im kernel-built-Fall ruft `ob_user` `ub_code`
direkt auf (`object.rs:2017-2026`, ein simpler `extern "C" fn`-Aufruf — sicher,
weil EL1-Code EL1-Code aufruft); im App-Fall wird der Aufruf **unterdrückt**
(`object.rs:2027-2030`, `new_state` unverändert zurückgegeben) — kein Crash,
aber der Hintergrund zeichnet sich nicht.

### 1.2 Warum Approach A (direkter Aufruf) auf AArch64 unmöglich ist — PXN

`ub_code` liegt auf einer EL0-Seite. Jede TPA-Seite wird mit `PageFlags::UserRwExec`
gemappt (`tos-api/src/bdos/process/loader.rs:505-517`,
`tos-api/src/bdos/process/mmu.rs:135-162`), und **jedes** Preset setzt
`D_PXN = 1<<53` ("EL1 must not execute", `mmu.rs:89-90,137,140,158`) — der Kern
**darf diese Seite unter keinen Umständen ausführen**, unabhängig davon, wessen
Trap gerade läuft oder ob `TTBR0_EL1` "zufällig" schon auf den richtigen Prozess
zeigt. Ein `blr` auf `ub_code` von EL1 aus löst einen Instruction-Abort
(Permission Fault wegen PXN) aus — exakt der dokumentierte WF_NEWDESK-Crash
(`object.rs:139-141`: *"→ PXN instruction abort at the app callback's VA"*).

Das ist ein **härterer, unbedingter** Boundary als in XaAES (§2.3): auf m68k
teilt der Supervisor-Modus denselben Adressraum wie der User-Modus des
*gerade laufenden* Prozesses — ein `jsr` von Supervisor nach User ist dort ein
gewöhnlicher, sicherer Funktionsaufruf, solange Aufrufer und Ziel zum selben
Prozess gehören (was `do_callout`s einzige *Sicherheits*-Prüfung ist, nicht eine
Notwendigkeit: `CHECK_PROGDEF_ADDR`, §2.3). AArch64 kennt diese Ausnahme nicht:
PXN gilt **immer**, auch wenn der aufrufende Client und der Tree-Owner derselbe
Prozess sind (unser TeraDesk-Fall!). Deshalb braucht NewTOS **immer** einen
EL0-Umweg — nie einen direkten Aufruf, selbst im Ein-Client-Fall.

### 1.3 Warum "die AES als eigener Thread" — nicht nur ein weiterer Upcall

Der bereits gelandete Upcall-Mechanismus (`10-upcall-…md`) liefert **Handler an
den Syscall-Grenze**: `deliver_upcall` schnappt den Frame **am natürlichen Ende
eines Traps** (nach dem Ergebnis-Write, vor dem `eret`,
`kernel/src/arch/aarch64/exceptions.rs:410-427`) — der populäre Fall
"Client kehrt gleich zu EL0 zurück, aber zuerst soll ein Handler laufen".

`ob_user`s Stall ist **kein** solcher Grenzfall: er passiert **mitten** in einem
`wind_*`-Trap, mehrere Rust-Aufrufebenen unterhalb von `rust_sync_handler`
(`rust_sync_handler` → `aes::dispatch` → `window::wind_open`/`compose` →
`paint_bg_clip` → `render_tree_app` → `draw_tree` → `just_draw` → `ob_user`,
`tos-api/src/aes/mod.rs:172-260`). Der ursprüngliche `wind_open`-Aufruf ist
**nicht fertig** — er darf nicht "vorzeitig" mit `deliver_upcall`s
Ergebnis-Snapshot-Semantik behandelt werden (die annimmt: das Ergebnis in
`frame.gpr[0]` steht schon fest, `10-upcall-…md` §6.2/§7.2). Was gebraucht wird:
der komplette, mehrstufige Rust-Aufrufstapel von `ob_user` abwärts muss
**mitten in der Ausführung** angehalten werden (mit allen lokalen Variablen:
Objekt-Index, Restbaum, Clip-Zustand), die CPU muss den Client EL0-Code
(`ub_code`) laufen lassen, und **wenn** der fertig ist, muss **genau dieser**
Rust-Aufrufstapel — nicht der ursprüngliche `rust_sync_handler`-Trap — an
exakt der Stelle weiterlaufen, an der er pausiert hat.

Das ist strukturell identisch mit dem, was `20-sleepwake-substrate-design.md`
§4.3 als **"S1 — stack-preserving"** einführt und explizit auf **Schritt 4**
verschiebt: *"jeder Thread bekommt einen eigenen EL1-Kernel-Stack; `sleep()`
tauscht den EL1-SP … sodass der Rust-Rahmen den Block ÜBERLEBT"*
(`20-sleepwake-…md:310-319`), nötig *"sobald ein blockierender Syscall
nicht-reproduzierbaren mid-Rechenzustand hat, oder für echt nebenläufige
EL1-interne Threads mit eigenem Rust-Stack"* (`20-sleepwake-…md:355-358`).
**`ob_user`s Stall ist genau dieser Moment.** Dieses Dokument baut S1 — aber
minimal, nur für die AES, nicht für jeden Prozess.

---

## 2. XaAES seziert: Thread, Trap-Routing, Userdef(B)

Alle Zeilen aus `references/freemint/xaaes/src.km/` (m68k, FreeMiNT-Kernel-Modul).

### 2.1 Der AES-Kernel-Thread (`k_main`) — was er *wirklich* tut

`init.c:242-246` (Kommentar "Module initialisation … start main kernel
thread") → `init.c:676-684`: `kthread_create(NULL, k_main, NULL, &(C.Aes->p),
"AESSYS")` — ein echter FreeMiNT-Kernel-Thread (eigener `struct proc`, eigener
Stack, läuft im Kernel-Modus, teilt sich Kernel-Adressraum/CWD/Files/Signale
mit anderen Kernel-Threads, `k_main.c:1592-1602` Kommentar). **Aber:** Nach
Initialisierung (`k_main`, `k_main.c:1605-1690` — Semaphoren anlegen,
`p_domain`, `appl_init` **für sich selbst**, `register_trap2` §2.2, `k_init`)
tut der Thread fast **nichts** mehr: `init.c:693-696`
```c
while (!(C.shutdown & QUIT_NOW))
{
    sleep(WAIT_Q, (long)&loader_pid);
}
```
— er schläft bis zum Shutdown. Es gibt **keine** durchlaufende Nachrichten-Pumpe
"AES-Thread verarbeitet Client-Requests aus einer Queue". Der Thread existiert,
um **globalen AES-Zustand zu besitzen** (Bildschirm-Settings, Fensterliste,
Widget-Baum-Arena — `C.Aes`, `screen`, `window_list`, `C`-Struktur global über
alle Clients) und einmalig `k_init` (GSX/VDI öffnen) auszuführen — **nicht** um
jeden Client-Call durch ihn hindurchzuschleusen.

### 2.2 Der eigentliche Trap-Routing-Pfad läuft **im Aufrufer**, nicht im AES-Thread

`k_main.c:1675-1679`: `register_trap2(XA_handler, 0, 0, 0)` — registriert
`XA_handler` im FreeMiNT-Kernel als **Trap-#2-Handler für alle Prozesse
systemweit**. `handler.c:25-31` (Modulkommentar): *"the main AES trap 2
handler, called from the FreeMiNT kernel trap wrapper"*. Entscheidend:
`XA_handler` (`handler.c:479-…`) läuft **synchron im Kernel-Kontext des
aufrufenden Prozesses selbst** (derselbe `struct proc`, derselbe
Kernel-Stack-Slot des Aufrufers) — **nicht** als Nachricht an den
`AESSYS`-Thread! `aes_tab[cmd].f` (die 220-Eintrag-Sprungtabelle,
`handler.c:87-462`) wird **direkt aufgerufen**, im Aufrufer-Kontext
(`handler.c:612-…`, `cmd_routine(...)`, nicht gezeigt aber unzweideutig aus dem
Kontrollfluss). Das ist strukturell **identisch** zu NewTOS' heutigem
"Inline-AES": ein `svc`/Trap dispatcht direkt in die AES-Logik, auf demselben
Stack, ohne Kontextwechsel zu einem separaten Thread.

**Korrektur der Aufgabenstellung:** `mscall.c` (vom Auftrag als "die
Client→AES-Aufruf-/Message-Routing" benannt) enthält **nicht** diese Routing —
es sind vier triviale, unzusammenhängende Helfer für den Maus-Sichtbarkeits-
Zähler (`hidem`/`showm`/`forcem`/`tellm`, `mscall.c:8-33`). Die tatsächliche
Routing-Instanz ist `register_trap2`+`XA_handler` (`k_main.c:1675`,
`handler.c:479`), oben zitiert.

### 2.3 Der Userdef-Callout — `do_callout` (A, **aktiv**) vs. `PROGDEF_BY_SIGNAL` (B, **deaktiviert**)

`draw_obj.c:43-46`:
```c
/* call progdef-function via SIGUSR2
 * (not good because of possible VDI-calls inside signal-handler)
 */
#define PROGDEF_BY_SIGNAL	0
```

**Wichtige Korrektur der Aufgabenstellung.** In diesem gepinnten Checkout ist
`PROGDEF_BY_SIGNAL` **0** — **deaktiviert**. `do_callout` (Approach A, direkter
`jsr`, `draw_obj.c:331-354`) ist der **aktiv kompilierte** Pfad
(`#if !PROGDEF_BY_SIGNAL`, `draw_obj.c:439-472`); der Signal-Pfad B
(`draw_obj.c:473-492`, `#else`-Zweig) ist **totes/alternatives Legacy-Code**,
mit einem expliziten **dokumentierten Risiko** ("nicht gut wegen möglicher
VDI-Aufrufe im Signal-Handler"). Die Aufgabenstellung nennt B
"XaAES-proven" — das ist im gelesenen Quellstand **nicht zutreffend**: B ist
dokumentiert, aber **nicht der produktive Pfad**. Das ändert die
Lead-Entscheidung nicht (Grund für B bei uns ist PXN, ein AArch64-Spezifikum,
nicht XaAES' Motivation), aber das o.g. Risiko (VDI-Aufrufe während des
Callbacks) ist real und gehört in unser Risiko-Kapitel (§6).

`d_g_progdef` (`draw_obj.c:370-532`) läuft **im Kernel-Kontext des Prozesses,
der die Baum-Traversierung gerade ausführt** (`client =
lookup_extension(NULL, XAAES_MAGIC)`, `draw_obj.c:376` — "current process"),
**nicht** notwendigerweise im AES-Thread. Das bestätigt §2.2: auch der
Userdef-Callout ist strukturell ein Vorgang **im aufrufenden Client-Kontext**,
kein Client→Server-Round-Trip zu `AESSYS`.

**`do_callout`** (A, `draw_obj.c:331-349`) — ein reines Inline-Assembler-`jsr`
(`move.l %2,-(%%sp); jsr (%1); addq.l #4,%%sp`) — funktioniert auf m68k, weil
Supervisor- und User-Modus **denselben Adressraum** teilen (§1.2); die einzige
Prüfung ist eine **Sicherheits**-Grenze (`CHECK_PROGDEF_ADDR`,
`draw_obj.c:441-456`: ist die Adresse innerhalb `client->p->p_mem->base->p_tbase
.. +p_tlen`?), nicht eine technische Notwendigkeit.

**`PROGDEF_BY_SIGNAL`** (B, `draw_obj.c:439,473-492`):
```c
act.sa_handler = client->ut->progdef_p;
act.sa_mask = 0xffffffff;
act.sa_flags = SA_RESETHAND;
p_sigaction(SIGUSR2, &act, &oact);
raise(SIGUSR2);                      /* liefert SIGUSR2 SOFORT innerhalb dieses Aufrufs */
p_sigaction(SIGUSR2, &oact, NULL);   /* alten Handler wiederherstellen */
*wt->state_mask = *ret(client->ut);  /* Ergebnis aus dem geteilten Slot lesen */
```
`raise(SIGUSR2)` löst die Signal-Zustellung **synchron, noch im selben
Aufruf** aus (FreeMiNTs `sendsig`, s. `10-upcall-…md` §2 — dieselbe Maschinerie,
die unser eigener Upcall-Mechanismus schon 1:1 nachbaut). Der Handler
(`client->ut->progdef_p`, s. §2.4) läuft **im User-Modus des Clients**, ruft
`ub_code` **dort** auf (sicher — User-Modus darf User-Code ausführen), schreibt
das Ergebnis in den geteilten `_retcode`-Slot, kehrt via Trampolin zurück; erst
danach liest `d_g_progdef` `*ret(client->ut)`. Kommentar `draw_obj.c:515-518`:
*"Ozk: Since it is possible that taskswitches happens during the callout of
progdef's, we need to restore the clip-rect used by this 'thread'"* —
bestätigt: FreeMiNT kann während der Signal-Auslieferung tatsächlich andere
Prozesse laufen lassen (präemptiv); `d_g_progdef` behandelt das defensiv (Clip
neu setzen nach der Rückkehr, `draw_obj.c:524`), verlässt sich aber **nicht**
auf eine explizite Blockier-/Wartefunktion — der ganze Callout ist aus
FreeMiNT-Sicht ein **synchroner** `raise()`-Aufruf, der erst zurückkehrt, wenn
der Handler fertig ist (FreeMiNTs Signal-Semantik, nicht ein
Client/Server-Rendezvous).

### 2.4 `xa_user_things` — der Pro-Client-Trampolin-Template

**Korrektur:** Die Aufgabenstellung nennt `references/freemint/sys/arch/
user_things.S`/`.h` — das ist das **generische** FreeMiNT-Kernel-Trampolin für
POSIX-artige Unix-Signale (`sig_return`/`Psigreturn`, bereits 1:1 in
`10-upcall-signal-design.md` §2.4/§4 disseziert und in unserem eigenen
Baustein-1-Trampolin nachgebaut). Der für den Userdef-Callback **relevante**
Mechanismus liegt in einer **separaten, XaAES-eigenen** Kopie:
`references/freemint/xaaes/src.km/xa_user_things.S`/`.h`.

`xa_user_things.h:34-41`:
```c
struct xa_user_things
{
    const long len;      /* Größe (dieses Struct + der Code) */
    long  progdef_p;     /* Adresse von progdef_callout (nach Relokation) */
    long  userblk_pp;    /* Slot: Adresse des USERBLK */
    long  ret_p;         /* Slot: Rückgabewert */
    long  parmblk_p;     /* Slot: Adresse des lokalen PARMBLK */
};
```
`xa_user_things.S:32-60`: ein Assembler-**Template** (Code `progdef_callout` +
Daten `_userblk`/`_retcode`/`_parmblk`) mit **relativen** Offsets
(`.long progdef_callout - SYM(xa_user_things)`). `progdef_callout`
(`:41-55`): lädt `_userblk` (Zeiger auf USERBLK), daraus die Callback-Adresse,
`jsr (a0)` mit `_parmblk` als Argument (ein **normaler** Funktionsaufruf, da
jetzt im User-Modus des Clients laufend), speichert das Ergebnis in
`_retcode`, `rts`.

**Wo das Template pro Client instanziiert wird** — `xa_appl.c:180-214`
(`init_client`, beim `appl_init` eines Clients):
```c
client->ut = umalloc(xa_user_things.len);     /* USER-Speicher des Clients! */
...
bcopy(&xa_user_things, client->ut, xa_user_things.len);  /* Template kopieren */
client->ut->progdef_p  += (long)client->ut;    /* relative → absolute Offsets */
client->ut->userblk_pp += (long)client->ut;
client->ut->ret_p      += (long)client->ut;
client->ut->parmblk_p  += (long)client->ut;
cpushi(client->ut, xa_user_things.len);        /* Cache-Flush über den Code! */
```
Das ist **exakt** das Muster, das `10-upcall-signal-design.md` §4.1 für unseren
eigenen Resume-Trampolin bereits gewählt hat (kernel-geschriebener Stub +
Cache-Maintenance) — hier zusätzlich mit **gemeinsamen Daten-Slots** (`ret_p`,
`parmblk_p`, `userblk_pp`) direkt neben dem Code, in **User**-Speicher des
Clients (`umalloc`, nicht `kmalloc`) — der Präzedenzfall für unser eigenes
"PARMBLK + Return-Slot in der EL0-Adresse des Clients" (§4).

### 2.5 Zusammenfassung der Korrekturen

| Aufgabenstellungs-Annahme | Befund | Beleg |
|---|---|---|
| "`mscall.c`" = Client→AES-Routing | Falsch — vier Maus-Sichtbarkeits-Helfer, unbeteiligt | `mscall.c:1-33` |
| Tatsächliches Routing | `register_trap2`+`XA_handler`, läuft **im Aufrufer**, nicht im AES-Thread | `k_main.c:1675`, `handler.c:25-31,479` |
| B ("`PROGDEF_BY_SIGNAL`") = "XaAES-proven"/Standard | Falsch — **deaktiviert** (`#define PROGDEF_BY_SIGNAL 0`); `do_callout` (A) ist der aktive Pfad | `draw_obj.c:46,331-354,439-492` |
| "`sys/arch/user_things.S`" trägt den Userdef-Callback | Falsch — das ist das generische Signal-Trampolin (schon in Baustein 1 verwendet); der Userdef-Callback nutzt XaAES' **eigene** Kopie | `xa_user_things.S/.h`, `xa_appl.c:183-214` |

Die **Grundrichtung (B statt A) bleibt unverändert richtig** — nicht weil B
"XaAES' bewährter Standard" ist, sondern weil AArch64s PXN (§1.2) A
**kategorisch unmöglich** macht, während m68k A als funktionierende (wenn auch
in XaAES bewusst gewählte) Option kennt. Das dokumentierte
VDI-im-Callback-Risiko von B fließt in §6 ein.

---

## 3. AArch64-Abbildung

### 3.1 Warum "AES als Thread" auch im Ein-Client-Fall nötig ist

In XaAES läuft der Userdef-Callout **im selben Adressraum, nur einem anderen
Privileg-Modus** — ein `raise(SIGUSR2)` genügt, weil Supervisor→User auf m68k
kein Adressraumwechsel ist (§1.2/§2.3). Bei uns ist die Grenze (EL1→EL0) **immer**
eine PXN-Grenze — der Rust-Aufrufstapel, der gerade `ob_user` ausführt (auf dem
einzigen, geteilten EL1-Exception-Stack, s. `kernel/src/arch/aarch64/
exceptions.rs:52-68` `ExceptionFrame`, on-stack pro Trap gebaut) **muss den
physischen Prozessor abgeben**, damit `eret` den Client EL0-Code laufen lassen
kann — und **danach wieder genau dort weitermachen**. Ein einzelner,
gemeinsam genutzter Exception-Stack kann das nicht: Sobald `rust_sync_handler`
zurückkehrt (damit `eret` passieren kann), ist sein gesamter Rust-Aufrufstapel
**abgewickelt** — er existiert beim nächsten Trap-Eintritt nicht mehr.

**Konsequenz:** Der Teil von `aes::dispatch` abwärts, der potenziell einen
Userdef-Stall auslösen kann, muss auf einem **eigenen, über einen Trap-Ein-/
Ausstieg hinweg persistenten EL1-Stack** laufen — der "AES-Thread"-Stack. Das
ist unabhängig davon, ob Aufrufer und Tree-Owner derselbe Client sind
(TeraDesk-Fall) oder unterschiedliche Clients (Mehr-App-Zukunft, §6).

### 3.2 Kernidee: Fiber-Switch statt CPS-Umbau (Empfehlung)

**Zwei Optionen wurden erwogen:**

**Option 1 (verworfen) — CPS/Fortsetzungs-Umbau ohne neue Assembler-Primitive.**
`draw_tree`s rekursiver Abstieg würde in eine explizite, seriali­sierbare
Position (Objekt-Index + Geschwister-/Vorfahren-Stack) umgebaut, die beim Stall
bis ganz nach oben (bis `rust_sync_handler`) über Rückgabewerte durchgereicht
und beim `AesUserdefReturn`-Wiedereintritt an genau dieser Position neu
angestoßen wird. **Verworfen:** das erzwingt einen CPS-Umbau **jedes**
Aufrufers in der Kette (`draw_tree`/`just_draw`/`compose`/`paint_bg_clip`/
`window::wind_open`/`objc_change`s `change_clip_t`, `object.rs:1230-1265`, die
**ebenfalls** `ob_user` erreicht) — eine dauerhafte, virale Bau-Vorschrift für
**jeden** zukünftigen AES-Zeichenpfad, der potenziell ein App-`G_USERDEF`
berührt.

**Option 2 (Empfehlung) — Fiber-/Grünthread-Switch, EmuTOS-`switchto`-Stil.**
Ein winziges, einmalig gebautes Assembler-Primitiv (Save/Restore von `SP` +
den AAPCS64-**callee-saved**-Registern `x19-x28/x29/x30`, ARM ARM/AAPCS64
§5.1.1) tauscht zwischen zwei Ausführungskontexten — exakt EmuTOS'
`switchto()` (`references/emutos/aes/gemasm.S:169-194`, `emutos-cooperative-
model.md` §2.2: *"Sichert d0-a5+a6+usp+sp in `UDA`, lädt sie für den nächsten
Thread, `rts`"*), nur mit AArch64-Registern statt m68k, und **ohne**
Adressraum-/Privilegwechsel (kein `TTBR0`, keine `ExceptionFrame`/`SPSR`/`ELR`
beteiligt — reines EL1-internes Bookkeeping). **Kein** anderer Aufrufer in der
Kette muss umgebaut werden: `ob_user` ruft eine gewöhnlich aussehende,
"blockierende" Rust-Funktion auf, die **synchron** einen `i16` liefert — die
Fiber-Maschinerie versteckt den Kontextwechsel vollständig in dieser einen
Funktion.

**Empfehlung: Option 2.** Kleinerer, lokal begrenzter Neubau (ein Primitiv),
kein CPS-Zwang für alle heutigen und zukünftigen AES-Zeichenpfade — und exakt
das von `20-sleepwake-…md` §4.3/§8.1.2 bereits als "S1" vorgezeichnete Modell,
hier auf die AES allein zugeschnitten (nicht auf jeden Prozess).

### 3.3 Bausteine

Alle **neu** (kein bestehendes NewTOS-Äquivalent); Platzierung: `tos-api` ist
bereits `forced-target = "aarch64-unknown-none-softfloat"`
(`tos-api/Cargo.toml:9`) und enthält bereits AArch64-Inline-Assembler an
mehreren Stellen (`tos-api/src/bdos/process/mmu.rs` — `TTBR0`/TLBI;
`bios/vectors.rs`, `bios/chardev.rs`, `gemdos/ssystem.rs`, `gemdos/console.rs`)
— **Empfehlung:** das Fiber-Primitiv lebt **in `tos-api`** (z.B.
`tos-api/src/aes/thread.rs`), nicht in `kernel`, weil `kernel` von `tos-api`
**abhängt** (nicht umgekehrt — `kernel/src/arch/aarch64/exceptions.rs` importiert
`tos_api::…`) und `ob_user` (der Aufrufer) bereits in `tos-api` sitzt; eine
Platzierung in `kernel` bräuchte eine Trait-/Callback-Indirektion, um die
Abhängigkeitsrichtung nicht zu brechen. Präzedenzfall für "AArch64-Asm direkt
in `tos-api`, co-lokalisiert mit der Business-Logik, die es braucht":
`mmu.rs`s `install_ttbr0`.

```
FiberCtx { sp: u64, x19..x28: [u64; 10], x29: u64, x30: u64 }   // 13 × u64
```
— ein Save/Restore-Primitiv `fiber_switch(save_into: *mut FiberCtx,
resume_from: *const FiberCtx) -> u64` (der Rückgabewert reist in `x0`, AAPCS64,
und trägt später das Userdef-Wortergebnis zurück in `block_for_userdef`s
"Rückgabe"). Zwei statische Slots (**genau ein** AES-Thread — s. §3.4/§6
Reentrancy):

```rust
static AES_THREAD_STACK: [u8; AES_THREAD_STACK_SIZE] = [0; AES_THREAD_STACK_SIZE];
static AES_ENTRY_CTX:  FiberCtx = FiberCtx::zeroed();  // "zurück zu rust_sync_handler"
static AES_THREAD_CTX: FiberCtx = FiberCtx::zeroed();  // "zurück in ob_users Stall"
static AES_THREAD_STATE: AtomicU8 = AtomicU8::new(IDLE);   // IDLE | SUSPENDED
static AES_CALL_ORIGIN: ParentFrame = ParentFrame::zeroed(); // s. §3.4/§4
static AES_CALL_ORIGIN_SLOT: AtomicUsize = AtomicUsize::new(NONE);
static PENDING_USERDEF: … // {ub_code_va, parmblk_ptr, target_slot} des aktuellen Stalls
```

`AES_THREAD_STACK_SIZE` — **OFFEN (Vorschlag: 16 KiB)**. Analogon: der
Haupt-Kernel-Stack ist 64 KiB + 16 KiB Notfall-Stack
(`kernel/src/arch/aarch64/link-rpi4.ld:79-91`); die AES-Baumtraversierung ist
flacher/genügsamer als der volle Kernel-Aufrufstapel — 16 KiB (analog zur
Notfall-Stack-Größe) ist ein plausibler Startwert, kein gemessener. Braucht
eine eigene Guard-Page-Betrachtung wie der Haupt-Stack (**OFFEN**, ob nötig für
Schritt 1 oder später).

**Warum ALLES global (nicht pro Prozess) sein darf:** Es existiert **eine**
physische AES-Verarbeitungsinstanz zur Zeit — genau EmuTOS' Invariante "`rlr`
ist ein einzelner Zeiger, nie ein Array" (`emutos-cooperative-model.md` §2.1:
*"es läuft physisch immer genau ein Thread"*). Kein Prozess-Tabellen-Schema-
Update nötig (§3.5).

### 3.4 Ablauf Schritt für Schritt

**(a) Kein Stall (der heutige, unveränderte Fall — jede bisherige AES-Funktion).**
`rust_sync_handler`s `TrapClass::Gem`-Zweig, Selektor 200
(`exceptions.rs:350-361`), ruft statt `tos_api::aes::dispatch(...)` direkt neu:
`tos_api::aes::thread::call(aespb_ptr, abi)`. Bei `AesThreadState == Idle`:
sichert die aktuelle (Exception-Stack-)`FiberCtx` in `AES_ENTRY_CTX`, setzt
`SP = AES_THREAD_STACK`-Top, ruft `aes::dispatch(...)` dort **als gewöhnlichen
Aufruf** (kein `fiber_switch` nötig für den Einstieg — analog EmuTOS'
`psetup()`, das den initialen Kontext eines *neuen* Threads so präpariert, dass
`switchto`s `rts` direkt zum Einstiegscode springt,
`emutos-cooperative-model.md` §1.2 `gempd.c:110-131` `psetup(px,pcode)`, unser
eigenes `Switch::fresh`, `tos-api/src/bdos/process/sched.rs:109-111`, ist die
GEMDOS-Ebenen-Entsprechung derselben Idee). Kehrt `aes::dispatch` **normal**
zurück (kein Stall passiert), wird `SP` zurückgetauscht und `AesCallOutcome::
Complete(sw)` an `rust_sync_handler` geliefert — **byte-identisch** zum
heutigen Pfad (Nullrisiko-Refactor für alle ~44 AES-Funktionen).

**(b) Stall — `ob_user` trifft ein App-`G_USERDEF`.** `ob_user`
(`object.rs:2027-2030`, heutiger "defer"-Zweig) wird ersetzt durch einen Aufruf
`aes_thread::block_for_userdef(target_slot, ub.ub_code, parmblk_ptr) -> i16`.
Intern: schreibt `{ub_code_va, parmblk_ptr, target_slot}` in `PENDING_USERDEF`,
sichert die **aktuelle** (AES-Thread-Stack-)`FiberCtx` in `AES_THREAD_CTX`, lädt
`AES_ENTRY_CTX` und kehrt via `fiber_switch` **direkt** zurück in `aes_thread::
call`s Aufrufstelle — **ohne** die dazwischenliegenden Rust-Rahmen
(`just_draw`/`draw_tree`/`paint_bg_clip`/…) regulär zu verlassen; sie bleiben
auf `AES_THREAD_STACK` eingefroren. `aes_thread::call` erkennt (über
`AES_THREAD_STATE == Suspended` bzw. den `PENDING_USERDEF`-Sentinel) diesen
Fall und liefert `AesCallOutcome::Stalled(desc)`.

**(c) `rust_sync_handler` liefert den Upcall aus.** Bei `Stalled(desc)`:
1. **Nur beim ALLERERSTEN Stall dieses Aufrufs** (erkennbar daran, dass
   `AES_CALL_ORIGIN_SLOT == NONE` war, bevor `aes_thread::call` lief): sichere
   den **unveränderten** `frame` (Client-Zustand exakt beim Eintritt in den
   ursprünglichen `wind_*`-Trap, **vor** jedem Ergebnis-Write) in
   `AES_CALL_ORIGIN` + `AES_CALL_ORIGIN_SLOT`. Analogon:
   `Process::parent_frame`/`saved_frame` (`tos-api/src/bdos/process.rs:236-271`),
   nur global statt pro-Prozess (§3.3).
2. `deliver_upcall(frame, UpcallDesc{ handler_va: USERDEF_TRAMPOLINE_VA, args:
   [desc.ub_code_va, desc.parmblk_ptr, 0, 0], mask_bit: UPCALL_MASK_AES_USERDEF,
   disposition: Disposition::UserdefReturn })` — **komplett unverändert
   wiederverwendet** (`exceptions.rs:787-846`); `deliver_upcall` kennt weder
   `handler_va` noch `args` inhaltlich, nur als Frame-Redirect-Nutzlast.
3. `return` — struktureller Zwilling der bestehenden Frame-mutierenden
   Sonderfälle `handle_pterm`/`handle_pexec`/`handle_pwait`
   (`exceptions.rs:283-306`), die ebenfalls **vor** der normalen
   Ergebnis-Write-Tail aus `rust_sync_handler` aussteigen.

**(d) Der Client läuft `ub_code` und meldet zurück.** Der neue EL0-Stub
`USERDEF_TRAMPOLINE_VA` (§4) lädt `x0`/`x1` (= `ub_code`, `parmblk_ptr`, aus den
Upcall-Args, AAPCS64), ruft `ub_code(parmblk_ptr)` (ein normaler `blr` — **User-
Modus ruft User-Code auf**, kein PXN-Konflikt, exakt XaAES' `progdef_callout`,
§2.4), schreibt `w0` in den Return-Slot (§4), fällt in den **bestehenden**
Resume-Trampolin (`TRAMPOLINE_VA`, `mov x8,#0x11A; svc #1`,
`loader.rs:889-907`) — **derselbe** `Psigreturn`-Aufruf wie jeder andere Upcall.

**(e) `handle_psigreturn` — neuer `Disposition`-Zweig.** Das bestehende Pop-
Bookkeeping (Maske löschen, Tiefe dekrementieren, `exceptions.rs:888-897`)
läuft unverändert; danach:
```rust
Disposition::UserdefReturn => {
    let word = read_userdef_ret_slot(slot_idx);       // §4, Kernel-Alias-Lesen
    match aes_thread::resume(word) {
        AesCallOutcome::Stalled(next_desc) => {
            deliver_upcall(frame, next_desc.into());   // nächstes Userdef-Objekt
            return;
        }
        AesCallOutcome::Complete(sw) => {
            restore_from(&AES_CALL_ORIGIN, frame);      // GPRs/elr/spsr/sp_el0
            frame.gpr[0] = 0;    // GEM-Trap-Konvention (Ergebnis steht im intout[], nicht x0)
            AES_CALL_ORIGIN_SLOT.store(NONE, …);
            complete_aes_call(frame, slot_idx, sw);      // GETEILTE Tail, s.u.
            return;
        }
    }
}
```
`aes_thread::resume(word)` tauscht via `fiber_switch` **zurück** in
`AES_THREAD_CTX` (in `block_for_userdef`s Stall-Punkt, `word` als "Rückgabewert"
in `x0`) — Ausführung setzt sich in `ob_user`, `just_draw`, `draw_tree`, …,
schließlich `aes::dispatch` fort, **exakt** dort, wo sie pausiert hatte. Trifft
sie **erneut** ein `G_USERDEF` (mehrere Userdef-Objekte im selben Baum), ruft
sie erneut `block_for_userdef` — der fiber-switcht wieder heraus, `resume`
liefert `Stalled(next_desc)`. Kehrt `aes::dispatch` am Ende **regulär** zurück
(kein weiterer Stall), fiber-switcht `block_for_userdef`s letzte
Aufrufkette gar nicht mehr — die normale Rust-Rückkehrkette bringt uns bis
`aes_thread::call`/`resume`s eigener Fiber-Grenze, die dann `Complete` an
`handle_psigreturn` liefert.

**`complete_aes_call`** — **Empfehlung:** die heutige Tail von
`rust_sync_handler` (Ergebnis-Write, `take_pending_terminate`,
`take_pending_upcall`, `exceptions.rs:396-427`) wird in eine gemeinsame
Funktion extrahiert, die **sowohl** der Kein-Stall-Pfad (a) **als auch** dieser
Stall-dann-fertig-Pfad (e) aufrufen — garantiert bytegleiches Verhalten für
beide, kein Doppelpfad-Drift.

### 3.5 Wiederverwendet vs. Neu

| Baustein | Status | Beleg |
|---|---|---|
| `deliver_upcall` (Frame-Snapshot + Redirect) | **wiederverwendet, unverändert** | `exceptions.rs:787-846` |
| `Psigreturn` (GEMDOS `0x11A`), Pop-Bookkeeping (Maske/Tiefe) | **wiederverwendet, unverändert** | `exceptions.rs:848-897` |
| `UpcallDesc` (handler_va/args/mask_bit) | **wiederverwendet, unverändert** | `process.rs:379-391` |
| `upcall_ctx[]`/`MAX_UPCALL_NEST`-Overflow-Schutz | **wiederverwendet** (jeder Stall pusht/poppt genau eine Ebene; Tiefe pendelt 0↔1, akkumuliert nicht, §6) | `process.rs:65-74,320-327` |
| Resume-Trampolin `TRAMPOLINE_VA` (Tail-Sprung nach `ub_code`) | **wiederverwendet, unverändert** | `loader.rs:893-907` |
| Kernel-High-Half-Alias-Schreiben (`populate_sysvars`-Muster) | **wiederverwendet** für den neuen Stub + PARMBLK/Ret-Slot | `loader.rs:66-68,758-887` |
| Cache-Maintenance-Fenster (`sync_code_cache`) | **wiederverwendet** (neuer Stub liegt im selben `[base_kva, image_top)`-Fenster) | `loader.rs:486-504` |
| `Disposition`-Enum + Bit-Encoding | **erweitert** (3. Variante, neues Tag-Bit) | `process.rs:400-449` |
| `ObjTree::app_at`/`NEWDESK_ABI` (ABI der App-Baums) | **wiederverwendet** für die PARMBLK-ABI-Wahl | `object.rs:126-149`, `window.rs:398-404` |
| `rust_sync_handler`s "Frame-mutierender Sonderfall, früher Return"-Idiom | **wiederverwendet** (Stil von `handle_pterm`/`handle_pexec`/`handle_pwait`) | `exceptions.rs:283-306` |
| Fiber-Switch-Primitiv (`FiberCtx`+Asm) | **NEU** | — |
| `AesThreadState`/`AES_THREAD_STACK`/`AES_ENTRY_CTX`/`AES_THREAD_CTX` | **NEU** (global, kein Prozess-Feld) | — |
| `AES_CALL_ORIGIN`/`AES_CALL_ORIGIN_SLOT` | **NEU** (global; ersetzt/ergänzt das übliche Pro-Prozess-`ParentFrame`-Muster) | vgl. `process.rs:236-271` |
| `USERDEF_TRAMPOLINE_VA`-Stub (ub_code-Aufruf + Ret-Slot-Schreiben) | **NEU** | — |
| PARMBLK/Ret-Slot-Scratch in der reservierten Lücke | **NEU** | s. §4 |
| `NEWDESK_SLOT` (Tree-Owner-Slot, s. §6) | **NEU** (fehlt heute — nur `NEWDESK_ABI` existiert) | `window.rs:383-404` |
| `Disposition::UserdefReturn` + `UPCALL_MASK_AES_USERDEF` | **NEU** | Vorschlag, s.u. |

---

## 4. Shared Return-Slot + PARMBLK-Platzierung

Wiederverwendung der **bestehenden** B1/Option-1-TPA-Lücke
(`loader.rs:179-197`):
```
0x670  ── SYSVAR_AREA_END ──
0x700  Resume-Trampolin (5 × 4 B = 20 B)              (loader.rs:693,906-907)
0x714  NEU: Userdef-Callout-Stub (~6 × 4 B = 24 B)     (Vorschlag)
0x730  NEU: PARMBLK-Scratch (ABI-breit, ≤ 48 B)        (Vorschlag)
0x760  NEU: Userdef-Ret-Slot (4 B, WORD-Ergebnis)      (Vorschlag)
0x1000 RESERVED_LOW_END = App-Image-Basis              (loader.rs:186)
```
Alle drei neuen Offsets liegen **innerhalb** der bereits reservierten Lücke
`[0x670, 0x1000)` (`loader.rs:227-228`, ≈2448 B, reichlich Reserve) — **keine**
Verschiebung von `RESERVED_LOW_END` nötig.

**Schreibpfad:** exakt das Muster von `populate_sysvars`/`write_upcall_trampoline`
(`loader.rs:66-68` `pa_to_kva`, `:758-887`, `:916-922`): Kernel schreibt über den
High-Half-Alias `PROCESS_TABLE[slot].basepage_kva + offset`
(`Process::basepage_kva`, `process.rs:210-214`, bereits die Kernel-VA von Page 0
des Ziel-TPAs — **kein neues Feld nötig**, direkt wiederverwendbar). Der
Userdef-Callout-Stub wird **einmalig beim Laden** geschrieben (wie der
Resume-Trampolin) — **kein** Re-JIT pro Aufruf: `ub_code`/`parmblk_ptr` reisen
als Upcall-**Argumente** (`x0`/`x1`), nicht als in den Stub eingebackene
Konstanten.

**Cache-Maintenance (Real-HW):** Der neue Stub liegt **innerhalb** des bereits
etablierten Cache-Maintenance-Fensters `[base_kva, base_kva+image_top)`
(`loader.rs:486-504`, `image_top ≥ RESERVED_LOW_END > 0x714+24`) — er wird von
`sync_code_cache` **automatisch mit erfasst**, sofern er im selben
`populate_and_map`-Aufruf geschrieben wird wie der Resume-Trampolin. **Keine**
zusätzliche Maintenance-Stelle nötig. Die PARMBLK/Ret-Slot-Scratch sind
**Daten**, kein Code — nur D-Cache-Kohärenz zählt, und die ist laut
`loader.rs:500-502`-Kommentar auf Cortex-A72 (PIPT) für By-VA-Zugriffe ein
No-Op — derselbe Grund, warum `populate_sysvars`s Daten-Schreiben schon ohne
extra Maintenance funktioniert.

**PARMBLK-ABI.** Das native `ParmBlk` (`object.rs:697-725`, LP64: `u64`-Felder
für `pb_tree`/`pb_parm`) ist die EL1-interne Repräsentation. Für einen App-Baum
muss die **in EL0-Speicher geschriebene** Kopie an der **Tree-ABI** ausgerichtet
sein (ILP32: 32-Bit `pb_tree`/`pb_parm`; die zehn `i16`-Felder sind ABI-neutral).
**Neu zu bauen:** ein ABI-bewusster PARMBLK-**Schreiber** — der Spiegel des
bereits bestehenden ABI-bewussten `UserBlk::read_abi`
(`object.rs:671-689`) und der `ObjTree`-Feldoffset-Maschinerie
(`object.rs:74-174`). Die ABI selbst ist bereits bekannt (`ObjTree::app_at`s
`abi`-Parameter, gespeist aus `NEWDESK_ABI`, `window.rs:398-404`) — nur der
Schreibpfad fehlt.

**OFFEN (klein):** exakte Byte-Offsets/Stub-Länge — der obige Entwurf ist
**Vorschlag**, im Implementierungsschritt fixierbar (analog zu `10-upcall-…md`
§4.1s "0x700 ist ein Vorschlag").

---

## 5. Bottom-up-Implementierungsplan

Jeder Schritt Developer→Tester, kein Wegwerf ([[limit-concurrent-workers-token-budget]]:
seriell).

### Schritt 1 — Fiber-Primitiv + Kein-Stall-Durchleitung (Nullrisiko-Refactor)

- `tos-api/src/aes/thread.rs` (neu): `FiberCtx`, `fiber_switch`-Assembler,
  `AesThreadState`, `AES_THREAD_STACK`, `AES_ENTRY_CTX`, `aes_thread::call(pb_ptr,
  abi) -> AesCallOutcome` — **nur** der Fresh-Einstieg (§3.4a), **kein**
  `block_for_userdef` noch angeschlossen (`ob_user` bleibt unverändert im
  "defer"-Zweig).
- `rust_sync_handler`s `200 => …`-Zweig ruft `aes_thread::call` statt
  `aes::dispatch` direkt auf.
- **Test:** **Alle** bestehenden AES-Selftests/QED-/TeraDesk-Regressionen
  bytegleich (0 Delta) — reiner Stack-Umzug, kein Verhaltensunterschied. Beweist
  den Fiber-Einstieg funktioniert, ohne den Stall-Pfad zu berühren.

### Schritt 2 — `block_for_userdef` + Userdef-Trampolin + Scratch-Bereich

- `USERDEF_TRAMPOLINE_VA`-Stub + PARMBLK/Ret-Slot-Offsets in `loader.rs`
  (§4), geschrieben in derselben Stelle wie `write_upcall_trampoline`.
- `Disposition::UserdefReturn` + `UPCALL_MASK_AES_USERDEF` in `process.rs`.
- `aes_thread::block_for_userdef`/`resume` (§3.4b/e) + `AES_CALL_ORIGIN`/
  `AES_CALL_ORIGIN_SLOT`.
- `handle_psigreturn`s neuer Disposition-Zweig; `complete_aes_call`-Extraktion
  (gemeinsame Tail, §3.4e).
- `ob_user` (`object.rs:2027-2030`): der "defer"-Zweig ruft jetzt
  `block_for_userdef(target_slot, ub.ub_code, parmblk_ptr)` — `target_slot`
  vorerst `slot_for(current())` (Ein-Client-Fall, s. §6 OFFEN zu `NEWDESK_SLOT`).
  ABI-bewusster PARMBLK-Schreiber (§4).
- **Test (das vom Auftrag geforderte Ziel):** QEMU `raspi4b`-Screendump —
  TeraDesk registriert `WF_NEWDESK` mit `ub_bckbox`, ein `wind_open`/Resize
  löst `compose()` aus, der Hintergrund **erscheint** (statt weiß/leer). Prüft:
  Stall-Erkennung, Upcall-Zustellung, `ub_code`-Ausführung bei EL0, Ret-Slot-
  Rundreise, korrekter Wiederanlauf von `ob_user` mit `word` als Rückgabewert,
  korrekte `wind_open`-Fertigstellung (Client bekommt sein **echtes**
  `wind_open`-Ergebnis, nicht den Upcall-Zwischenzustand).

### Schritt 3 — Mehrfach-Stall im selben Repaint + Reentrancy-Guard

- Test-Fixture mit **zwei** `G_USERDEF`-Objekten im selben registrierten Baum
  (z.B. Hintergrund-Box + ein zweites Custom-Icon) — beweist die
  Sequential-Stall-Kette (§3.4e, Tiefe pendelt 0↔1, kein Überlauf).
- Reentrancy-Guard: `aes_thread::call` erkennt `AesThreadState != Idle`
  (Aufruf während eines laufenden Stalls) und liefert einen definierten
  Fehler/No-op statt den globalen Zustand zu korrumpieren (§6).
- **Test:** zwei-Objekt-Fixture zeichnet beide korrekt; ein absichtlich
  provozierter verschachtelter AES-Aufruf **aus** einem `ub_code` heraus wird
  sauber abgewiesen (kein Hang, kein korrupter globaler Zustand) statt
  stillschweigend falsch zu laufen.

### Schritt 4 (außerhalb dieses Deliverables) — volle EmuTOS-koop. AES

Permanenter, `rlr`-artiger AES-Thread mit eigener Scheduling-Warteschlange,
koexistierend mit Screen-Manager/Accessories als **EL1-interne** Peers
(Option B Hybrid, `20-sleepwake-…md` §2, bereits als `el1_internal`-Flag am
`Switch` **vorbereitet**, `sched.rs:116-126`) — der hier gebaute
Ein-Client-Fiber ist das **Fundament**, nicht die Endstufe
(`00-effort-comparison.md` Schritt 4).

---

## 6. OFFEN + Risiken

### 6.1 OFFEN (Optionen + Empfehlung markiert)

1. **`AES_THREAD_STACK_SIZE`.** *Empfehlung:* 16 KiB (Analogie zum
   Notfall-Stack, `link-rpi4.ld:87-91`). **OFFEN:** ob eine Guard-Page wie beim
   Haupt-Stack (`layout.rs:241` `stack_guard_start`) nötig ist, und die exakte
   Zahl (nicht gemessen).
2. **Fiber-Primitiv-Ort.** *Empfehlung:* `tos-api/src/aes/thread.rs` (§3.3,
   Präzedenz `mmu.rs`). **OFFEN:** ob der Lead eine striktere Kernel/tos-api-
   Trennung wünscht (dann Trait-Indirektion nötig).
3. **`NEWDESK_SLOT`.** Fehlt heute — nur `NEWDESK_TREE`/`ROOT`/`APP`/`ABI`
   (`window.rs:383-404`). Der Registrierungs-Kommentar selbst sagt: *"the
   registering app may no longer be the current process"*
   (`object.rs:126-136`, konkret `:129`) — das widerspricht der naiven Annahme
   "`slot_for(current())` zur Stall-Zeit ist immer der Tree-Owner". *Empfehlung:*
   `NEWDESK_SLOT: AtomicUsize`, in `set_newdesk` analog zu `NEWDESK_ABI`
   erfasst (`window.rs:428-429`), von `ob_user`s Stall-Pfad statt
   `slot_for(current())` genutzt. Für Schritt 1/2 (Ein-Client, TeraDesk ist
   der einzige EL0-Prozess) fallen beide Werte praktisch zusammen — die Lücke
   ist eine **Korrektheits**-Frage, keine Blockade für die Verifikation.
4. **Wird `NEWDESK_TREE`/`APP`/`ABI` beim `Pterm` des registrierenden Prozesses
   invalidiert?** **Nicht verifiziert** in dieser Session (außerhalb des
   unmittelbaren Crash-Scopes) — falls nicht, muss der Stall-Pfad defensiv
   `PROCESS_TABLE[slot].read_state() == State::Running` prüfen, bevor er einen
   Upcall an einen ggf. bereits freigegebenen Slot ausliefert, sonst auf das
   heutige "deferred, kein Crash"-Verhalten zurückfallen. **An den Tester
   flaggen.**
5. **Exakte Stub-/Scratch-Offsets** (§4). *Empfehlung:* `0x714`/`0x730`/`0x760`.
   **OFFEN:** exakte Werte, im Implementierungsschritt fixierbar.
6. **`Disposition::UserdefReturn`-Tag-Bit.** *Empfehlung:* `1 << 61` (neben den
   bestehenden `TERM_TAG = 1<<63`/`RESERVE_TAG = 1<<62`, `process.rs:421-423`).
7. **Mikro-Optimierung (nicht für Schritt 1):** jeder Stall sichert/verwirft
   30 GPRs+lr+elr+spsr+sp_el0 in `upcall_ctx[]`, obwohl `UserdefReturn`s
   Snapshot nie zurückgespielt wird (§3.4e) — für die erwartete Größenordnung
   (wenige Userdef-Objekte pro Repaint, kein Hot-Path) irrelevant; eine
   spätere Variante könnte diesen Save für `UserdefReturn`-Ebenen überspringen.

### 6.2 Risiken

**Reentrancy.** Genau **eine** globale AES-Thread-Instanz (§3.3) — ein
verschachtelter Opcode-200-Aufruf **aus einem laufenden `ub_code` heraus**
(während `AesThreadState == Suspended`) würde ohne Schutz den globalen
`AES_THREAD_STACK`/`AES_THREAD_CTX` korrumpieren. **Mitigation (§5 Schritt 3):**
expliziter Zustands-Check in `aes_thread::call`, definierter Fehler statt
Korruption. **Entwarnung:** VDI-Aufrufe (Selektor 115) durchlaufen diesen Pfad
**gar nicht** (separater Dispatch-Zweig, `exceptions.rs:354`) — ein
gewöhnliches, nur zeichnendes `ub_code` (der Normalfall) ist **nicht**
betroffen; nur ein `ub_code`, das **selbst** eine weitere AES-Funktion
aufruft, wäre es. Das ist genau das von XaAES' eigenem Kommentar
geflaggte Risiko (§2.3, "VDI-Aufrufe im Signal-Handler nicht gut") — bei uns
verschärft sich das nur für AES- (nicht VDI-)Re-Entry.

**Verschachtelung (`MAX_UPCALL_NEST`).** Sequentielle (nicht verschachtelte)
Userdef-Stalls im selben Repaint erschöpfen den Nest-Zähler **nicht** — jeder
Stall wird vor dem nächsten wieder gepoppt, Tiefe pendelt 0↔1 (§3.5). Eine
**echte** Verschachtelung (ein Upcall-Handler löst *während seiner Ausführung*
einen *weiteren*, andersartigen Upcall aus, z.B. `etv_term` mitten in einem
Userdef) bleibt vom bestehenden `MAX_UPCALL_NEST=8`-Schutz
(`process.rs:65-74`) unverändert abgedeckt.

**`TTBR0` während des Userdefs.** `deliver_upcall` fasst `TTBR0_EL1` **nicht**
an (reine Frame-Mutation, `exceptions.rs:787-846`) — es bleibt durchgehend das
des Traps, der gerade läuft. Für Schritt 1/2 ist das korrekt, **weil** der
stallende Client (TeraDesk) im Ein-Client-Modell immer auch der **einzige**
laufende EL0-Prozess ist ([[singletasking-app-owns-full-screen]]) — Tree-Owner
und aktiver `TTBR0` fallen zwangsläufig zusammen. **Echte Lücke für die
Zukunft:** sobald ein **anderer** Client (nicht der Tree-Owner) einen Repaint
auslöst, der den registrierten Baum eines **dritten**, gerade nicht laufenden
Prozesses zeichnet, müsste die Upcall-Zustellung **zusätzlich** einen
`TTBR0`-Wechsel einschließen — eine Erweiterung, die die bestehende
Deliver-on-Resume-Maschinerie (`20-sleepwake-…md` §6, `apply_switch`s
`!fresh`-Zweig, `exceptions.rs:560-564`) bereits für den *blockierten
`evnt_multi`-Client*-Fall löst, hier aber **nicht** (ein synchroner Mid-Call-
Stall ist kein Scheduler-Resume). **OFFEN**, explizit auf Mehr-App-Zukunft
verschoben — Schritt 1/2 sind sicher, weil Owner==aktueller Prozess strukturell
garantiert ist.

**Real-HW Cache/PXN.** PXN selbst ist eine reine Seiten-Attribut-Prüfung
(`mmu.rs:89-90`), kein Cache-Thema. Die **Code**-Cache-Maintenance für den
neuen Stub reitet auf dem bestehenden Fenster (§4) — kein neuer QUIRK erwartet.
Die **einzige** unverifizierte Real-HW-Frage: ob das PARMBLK/Ret-Slot-
Daten-Schreiben (Kernel, High-Half-Alias) → Lesen (Client, EL0, Low-Half-VA,
selbe physische Seite) auf echtem Cortex-A72 wirklich ohne explizite
Daten-Cache-Maintenance kohärent ist — laut `loader.rs:500-502`-Kommentar JA
(PIPT-Caches, By-VA-Maintenance ist dort bereits ein No-Op), aber **nicht**
in dieser Session auf realer Hardware nachgemessen (gehört in die
`_hw-test-checklist.md`, wie schon der Baustein-1-Trampolin, `open-tasks-
post-2a.md:135-137` "Real-HW-Rest: J1/J2").

---

## Out-of-scope observations

- `objc_change` (Opcode 47) erreicht `ob_user` über einen **eigenen** Pfad
  (`change_clip_t`, `object.rs:1230-1265`, AUD-P3-002) mit demselben
  "app-tree → deferred"-Verhalten wie `paint_bg_clip`. Der hier entworfene
  Fix behebt dieses Gap **als Nebeneffekt** (dieselbe `ob_user`-Stelle), aber
  **nicht verifiziert** in dieser Session — ein zusätzlicher Test für Schritt 2
  (ein App-Baum mit `G_USERDEF`, direkt per `objc_change` neu gezeichnet) wäre
  günstig, aber nicht Teil des WF_NEWDESK-Verifikationsziels.
- `d_g_progdef`s Clip-Rect-Neusetzen-nach-Rückkehr (`draw_obj.c:515-524`,
  wegen möglicher Taskwechsel während des Callouts) wurde nicht auf ein
  NewTOS-Äquivalent geprüft — unser `ob_user`/`draw_tree` setzt den VDI-Clip
  vor jedem Objekt neu (nicht am Ende eines Stalls); ob ein Wiederanlauf nach
  `block_for_userdef` einen stale Clip-Zustand hinterlassen kann, wurde nicht
  bis zum Ende verfolgt (außerhalb des Auftragsumfangs — VDI-Audit-Thema).

---

### Quellen

- **FreeMiNT/XaAES:** `xaaes/src.km/init.c` (:242-246,600-684 Modul-Init +
  Kernel-Thread-Start), `k_main.c` (:1592-1730 `k_main`, :1675-1679
  `register_trap2`), `handler.c` (:25-31 Modulkommentar, :87-462 `aes_tab`,
  :479-… `XA_handler`), `mscall.c` (:1-33, unbeteiligt — Korrektur),
  `draw_obj.c` (:43-46 `PROGDEF_BY_SIGNAL`-Flag+Kommentar, :331-354
  `do_callout`, :370-532 `d_g_progdef`, :439-492 die A/B-Verzweigung),
  `xa_user_things.h` (:34-41 `struct xa_user_things`), `xa_user_things.S`
  (:32-60 Template+`progdef_callout`), `xa_appl.c` (:150-229 `init_client`,
  :183-214 Pro-Client-Instanziierung+Cache-Flush), `xa_types.h` (:2134
  `struct xa_client.ut`).
- **NewTOS:** `tos-api/src/aes/window.rs` (:60,371-612,634-669,2236-2301
  `paint_bg_clip`/`compose`/`repaint_vacated`/`drag_size`, :383-436
  `NEWDESK_*`), `tos-api/src/aes/object.rs` (:74-174 `ObjTree`, :659-689
  `UserBlk`, :691-725 `ParmBlk`, :787-927 `render_tree*`, :1230-1265
  `change_clip_t`, :1825,2033-2361 `draw_tree`/`just_draw`, :1972-2031
  `ob_user`), `tos-api/src/aes/mod.rs` (:158-260 `dispatch`),
  `tos-api/src/bdos/process.rs` (:56-78,188-455 `MAX_UPCALL_NEST`, `Process`,
  `UpcallDesc`, `Disposition`), `tos-api/src/bdos/process/loader.rs`
  (:55-68,161-197,660-922 VA-Layout, Trampolin, `populate_sysvars`),
  `tos-api/src/bdos/process/mmu.rs` (:89-90,135-162 `PXN`/`PageFlags`),
  `tos-api/src/bdos/process/sched.rs` (:37-47,102-142,202-243 `Switch`,
  `el1_internal`), `kernel/src/arch/aarch64/exceptions.rs` (:52-68
  `ExceptionFrame`, :210-428 `rust_sync_handler`, :283-306 Sonderfall-Idiom,
  :787-930 `deliver_upcall`/`handle_psigreturn`), `kernel/src/arch/aarch64/
  link-rpi4.ld` (:79-91 Stack-Größen), `kernel/src/desk/mod.rs` (:101-183,
  512-533 EL1-Desktop-`svc #2`-Muster).
- **Design:** `docs/analysis/coop-kernel/10-upcall-signal-design.md` (§2, §4,
  §6, §7), `20-sleepwake-substrate-design.md` (§2, §4.3, §7.2 — S1/Schritt-4-
  Vorzeichnung), `emutos-cooperative-model.md` (§1.2, §2.2, §2.4 —
  `psetup`/`switchto`-Analogie), `00-effort-comparison.md` (Schritt 4).
