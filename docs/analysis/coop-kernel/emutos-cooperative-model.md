# EmuTOS Cooperative-AES-Multitasking-Modell — Source-Analyse

**Scope:** Extraktion der AES-internen Scheduling-Maschinerie (Prozessmodell,
Dispatcher, Block/Wake, Event-Routing, Koexistenzmodell) aus
`references/emutos/aes/` — nicht die einzelnen AES-Funktions-APIs. Ziel: Grundlage
für die Lead-Aufwandsschätzung "kooperativer Kernel EmuTOS-Style vs.
FreeMiNT-Style". Track A nach `docs/SOURCE-ANALYSIS-PROTOCOL.md`.

Jede strukturelle Aussage ist mit `datei:zeile` aus dem gepinnten
`references/emutos/`-Checkout belegt. Wo der Code mehrdeutig ist oder ich einen
Mechanismus nicht bis zum Ende verfolgt habe, steht das explizit da (kein
Erfinden).

---

## 1. Prozessmodell: `AESPD`, `AESPROCESS`, `THEGLO`

### 1.1 Die Datenstrukturen

`struct aespd` (typedef `AESPD`) ist der Prozess-Deskriptor, definiert in
`aes/struct.h:119-155`:

```
struct aespd {
    AESPD   *p_link;        /*  0  Verkettung: Ready-/Not-Ready-Liste */
    AESPD   *p_thread;       /*  4  deklariert, aber kein Reader/Writer
                                    in aes/ gefunden — vestigial */
    UDA     *p_uda;          /*  8  User-/Supervisor-Stack + Register-Save */
    char    p_name[8];       /*  C  8-Zeichen-Name (appl_find) */
    CDA     *p_cda;          /* 14  Konsolen-/Eingabe-Datenbereich */
    LONG    p_ldaddr;        /* 18  Ladeadresse (Basepage) */
    WORD    p_pid;           /* 1C */
    WORD    p_stat;          /* 1E  Bit WAITIN = wartet auf Event */
    EVSPEC  p_unused;        /* 20  Altlast (früher p_evbits) */
    EVSPEC  p_evwait;        /* 22  Event-Wartemaske */
    EVSPEC  p_evflg;         /* 24  erfüllte Event-Flags */
    UWORD   p_flags;         /* 26  AP_OPEN/AP_MESAG/AP_ACCLOSE */
    EVB     *p_evlist;        /* 28  Liste offener EVBs dieses Prozesses */
    EVB     *p_qdq, *p_qnq;   /* 2C/30 Warteschlangen für Message-Pipe */
    struct { WORD action; WORD wh; } p_msg;  /* WM_ARROWED-Fastpath */
    MFORM   p_mouse;          /* graf_mouse(SAVE/RESTORE), CONF_WITH_GRAF_MOUSE_EXTENSION */
    char    *p_qaddr;         /* Zeiger auf p_queue (Message-Pipe-Puffer) */
    WORD    p_qindex;         /* Füllstand der Message-Pipe */
    char    p_queue[128];     /* QUEUE_SIZE=128 Byte Message-Pipe (struct.h:56) */
    char    p_appdir[LEN_ZPATH+2]; /* Verzeichnis der ausführbaren Datei */
};
```

Ein `AESPD` wird nie einzeln alloziert; er ist immer Teil von `AESPROCESS`
(`aes/gemlib.h:108-113`):

```
typedef struct {
    UDA   a_uda;             /* User Data Area: Register+Stacks */
    AESPD a_pd;               /* der Prozess-Deskriptor selbst */
    CDA   a_cda;               /* Konsolen-Datenbereich (Tastaturpuffer etc.) */
    EVB   a_evb[EVBS_PER_PD];  /* 6 (bzw. 7 mit Menu-Extension) Event-Blöcke */
} AESPROCESS;
```

`EVBS_PER_PD` ist 6 bzw. 7 (`aes/struct.h:38-51`) — genau ein `EVB` pro
möglicher `evnt_multi()`-Bitmaske (`MU_KEYBD, MU_BUTTON, MU_M1, MU_M2,
MU_MESAG, MU_TIMER`, plus `MU_M3` bei `CONF_WITH_MENU_EXTENSION`). Das ist
eine harte statische Obergrenze — pro Prozess kann höchstens eine
ausstehende Warte-Operation pro Event-Typ existieren.

Die globale AES-Datenregion `THEGLO D` (`aes/gemlib.h:116-144`) enthält:

```
AESPROCESS g_int[2];   /* interne Prozesse: [0]=DOS-Anwendung(Slot), [1]=Screen-Manager */
...
AESPROCESS *g_acc;     /* dynamisch allozierte Accessory-Slots, bis zu NUM_ACCS=6 */
```

`NUM_ACCS = 6` (`include/sysconf.h:37`); `NUM_PDS = NUM_ACCS + 2` (`aes/struct.h:36`,
mit dem Kommentar "acc's + ctrlpd + dos appl."). `pd_index()` bildet einen
laufenden Index `i` auf den AESPD ab: `i<2 → D.g_int[i].a_pd`, sonst
`D.g_acc[i-2].a_pd` (`aes/gempd.c:32-35`).

Fazit: EmuTOS legt **höchstens 8 gleichzeitig geplante AES-"Threads"** an
— fest, keine Allokation on-demand:
- **pid 0** — der "DOS-Anwendungs"-Slot: Desktop oder gerade laufendes
  GEM-Programm, **exklusiv genutzt** (siehe §6).
- **pid 1** — der Screen-Manager-Prozess `ctlmgr()` (permanent, terminiert
  nie).
- **pid 2..7** — bis zu 6 Accessories (`.ACC`-Dateien), permanent für die
  Boot-Session.

### 1.2 Prozess-Erzeugung: `pstart()`/`getpd()`

`aes/gempd.c:110-131` — `pstart(pcode, pfilespec, ldaddr)`:
1. `getpd()` (`gempd.c:60-70`) liefert `pd_index(curpid)` und vergibt
   `p_pid = curpid++` — reine Slot-Zuteilung aus dem statischen Array, kein
   `malloc`.
2. `p_nameit()` (`gempd.c:77-86`) füllt die 8-Zeichen-`p_name` (bis zum
   ersten `.`, rechts leerraumgepolstert).
3. `p_setappdir()` (`gempd.c:90-107`) setzt das Anwendungsverzeichnis.
4. `psetup(px, pcode)` (`aes/gemasm.S:48-70`) präpariert den privaten
   Supervisor-Stack des Prozesses so, dass ein `rts` am Ende von
   `switchto()` direkt zu `pcode` springt — das ist der initiale
   "Kontext" eines neuen AES-Threads (siehe §2.2).
5. Der neue AESPD wird vorn in die Dispatch-Ready-Liste `drl`
   eingehängt: `px->p_link = drl; drl = px;` (`gempd.c:126-128`).

