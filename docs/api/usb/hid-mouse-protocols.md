# USB-Maus: Protokolle, Defektstand, Hotplug, Diagnosewerkzeug (USB01 Schritt 1)

> Ergaenzung zu [`hid-mouse.md`](hid-mouse.md) (dort: nur Boot-Maus →
> IKBD-Relativpaket). Dieses Dokument ist **Analyse, kein Code**: es vergleicht
> unseren Treiber mit Circle und FreeMiNT, belegt den Stand der vier Defekte vom
> 2026-09-09, beschreibt das Hotplug-Verhalten und schlaegt den Weg fuer das
> Diagnosewerkzeug und die Reparaturen vor. Ticket: `TASK/USB01.md`.
> Spec-Gruppe: [`_state.md`](_state.md).

**Scope in einem Satz:** Wie praesentieren sich USB-Maeuse, wie erkennen und
dekodieren Circle und FreeMiNT sie, was macht `kernel/src/drivers/usb/` heute
damit, in welchem Zustand sind die Defekte 1–4, was passiert beim Umstecken,
und welcher Weg fuehrt zu einer Diagnosedatei pro Geraet.

## 0. Quellenlage und Kennzeichnung

* **[G]** = gelesen, mit `Datei:Zeile` belegt (diese Sitzung, Stand `main`
  `baf36a98`). **[S]** = Schluss aus dem Gelesenen, nicht selbst beobachtet.
  **[?]** = nicht entscheidbar aus den lokalen Quellen.
* **Lokal in `references/` liegt keine USB-2.0-, HID- oder xHCI-Spezifikation**
  (`references/datasheets/` enthaelt nur das BCM2711-Datenblatt). Darum:
  * HID-Aussagen stuetzen sich auf **HID 1.11** (usb.org, `hid1_11.pdf`), das in
    dieser Sitzung geholt und per `pdftotext` gelesen wurde (Kopie nur im
    Session-Scratchpad `…/scratchpad/usb01/hid1_11.pdf`, **nicht** im Repo).
    Zitiert wird nach Kapitel (§4.2, §4.3, §7.2.4, §7.2.6, §8.1, Anhang B.2).
  * Aussagen zu **USB 2.0 / xHCI** (Ringregeln, Verhalten bei Unterlauf/Overrun,
    Portstatus-Events) stehen auf **Circle- und FreeMiNT-Code** bzw. auf Schluss;
    sie sind unten als [S] oder [?] markiert. Empfehlung: die xHCI-1.2-Spec und
    USB 2.0 nach `references/` legen (gitignored), bevor Hotplug gebaut wird.
* **QEMU-Deskriptoren** (`usb-mouse`, `usb-tablet`) stammen aus QEMU
  `hw/usb/dev-hid.c` (extern per Web geholt, tertiaere Quelle, nicht lokal).
* Alle Zeilennummern unserer Dateien gelten fuer den heutigen Stand.

## 1. Wie sich Maeuse praesentieren

### 1.1 Grundregeln aus HID 1.11

* `bInterfaceSubClass` = 1 → „Boot Interface Subclass“; **nur dann** hat
  `bInterfaceProtocol` (1 Tastatur, 2 Maus) eine Bedeutung, sonst ist es 0
  (HID 1.11 §4.2, §4.3). [G]
* `SET_PROTOCOL` (wValue 0 = Boot, 1 = Report) „is supported by devices in the
  boot subclass“; nach dem Einschalten steht jedes Geraet im **Report**-Protokoll,
  der Host soll es trotzdem immer setzen (§7.2.6). [G]
* Boot-Maus-Bericht: Byte 0 Tasten 1–3 (Bits 0–2), Byte 1 X, Byte 2 Y, „3 to n
  device specific (optional)“ (Anhang B.2). [G]
* Report-Protokoll: das Format steht **nur** im Report-Deskriptor
  (`GET_DESCRIPTOR` Typ 0x22 an das Interface). Kommt in dem Deskriptor irgendwo
  ein Report-ID-Item vor, tragen **alle** Berichte ein fuehrendes ID-Byte
  (§6.2.2.7 / §8.1). Felder sind bitgepackt, nicht byteausgerichtet (§8.1). [G]
* Standard-Idle fuer Maeuse ist „infinity“ (§7.2.4), `SET_IDLE` ist fuer Maeuse
  also nicht noetig. [G]

### 1.2 Katalog

Legende Spalte „NewTOS heute“: **geht** / **stumm** / **falsch** / **[S]** =
nur aus dem Code gelesen, nicht an Hardware beobachtet.

