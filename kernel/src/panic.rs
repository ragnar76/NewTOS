//! Panic handler — PHASE-01 §D5.
//!
//! Prints the panic message via the kernel console and then halts the core
//! in `wfe`. No unwinding (compile profile sets `panic = "abort"`).
//!
//! Formatting goes through `core::fmt` directly into the UART; we never
//! allocate (the allocator does not exist yet in this phase).

use core::fmt::Write;
use core::panic::PanicInfo;

use crate::arch::aarch64::halt;
use crate::console::CONSOLE;
use crate::hal::uart::{Uart, UartWriter};

#[panic_handler]
fn on_panic(info: &PanicInfo) -> ! {
    // Best-effort: the UART might not have been initialized yet if the
    // panic happens very early. `init` is idempotent on PL011 (we re-write
    // the same registers), so calling it again is safe.
    CONSOLE.init();
    CONSOLE.write_str("\n[PANIC] ");

    let mut w = UartWriter(&CONSOLE);
    // Ignore the formatter result: there is nowhere to report it and
    // `UartWriter::write_str` cannot fail.
    let _ = writeln!(w, "{}", info.message());

    if let Some(loc) = info.location() {
        let _ = writeln!(w, "  at {}:{}:{}", loc.file(), loc.line(), loc.column());
    }

    halt();
}