Aufrufer von `pstart()`: `iprocess()` für den Screen-Manager
(`geminit.c:215-227`, aufgerufen mit `ctlmgr` als `pcode`,
`geminit.c:888`) und `load_one_acc()` für jede geladene Accessory
(`geminit.c:236-257`, mit `gotopgm` — dem asm-Trampolin, das in den
User-Modus der Accessory springt, `gemasm.S:83-119` — als `pcode`).
**Für die Hauptanwendung (Desktop/gestartetes Programm) wird `pstart()`
nirgends aufgerufen** — das ist der zentrale Befund für das
Koexistenzmodell, siehe §6.

### 1.3 `UDA` — der Kontext-Save-Bereich

`struct uda` (`aes/struct.h:81-89`):

```
struct uda {
    ULONG  u_regs[15];        /*  0  d0-d7, a0-a6 (gesicherte Register) */
    ULONG  *u_spsuper;        /* 3C  privater Supervisor-Stack-Pointer */
    ULONG  *u_spuser;         /* 40  User-Stack-Pointer beim letzten Trap-Eintritt */
    ULONG  *u_oldspsuper;     /* 44  SSP vor dem AES-Trap (für trapaes-Rückkehr) */
    ULONG  u_super[AES_STACK_SIZE]; /* 48  der private AES-Supervisor-Stack selbst */
    ULONG  u_supstk;
};
```

`AES_STACK_SIZE` ist konfigurationsabhängig 590 oder 2048 LONGs
(`include/config.h:1250-1251,176-177,657-658`) — **jeder** der bis zu 8
AES-Threads bekommt seinen eigenen, fest großen privaten Stack-Bereich
(kein Heap, kein dynamisches Wachstum).

### 1.4 `appl_init` — Registrierung eines AES-Prozesses am Trap

Wichtig: `appl_init()` (Opcode `APPL_INIT`, `aes/gemsuper.c:90-99`) **legt
keinen neuen AESPD an**. Es setzt nur `rlr->p_flags |= AP_OPEN` via
`ap_init()` (`aes/gemaplib.c:54-63`) auf dem **bereits existierenden**
`rlr` (dem aktuell laufenden AESPD — für die Hauptanwendung immer pid 0,
für eine Accessory ihr fester pid-Slot). Der AESPD selbst wurde beim
Programmstart via GEMDOS `Pexec` (Hauptanwendung, §6) bzw. beim
Accessory-Laden via `pstart()` (§1.2) angelegt.

---

## 2. Der kooperative Scheduler (`gemdisp.c`)

### 2.1 Ready-Listen und Warteschlangen

Globale Listen-Wurzeln (`aes/geminit.c:131-132`, deklariert in
`aes/aesvars.h:17-20`):

- `rlr` — **Running-List-Root**: der *aktuell laufende* AESPD (kein Array,
  ein einzelner Zeiger — es läuft physisch immer genau ein Thread).
- `drl` — **Dispatch-Ready-List**: Prozesse, die bereit sind, in `rlr`
  aufgenommen zu werden.
- `nrl` — **Not-Ready-List**: Prozesse, die auf ein Event warten
  (`WAITIN` gesetzt).
- `eul`/`dlr`/`zlr` — Event-Block-Freiliste / Delay-Liste / Zombie-Liste
  (Events, die fertig sind und auf Abholung warten).

Alle Listen sind einfach verkettete Listen über `p_link` (AESPD) bzw.
`e_link`/`e_nextp` (EVB) — keine dynamische Allokation, alle Knoten kommen
aus den festen `AESPROCESS`/`EVB`-Arrays.

### 2.2 `dsptch()` / `disp()` / `switchto()` — echter Registersatz-Wechsel

Das ist **kein** `setjmp`/`longjmp` und **kein** reines State-Machine-Polling,
sondern ein waschechter, wenn auch sehr schlanker, kooperativer
Thread-Kontextwechsel in Assembler (`aes/gemasm.S:121-194`):

**`dsptch()`** (`gemasm.S:129-167`, aufgerufen aus C via
`extern void dsptch(void)` in `aes/gemasm.h:27`):
1. Prüft `indisp` (Reentrancy-Schutz — falls bereits mitten in einem
   Dispatch, sofort `rts`, `gemasm.S:130-132`).
2. Sichert `d0-a5` und das alte `a6` in `rlr->p_uda->u_regs`
   (`UDA_REGS`), sichert `usp` in `UDA_SPUSER`, sichert den **aktuellen
   Supervisor-Stack-Pointer `sp`** in `UDA_SPSUPER` (`gemasm.S:151-158`).
3. Wechselt auf einen **gemeinsamen** Scratch-Stack (`gemasm_stack`,
   `gemasm.S:160,204-205`) und springt zu `_disp` (C-Funktion `disp()`,
   `aes/gemdisp.c:221-253`).

**`disp()`** (`gemdisp.c:219-253`) — reiner C, läuft auf dem Scratch-Stack:
1. Nimmt den bisherigen `rlr` von der Ready-Liste
   (`p = rlr; rlr = p->p_link;`).
2. Führt anhand von `p->p_stat & WAITIN` entweder `disp_act(p)` (zurück auf
   `rlr`-Kandidatenliste) oder `mwait_act(p)` (auf `nrl`, falls das
   Event noch nicht erfüllt ist) aus — das ist die eigentliche
   State-Transition des zuvor laufenden Prozesses.
3. Führt in einer Schleife `forker()` (alle anstehenden Fork-Queue-Einträge,
   §3.3) und `schedule()` aus, bis `rlr` (die neue Kopfzeile der
   Ready-Liste) besetzt ist.
4. Ruft `switchto(rlr->p_uda)` — **kehrt nie zurück**.

**`switchto()`** (`gemasm.S:169-194`) — der eigentliche Restore:
1. Nimmt `UDA*` als Parameter, lädt `d0-a5` aus `UDA_REGS`,
   `usp` aus `UDA_SPUSER`, `a6` aus `UDA_REG_A6`.
2. Lädt **`sp` aus `UDA_SPSUPER`** — das ist der Sprung auf den privaten
   Supervisor-Stack des *neuen* `rlr` (also der komplette C-Aufrufkeller
   dieses Threads: `super()`→`crysbind()`→…→`mwait()`→`dsptch()`, alles
   liegt dort noch, wie es beim letzten `dsptch()`-Aufruf dieses Threads
   verlassen wurde).
3. `clr.b _indisp` und `rts` — kehrt zur Rücksprungadresse **auf dem
   wiederhergestellten Stack** zurück, d.h. mitten in die
   `dsptch()`-Aufrufstelle des *anderen* Threads, oder — beim allerersten
   Anlauf eines neuen Threads — zur von `psetup()` präparierten
   Einstiegsadresse (§1.2).