| # | Praesentation | Erkennung (Circle) | Erkennung (FreeMiNT) | NewTOS heute |
|---|---|---|---|---|
| A | **Boot-Maus** (3,1,2), ein Interface, 3–4 Byte | `int3-1-2` → `CUSBMouseDevice` (`usbdevicefactory.cpp:103-106`); parst trotzdem den Report-Deskriptor (`usbmouse.cpp:66-113`) | `mouse-boot`: Protokoll 2, genau 1 Endpoint, Interrupt (`mouse-boot/usb_mouse.c:386-405`); `mouse`: dieselbe Pruefung plus Deskriptor-Parser (`mouse/usb_mouse.c:650-681`) | **geht** — auf echter Pi 4 belegt (Apple `05ac:0304`, Roccat `1e7d:2cee`: `STATUS.md:331`, Log `docs/analysis/usb-port-investigation/run3-fixed-both-in-usb3.log:87-101`). Rad-Byte 3 nur bei `len > 3` (`hid.rs:163-181`). **Mittel-/Seitentasten werden nicht uebersetzt** (`hid.rs:112` nutzt nur Bit 0/1) |
| B | **Boot-Maus, im Report-Protokoll mit 12/16-Bit-Deltas, Report-ID, >8 Byte, Hi-Res** | wie A, aber Circle schaltet die Maus bewusst auf **Report**-Protokoll (`usbhiddevice.cpp:107-121`: Protokoll 2 → `REPORT_PROTOCOL`) und liest Deltas bis 32 Bit, klemmt auf ±127 (`usbmouse.cpp:128-139`) | `mouse`: `SET_PROTOCOL(hid_mode)` — Report, wenn der Parser Tasten+X+Y fand, sonst Boot (`mouse/usb_mouse.c:705-733`); Delta wird auf s8 geklemmt (`:361-370`, `:498-499`) | **geht** (in 8 Bit): wir schicken immer `SET_PROTOCOL(boot)` (`xhci/mod.rs:1450`, `:1353-1355`), ein Boot-Subclass-Geraet muss dann das 3-Byte-Format liefern (HID 1.11 §7.2.6, B.2) [S]. Verloren gehen Hi-Res, Rad-Aufloesung, AC Pan |
| C | **Report-only-Maus** (Subclass 0, Protokoll 0): Format nur im Deskriptor, ggf. mit Report-ID | Circle routet `int3-0-0`/`int3-0-2`/`int3-1-0` **nicht** zur Maus, sondern zu `GetGenericHIDDevice` → Touchscreen bei Usage Page Digitizer, sonst **Gamepad** (`usbdevicefactory.cpp:108-121`, `:237-316`) — d. h. eine solche Maus ist in Circle selbst **nicht** unterstuetzt | `mouse`/`mouse-boot` lehnen sie ab: `bInterfaceProtocol != 2 → -1` (`mouse/usb_mouse.c:655-658`, `mouse-boot/usb_mouse.c:386-389`). Nur `tablet` nimmt jedes HID-Interface (`tablet/usb_tablet.c:860`) | **stumm** [S]: `read_config` liefert Protokoll 0 (`xhci/mod.rs:1303-1304`), `poll` verwirft Protokoll ≠ 1/2 (`mod.rs:145-152`). Zusaetzlich geht `SET_PROTOCOL(boot)` an ein Nicht-Boot-Interface (`xhci/mod.rs:1450`) — nach HID 1.11 §7.2.6 nicht vorgesehen; stalled es, wird das Geraet ganz verworfen (`:1450-1453`, `:1573-1575` setzt `dci` nicht) |
| D | **Verbundgeraet / Funkempfaenger**: mehrere HID-Interfaces (z. B. Tastatur + Maus + Hersteller) | jedes Interface wird eine eigene Funktion, bis `USBDEV_MAX_FUNCTIONS` = 10 (`usbdevice.cpp:393-481`, `usbdevice.h:31`) | `usb_new_device` ruft fuer **jedes** Interface `usb_find_interface_driver` (`usb.c:1031-1034`); Treiber nimmt, was `probe` annimmt | **nur ein Interface** [S]: `read_config` nimmt den **ersten** Interrupt-IN-Endpoint der ganzen Konfiguration und paart ihn mit dem *zuletzt gesehenen* Interface-Deskriptor (`xhci/mod.rs:1293-1325`); DevDma hat nur einen Interrupt-Ring und einen 8-Byte-Puffer je Geraet (`:117-121`). Steht die Tastatur zuerst, bleibt die Maus unkonfiguriert und umgekehrt. Beleg fuer Mehr-Interface-Geraete: Roccat hat (3,1,2)+(3,0,1) (`run3…log:92-97`); Apple-Tastatur (3,1,1)+(3,0,0) (`:144-149`) |
| E | **Kein HID, aber Interrupt-IN-Endpoint mit Protokoll 1/2** (z. B. Kommunikations-/Funk-Schnittstellen) | Name `int<klasse>-…` filtert nach Klasse, kein Treffer | `probe` prueft Klasse HID (bzw. Subclass Boot) | **falsch** [S]: `read_config` prueft weder `bInterfaceClass` noch `bInterfaceSubClass` (Kommentar `xhci/mod.rs:1257-1264`, Code `:1301-1304`), nimmt `bInterfaceProtocol` jeder Klasse; Protokoll 1 wuerde als Tastatur dekodiert (`mod.rs:146`). Welche reale Klasse das trifft, ist **nicht** beobachtet |
| F | **Absoluter Zeiger** (Tablet, KVM, QEMU `usb-tablet`) | Digitizer → Touchscreen; Generic-Desktop-Absolutzeiger → Gamepad-Pfad [S] (`usbdevicefactory.cpp:269-316`); `usbmouse.cpp` unterscheidet nicht relativ/absolut (`:354-356` prueft nur Input-Bits 0/1) | `tablet`: Deskriptor-Parser fuer X/Y/Druck/Tip, **Umrechnung in Relativpakete** (`tablet/usb_tablet.c:615-690`, Anfahren der Ecke 0/0 bei „in range“ `:663`, `send_mouse_packets` `:572-592`); echte Absolutpakete 0xF7 sind dort als „no reaction“ auskommentiert (`:746-790`) | **stumm** [S]: QEMU `usb-tablet` = HID, Subclass 0, Protokoll 0 (QEMU-Quelle, extern) → wie C. Dazu verwirft `ikbd.rs` das Absolutpaket 0xF7 (`ikbd.rs:61`, `:194`); es gibt keinen Weg, absolute Koordinaten an die VDI zu geben |

### 1.3 Anforderungen der Geraete an den Host (Anfragen)

