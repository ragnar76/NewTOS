//! Configuration-descriptor scan: pick the HID **boot** interface the driver
//! can decode, and name every interface it does not use.
//!
//! Pure byte parsing with no register access and no kernel dependency, so the
//! host test suite (`tools/xtask`, which includes this file by path) runs its
//! tests; `xhci::read_config` fetches the bytes and logs the result.
//!
//! Sources:
//! * USB HID 1.11 (`references/usb/hid1_11.pdf`) §4.1: "The bInterfaceClass
//!   member of an Interface descriptor is always 3 for HID class devices";
//!   §4.2: `bInterfaceSubClass` 1 = Boot Interface Subclass, otherwise 0;
//!   §4.3: `bInterfaceProtocol` 1 = Keyboard, 2 = Mouse, and it "only has
//!   meaning if the bInterfaceSubClass member declares that the device
//!   supports a boot interface".
//! * Descriptor layouts (interface = type 4, 9 bytes; endpoint = type 5,
//!   7 bytes plus the audio-only `bRefresh`/`bSynchAddress`): FreeMiNT
//!   `sys/usb/src.km/usb.h:103-127` (`struct usb_endpoint_descriptor`,
//!   `struct usb_interface_descriptor`), the same offsets the previous
//!   `read_config` walk used. No USB 2.0 core specification is in
//!   `references/`.
//!
//! Why the boot subclass, not just class 3: everything downstream relies on the
//! **boot** report layout — `configure_hid` sends `SET_PROTOCOL(boot)` (HID
//! 1.11 §7.2.6: supported by devices in the boot subclass) and `hid.rs`
//! decodes the boot keyboard/mouse report (HID 1.11 Appendix B). A
//! report-protocol-only interface (subclass 0) has neither, so until a report
//! descriptor parser exists (`docs/api/usb/hid-mouse-protocols.md` F4) it is
//! not an input device here. A non-HID interface (mass storage, hub, audio,
//! vendor …) never is, whatever its `bInterfaceProtocol` says.

/// HID class code (HID 1.11 §4.1).
pub const CLASS_HID: u8 = 3;
/// Boot Interface Subclass (HID 1.11 §4.2).
pub const SUBCLASS_BOOT: u8 = 1;
/// Boot protocol: keyboard (HID 1.11 §4.3).
pub const PROTOCOL_KEYBOARD: u8 = 1;
/// Boot protocol: mouse (HID 1.11 §4.3).
pub const PROTOCOL_MOUSE: u8 = 2;

const DESC_INTERFACE: u8 = 4;
const DESC_ENDPOINT: u8 = 5;
/// `bLength` of an interface descriptor (FreeMiNT `usb.h:116-127`).
const INTERFACE_LEN: usize = 9;
/// Bytes of an endpoint descriptor up to `bInterval` (FreeMiNT `usb.h:103-113`).
const ENDPOINT_LEN: usize = 7;

/// A HID boot device's configuration, as parsed from its config descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HidInfo {
    /// `bConfigurationValue` for SET_CONFIGURATION.
    pub config_value: u8,
    /// The HID interface number.
    pub iface: u8,
    /// `bInterfaceProtocol`: 1 = keyboard, 2 = mouse.
    pub protocol: u8,
    /// Interrupt-IN endpoint address (bit 7 = IN, low 4 = number).
    pub ep_addr: u8,
    /// Endpoint max packet size.
    pub ep_max: u16,
    /// Endpoint polling interval (`bInterval`).
    pub interval: u8,
}

/// Why an interface descriptor was not taken as the device's input interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// `bInterfaceClass` != 3: not a HID interface at all.
    NotHid,
    /// HID, but not the boot subclass: the report format is only in the
    /// report descriptor, for which there is no parser yet.
    NotBootSubclass,
    /// HID boot subclass with a protocol other than keyboard/mouse
    /// (HID 1.11 §4.3: 0 none, 3–255 reserved).
    UnknownBootProtocol,
    /// A usable HID boot interface without an interrupt-IN endpoint.
    NoInterruptIn,
    /// A usable HID boot interface after the one already chosen: the driver
    /// configures one interface per device (`hid-mouse-protocols.md` F3).
    OneInterfacePerDevice,
}