**Fazit auf die Leitfrage "Stack-Switch oder longjmp":** Es ist ein
**echter Stack-Switch mit vollständigem Registersatz-Save/Restore**, sehr
nah an einem klassischen Coroutine-/Green-Thread-Kontextwechsel — nur ohne
MMU/Privilegwechsel (68k Supervisor-Modus für alle AES-Threads
gleichermaßen) und ohne Preemption (kein Timer-IRQ ruft `dsptch()` auf).

### 2.3 Yield-Punkte — wo genau wird umgeschaltet?

`dsptch()` wird **nur** direkt aus C-Code aufgerufen, nie implizit durch
einen Interrupt. Alle Aufrufstellen (`grep dsptch() aes/*.c`):

| Datei:Zeile | Kontext |
|---|---|
| `gemasync.c:123` | `mwait()` — Event noch nicht erfüllt, Prozess parkt |
| `gemflag.c:134` | `unsync()` — Mutex an nächsten Warter übergeben, dann selbst yielden |
| `gemctrl.c:101` | `ct_msgup()` — wartet, bis Maustaste losgelassen wird |
| `gemctrl.c:135` | `handle_arrow_msg()` — Dauerfeuer von `WM_ARROWED`, solange Taste gedrückt |
| `gemctrl.c:154` | `perform_untop()` — gibt jeder Accessory eine Ausführungschance |
| `geminit.c:561` | `all_run()` — jeder Accessory einmal Rechenzeit geben |
| `geminit.c:736` | Start des Screen-Manager-Prozesses beim Boot |
| `gemgrlib.c:53` | `gr_stilldn()` — Maus-Drag-Loop |
| `gemsuper.c:117` | `APPL_YIELD` (nur `CONF_WITH_PCGEM`) — expliziter `appl_yield()`-Trap |
| `gemsuper.c:462` | Trap-Subcode 201 ("undocumented TOS feature") — jeder AES-Trap mit `d0=201` löst sofort einen Yield aus |
| `gemaplib.c:167` | `ap_tplay()` — Event-Wiedergabe, nach jedem simulierten Event `dsptch()` |

**Kernaussage:** Ein AES-Thread gibt die CPU **ausschließlich** her, wenn
er (a) eine blockierende AES-Funktion aufruft, die über `mwait()`
tatsächlich blockiert (Event noch nicht erfüllt), oder (b) explizit
`dsptch()`/`appl_yield()` aufruft. Zwischen zwei AES-Traps läuft der
User-Code (App oder Accessory) völlig ungestört — es gibt **keine**
Preemption. Das gilt auch für den GEM-Anwendungs-Slot (pid 0): Solange die
Anwendung keinen AES-Call absetzt, hat sie exklusiv den Prozessor; die
Accessories und der Screen-Manager laufen nicht "im Hintergrund" im Sinne
von Zeitscheiben, sondern nur, wenn *irgendjemand* — typischerweise die
Anwendung selbst über ihre eigenen AES-Aufrufe (`evnt_multi` etc.) — den
Dispatcher anstößt.

### 2.4 `schedule()` und die "Idle"-Schleife

`schedule()` (`gemdisp.c:181-205`), aufgerufen aus `disp()`:
```
for (;;) {
    chkkbd();                 /* pollt Tastatur, erzeugt ggf. forkq()-Eintrag */
    while (drl) { ... disp_act(p); }   /* neu bereite Prozesse auf rlr-Kandidatur */
    if (rlr || fpcnt) break;  /* etwas zu tun? */
    #if USE_STOP_INSN_TO_FREE_HOST_CPU
    stop_until_interrupt();
    #endif
}
```
Ist nichts zu tun (`rlr` leer, keine Fork-Queue-Einträge), wird die CPU
per `STOP`-Instruktion angehalten, bis der nächste Interrupt (Tick, Tastatur,
Maus) sie über `forkq()` wieder mit Arbeit versorgt (`gemdisp.c:201-203`).
Das ist der einzige "echte" Idle-Mechanismus des Systems.

---

## 3. Block/Wake/Cancel (`gemasync.c`, `gemflag.c`, `geminput.c`)

### 3.1 Event-Blöcke (`EVB`) als Warte-Deskriptor

`struct evb` (`aes/struct.h:97-107`) — ein `EVB` beschreibt *eine*
ausstehende Warteoperation eines Prozesses auf *einen* Event-Typ
(`e_mask`, eine der `MU_*`-Bitmasken). Pro Prozess gibt es max.
`EVBS_PER_PD` (6/7) davon, aus dem festen `a_evb[]`-Array (§1.1), verwaltet
über eine globale Freiliste `eul` (`iasync()`, `gemasync.c:130-179`:
`e = eul; eul = e->e_nextp;` — Slab-Allokator ohne `malloc`).

### 3.2 `iasync()` — Warte-Registrierung je Event-Typ

`iasync(afunc, aparm)` (`gemasync.c:130-179`) holt einen freien `EVB`,
hängt ihn in `rlr->p_evlist` und verzweigt je nach `afunc` zu einer der
Registrierfunktionen in `geminput.c`/`gemflag.c`:

| `afunc` | Handler | Datei:Zeile |
|---|---|---|
| `MU_KEYBD` | `akbin()` | `geminput.c:486-493` |
| `MU_BUTTON` | `abutton()` | `geminput.c:545-565` |
| `MU_M1`/`MU_M2`/`MU_M3` | `amouse()` | `geminput.c:568-587` |
| `MU_MESAG` | `aqueue(FALSE,…)` | `gemqueue.c:85-125` |
| `MU_SDMSG` | `aqueue(TRUE,…)` | `gemqueue.c:85-125` |
| `MU_TIMER` | `adelay()` | `geminput.c:496-542` |
| `MU_MUTEX` | `amutex()` | `gemflag.c:89-102` |

Jeder Handler prüft zunächst, ob die Bedingung **bereits jetzt** erfüllt
ist (z.B. `akbin()`: ist schon ein Zeichen im Tastaturpuffer?) und ruft in
dem Fall sofort `azombie(e, ret)` auf — **kein** Blockieren nötig. Sonst
wird der `EVB` per `evinsert()` (`gemasync.c:81-93`) in die passende
Warteliste eingehängt (z.B. `rlr->p_cda->c_iiowait` für Tastatur,
`p_cda->c_bsleep`/`c_msleep` für Maus, `dlr` für Timer, `sy_wait` für
Mutex) und **bleibt unerfüllt**, bis ein späteres Ereignis sie abräumt.

### 3.3 Zombie-Mechanismus, `mwait()`, `apret()`

