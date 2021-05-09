use core::{
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use cortex_m::{
    interrupt::{self, CriticalSection},
    prelude::_embedded_hal_blocking_serial_Write,
    register,
};

// defined in main.rs
use crate::DEBUG_UART_TX;

static TAKEN: AtomicBool = AtomicBool::new(false);
static INTERRUPTS_ACTIVE: AtomicBool = AtomicBool::new(false);

/// An empty struct implementing the `defmt::Logger` trait returning a `UARTLoggerWriter`, which in
/// turn writes the `defmt` messages over the global debug UART device defined in `main.rs`
///
/// Implementation is basically copied straight from the [`defmt-rtt` implementation](https://docs.rs/defmt-rtt/0.2.0/src/defmt_rtt/lib.rs.html#25-64)
/// and modified to construct and return a `UARTLoggerWriter` instead.
#[defmt::global_logger]
struct UARTLogger;

unsafe impl defmt::Logger for UARTLogger {
    fn acquire() -> Option<NonNull<dyn defmt::Write>> {
        let primask = register::primask::read();
        interrupt::disable();
        if !TAKEN.load(Ordering::Relaxed) {
            // no need for CAS because interrupts are disabled
            TAKEN.store(true, Ordering::Relaxed);

            INTERRUPTS_ACTIVE.store(primask.is_active(), Ordering::Relaxed);

            Some(NonNull::from(&UARTLoggerWriter {
                cs: unsafe { CriticalSection::new() },
            } as &dyn defmt::Write))
        } else {
            if primask.is_active() {
                // re-enable interrupts
                unsafe { interrupt::enable() }
            }
            None
        }
    }

    unsafe fn release(_: NonNull<dyn defmt::Write>) {
        TAKEN.store(false, Ordering::Relaxed);
        if INTERRUPTS_ACTIVE.load(Ordering::Relaxed) {
            // re-enable interrupts
            interrupt::enable()
        }
    }
}

struct UARTLoggerWriter {
    cs: CriticalSection,
}

impl defmt::Write for UARTLoggerWriter {
    fn write(&mut self, bytes: &[u8]) {
        DEBUG_UART_TX.try_borrow_mut(&self.cs, |uart| {
            uart.bwrite_all(bytes).unwrap();
            Some(())
        });
    }
}

// The rest of this module is taken from https://github.com/knurling-rs/app-template/blob/main/src/lib.rs

// same panicking *behavior* as `panic-probe` but doesn't print a panic message
// this prevents the panic message being printed *twice* when `defmt::panic` is invoked
#[defmt::panic_handler]
fn panic() -> ! {
    cortex_m::asm::udf()
}

static COUNT: AtomicUsize = AtomicUsize::new(0);
defmt::timestamp!("{=usize}", {
    // NOTE(no-CAS) `timestamps` runs with interrupts disabled
    let n = COUNT.load(Ordering::Relaxed);
    COUNT.store(n + 1, Ordering::Relaxed);
    n
});