| Anfrage | Circle | FreeMiNT | NewTOS heute |
|---|---|---|---|
| `SET_CONFIGURATION` | vor den Funktionen, `usbdevice.cpp:381` | `usb_new_device`, `usb.c:1004` | in `configure_hid` **nach** Configure Endpoint (`xhci/mod.rs:1442-1449`, Reihenfolge begruendet `:1438-1441`) |
| `GET_DESCRIPTOR` HID-Report (0x22) | ja, Laenge aus HID-Deskriptor `wReportDescriptorLength` (`usbmouse.cpp:66-90`) | ja: Laenge per Konfigurations-Walk (`hid-common/hid.c:56-132`), Abruf `:134-155` | **nie** — es gibt keinen Aufruf; `get_descriptor` kennt nur Geraete-/Konfigurationsabruf (`xhci/mod.rs:1125-1142`) |
| `SET_PROTOCOL` | nur wenn Klasse 3, Subclass 1, Protokoll > 0: Maus → Report (1), Tastatur → Boot (0) (`usbhiddevice.cpp:107-121`) | `mouse`: Report/Boot je nach Parser (`mouse/usb_mouse.c:733`); `mouse-boot`: immer Boot (`mouse-boot/usb_mouse.c:435`; der Report-Zweig ist auskommentiert `:427-434`) | **immer** Boot, unabhaengig von Subclass (`xhci/mod.rs:1450`) |
| `SET_IDLE` | nie (kein Treffer in `lib/usb`) | `mouse` `:731`, `mouse-boot` `:425`, `tablet` `:936`: Dauer 0, Report-ID 0 | nie |
| Interface-Auswahl | alle Interfaces der Konfiguration, Alternate-Settings > 0 ignoriert (`usbdevice.cpp:393-406`) | alle Interfaces, jeder Treiber `probe` (`usb.c:1062-1091`) | erstes Interrupt-IN der Konfiguration (s. D) |
| Mehrere gleiche Geraete | mehrere Funktionen moeglich | **nur eine Maus**: `if (mse_data.pusb_dev) return -1` (`mouse/usb_mouse.c:630-633`, `mouse-boot:361-364`) | bis `MAX_HID` = 8 Slots, Hubs eingerechnet (`xhci/mod.rs:96`) |

### 1.4 Dekodierung des Berichts

| Punkt | Circle | FreeMiNT `mouse` | NewTOS heute |
|---|---|---|---|
| Berichtslaenge | aus Deskriptor berechnet; Bericht mit anderer Laenge wird verworfen (`usbmouse.cpp:117-119`, `:385`) | liest hoechstens 8 Byte (`mouse/usb_mouse.c:420-424`); `actlen < 3` → verworfen (`:437-440`) | TRB-Laenge fest 8 (`xhci/mod.rs:626`), Puffer `intbuf: [u8; 8]` (`:121`); Laenge aus Restgroesse des Events (`:719`). **Berichte >8 Byte sind strukturell nicht vorgesehen**; wie der Controller sie meldet, ist **[?]** (keine xHCI-Spec lokal); ein Fehlercode fuehrt nur zum Neuarmieren (`:722-734`) |
| Report-ID | Item `0x84`; erste ID wird gemerkt, 8 Bit Offset addiert (`usbmouse.cpp:293-313`) | Bericht beginnt mit ID, wird uebersprungen, falsche ID → Wert 0 (`hid-common/hid.c:191-199`) | keine Behandlung |
| Feldbreiten | Bit-Extraktion bis 32 Bit (`usbmouse.cpp:170-226`) | `fill_extract_bits`/`hid_get_data` (`hid.c:160-218`, `:220-232`) | feste 8 Bit (`hid.rs:114-115`) |
| Klemmen | ±127 (`usbmouse.cpp:130-139`) | `clip_to_s8` (−128..127) (`mouse/usb_mouse.c:361-370`) | entfaellt (8 Bit) |
| Tasten | Bits 1–5 → links/rechts/mitte/Seite1/Seite2 (`usbmouse.cpp:143-163`) | Taste 3 → Ziffernblock-Enter, 4/5 → Shift+Pfeil (`mouse/usb_mouse.c:501-526`) | nur Bit 0/1 (`hid.rs:112`) |
| Rad | Wheel-Usage 0x38 (`usbmouse.cpp:342-345`) | Wheel 0x38 und AC Pan (Consumer 0x238) (`hid-common/hid.h:42-44`; `mouse/usb_mouse.c:321-351`), als Pfeiltasten (`:529-576`) | Byte 3 bei `len > 3` als Scancode 0x59/0x5a (`hid.rs:163-181`); **kein** horizontales Rad (`0x5c/0x5d` unbenutzt) |
| Parser-Umfang | nur Items Usage Page/Usage/Collection/Report ID/Size/Count/Input; Usage Min/Max, Push/Pop, Relativ-/Absolut-Bit werden nicht ausgewertet (`usbmouse.cpp:228-386`) | `find_usage` ueber Pfadknoten, Usage Min/Max in Tabelle, `report_count`/`size` als `bit8u` (`hidparse.c:93-332`, `:280-288`, `:355-366`) | kein Parser |

**Folgerung fuer einen eigenen Parser [S]:** beide Referenzen sind bewusst
schmal (Maus: Tasten, X, Y, Rad, ggf. AC Pan). Keine von beiden behandelt
absolute Zeiger als Maus; FreeMiNT behilft sich mit Relativ-Umrechnung.

### 1.5 Weitere Luecken im Enumerationspfad, die Maeuse betreffen

Alle [S] aus dem Code, keine an Hardware beobachtet:

1. **Konfigurationspuffer 256 Byte.** `read_config` kappt `wTotalLength` auf 255
   (`xhci/mod.rs:1244-1247`; Puffer `buf: [u8; 256]` `:120`). Eine groessere
   Konfiguration (viele Interfaces, Funkempfaenger) wird mitten im Deskriptor
   abgeschnitten, ohne Meldung. Ein HID-Report-Deskriptor kann ebenfalls groesser
   sein; es gibt keinen Pfad dafuer.