`azombie(e, ret)` (`gemasync.c:64-78`) markiert einen `EVB` als
`COMPLETE`, hängt ihn in die globale Zombie-Liste `zlr` und ruft
`signal(e)` (`gemasync.c:35-61`) auf — das ist die **Wake-Funktion**: sie
setzt `p->p_evflg |= e->e_mask` und, falls `p` **nicht** der laufende
Prozess ist und seine gesamte Wartemaske jetzt erfüllt ist
(`p->p_evflg & p->p_evwait`), verschiebt sie `p` von `nrl` nach `drl`
(bereit zum Wiederanlauf).

`mwait(mask)` (`gemasync.c:117-127`) ist der eigentliche Block-Punkt:
setzt `rlr->p_evwait = mask`; ist die Maske noch nicht (ganz) erfüllt,
`rlr->p_stat |= WAITIN; dsptch();` — der Yield-Punkt aus §2.3. Nach der
Rückkehr (also nachdem der Scheduler diesen Thread wieder ausgewählt hat)
liefert `mwait()` `rlr->p_evflg` zurück.

`apret(mask)` (`gemasync.c:182-214`) holt für einen erfüllten Event-Typ
den zugehörigen `EVB` aus `rlr->p_evlist`, entfernt ihn aus der
Zombie-Liste, löscht die entsprechenden Bits in `p_evwait`/`p_evflg` und
gibt `e->e_return` zurück — der eigentliche "Rückgabewert" der
Warteoperation (z.B. das gedrückte Zeichen).

`ev_block()` (`aes/gemevlib.c:64-69`) ist der öffentliche
Ein-Event-Wrapper: `mwait(iasync(code, lvalue)); return apret(code);` —
das Muster hinter `evnt_keybd`, `evnt_button`, `evnt_mouse`, `evnt_timer`.
`ev_multi()` (`gemevlib.c:152-294`) ist dasselbe Muster für mehrere
gleichzeitige Event-Typen mit einer "Quick-Check" -Phase davor (bereits
erfülltes Event sofort zurückgeben, ohne den Scheduler zu bemühen —
`gemevlib.c:169-229`).

### 3.4 `acancel()` — Abbruch nicht mehr benötigter Waits

`acancel(m)` (`gemasync.c:217-243`) durchläuft `rlr->p_evlist`, entfernt
für jede Maske in `m`, die *nicht* `NOCANCEL`/`COMPLETE` ist, den `EVB`
aus seiner Warteliste (`takeoff()`, `gemasync.c:96-114`) und markiert sie
als storniert. Wird von `ev_multi()` nach der Rückkehr aus `mwait()`
aufgerufen, um die *nicht* erfüllten Event-Registrierungen wieder
loszuwerden (`gemevlib.c:264`).

### 3.5 Zeit-/IRQ-getriebenes Wecken

`tchange(c)` (`gemflag.c:32-68`) — vom Tick-Interrupt (`tikcod`, indirekt
über `forkq(tchange,…)`, siehe §3.6) — läuft die Delay-Liste `dlr` ab und
ruft `evremove(d, 0)` (`geminput.c:279-285`, entfernt aus der Liste + ruft
`azombie()`) für jeden Timer auf, dessen Restzeit abgelaufen ist. Für den
noch nicht abgelaufenen Rest wird das Hardware-Compare-Register
`CMP_TICK`/`NUM_TICK` neu programmiert (`gemflag.c:61-65`) — EmuTOS nutzt
also einen **programmierbaren einzelnen** Compare-Timer, nicht einen
Timer pro wartendem Prozess.

`post_keybd()`/`post_button()`/`post_mouse()`
(`geminput.c:299-308,463-483,446-456`) sind die Wake-Funktionen für
Tastatur/Maustaste/Mausposition: Sie laufen die passende Warteliste
(`c_iiowait`, `c_bsleep`, `c_msleep`) ab und rufen `evremove()` für jeden
`EVB` auf, dessen Bedingung jetzt erfüllt ist.

### 3.6 `forkq()`/`forker()` — die Interrupt→Thread-Brücke

`forkq(fcode, fdata)` (`gemdisp.c:52-71`) ist ein Ringpuffer
(`D.g_fpdx[NFORKS]`, `NFORKS=32`, `aes/struct.h:57`), in den
**Interrupt-Handler** (Tick-ISR `tikcod`, Tastatur-/Maus-ISR über
`b_click()`/`b_delay()`/`chkkbd()`) Funktionszeiger + Datum ablegen, **statt**
direkt AES-Zustand zu verändern — er ist "expected to be called with
interrupts disabled" (`gemdisp.c:48`). `forker()` (`gemdisp.c:101-155`)
leert diesen Ring **synchron aus dem Dispatcher-Kontext** (`disp()` ruft
`forker()` vor `schedule()`, `gemdisp.c:237-244`) und ruft für jeden
Eintrag die hinterlegte Funktion auf — `kchange()`, `bchange()`,
`mchange()`, `tchange()` — die dann ihrerseits `post_keybd()` etc.
aufrufen. Das ist der **einzige** Übergabepunkt zwischen
Interrupt-Kontext und dem kooperativen AES-Zustand: Interrupts fassen nie
direkt `p_link`/`rlr`/`EVB`-Listen an.

`chkkbd()` (`gemdisp.c:158-178`) ist kein ISR, sondern wird aus
`schedule()` und `ev_multi()` (`gemevlib.c:169`) **gepollt** — sie fragt
den GSX/VDI-Tastaturstatus ab und legt bei Änderung `forkq(kchange,…)` an.
Die Interrupt-Seite selbst (`b_click`/`b_delay`, `aes/geminput.c:114-182`)
wird aus dem Maus-/Tick-ISR **in Assembler** angesprungen (`far_bcha`
in `gemdosif.S:306ff`, nicht Teil dieses Scopes — siehe "Out-of-scope").

### 3.7 Mutex/Semaphor (`gemflag.c`)

`SPB` (`aes/struct.h:165-170`) ist ein Zähl-Semaphor mit Warteliste:
`sy_tas` (Zähler), `sy_owner`, `sy_wait` (EVB-Kette). `tak_flag()`
(`gemflag.c:71-86`) versucht atomar zu erwerben (rekursiv für den
aktuellen Owner: `sy_owner==rlr` zählt einfach hoch). `unsync()`
(`gemflag.c:105-139`) gibt frei; ist danach jemand in `sy_wait`, wird ihm
der Besitz per `azombie()` übergeben **und sofort `dsptch()` aufgerufen**
— ein expliziter Yield direkt bei der Übergabe. Genutzt für `wind_spb`
(Fensterbildschirm-Zugriffssperre, `aes/geminit.c:138`,
`aes/gemshlib.c:668-669`) — der einzige Nutzer im gelesenen Code.

---

## 4. Event-/Message-Routing (`gemctrl.c`, `gemqueue.c`, `gemwmlib.c`)

### 4.1 `ctlmgr()` — der permanente Screen-Manager-Thread

