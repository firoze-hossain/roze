// compiler/tests/embedded.rs
//
// Microcontroller IoT & Embedded Firmware Target Integration Tests (M6-T08)
// Verifies ARM Cortex-M, RISC-V, and RP2040 cross-compilation, vector tables,
// linker scripts, Intel HEX and raw flat binary generation, and flash engine.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn scratch_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("roze-embedded-test-{}", test_name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch dir for embedded test");
    dir
}

#[test]
fn test_embedded_thumbv7m_stm32_build() {
    let dir = scratch_dir("thumbv7m");
    let src_file = dir.join("blink_stm32.roze");
    fs::write(
        &src_file,
        r#"
import "embedded_hal";

func main() {
    let gpio = make_stm32_gpio(stm32_gpioa_base());
    gpio.set_high(5);
    let is_on = gpio.is_set_high(5);
}
"#,
    )
    .unwrap();

    let roze_bin = env!("CARGO_BIN_EXE_roze");

    // 1. Build --target thumbv7m-none-eabi
    let build_output = Command::new(roze_bin)
        .arg("build")
        .arg(&src_file)
        .arg("--target")
        .arg("thumbv7m-none-eabi")
        .arg("--chip")
        .arg("stm32f103")
        .current_dir(&dir)
        .output()
        .expect("invoke roze build --target thumbv7m-none-eabi");

    assert!(
        build_output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    // Verify all generated embedded artifacts
    let elf_file = dir.join("blink_stm32.elf");
    let bin_file = dir.join("blink_stm32.bin");
    let hex_file = dir.join("blink_stm32.hex");
    let ld_file = dir.join("blink_stm32_stm32f103.ld");

    assert!(elf_file.exists(), "ELF binary was not generated");
    assert!(bin_file.exists(), "Raw flat binary was not generated");
    assert!(hex_file.exists(), "Intel HEX file was not generated");
    assert!(ld_file.exists(), "Linker script was not generated");

    // Check ELF magic bytes
    let elf_bytes = fs::read(&elf_file).unwrap();
    assert_eq!(&elf_bytes[0..4], &[0x7f, b'E', b'L', b'F']);

    // Check Intel HEX format (colon start, checksum at end, EOF record)
    let hex_str = fs::read_to_string(&hex_file).unwrap();
    assert!(hex_str.starts_with(':'));
    assert!(hex_str.ends_with(":00000001FF\n"));

    // Check Linker script contents
    let ld_str = fs::read_to_string(&ld_file).unwrap();
    assert!(ld_str.contains("FLASH (rx)  : ORIGIN = 0x08000000"));
    assert!(ld_str.contains("RAM   (rwx) : ORIGIN = 0x20000000"));
    assert!(ld_str.contains("ENTRY(Reset_Handler)"));
}

#[test]
fn test_embedded_riscv32imc_esp32_build() {
    let dir = scratch_dir("riscv32");
    let src_file = dir.join("esp32_sensor.roze");
    fs::write(
        &src_file,
        r#"
import "embedded_hal";

func main() {
    let gpio = make_esp32c3_gpio();
    gpio.set_high(2);
}
"#,
    )
    .unwrap();

    let roze_bin = env!("CARGO_BIN_EXE_roze");

    let build_output = Command::new(roze_bin)
        .arg("build")
        .arg(&src_file)
        .arg("--target")
        .arg("riscv32imc")
        .arg("--chip")
        .arg("esp32c3")
        .current_dir(&dir)
        .output()
        .expect("invoke roze build --target riscv32imc");

    assert!(
        build_output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let elf_file = dir.join("esp32_sensor.elf");
    let bin_file = dir.join("esp32_sensor.bin");
    let hex_file = dir.join("esp32_sensor.hex");
    let ld_file = dir.join("esp32_sensor_esp32c3.ld");

    assert!(elf_file.exists());
    assert!(bin_file.exists());
    assert!(hex_file.exists());
    assert!(ld_file.exists());

    let ld_str = fs::read_to_string(&ld_file).unwrap();
    assert!(ld_str.contains("ENTRY(_start_riscv)"));
}

#[test]
fn test_embedded_rp2040_build() {
    let dir = scratch_dir("rp2040");
    let src_file = dir.join("pico_led.roze");
    fs::write(
        &src_file,
        r#"
import "embedded_hal";

func main() {
    let gpio = make_rp2040_gpio();
    gpio.set_high(25);
    gpio.toggle(25);
}
"#,
    )
    .unwrap();

    let roze_bin = env!("CARGO_BIN_EXE_roze");

    let build_output = Command::new(roze_bin)
        .arg("build")
        .arg(&src_file)
        .arg("--target")
        .arg("thumbv6m-none-eabi")
        .arg("--chip")
        .arg("rp2040")
        .current_dir(&dir)
        .output()
        .expect("invoke roze build --target thumbv6m-none-eabi");

    assert!(
        build_output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    assert!(dir.join("pico_led.bin").exists());
    assert!(dir.join("pico_led.hex").exists());
}

#[test]
fn test_roze_flash_cli() {
    let dir = scratch_dir("flash-cli");
    let src_file = dir.join("firmware.roze");
    fs::write(
        &src_file,
        r#"
import "embedded_hal";

func main() {
    let board = make_mock_embedded_board("stm32");
    board.set_high(13);
}
"#,
    )
    .unwrap();

    let roze_bin = env!("CARGO_BIN_EXE_roze");

    // Test roze flash command directly compiling and flashing
    let flash_output = Command::new(roze_bin)
        .arg("flash")
        .arg(&src_file)
        .arg("--chip")
        .arg("stm32")
        .current_dir(&dir)
        .output()
        .expect("invoke roze flash");

    let stdout = String::from_utf8_lossy(&flash_output.stdout);
    assert!(
        flash_output.status.success(),
        "flash failed: {}",
        String::from_utf8_lossy(&flash_output.stderr)
    );
    assert!(stdout.contains("Programming"));
    assert!(stdout.contains("Flashing completed successfully"));
}

#[test]
fn test_roze_svd_cli() {
    let dir = scratch_dir("svd-cli");
    let out_dir = dir.join("pac_gen");

    let roze_bin = env!("CARGO_BIN_EXE_roze");

    let svd_output = Command::new(roze_bin)
        .arg("svd")
        .arg("stm32f103")
        .arg("--out-dir")
        .arg(&out_dir)
        .current_dir(&dir)
        .output()
        .expect("invoke roze svd");

    assert!(
        svd_output.status.success(),
        "roze svd failed: {}",
        String::from_utf8_lossy(&svd_output.stderr)
    );
    assert!(out_dir.join("stm32f103_pac.roze").exists());
    assert!(out_dir.join("README.md").exists());
}