2. **Alternate Settings** werden nicht beachtet: `bAlternateSetting` wird nicht
   gelesen, der Endpoint wird dem zuletzt gesehenen Interface-Deskriptor
   zugeordnet (`:1301-1304`, `:1306-1325`). Circle ignoriert Alt > 0
   (`usbdevice.cpp:393-406`).
3. **Interrupt-Intervall** fuer SuperSpeed wird mit der FS/LS-Formel kodiert
   (`:1408-1415`, nur `speed == 3` ist High-Speed-Sonderfall). [?] ob das fuer
   SS-HID-Geraete falsch ist — xHCI-Spec nicht lokal.
4. **Mittel-/Seitentasten** gehen verloren (siehe Katalog A), obwohl die
   Empfaengerseite fertig ist (`hid-mouse.md`, „Middle button“).
5. **Befehlsring ohne Link-TRB — harte Grenze von 15 Befehlen** (siehe §3.1;
   betrifft jede Steckkombination, nicht nur Maeuse).

## 2. Stand der vier Defekte vom 2026-09-09

Die Reparaturen liegen **gemeinsam in `4cbd7485` (2026-09-13)**, einem Commit mit
dem Titel „fix(aes): repair the resource load path, …“, der laut Body auch „the
USB/xHCI port investigation, the usb-diag boot-log capture and the QED trace
instrumentation“ mitnimmt (`git show --stat 4cbd7485`: `kernel/src/bootlog.rs`,
`drivers/usb/mod.rs`, `drivers/usb/xhci/mod.rs` +625 Zeilen, `hal/uart.rs`).
Es gibt kein eigenes Ticket und keinen eigenen Eintrag in `STATUS.md`; die Phase-16-Zeile
(`STATUS.md:331`) und der Eintrag vom 2026-09-09 (`:738`) stehen unveraendert auf „offen“.

| # | Defekt (2026-09-09) | Stand heute | Beleg |
|---|---|---|---|
| 1 | `wait_transfer` nimmt das erste Transfer-Event ohne Slot/Endpoint-Pruefung; Interrupt-IN eines schon konfigurierten Geraets wird sofort scharf | **behoben** [G] | `wait_transfer` quittiert nur Events mit passendem Slot **und** Endpoint, fremde werden geparkt statt verworfen (`xhci/mod.rs:1022-1066`, Parken `:753-768`, Abholen `:772-781`); Endpoints werden erst nach der Enumeration des ganzen Baums scharf gemacht (`arm_hid_endpoints` `:1506-1512`, aufgerufen `:1489`) |
| 2 | `poll_interrupt` ordnet nur ueber die Slot-ID zu, ohne Completion-Code und Endpoint | **behoben** [G] | Besitzer = Slot **und** `dci` (`:694-697`), `dci == 0` (Hub, fehlgeschlagene Konfiguration) besitzt nie ein Event (`:672-679`); Completion-Code wird geprueft, `Success`/`ShortPacket` liefern den Bericht (`:707-721`), jeder andere Code wird protokolliert und das Endpoint **neu armiert** (`:722-734`); `queue_interrupt` ignoriert `dci == 0` (`:609-621`) |
| 3 | Alles ausser Protokoll 1 wurde Maus | **teilweise behoben** | `poll` dekodiert nur Protokoll 1 (Tastatur) und 2 (Maus), alles andere wird ignoriert (`mod.rs:145-152`); Hubs tragen Protokoll 9 (`xhci/mod.rs:1567-1568`). **Offen**: das Protokoll stammt weiter aus `bInterfaceProtocol` *jeder* Klasse und jeder Subclass (`xhci/mod.rs:1301-1304`), siehe Katalog E und HID 1.11 §4.3. Zudem wird ein Hub nur erkannt, wenn `bDeviceClass == 9` (`:1567`); ein Hub als Interface eines Verbundgeraets (Geraeteklasse 0) fiele in `configure_hid` [S] |
| 4 | Hub-Port-Reset Einzelschuss 60 ms statt FreeMiNT 5 × 150 ms | **behoben** [G] | `hub_port_reset`: bis zu 5 Versuche, je `SET_FEATURE(PORT_RESET)` + 150 ms + neu lesen, Abbruch bei Verbindungswechsel, `C_PORT_RESET` nur nach Erfolg (`xhci/mod.rs:1831-1904`), 1:1 zu FreeMiNT `hub.c:225-272`. **Nicht** betroffen: der Reset eines **Root**-Ports ist weiter ein Einzelschuss mit Spin-Schleife ohne Wiederholung (`port_reset` `:896-909`) — dafuer gibt es in FreeMiNT keine Entsprechung im Hub-Code, Circle wartet dort 100 ms (`xhcirootport.cpp:75`) |

**Was die Messung belegt und was nicht.** `docs/analysis/usb-port-investigation/`
enthaelt drei Boot-Logs derselben Kette (Root-Port 1 → VIA-Hub `2109:3431` →
Roccat-Maus / Apple-Hub `05ac:1003` → Apple-Tastatur `05ac:020c`). In `run3`
(Build nach der Reparatur) enumerieren **alle** Geraete (`run3…log:61-157`), jeder
Hub-Port war nach dem ersten 150-ms-Reset aktiv (`:80-82`, `:107-109`, `:132-134`).
Die Logs enden mit der Enumeration; **ob danach Maus und Tastatur Eingaben liefern, zeigt
keiner der Laeufe.** Ob der Steckkombinations-Fehler mit `4cbd7485` verschwunden ist,
ist daher **[?]** — dafuer braucht es einen Hardwarelauf in beiden Kombinationen
(der Anwender hat Hardware, ich nicht). Dass (1) die Ursache war, ist ein
konsistenter Schluss [S]: in `run2` (vor der Reparatur) wird die Maus *vor* dem
Tastatur-Hub konfiguriert (`run2…log:100`, Tastatur erst `:151`), und nach
`defer_transfer_event`s Kommentar verlor genau diese Reihenfolge das Event der
Maus (`xhci/mod.rs:1014-1021` verweist auf `run2`).