`ctlmgr()` (`gemctrl.c:427-486`) ist selbst ein AES-Thread (pid 1, §1.1)
mit einer Endlosschleife: `ev_multi()` auf Tastatur+Maustaste(+Menüzeile)
wartend, dann je nach erfülltem Event `hctl_window()` (Klick auf ein
Fenstergadget, `gemctrl.c:158-315`) oder `hctl_rect()` (Klick auf die
Menüzeile → `mn_do()`, `gemctrl.c:318-371`) aufrufend. `ctlmgr` besitzt
initial Maus **und** Tastatur (`gl_mowner = ctl_pd = iprocess(...)`,
`geminit.c:888`).

### 4.2 Eigentümerwechsel: `gl_mowner`, `ctl_pd`, `set_mown()`

`gl_mowner` (`aes/geminput.c:52`) ist der AESPD, der aktuell Maus- **und**
Tastaturereignisse empfängt. `chkown()` (`geminput.c:316-325`) entscheidet
bei Tastendruck anhand der Mausposition, wer sie bekommt: innerhalb der
aktiven Steuer-Rechteck (`ctrl`, von der aktiven Anwendung via
`set_ctrl()`/`ct_chgown()` gesetzt) → `gl_cowner` (die App selbst); über
Menüzeile/fremdem Fenster → `ctl_pd` (der Screen-Manager); sonst → der
Desktop-Fenster-Owner (`D.w_win[0].w_owner`). `set_mown()`
(`geminput.c:220-235`) wechselt den Eigentümer nur, wenn keine Maustaste
gedrückt ist, und postet sofort einen simulierten Maus-/Button-Event an
den neuen Eigentümer, damit dieser sich sofort "richtig" fühlt.

**Das ist die zentrale Regel für "wer besitzt den Bildschirm":** zu jedem
Zeitpunkt genau **ein** `gl_mowner`. Die aktive Anwendung besitzt ihn
für ihr Steuer-Rechteck (typischerweise die Arbeitsfläche des obersten
Fensters), der Screen-Manager für Menüzeile/Fenstergadgets/andere Fenster.

### 4.3 Die Message-Pipe (`p_queue`/`aqueue`/`doq`)

Jeder AESPD hat eine 128-Byte-Byte-Queue `p_queue`/`p_qindex`
(§1.1). `aqueue(isqwrite, e, lm)` (`gemqueue.c:85-125`) ist der
Ein-/Auslese-Handler: bei genug Platz (Schreiben) bzw. vorhandenen Daten
(Lesen) wird `doq()` (`gemqueue.c:31-82`) direkt ausgeführt und der
Aufrufer per `azombie()` sofort freigegeben; sonst wird der `EVB` in eine
Warteliste (`p_qnq` für "will schreiben, Queue voll" bzw. `p_qdq` für
"will lesen, Queue leer") gehängt — das ist der Beleg, dass ein
**Schreiber blockiert wird, wenn die Empfänger-Queue voll ist** (nicht
verwirft, nicht überschreibt).

`doq()` koalesziert beim Schreiben `WM_REDRAW`/`WM_ARROWED`/`WM_HSLID`/
`WM_VSLID`-Nachrichten mit bereits wartenden Nachrichten desselben Typs
(Rect-Union für `WM_REDRAW`, Overwrite für `WM_ARROWED`,
`gemqueue.c:44-71`).

`ap_sendmsg()` (`aes/gemwmlib.c:851-863`) ist die High-Level-API darüber:
füllt ein 8-Word-Standardmessage (`msg[0]=type, msg[1]=rlr->p_pid, …`) und
ruft `ap_rdwr(MU_SDMSG, towhom, 16, msg)` → `ev_block(MU_SDMSG,…)` →
letztlich `aqueue(TRUE,…)`.

### 4.4 `AC_OPEN`/`AC_CLOSE` — Accessory-Lebenszyklus-Nachrichten

`AC_OPEN=40`, `AC_CLOSE=41` (`include/aesdefs.h:242-243`).
`hctl_rect()` (`gemctrl.c:336-350`) sendet `AC_OPEN` an eine Accessory,
wenn ihr Menüeintrag im System-Menü (`THEDESK`) angeklickt wird.
`mn_cleanup()` (`aes/gemmnlib.c:740-747`) sendet `AC_CLOSE` an **alle**
registrierten Accessories (`desk_ppd[]`), aufgerufen aus `ap_exit()`
(`aes/gemaplib.c:251-261`) — d.h. **jede** terminierende Anwendung
(nicht nur der Desktop) löst einen `AC_CLOSE`-Broadcast an alle
Accessories aus, gefolgt von `wait_for_accs(AP_ACCLOSE)`
(`geminit.c:599-628`), das blockiert, bis jede Accessory das
`p_flags |= AP_ACCLOSE`-Bit gesetzt hat (also die Nachricht tatsächlich
über `evnt_mesag()`/`evnt_multi()` abgeholt hat, `gemsuper.c:145-149`) —
maximal `WAIT_TIMEOUT=500` Scheduler-Runden (`geminit.c:102,609`), danach
Timeout mit `KDEBUG`-Warnung, kein Hard-Fail.

**Anmerkung zur Aufgabenstellung:** `AP_TERM` wurde in der Aufgabe als zu
prüfendes Symbol genannt, kommt aber in diesem EmuTOS-Checkout **nicht
vor** (`grep -rn AP_TERM include/ aes/` → kein Treffer). Es existieren nur
`AP_OPEN`, `AP_MESAG`, `AP_ACCLOSE` (`aes/struct.h:115-117`). Vermutlich
eine veraltete/verwechselte Bezeichnung — nicht weiterverfolgt (außerhalb
des Scopes, kein FreeMiNT-Grep in diesem Auftrag).

---

## 5. Koexistenzmodell: Desktop + 6 Accessories + 1 Hauptanwendung

### 5.1 Boot-Reihenfolge (`geminit.c`, `gemstart.S`)

1. `gem_main()` (`geminit.c:753-903`): zählt Accessories
   (`count_accs()`, max. `NUM_ACCS=6`), initialisiert alle `NUM_PDS`
   AESPDs (Schleife `geminit.c:851-874`), setzt den AES-Trap
   (`set_aestrap()`), startet den Screen-Manager-Thread `ctlmgr` als
   `pid 1` (`geminit.c:888`), und ruft dann
   `aes_run_rom_program(accdesk_start)` (`geminit.c:894`,
   `aes_run_rom_program` in `aes/gemshlib.c:570-580`) — das ist ein
   **GEMDOS `Pexec`**, kein AES-`pstart()`.
