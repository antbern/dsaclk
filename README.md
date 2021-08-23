# dsaclk - Data Science Alarm Clock

The idea is to construct an alarm clock that can be used to record and analyze sleep patterns using the STM32F401 Nucleo-64 board.




## Useful resources

- [STM32L0 Rust Part 1 - Getting Started](https://craigjb.com/2019/12/31/stm32l0-rust/)
- [STM32 cross-series timer overview application note](https://www.st.com/resource/en/application_note/dm00042534-stm32-crossseries-timer-overview-stmicroelectronics.pdf)
- [Rusted brains: Running Rust firmware on a Cortex-M microcontroller](https://dev.to/minkovsky/rusted-brains-running-rust-firmware-on-a-cortex-m-microcontroller-3had)

This probject is based on the [cortex-m-quickstart](https://github.com/rust-embedded/cortex-m-quickstart) template repository.


## Useful commands

See size of the different sections (requires [cargo-binutils](https://github.com/rust-embedded/cargo-binutils))
```
cargo size --release -- -A
```


Run clippy and treat warnings as errors
```
cargo clippy -- -D warnings
```


### Using `defmt`

Install [`defmt-print`](https://crates.io/crates/defmt-print)
```bash
cargo install defmt-print
```

Pipe the serial port output through `defmt-print` directly
```bash
cat /dev/ttyACM0 | defmt-print -e target/thumb*/debug/dsaclk
```
or use the bash script that additionally configures the TTY device in raw mode
```bash
./defmt_uart.sh /dev/ttyACM0
```

Useful links:

* [knurling-rs/app-template](https://github.com/knurling-rs/app-template)
* [defmt book](https://defmt-next.ferrous-systems.com/)
* https://ferrous-systems.com/blog/defmt/
* Original [mpu6050 library](https://github.com/juliangaal/mpu6050) by [juliangaal](https://github.com/juliangaal).