## 3. Weitere Abhaengigkeiten vom Steckplatz / von der Kombination

### 3.1 Befehlsring: nach 15 Befehlen ist Schluss [S, hoch]

`command()` schreibt in einen 16-Eintraege-Ring und springt nach Index 14 auf 0
zurueck, **ohne** Link-TRB und **ohne** das Cycle-Bit umzuschalten
(`xhci/mod.rs:54`, `:810-814`; der Kommentar gibt es zu: „the ring does not wrap
— a Link TRB / wrap is a later refinement“ `:799-801`). Eintrag 15 bleibt null.
Ein xHCI-Controller wandert am Ende des Segments zu Eintrag 15, sieht dort ein
ungueltiges (Cycle 0) TRB und wartet; der 16. Befehl liegt auf Index 0 und wird
nie geholt → `cmd timeout` nach 200 Mio. Spin-Schleifen (`:820-851`). Circle legt
dagegen einen Link-TRB mit Toggle-Cycle an (`xhciring.cpp:50-55`, `:142-148`).
Die Ringregel selbst ist nicht aus lokaler xHCI-Spec belegt [?], folgt aber
zwingend aus dem Code, der sie umgeht.

Befehlsverbrauch des `run3`-Baums (aus den Logs gezaehlt, [S]): VIA-Hub 3
(Enable Slot, Address Device, Evaluate Context fuer Hub), Roccat-Maus 4
(+ Evaluate fuer EP0 64, `:1162`), Apple-Hub 3, Apple-Tastatur 3 → **13 von 15**.
Ein weiteres Geraet mit 64-Byte-EP0 (4 Befehle) oder zwei weitere
(je 3) laeuft ueber. **Das macht das Ergebnis von der Anzahl und Art der
angeschlossenen Geraete abhaengig**, nicht von der Portnummer. Hotplug
(Re-Enumeration nach dem Boot, braucht 3–4 Befehle) hat nur 2 Befehle Luft.

### 3.2 Slot-Budget `MAX_HID` = 8

Jedes Geraet — Hubs und Nicht-HID eingeschlossen — belegt einen `HidDev`-Eintrag
(`xhci/mod.rs:96`, `:1538-1546`); `n_hid` wird nie zurueckgesetzt. Ist das Budget
erschoepft, brechen die Schleifen **ohne Meldung** ab (`:1469-1471`, `:1529-1531`,
`:1610-1612`). Geraete an hoeheren Hub-Ports bzw. spaeter eingesteckte fallen so
stumm heraus. Der Pi-Baum aus `run3` braucht 4.

### 3.3 Hub-Beschreibung und Geschwindigkeit

* `hub_get_descriptor` fragt immer Typ `0x29` (`:1770`); akzeptiert aber auch
  `0x2A` in der Antwort (`:1777`). Ein SuperSpeed-Hub (Root-Ports 2–5 an der Pi)
  erwartet [?] `0x2A`; **weder Circle** (`DESCRIPTOR_HUB = 0x29`, `usbhub.h:38`)
  **noch FreeMiNT** (kein xHCI) sind dafuer Referenz.
* Die Geschwindigkeit eines Hub-Kindes kommt aus den Portstatus-Bits 9/10
  (`:1639-1645`), SuperSpeed hinter einem Hub ist nicht abgedeckt.
* Root-Ports werden nur beim Boot und nur bei `current_connect_status`
  beruecksichtigt (`:1477-1479`).

## 4. Hotplug

### 4.1 Heute

* **Enumeration passiert genau einmal**, in `usb::init` (`mod.rs:46-71`,
  aufgerufen `lib.rs:430`). Es gibt keinen weiteren Aufrufer von
  `enumerate_all` (Suche im Baum).
* **Ohne Geraet beim Boot kein Controller-Objekt:** `XHCI` wird nur gesetzt,
  wenn `enumerate_all() > 0` (`mod.rs:64-67`); sonst kehrt `poll` sofort zurueck
  (`mod.rs:135-137`). Ein erst spaeter eingestecktes Geraet wird nie gesehen.
* **Port-Status-Change-Events werden verworfen:** `poll_interrupt` (`:688-690`),
  `command` (`:826-830`) und `wait_transfer` (`:1037`) konsumieren sie ohne
  Auswertung. `PORTSC` wird nach dem Boot nie gelesen oder quittiert;
  `scan_ports` ist reine Ausgabe (`:1930-1956`). Ob stehengebliebene Change-Bits
  weitere Events unterdruecken, ist **[?]** (xHCI-Spec nicht lokal).
* **Hubs bekommen kein Status-Change-Interrupt-Endpoint**: `enumerate_hub`
  macht nur `SET_CONFIGURATION` (`:1599`), `dci` bleibt 0 (`:1567-1572`), also
  wird auch nichts armiert (`:1506-1511`).
* **Kein Abbau:** es gibt weder Disable Slot noch Reset/Stop Endpoint noch eine
  Disconnect-Behandlung (Suche in `xhci/mod.rs`/`mod.rs`: kein Treffer);
  `UsbError::NoDevice` existiert nur als Typ (`mod.rs:170-171`). Zieht man ein
  enumeriertes Geraet ab, bleibt sein `HidDev`/Slot stehen. Was der Controller
  dann am Endpoint meldet, ist **[?]**; der Code reagiert auf jeden
  Nicht-Erfolgscode mit Protokollzeile + Neuarmieren (`:722-734`) — das kann
  pro Poll eine Zeile Log erzeugen [S].