2. `accdesk_start` (`gemstart.S:139-163`) läuft **auf dem privaten
   Supervisor-Stack von AES-Prozess 0** (`UDA_SPSUPER(a0)`,
   `gemstart.S:142`) und ruft `run_accs_and_desktop()`
   (`geminit.c:654-750`): GSX-Öffnen, Mauszeiger, `load_accs()` — lädt
   jede Accessory via `load_one_acc()` → `pstart(gotopgm,…)` (§1.2, legt
   also **echte** neue AES-Threads pid 2..7 an) —, dann `dsptch()`
   (`geminit.c:736`, gibt allen Accessories/dem Screen-Manager die erste
   Gelegenheit zu laufen), `wait_for_accs(AP_MESAG)`
   (`geminit.c:737`, wartet bis alle Accessories initialisiert
   sind — siehe §4.4-Mechanismus, hier mit `AP_MESAG`), dann
   `sh_main(isauto, isgem)` (`geminit.c:739`) — die eigentliche
   Shell-Schleife.

### 5.2 `sh_main()`/`sh_ldapp()` — Desktop und Anwendung teilen sich EINEN Slot

Das ist der zentrale, in der Aufgabenstellung explizit erfragte Punkt:

`sh_main()` (`aes/gemshlib.c:692-731`) ist eine **Schleife**, die
wiederholt `sh_ldapp()` (`gemshlib.c:592-689`) aufruft. `sh_ldapp()`
benennt den **aktuell laufenden** AESPD um (`p_nameit(rlr, fname)`,
`gemshlib.c:604,634`) und startet dann entweder den ROM-Desktop
(`aes_run_rom_program(deskstart)`, `gemshlib.c:606`) oder eine normale
Anwendung (`dos_exec(PE_LOADGO, D.s_cmd, …)`, `gemshlib.c:645`) — **beides
GEMDOS-`Pexec`-Aufrufe** (Modus `PE_BASEPAGEFLAGS`+`PE_GOTHENFREE` bzw.
`PE_LOADGO`), **kein** `pstart()`.

`dos_exec()` (`aes/gemstart.S:169-243`) ist ein handgeschriebener Wrapper
um den GEMDOS-`Pexec`-Trap (`#1`, Funktion `0x4B`), der explizit
kommentiert (`gemstart.S:178-184`): *"We are here on the private
supervisor stack of the AES process 0. We are going to call Pexec() from
the supervisor mode… the child program (desktop or other) will call the
AES through a trap #2… this AES process will reenter itself."* — d.h. der
Desktop **und** jede von ihm gestartete Anwendung laufen **verschachtelt
auf demselben physischen Aufrufkeller**, unter derselben AESPD-Identität
`pid 0`. GEMDOS `Pexec` ist dabei ein ganz normaler **synchroner**
Funktionsaufruf im BDOS-Sinne (lädt das Kindprogramm, springt hinein,
kehrt erst zurück, wenn das Kind `Pterm` aufruft) — er hat **keine**
Verbindung zum AES-`dsptch()`-Scheduler. Während die Kindanwendung läuft
(zwischen ihren eigenen AES-Traps), laufen Accessories/Screen-Manager
**nicht** — nur wenn die Kindanwendung selbst einen blockierenden
AES-Call absetzt, bekommen sie über `dsptch()` eine Chance.

Nach Rückkehr aus `dos_exec()` räumt `sh_ldapp()` auf (offene `AP_OPEN`
ohne `appl_exit()` wird nachgeholt, `gemshlib.c:650-657`; `wind_spb` wird
freigegeben falls die Anwendung sie noch hielt, `gemshlib.c:668-669`) und
bestimmt das nächste zu ladende Programm (nach einer normalen Anwendung
per Default wieder der Desktop, `gemshlib.c:639-643`).

**Konsequenz für das Koexistenzmodell:**
- **Höchstens eine** "Hauptanwendung" (Desktop *oder* gestartetes
  GEM-Programm — nie beide gleichzeitig) läuft im System, seriell
  nacheinander unter derselben AESPD-Identität `pid 0`. Ein neu
  gestartetes Programm ist **kein** neuer AES-Thread, sondern ein neuer
  GEMDOS-Prozess (neue Basepage), der die **gleiche** AES-Thread-Identität
  weiterbenutzt (analog zu einem UNIX-`exec()`, das das Programmimage
  austauscht, aber die PID behält).
- Bis zu **6 Accessories** (`pid 2..7`) laufen als eigenständige,
  permanente AES-Threads über die volle Boot-Session, kooperativ über
  `dsptch()`/`disp()`/`switchto()` geplant.
- Der **Screen-Manager** (`pid 1`, `ctlmgr()`) läuft ebenfalls permanent
  als eigener AES-Thread und ist der einzige, der Klicks auf
  Menüzeile/Fenstergadgets direkt behandelt.
- Der Bildschirm (genauer: Maus+Tastatur-Fokus) gehört **immer genau
  einem** `gl_mowner` — der aktiven Hauptanwendung für ihr
  Steuer-Rechteck, sonst dem Screen-Manager (§4.2).

---

## 6. Die 44 Funktionen — Gruppierung nach Scheduling-Rolle

Exakte Funktionszählung aus den gelesenen Dateien (öffentlich + `static`
Helfer, Deklarationen ohne Body nicht mitgezählt):

| Datei | Anzahl | Rolle |
|---|---|---|
| `gemasync.c` | 8 (2 static: `signal`, `takeoff`) | Event-Block-Lebenszyklus: registrieren (`iasync`), blockieren (`mwait`), erfüllen+wecken (`azombie`/`signal`), abholen (`apret`), stornieren (`acancel`) |
| `gemdisp.c` | 8 (3 static: `disp_act`, `mwait_act`, `schedule`; 1 Doppel-Deklaration `disp`) | Der eigentliche Dispatcher: Fork-Queue (`forkq`/`forker`), Tastatur-Poll (`chkkbd`), State-Transition + Scheduler-Loop (`disp`/`schedule`) |
| `gemflag.c` | 4 | Timer-Ablauf (`tchange`), Zähl-Semaphor (`tak_flag`/`amutex`/`unsync`) |
| `geminput.c` | 26 (10 static Helfer, 1 bedingt kompiliert `wheel_change`) | Maus-/Tastatur-Rohdaten → Event-Wecken: ISR-nahe Vorverarbeitung (`b_click`/`b_delay`), Ownership (`set_mown`/`get_mown`/`chkown`), Queue-Primitive (`nq`/`dq`/`fq`), Wake-Funktionen (`post_keybd`/`post_button`/`post_mouse`/`evremove`), Registrierfunktionen (`akbin`/`abutton`/`amouse`/`adelay`) |
| `gemctrl.c` | 9 (6 static Helfer) | Screen-/Menü-Eigentümer-Verwaltung (`ct_chgown`/`ct_mouse`), der permanente Screen-Manager-Thread selbst (`ctlmgr`) und seine Klick-Handler (`hctl_window`/`hctl_rect`/`handle_arrow_msg`/`perform_untop`) |

