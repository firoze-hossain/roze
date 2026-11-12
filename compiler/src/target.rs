// compiler/src/target.rs
//
// Target architectures supported by the Roze compiler toolchain.

use clap::ValueEnum;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Default)]
pub enum Target {
    /// Compile to JVM bytecode (.class) via javac.
    #[default]
    Jvm,
    /// Compile to native machine code via Cranelift.
    Native,
    /// Compile freestanding native binary (zero libc, zero OS runtime) for kernels & bare metal.
    Freestanding,
    /// Compile to WebAssembly MVP binary (.wasm).
    #[value(name = "wasm32", alias = "wasm")]
    Wasm32,
    /// Compile to WebAssembly System Interface (WASI snapshot-preview1).
    #[value(name = "wasm32-wasi", alias = "wasi")]
    Wasm32Wasi,
    /// Compile to ARM Cortex-M3 / M4 bare-metal microcontroller (STM32F1/F4).
    #[value(name = "thumbv7m-none-eabi", alias = "thumbv7m", alias = "cortex-m3", alias = "cortex-m4", alias = "stm32")]
    Thumbv7mNoneEabi,
    /// Compile to ARM Cortex-M0 / M0+ / RP2040 microcontroller (Raspberry Pi Pico).
    #[value(name = "thumbv6m-none-eabi", alias = "thumbv6m", alias = "cortex-m0", alias = "rp2040")]
    Thumbv6mNoneEabi,
    /// Compile to RISC-V 32-bit IMC bare-metal microcontroller (ESP32-C3 / GD32VF103).
    #[value(name = "riscv32imc", alias = "riscv32", alias = "esp32c3")]
    Riscv32imc,
    /// Compile to Xtensa ESP32 bare-metal microcontroller.
    #[value(name = "xtensa-esp32-none-elf", alias = "esp32")]
    XtensaEsp32NoneElf,
}
