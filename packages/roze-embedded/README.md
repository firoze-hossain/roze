# 🌹 Roze-Embedded: Microcontroller Hardware Platform & embedded-hal Driver Suite

**roze-embedded** is the official microcontroller hardware platform, `embedded-hal` driver framework, and CMSIS-SVD peripheral access engine for Roze. It provides bare-metal hardware drivers, volatile MMIO register manipulation, interrupt vector tables, cross-compilation pipeline support, and flashing CLI tooling for ARM Cortex-M and RISC-V microcontrollers.

---

## 🏛️ Supported Architectures & Microcontrollers

1. **ARM Cortex-M**:
   - `thumbv6m-none-eabi`: ARM Cortex-M0 / Cortex-M0+ (Raspberry Pi RP2040)
   - `thumbv7m-none-eabi`: ARM Cortex-M3 (STMicroelectronics STM32F103 'Blue Pill')
   - `thumbv7em-none-eabi`: ARM Cortex-M4 with hardware FPU (STMicroelectronics STM32F401 'Black Pill')
2. **RISC-V**:
   - `riscv32imc-unknown-none-elf`: 32-bit Integer, Multiplication, Compressed instructions (Espressif ESP32-C3, GigaDevice GD32VF103)
   - `riscv64gc-unknown-none-elf`: 64-bit General-purpose RISC-V with Atomic, Floating-point, Compressed instructions

---

## 🔌 Core 'embedded-hal' Peripheral Traits

- **GPIO Digital I/O**: `OutputPin` (`set_high`, `set_low`, `is_set_high`, `is_set_low`, `toggle`) and `InputPin` (`is_high`, `is_low`).
- **I2C Serial Bus**: `I2cBus` master write, read, write-read transactions, standard (100 kHz) and fast (400 kHz) clock rates.
- **SPI Serial Bus**: `SpiBus` full-duplex byte transfer, bulk write, SPI Modes 0..3 (CPOL, CPHA).
- **UART Serial Communication**: `UartSerial` asynchronous serial interface, baud rate divisors, 8N1 framing, TX/RX status checks.
- **PWM Timer Channels**: `PwmPin` timer channels, configurable frequency, duty cycle period scaling (0..1000).
- **ADC Voltage Sensing**: `AdcChannel` analog sampling, 10-bit / 12-bit / 16-bit resolution, millivolt conversion.
- **SysTick Delay**: `TimerDelay` microsecond and millisecond busy-wait timing.

---

## ⚙️ SVD PAC Generator & Flashing Pipeline

- **`roze svd`**: Automated CMSIS-SVD XML to type-safe pure Roze Peripheral Access Crate (PAC) compiler.
- **`roze flash`**: Direct firmware programming CLI targeting STM32 (ST-Link / OpenOCD), RP2040 (picotool / UF2), and ESP32 (esptool / UART bootloader).
- **Bare-Metal Linker Scripts & Vector Tables**: Memory maps and interrupt vector tables for immediate bare-metal deployment.