Ergänzend (nicht in der Aufgabenliste, aber Teil derselben Maschinerie und
mitgelesen, da direkte Abhängigkeiten): `gemqueue.c` (2 Funktionen —
`doq`/`aqueue`, die Message-Pipe), `gempd.c` (7 Funktionen — Prozess-Slot-
Verwaltung), `gemevlib.c` (9 Funktionen — die öffentliche
`evnt_*`/`ev_*`-API, die auf `gemasync`/`gemdisp` aufsetzt), `gemaplib.c`
(6 Funktionen — `appl_*`-API, u.a. `ap_exit()` mit dem
Accessory-Shutdown-Protokoll aus §4.4).

Die in der Aufgabenstellung genannten Richtwerte ("~8/~7/~4/~21/~4")
treffen für `gemasync`/`gemdisp`/`gemflag` exakt zu; für `geminput`
(tatsächlich 26, nicht 21) und `gemctrl` (tatsächlich 9, nicht 4) liegt
die Differenz an mitgezählten `static`-Helfern, die für das
Scheduling-Verständnis notwendig waren (z.B. `chk_ctrl`/`chkown`/
`inorout`/`post_mouse`/`post_button` in `geminput.c`; `hctl_window`/
`hctl_rect`/`handle_arrow_msg`/`perform_untop`/`ct_msgup` in `gemctrl.c`)
— keine Abweichung in der Funktions**gruppe** selbst, nur in der
Zähltiefe (öffentliche API vs. öffentliche API + private Helfer).

---

## 7. Bezug zu NewTOS — heutiger Stand und Übertragbarkeit

### 7.1 Heutiger NewTOS-Stand (Grep-Befund, nicht spekuliert)

`tos-api/src/aes/pd.rs:1-19` (Kommentar-Header) und `event.rs:7-12`
dokumentieren den Bring-up-Zustand explizit: **eine** AES-Anwendung, kein
Scheduler. Konkret:

- `pd.rs` (`Aespd`/`Table`) hat **keine** Scheduling-Felder — kein
  `p_link`, kein `p_stat`/`WAITIN`, keine Ready-/Not-Ready-Liste. Es ist
  eine reine Slot-Tabelle (`pid`, `flags`, `name`, `queue`), keine
  Thread-Verwaltung (`tos-api/src/aes/pd.rs:244-261`).
- `event.rs` implementiert `evnt_button`/`evnt_mouse`/`evnt_multi`/
  `evnt_keybd`/`evnt_mesag`/`evnt_timer` (`event.rs:131-467`) jeweils als
  **Busy-Poll-Schleife** (`while !cond() { h.pump_input(); }` bzw.
  `loop { …; h.pump_input(); }`) — funktional das Analogon zu EmuTOS'
  `mwait()`/`ev_block()`, aber **ohne** Kontextwechsel: die aufrufende
  Exception-Handler-Ebene bleibt die ganze Wartezeit über aktiv, es gibt
  keinen zweiten Thread, dem währenddessen die CPU gegeben werden könnte.
- Die `MsgQueue` in `pd.rs:59-225` bildet EmuTOS' `p_queue`/`doq`
  (inkl. `WM_REDRAW`-Rect-Union-Koaleszenz, `pd.rs:121-177`) funktional
  nach, aber mit einem dokumentierten Kontraktbruch gegenüber EmuTOS: eine
  volle Queue liefert `false` statt den Schreiber zu blockieren
  (`pd.rs:136-143`, explizit kommentiert als Bring-up-Kompromiss, weil es
  "keinen AES-Scheduler zum Parken eines synchronen Kernel-Posters" gibt).

### 7.2 Vorhandene, artverwandte Infrastruktur: der GEMDOS-Scheduler (`sched.rs`)

`tos-api/src/bdos/process/sched.rs` (127 Zeilen) existiert bereits als
**GEMDOS-Ebene** kooperativer Round-Robin-Scheduler für `Pwait`
(Phase-06 EC4, siehe `docs/STATUS.md` Eintrag 2026-06-22). Er verwaltet
ein Run-Set über die Prozess-Tabelle, sichert bei einem Yield den **vollen
EL0-Exception-Frame** (`saved_frame`) und liefert eine `Switch`-Struktur
mit `TTBR0_EL1`-Seitentabellenwurzel + Fresh/Resume-Unterscheidung
(`sched.rs:33-45`). Architektonisch ist das bereits **genau** das, was ein
AES-Level-`dsptch()`/`switchto()` bräuchte: vollständiger Kontext-Save +
späteres Resume, nur eine Ebene höher angesiedelt (GEMDOS-Prozess- statt
AES-Thread-Granularität) und bisher nur an den `Pwait`-Trap gekoppelt statt
an AES-Event-Waits.

### 7.3 Wo die EmuTOS-Mechanik sauber auf das NewTOS-Modell passt — und wo nicht

**Passt tendenziell sauber:**
- Die **Datenstruktur-Seite** (AESPD-Äquivalent als feste Slot-Tabelle,
  Ready-/Wait-Listen, EVB-Freiliste) ist ABI-neutral — reine
  Buchführungsstruktur, unabhängig von EL0/EL1.
- Das **Yield-nur-beim-Blockieren-Prinzip** (§2.3) ist unabhängig von der
  Zielarchitektur: Es ist eine Scheduling-*Policy*, keine
  68k-Spezifika.
- `sched.rs` liefert bereits den **Mechanik-Baustein** (Frame-Save +
  TTBR0-Switch) für einen Kontextwechsel bei einem AES-Trap, der
  blockieren muss.

**Passt nicht 1:1, braucht explizite Entscheidung:**
- EmuTOS' AES-Threads laufen **alle im selben Adressraum und Privileglevel**
  (68k Supervisor-Modus, flach, kein MMU) — ein `switchto()` ist ein reiner
  SP+Register-Tausch **ohne** Adressraumwechsel. Bei NewTOS liegt die
  Anwendung bei **EL0** hinter `TTBR0_EL1` (siehe `docs/MEMORY.md`
  `el0-app-memory-must-be-mapped`), der AES-Kern aber bei **EL1** — ein
  AES-Thread-Wechsel zwischen zwei *Anwendungen* wäre dann kein reiner
  SP-Tausch mehr, sondern ein `TTBR0_EL1`-Wechsel wie bei `sched.rs`.
  Zwischen Accessories/Screen-Manager (die, wie der Desktop, vermutlich
  privilegierten/EL1-nahen Code darstellen — vgl. `desk-bypasses-app-abi-path`
  in `docs/MEMORY.md`) wäre dagegen ggf. kein Adressraumwechsel nötig. Diese
  Unterscheidung (EL0-Anwendungs-Thread vs. EL1-interner AES-Thread) hat in
  EmuTOS **keine** Entsprechung — dort sind Anwendung, Accessory und
  Screen-Manager architektonisch identisch (alle Supervisor-Modus-Threads
  mit privatem Stack). Das ist eine echte Design-Entscheidung, keine reine
  Übernahme.