* **Folge: Umstecken = Neustart.** Wieder eingesteckt wird ein Geraet nur, wenn
  es zum naechsten Boot angeschlossen ist.

### 4.2 Referenzen

| | Circle | FreeMiNT |
|---|---|---|
| Root-Port | Port-Status-Change-Event → `PortStatusChanged` → `HandlePortStatusChange`: verbunden und kein Geraet → `ReScanDevices`; getrennt → `RemoveDevice` (`xhcirootport.cpp:202-243`) | Root-Hub ereignisgetrieben: `usb_hub_thread` ruft `usb_hub_events` bei Wakeup (`hub.c:767-778`) |
| Hub | Status-Change-Interrupt-Endpoint des Hubs, nur bei Plug-and-Play (`usbstandardhub.cpp:188-189`, `:480-514`); `HandlePortStatusChange` liest den Portstatus, quittiert `C_PORT_CONNECTION` und ruft `ReScanDevices`/`RemoveDeviceAt` (`:528-628`) | **Polling** jede Sekunde (`usb_hub_poll_thread`, `addtimeout(1000L…)`, `hub.c:606-621`); `usb_hub_events` liest jeden Port, bei `C_CONNECTION` → `usb_hub_port_connect_change`: Kind trennen (`usb_disconnect` ruft `disconnect` der Treiber), neu zuruecksetzen, `usb_new_device` (`hub.c:275-330`, `:472-560`, `usb.c:768-803`) |
| Voraussetzung | `bPlugAndPlay` im Konstruktor + Aufruf von `UpdatePlugAndPlay` in der Hauptschleife, nicht aus IRQ (`doc/usb-plug-and-play.txt`, `usbhostcontroller.cpp:148-188`) | TOS-only-Variante: **kein** Hotplug, nur ein Durchlauf beim Start (`usb_hub_init`, `hub.c:784-832`) |
| Treiber-Disconnect | Geraeteobjekte werden zerstoert | `mouse_disconnect` setzt `pusb_dev = NULL` (`mouse/usb_mouse.c:183-195`) |

**Einschraenkung fuer NewTOS [S, belegt durch den Code]:** `usb::poll()` laeuft
auch aus dem 200-Hz-Timer-Interrupt (`host_env.rs:476-485`, `:505-`, INPUT01).
Eine Hotplug-Enumeration mit `delay_ms` (50 ms nach Address Device `:984`, 150 ms
je Reset `:1843`, 100 ms Hub-Power `:1754`) darf dort **nicht** laufen; sie
gehoert in den trapseitigen Pump bzw. in einen eigenen Aufruf ausserhalb des
IRQ-Pfads.

## 5. Diagnosewerkzeug (Ticket Schritt 2) — Vorschlag

**Ziel des Anwenders:** verschiedene Maeuse einstecken, je Geraet eine Datei auf
`C:` mit Geraete-/Konfigurations-/Interface-/Endpoint-/HID-Report-Deskriptor (roh,
hex), Hub-/Port-Weg, Geschwindigkeit, gewaehltem Protokoll und einigen rohen
Berichten bei Bewegung/Klick.

### 5.1 Was beide Wege brauchen (gemeinsamer Kern, heute nicht vorhanden)

1. **Deskriptoren aufbewahren.** Heute werden sie nur gedruckt (`usb-diag`,
   `xhci/mod.rs:1265-1292`) und der Puffer wird bei jedem Transfer ueberschrieben
   (`:1128`); behalten werden nur Klasse, VID, PID (`:1177-1179`).
2. **Report-Deskriptor holen** (Klassenanfrage `GET_DESCRIPTOR` 0x22 an das
   Interface, Laenge aus dem HID-Deskriptor): existiert nicht (§1.3). Dafuer
   groesserer Puffer als 256 Byte.
3. **Rohe Berichte aufzeichnen:** `poll_interrupt` liefert sie (`:680-721`), der
   Aufrufer wirft sie nach der Dekodierung weg (`mod.rs:138-155`). Noetig: ein
   Ring je Geraet (Zeitpunkt, Laenge, Bytes).
4. **Pfad/Geschwindigkeit/Protokoll** liegen in `HidDev` bereits vor (`port`,
   `route`, `speed`, `protocol`, `dci`: `:165-189`).
5. **Holen waehrend der Enumeration** (kein neuer Transfer spaeter): dann laeuft
   alles im ruhigen Boot-Pfad und es entsteht keine Wiedereintritts-Frage mit
   dem Timer-IRQ-Pump (`host_env.rs:476-485`).

### 5.2 (a) EL0-Programm ueber eine neue Kernel-Schnittstelle

* **Existiert in FreeMiNT eine USB-Auskunft zum Uebernehmen?** Nein als
  Systemaufruf: `Ssystem` kennt keinen USB-Modus (`sys/ssystem.c`,
  `sys/mint/ssystem.h`: kein Treffer fuer „usb“). FreeMiNTs Weg ist der
  `_USB`-Cookie, der auf `struct usb_module_api` zeigt — eine Tabelle von
  **Funktionszeigern im selben Adressraum** (`usb_api.h:127-186`, gesetzt
  `init.c:182-199`); das Tool `usbtool` liest damit `usb_device`-Strukturen und
  pollt `usb_hub_events` jede Sekunde (`tools/usbtool/usbtool.c:150-170`,
  `:249-262`). Das laesst sich auf einem EL0-Programm ohne gemeinsamen
  Adressraum nicht uebernehmen; uebernehmbar ist das **Datenmodell**
  (Geraeteliste, Deskriptoren, Hub-Kind-Baum).