/// An interface descriptor the scan did not use, with the fields a log line
/// needs to say what it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkippedInterface {
    /// `bInterfaceNumber`.
    pub number: u8,
    /// `bAlternateSetting`.
    pub alt: u8,
    /// `bInterfaceClass`.
    pub class: u8,
    /// `bInterfaceSubClass`.
    pub subclass: u8,
    /// `bInterfaceProtocol`.
    pub protocol: u8,
    /// Why it was not used.
    pub reason: SkipReason,
}

/// Outcome of [`scan`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scan {
    /// The first HID boot interface with an interrupt-IN endpoint.
    pub hid: Option<HidInfo>,
    /// Byte offset of a descriptor whose `bLength` is 0 or runs past the end
    /// of `cfg` (a truncated or malformed configuration); the scan stopped
    /// there. `None` = every descriptor was walked.
    pub stopped_at: Option<usize>,
}

/// Whether an interface's class triple is a boot keyboard or boot mouse the
/// driver decodes.
pub fn is_boot_hid(class: u8, subclass: u8, protocol: u8) -> bool {
    class == CLASS_HID
        && subclass == SUBCLASS_BOOT
        && (protocol == PROTOCOL_KEYBOARD || protocol == PROTOCOL_MOUSE)
}

fn classify(class: u8, subclass: u8, protocol: u8) -> Option<SkipReason> {
    if class != CLASS_HID {
        Some(SkipReason::NotHid)
    } else if subclass != SUBCLASS_BOOT {
        Some(SkipReason::NotBootSubclass)
    } else if !is_boot_hid(class, subclass, protocol) {
        Some(SkipReason::UnknownBootProtocol)
    } else {
        None
    }
}

/// Walk a configuration descriptor (`cfg` = the bytes read, starting with the
/// 9-byte configuration header) and choose the first HID boot interface that
/// has an interrupt-IN endpoint. `skipped` is called once for every other
/// interface descriptor, in descriptor order.
pub fn scan(cfg: &[u8], mut skipped: impl FnMut(SkippedInterface)) -> Scan {
    let config_value = cfg.get(5).copied().unwrap_or(0);
    let mut hid: Option<HidInfo> = None;
    // The interface descriptor whose endpoints follow, and whether it is the
    // candidate still looking for its interrupt-IN endpoint.
    let mut cur: Option<SkippedInterface> = None;
    let mut cur_open = false;
    let mut stopped_at = None;

    // An interface that qualified but whose endpoints held no interrupt IN.
    fn close(cur: Option<SkippedInterface>, open: bool, out: &mut impl FnMut(SkippedInterface)) {
        if let (Some(c), true) = (cur, open) {
            out(SkippedInterface { reason: SkipReason::NoInterruptIn, ..c });
        }
    }

    let mut i = 0usize;
    while i + 2 <= cfg.len() {
        let dlen = cfg[i] as usize;
        let dtype = cfg[i + 1];
        if dlen == 0 || i + dlen > cfg.len() {
            stopped_at = Some(i);
            break;
        }
        match dtype {
            DESC_INTERFACE if dlen >= INTERFACE_LEN => {
                close(cur, cur_open, &mut skipped);
                let d = SkippedInterface {
                    number: cfg[i + 2],
                    alt: cfg[i + 3],
                    class: cfg[i + 5],
                    subclass: cfg[i + 6],
                    protocol: cfg[i + 7],
                    reason: SkipReason::NotHid,
                };
                cur = Some(d);
                cur_open = false;
                match classify(d.class, d.subclass, d.protocol) {
                    Some(reason) => skipped(SkippedInterface { reason, ..d }),
                    None if hid.is_some() => skipped(SkippedInterface {
                        reason: SkipReason::OneInterfacePerDevice,
                        ..d
                    }),
                    None => cur_open = true,
                }
            }
            DESC_ENDPOINT if dlen >= ENDPOINT_LEN && cur_open => {
                let addr = cfg[i + 2];
                let attr = cfg[i + 3];
                // Interrupt (bmAttributes 1:0 = 3) and IN (address bit 7).
                if attr & 0x3 == 3 && addr & 0x80 != 0 {
                    let c = cur.expect("cur_open implies an interface");
                    hid = Some(HidInfo {
                        config_value,
                        iface: c.number,
                        protocol: c.protocol,
                        ep_addr: addr,
                        ep_max: u16::from_le_bytes([cfg[i + 4], cfg[i + 5]]),
                        interval: cfg[i + 6],
                    });
                    cur_open = false;
                }
            }
            _ => {}
        }
        i += dlen;
    }
    close(cur, cur_open, &mut skipped);
    Scan { hid, stopped_at }
}