- EmuTOS' "ein Anwendungs-Slot, seriell über GEMDOS-`Pexec` befüllt"-Modell
  (§5.2) ist der Grund, warum EmuTOS mit nur 8 AES-Threads auskommt und
  **kein** "mehrere GEM-Anwendungen gleichzeitig"-Multitasking kennt. Falls
  NewTOS (per `docs/MEMORY.md` `native-64bit-abi-future` /
  Post-2a-Roadmap) auf **mehrere gleichzeitig laufende** GEM-Anwendungen
  zielt (echtes Multitasking, nicht nur Desktop+Accessories), ist das
  EmuTOS-Modell an dieser Stelle **kein** Vorbild mehr, sondern nur noch
  Vorbild für Accessories/Screen-Manager-artige interne Dienste — die
  Aufwandsschätzung müsste hier explizit zwischen "EmuTOS-treue
  Ein-App-Semantik nachbauen" und "FreeMiNT-artiges Mehr-App-Modell
  aufbauen" unterscheiden (das ist exakt die eingangs genannte
  Lead-Fragestellung).
- Der **Interrupt→Fork-Queue→Dispatcher**-Übergabepunkt (§3.6) ist
  architektur-neutral übertragbar (ein Ringpuffer + "im Dispatcher-Kontext
  synchron abarbeiten" ist unabhängig von 68k/AArch64), sofern NewTOS eine
  vergleichbare Trennung zwischen "IRQ darf nur eine Notiz hinterlassen"
  und "Trap-/Dispatcher-Kontext arbeitet sie ab" einführt — aktuell arbeiten
  `event.rs`s Poll-Schleifen (`h.pump_input()`) direkt im Trap-Handler,
  ohne diese Trennung.

Diese Bewertung ist eine **Einordnung**, keine Entscheidung — die
Umsetzung (EmuTOS-Stil vs. FreeMiNT-Stil) liegt beim Lead.

---

## Key primitives table

| Dimension | EmuTOS-Befund | Fundstelle |
|---|---|---|
| **Scheduler-Typ** | Kooperativ, kein Preemption; Yield ausschließlich an expliziten `dsptch()`-Aufrufstellen (Blockieren in `mwait()`, `appl_yield`, diverse Wait-Loops) | `aes/gemdisp.c:221-253`, Aufrufstellen §2.3 |
| **Kontextwechsel-Mechanismus** | Echter Register+SP-Tausch in Assembler (`dsptch`/`disp`/`switchto`); jeder Thread hat privaten Supervisor-Stack (`UDA.u_super[AES_STACK_SIZE]`); kein `setjmp`/`longjmp`, kein Adressraumwechsel (68k Supervisor-Modus, flach) | `aes/gemasm.S:121-194`, `aes/struct.h:81-89` |
| **Prozess-/Thread-Zustände** | `rlr` (laufend, genau 1), `drl` (bereit), `nrl` (wartet, `p_stat & WAITIN`), plus pro Event ein `EVB` in `p_evlist`/Wartelisten/`zlr` (erfüllt, ungeholt) | `aes/aesvars.h:15-20`, `aes/struct.h:112-113`, §2.1/§3 |
| **Anzahl gleichzeitiger AES-Threads** | Fest max. 8: pid 0 (Desktop **oder** eine Anwendung, seriell über GEMDOS `Pexec`), pid 1 (Screen-Manager, permanent), pid 2..7 (bis zu 6 Accessories, permanent) | `aes/struct.h:36`, `include/sysconf.h:37`, §1.1/§5.2 |
| **IPC/Message-Mechanismus** | Pro-Prozess 128-Byte-Byte-Queue (`p_queue`/`p_qindex`), Schreiber blockiert bei voller Queue (`evinsert` in `p_qnq`), Rect-Union-Koaleszenz für `WM_REDRAW`; dazu Event-Flags (`p_evwait`/`p_evflg`) + Zähl-Semaphor (`SPB`) für Ressourcen wie den Bildschirm | `aes/gemqueue.c:31-125`, `aes/gemflag.c:71-139`, §4.3/§3.7 |
| **Wecken (Wake-Quellen)** | Interrupt (Tick/Tastatur/Maus) → `forkq()`-Ringpuffer → im Dispatcher-Kontext synchron per `forker()` abgearbeitet → `post_keybd`/`post_button`/`post_mouse`/`tchange` → `evremove`/`azombie` → `signal()` verschiebt den Zielprozess `nrl`→`drl` | `aes/gemdisp.c:52-155`, §3.5/§3.6 |
| **Speicher-/Privilegmodell** | Kein MMU, flacher Adressraum, alle AES-Threads durchgängig im 68k-Supervisor-Modus (User-Modus nur für den *Code* der Anwendung/Accessory zwischen Traps); AES-Trap (`trap #2`) schaltet auf den privaten Supervisor-Stack des `rlr` um | `aes/gemdosif.S:222-285`, `aes/gemasm.S:83-119` |
| **Präemption** | Keine — Zeit-/Tastatur-/Maus-Interrupts erzeugen nur `forkq()`-Einträge, ändern nie direkt `rlr` | §3.6, §2.3 |

---

## Out-of-scope observations

- `far_bcha`/`far_mcha`/`aes_wheel`/`tikcod` (die eigentlichen
  ISR-Assembler-Trampoline für Maus/Tick, `aes/gemdosif.S:306ff`) wurden
  nicht bis ins Detail gelesen — sie liegen unterhalb des in der Aufgabe
  genannten Dateisatzes (`gemasync.c`/`gemdisp.c`/`gemflag.c`/
  `geminput.c`/`gemctrl.c`/`gempd.c`/`gemevlib.c`/`geminit.c`) und ändern
  nichts an den hier dokumentierten Kernaussagen (sie speisen nur
  `forkq()`, §3.6).
- `gemmnlib.c`/`gemwmlib.c` (Menü- bzw. Fenstermanager) wurden nur so
  weit gelesen, wie sie das Event-Routing/die Message-Pipe berühren
  (`ap_sendmsg`, `mn_cleanup`) — die volle Fenstermanager-Logik ist ein
  eigenes, größeres Thema und nicht Teil dieses Scheduling-Scopes.
- `bdos/`-seitiges `Pexec`/`Pwait` (die GEMDOS-Prozessverwaltung, die
  hinter `dos_exec()` steht) wurde nur so weit verifiziert, wie nötig, um
  die "ein Slot, seriell"-Aussage in §5.2 zu belegen (`gemstart.S`s
  Kommentare + `gemshlib.c`); eine vollständige GEMDOS-Prozess-Spec ist
  nicht Teil dieses Auftrags.