* **Passender Weg im Projekt:** NewTOS-eigene `Ssystem`-Modi ab 2000, bereits
  etabliert (SYSINFO01: 2003–2020 Skalare, **2019 als String in den Puffer des
  Aufrufers**, `tos-api/src/gemdos/ssystem.rs:1-30`, `:182-209`; Spec
  `docs/api/gemdos/154-ssystem.md`; Nummernraum 2000–2999 frei, `:17-23`;
  Kernel-Anschluss ueber das `Host`-Trait `tos-api/src/host.rs:330-337`,
  `kernel/src/host_env.rs:406-413`). Ein neuer Modus, z. B. „USB-Schnappschuss
  in Puffer (Geraeteindex, Puffer, Laenge)“, haengt sich genau so ein.
* **Programm:** Konsolenprogramm in `sdk/apps/`, ruft den Modus je Geraet,
  schreibt per `Fopen/Fwrite` `C:\USB\…`, bittet zum Bewegen/Klicken, liest die
  Berichtsringe erneut.
* **Kosten (Schaetzung, nicht gemessen):** gemeinsamer Kern (§5.1) + Host-Trait-
  Methode + Ssystem-Modus + Spec-Zeile + EL0-Programm + Test: **M–L**
  (ca. 2–4 Tage).
* **Was es freischaltet:** Programm laeuft *im laufenden System* beliebig oft;
  Dateien je Geraet (VID/PID im Namen) ohne Neustart — sobald Hotplug existiert,
  auch Mausumstecken ohne Reboot; Berichte *nach* dem Boot (Anwender bewegt die
  Maus, wenn das Programm sagt „jetzt“); spaeter nutzbar als Systeminfo-Werkzeug
  (SYSINFO01-Linie). **Wegwerf:** nur das Programm-Textformat; der Kern bleibt.
* **Fit:** passt zum Projekt (Ssystem-Erweiterung, FreeMiNT-Richtung, Regel 5:
  Spec vor Code); belastet die Hardware-Pi nicht mit Sonderbuild.

### 5.3 (b) `usb-diag`/`bootlog` ausbauen

* **Heute:** Feature `usb-diag` schneidet `kprint!` in 192 KiB `.bss` mit und
  schreibt es per `svc #1` (`Fcreate/Fwrite/Fclose`) nach `C:\USBDIAG.LOG`
  (`bootlog.rs:27-114`, Tee in `hal/uart.rs`), nur **waehrend des Boots**
  (zwei Aufrufe: `lib.rs:438` und nach `initinfo`). Es enthaelt schon alle
  Interface-/Endpoint-Zeilen (`xhci/mod.rs:1265-1292`), aber **weder** rohe
  Deskriptoren (nur Auszuege), **noch** den Report-Deskriptor, **noch** Berichte.
* **Ausbau:** Kern (§5.1) + Hex-Formatierung in der Kernel-Ausgabe + ein
  „Mess-Fenster“ im Boot (Kernel pollt N Sekunden und zeichnet Berichte auf, dann
  `flush`). Der `flush` darf **nur aus normalem Kernelkontext** laufen, nicht aus
  einem Exception-Handler (`bootlog.rs:75-77`) — das schliesst Flush aus dem
  Timer-IRQ-Pump aus; „Berichte nach dem Boot“ geht also nur ueber das
  Boot-Fenster.
* **Kosten (Schaetzung):** Kern + Formatierung + Fenster: **S–M** (ca. 1–2 Tage).
* **Was es freischaltet:** schnellster Weg zu *Daten* (Descriptors + Report-
  Deskriptor je Mausmodell). Datei heisst fest `USBDIAG.LOG`, ein Boot = ein Lauf
  = ein (ueberschriebenes) Log; **je Maus ein Neustart und Umbenennen**; kein
  Hotplug, keine Berichte nach dem Boot-Fenster. Sonderbuild noetig
  (`cargo xtask build --board rpi4 --release --features usb-diag`,
  `kernel/Cargo.toml:96-97`) und Aufspielen auf die Karte.
* **Wegwerf:** das Boot-Fenster und die Formatierung (fuer ein Dauerwerkzeug
  unbrauchbar); der Kern bleibt.

### 5.4 Empfehlung

**(a).** Der aufwendige Teil (§5.1) ist fuer beide gleich; (a) kostet dazu nur
die Schnittstelle und ein kleines Programm, bleibt als Werkzeug stehen und
erfuellt den Anwenderwunsch wortlich (Programm, Datei je Geraet, Berichte bei
Bewegung/Klick). (b) ist nur dann zu bevorzugen, wenn die **allerersten Daten
sofort** gebraucht werden und ein Reboot je Maus akzeptiert wird; dann sollte
man den Kern so bauen, dass (a) ihn unveraendert weiterverwendet.
**Abhaengigkeit:** „Maus umstecken ohne Reboot“ gelingt in *keinem* der beiden
Wege vor §4 / Fix F6; bis dahin heisst der Ablauf: Maus einstecken → neu booten →
Programm laufen lassen.
Testbar ohne Pi: QEMU `qemu-xhci` mit `usb-mouse` laeuft schon
(`tools/xtask/src/main.rs:664-667`); `usb-tablet` und `device_add`/`device_del`
per QMP erlauben Absolutzeiger und Hotplug ohne Hardware [S, nicht probiert].

## 6. Rangliste der Reparaturkandidaten (nichts davon gebaut)

Aufwand ist **meine Schaetzung** (S ≤ 0,5 Tag, M 1–2, L 3+), nicht gemessen.