/// Short human-readable text for a [`SkipReason`] (the log line).
pub fn reason_text(r: SkipReason) -> &'static str {
    match r {
        SkipReason::NotHid => "not a HID interface (class != 3)",
        SkipReason::NotBootSubclass => {
            "HID without boot subclass: report protocol only, no report parser yet"
        }
        SkipReason::UnknownBootProtocol => "HID boot subclass, protocol neither keyboard nor mouse",
        SkipReason::NoInterruptIn => "HID boot interface without an interrupt-IN endpoint",
        SkipReason::OneInterfacePerDevice => "second HID boot interface: one interface per device",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(body: &[&[u8]]) -> Vec<u8> {
        let mut v = vec![9, 2, 0, 0, 1, 7, 0, 0x80, 50];
        for d in body {
            v.extend_from_slice(d);
        }
        let total = v.len() as u16;
        v[2..4].copy_from_slice(&total.to_le_bytes());
        v
    }
    fn iface(n: u8, class: u8, sub: u8, proto: u8) -> Vec<u8> {
        vec![9, 4, n, 0, 1, class, sub, proto, 0]
    }
    const HID_DESC: [u8; 9] = [9, 0x21, 0x11, 0x01, 0, 1, 0x22, 52, 0];
    fn ep(addr: u8, attr: u8, max: u16, interval: u8) -> Vec<u8> {
        let m = max.to_le_bytes();
        vec![7, 5, addr, attr, m[0], m[1], interval]
    }

    fn run(cfg: &[u8]) -> (Scan, Vec<SkippedInterface>) {
        let mut s = Vec::new();
        let r = scan(cfg, |x| s.push(x));
        (r, s)
    }

    /// QEMU `usb-mouse`: one HID boot-mouse interface, EP 0x81 interrupt IN.
    #[test]
    fn boot_mouse_is_chosen() {
        let cfg = config(&[&iface(0, 3, 1, 2), &HID_DESC, &ep(0x81, 3, 4, 10)]);
        let (r, s) = run(&cfg);
        assert_eq!(
            r.hid,
            Some(HidInfo {
                config_value: 7,
                iface: 0,
                protocol: 2,
                ep_addr: 0x81,
                ep_max: 4,
                interval: 10
            })
        );
        assert!(s.is_empty());
        assert_eq!(r.stopped_at, None);
    }

    /// Mass storage (class 8, subclass 6, protocol 0x50 BBB) with bulk
    /// endpoints only, and a vendor interface whose protocol byte happens to
    /// be 1 with an interrupt-IN endpoint: neither is a keyboard.
    #[test]
    fn non_hid_interfaces_are_never_input_devices() {
        let storage = config(&[&iface(0, 8, 6, 0x50), &ep(0x81, 2, 512, 0), &ep(0x02, 2, 512, 0)]);
        let (r, s) = run(&storage);
        assert_eq!(r.hid, None);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].class, s[0].reason), (8, SkipReason::NotHid));

        let vendor = config(&[&iface(0, 0xff, 1, 1), &ep(0x81, 3, 8, 10)]);
        let (r, s) = run(&vendor);
        assert_eq!(r.hid, None, "protocol 1 of a non-HID class is not a keyboard");
        assert_eq!(s[0].reason, SkipReason::NotHid);
    }

    /// The old pairing took the first interrupt-IN endpoint of any interface.
    /// A non-HID interface in front of the boot keyboard must not win.
    #[test]
    fn boot_interface_behind_a_non_hid_interface_is_found() {
        let cfg = config(&[
            &iface(0, 0xff, 0, 1),
            &ep(0x83, 3, 16, 4),
            &iface(1, 3, 1, 1),
            &HID_DESC,
            &ep(0x81, 3, 8, 10),
        ]);
        let (r, s) = run(&cfg);
        let h = r.hid.unwrap();
        assert_eq!((h.iface, h.protocol, h.ep_addr), (1, 1, 0x81));
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].number, s[0].reason), (0, SkipReason::NotHid));
    }

    /// Report-only HID (subclass 0) — QEMU `usb-tablet`, many receivers — and
    /// a boot subclass with a reserved protocol are skipped, with the reason.
    #[test]
    fn report_only_and_reserved_protocol_are_skipped() {
        let tablet = config(&[&iface(0, 3, 0, 0), &HID_DESC, &ep(0x81, 3, 8, 4)]);
        let (r, s) = run(&tablet);
        assert_eq!(r.hid, None);
        assert_eq!(s[0].reason, SkipReason::NotBootSubclass);

        let reserved = config(&[&iface(0, 3, 1, 3), &ep(0x81, 3, 8, 4)]);
        let (r, s) = run(&reserved);
        assert_eq!(r.hid, None);
        assert_eq!(s[0].reason, SkipReason::UnknownBootProtocol);
    }

    /// Roccat-style composite (3,1,2)+(3,0,1) and a keyboard+mouse receiver
    /// (3,1,1)+(3,1,2): the first boot interface is used, every later one is
    /// reported, not silently dropped.
    #[test]
    fn later_interfaces_are_reported() {
        let roccat = config(&[
            &iface(0, 3, 1, 2),
            &HID_DESC,
            &ep(0x81, 3, 8, 1),
            &iface(1, 3, 0, 1),
            &HID_DESC,
            &ep(0x82, 3, 16, 1),
        ]);
        let (r, s) = run(&roccat);
        assert_eq!(r.hid.map(|h| (h.iface, h.protocol)), Some((0, 2)));
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].number, s[0].reason), (1, SkipReason::NotBootSubclass));

        let receiver = config(&[
            &iface(0, 3, 1, 1),
            &ep(0x81, 3, 8, 10),
            &iface(1, 3, 1, 2),
            &ep(0x82, 3, 8, 10),
        ]);
        let (r, s) = run(&receiver);
        assert_eq!(r.hid.map(|h| h.iface), Some(0));
        assert_eq!((s[0].number, s[0].reason), (1, SkipReason::OneInterfacePerDevice));
    }

    /// A boot interface whose only endpoint is interrupt OUT (or bulk) has no
    /// report pipe; it is named and the scan moves on.
    #[test]
    fn boot_interface_without_interrupt_in_is_reported() {
        let cfg = config(&[
            &iface(0, 3, 1, 1),
            &ep(0x01, 3, 8, 10),
            &iface(1, 3, 1, 2),
            &ep(0x82, 3, 4, 10),
        ]);
        let (r, s) = run(&cfg);
        assert_eq!(r.hid.map(|h| h.iface), Some(1));
        assert_eq!((s[0].number, s[0].reason), (0, SkipReason::NoInterruptIn));

        let last = config(&[&iface(0, 3, 1, 2), &ep(0x81, 2, 64, 0)]);
        let (r, s) = run(&last);
        assert_eq!(r.hid, None);
        assert_eq!(s[0].reason, SkipReason::NoInterruptIn);
    }

    /// A configuration cut short (the driver reads at most 255 bytes) stops
    /// at the broken descriptor and says where.
    #[test]
    fn truncated_configuration_reports_where_it_stopped() {
        let mut cfg = config(&[&iface(0, 3, 1, 2), &ep(0x81, 3, 4, 10), &iface(1, 3, 1, 1)]);
        let cut = cfg.len() - 4;
        cfg.truncate(cut);
        let (r, s) = run(&cfg);
        assert_eq!(r.hid.map(|h| h.iface), Some(0));
        assert!(s.is_empty());
        assert_eq!(r.stopped_at, Some(9 + 9 + 7));

        let zero = [9u8, 2, 18, 0, 1, 1, 0, 0x80, 50, 0, 4];
        assert_eq!(run(&zero).0.stopped_at, Some(9));
    }

    /// Every reason has its own log text.
    #[test]
    fn reason_texts_are_distinct() {
        let all = [
            SkipReason::NotHid,
            SkipReason::NotBootSubclass,
            SkipReason::UnknownBootProtocol,
            SkipReason::NoInterruptIn,
            SkipReason::OneInterfacePerDevice,
        ];
        for (i, a) in all.iter().enumerate() {
            assert!(!reason_text(*a).is_empty());
            for b in &all[i + 1..] {
                assert_ne!(reason_text(*a), reason_text(*b));
            }
        }
    }

    #[test]
    fn is_boot_hid_truth_table() {
        assert!(is_boot_hid(3, 1, 1));
        assert!(is_boot_hid(3, 1, 2));
        assert!(!is_boot_hid(3, 0, 2));
        assert!(!is_boot_hid(3, 1, 0));
        assert!(!is_boot_hid(9, 1, 1));
        assert!(!is_boot_hid(0, 1, 2));
    }
}
