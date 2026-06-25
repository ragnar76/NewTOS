# USB hub driver

> Extracted from FreeMiNT `sys/usb/src.km/hub.c`. The hub turns a multi-port
> hub device into per-port connect/disconnect events that drive enumeration.
> The Pi-4 needs this because its USB-A ports sit behind an **internal hub**
> (and QEMU's `usb-hub` / the xHCI root hub behave the same). Authority: USB
> 2.0 spec ch. 11. Spec group: [`_state.md`](_state.md).

## Configure a hub — `usb_hub_configure` (`hub.c:339`)

1. `GET_DESCRIPTOR(HUB)` → `bNbrPorts` → `dev->maxchild` (`hub.c:392`).
2. Read hub characteristics (power-switching, over-current).
3. **Power each port** (`SET_FEATURE(PORT_POWER)`), wait `bPwrOn2PwrGood`.
4. Read each port's status; kick the port state machine.

## Port state machine — `usb_hub_port_connect_change` (`hub.c:275`)

Per port, on a **connection-status change**:

1. `hub_port_reset(dev, port)` (`hub.c:225`): `SET_FEATURE(PORT_RESET)`, poll
   `GET_STATUS` until reset completes + the port is enabled; the speed bits in
   the port status give low/full/high speed.
2. Allocate a `usb_device` for the new device, parent = this hub, and run
   `usb_new_device` ([`core-enumeration.md`](core-enumeration.md)).
3. On disconnect: tear the child down (`disconnect` its driver, free the slot,
   recurse for a child hub).

`usb_hub_events` / the root-hub wakeup (`usb_rh_wakeup`,
[`host-controller-interface.md`](host-controller-interface.md)) is what pumps
this when a port status changes.

## Port-status / feature constants (USB spec)

`PORT_CONNECTION`, `PORT_ENABLE`, `PORT_RESET`, `PORT_POWER`,
`PORT_LOW_SPEED`/`PORT_HIGH_SPEED`, and their `C_*` change bits — from the USB
spec; FreeMiNT mirrors them in `usb.h`/`usb_defs.h`.

## NewTOS shape

A `Hub` over a `UsbDevice` with `maxchild` ports; a `poll_ports()` that
issues the reset → `usb_new_device` flow. The **xHCI root hub** is the first
hub (its ports come from the controller's `root_hub` op); a downstream
**device hub** (the Pi-4 internal hub) is enumerated like any other device and
then walked the same way. For D3 a single level (root hub → device) is enough
to reach a keyboard/mouse; nested hubs are the same code applied recursively.