| Rang | Kandidat | Warum | Aufwand | Abhaengig von |
|---|---|---|---|---|
| 1 | **F0 Diagnose-Kern** (§5.1): Deskriptoren behalten, Report-Deskriptor holen, Berichtsring | ohne Daten ist die Wahl des Parsers geraten; die Anwenderfrage „welche Maus spricht was“ ist nur so zu beantworten | M | — |
| 2 | **F1 Befehlsring mit Link-TRB** (§3.1) | stille, kombinationsabhaengige Grenze (13 von 15 im Pi-Baum), Voraussetzung fuer Hotplug und fuer mehr Geraete | S | — |
| 3 | **F2 Klassenpruefung** `bInterfaceClass == 3` (und Hub per Interface-Klasse) vor Protokoll 1/2 (Katalog E, Defekt 3 Rest) | verhindert Fehlzuordnung fremder Geraete | S | — |
| 4 | **F3 Mehrere Interfaces je Geraet** (Interface-Schleife, je HID-Interface eigener Endpoint/Ring/Puffer, Alt-Settings ignorieren) | Funkempfaenger/Verbundgeraete: sonst haengt es an der Reihenfolge im Deskriptor | M–L | F0 hilfreich, F1 |
| 5 | **F4 Report-Protokoll-Parser fuer Maeuse** (Tasten, X, Y, Rad, AC Pan, Report-ID, Bitbreiten; kein `SET_PROTOCOL(boot)` an Nicht-Boot-Interfaces; Zerlegung grosser Deltas in mehrere ±127-Pakete wie FreeMiNT `send_mouse_packets`; Berichte >8 Byte, Puffer >256) | schliesst Katalog C und die Hi-Res-Verluste aus B | L | F0 (Daten), F3 |
| 6 | **F5 Tasten 3–5 und horizontales Rad** (Bit 2..4 → IKBD-Zusatzscancodes `0x37/0x5e/0x5f`, AC Pan → `0x5c/0x5d`) | Empfaengerseite ist fertig (`hid-mouse.md`); fuer A schon mit Boot-Bericht fuer Taste 3 sofort machbar | S (Taste 3), mit F4 den Rest | — / F4 |
| 7 | **F6 Hotplug** (Root-Port-PSC-Events, Hub-Status-Change-Endpoint oder FreeMiNT-artiges Polling alle 1 s, Abbau: Disable Slot / Slot-Freigabe, Neuenumeration nur ausserhalb des Timer-IRQ) | „Umstecken ohne Reboot“; heute gar nicht vorhanden | L | F1; xHCI-Spec lokal ablegen |
| 8 | **F7 Absolute Zeiger** (QEMU-Tablet, KVM): Entscheidung Relativ-Umrechnung wie FreeMiNT `tablet` oder echter Absolutweg zur VDI | `ikbd.rs` verwirft 0xF7 (`:61`); braucht eine Produktentscheidung | M | F4, Anwenderentscheid |
| 9 | **F8 Slot-Budget/Stille-Abbrueche** melden, Budget pruefen (`MAX_HID`) | Diagnose „Geraet fehlt“ ohne Logzeile | S | — |
| 10 | **F9 Root-Port-Reset mit Wiederholung, SuperSpeed-Hub/Intervall** | nur falls Messung es zeigt; keine Referenz lokal | M | Hardware-Daten |

Die Defekte 1, 2, 4 brauchen **keine** weitere Reparatur (§2); offen ist ihre
**Hardware-Abnahme** in beiden Steckkombinationen.

## 7. Offene Fragen an den Anwender

1. **Diagnoseweg:** (a) Ssystem-Modus + EL0-Programm (empfohlen) oder (b)
   `usb-diag` ausbauen? Reicht „je Maus ein Neustart“ fuer den ersten Datensatz?
2. **Hardware-Abnahme der Reparatur `4cbd7485`:** Maus und Tastatur in beiden
   Kombinationen (Maus USB2/Tastatur USB3 und umgekehrt) — liefert *Eingabe*,
   nicht nur Enumeration? Mit `usb-diag`-Build waere das Log ein Beleg.
3. **Hotplug:** muss Umstecken ohne Reboot jetzt gehen (F6, L), oder genuegt
   Neustart je Maus?
4. **Absolute Zeiger:** Relativ-Umrechnung (FreeMiNT-Weg) oder echter Absolutweg?
   Gibt es einen konkreten Anwendungsfall (KVM, Tablet)?
5. **Spezifikationen:** darf ich USB 2.0, HID 1.11 und xHCI 1.2 nach `references/`
   legen (gitignored)? Aussagen zu Ringregeln und Hotplug stuetzen sich sonst
   nur auf Code.
6. **Welche Maeuse haben tatsaechlich versagt?** Ein konkretes Modell mit
   Deskriptor waere der staerkste Beleg fuer Katalog C/D.

## 8. Ausserhalb des Scopes beobachtet

* `docs/STATUS.md:331` und `:738` fuehren die Defekte 1, 2, 4 weiter als offen,
  obwohl der Code (`4cbd7485`) sie behebt; es fehlt ein Statuseintrag.
* `4cbd7485` bundelt die USB-Reparaturen unter einem AES-Titel; die Historie ist
  mit `git log` auf USB allein schwer auffindbar.
* `docs/api/usb/_state.md` fuehrt den HID-Parser als „deferred“; dieses Dokument
  macht ihn zur Voraussetzung (F4).
* `hid-mouse.md` nennt „request the boot protocol so no HID report-descriptor
  parsing is needed“ — das gilt nur fuer Boot-Subclass-Geraete (HID 1.11 §7.2.6).
* Blockierende Timeouts bei stummen Geraeten: 200 Mio. bzw. 50 Mio. Spin-
  Iterationen je Befehl/Transfer (`xhci/mod.rs:820`, `:1036`) — siehe
  `TASK/PORT06-KEYS-DIAGNOSIS.MD:165`.
