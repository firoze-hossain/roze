// tools/roze-svd/tests/test_svd.rs
//
// Tests for roze-svd CMSIS-SVD parser and PAC code generator

use std::fs;
use std::path::PathBuf;

fn scratch_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("roze-svd-test-{}", test_name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch dir for svd test");
    dir
}

#[test]
fn test_svd_parsing_stm32f103() {
    let svd_text = roze_svd::reference_stm32f103_svd();
    let dev = roze_svd::parse_svd(svd_text).expect("parse stm32 svd");

    assert_eq!(dev.name, "STM32F103");
    assert_eq!(dev.cpu_name, "CM3");
    assert_eq!(dev.peripherals.len(), 2);

    let gpioa = dev.peripherals.iter().find(|p| p.name == "GPIOA").expect("GPIOA peripheral");
    assert_eq!(gpioa.base_address, 0x40010800);
    assert_eq!(gpioa.registers.len(), 3);
    assert_eq!(gpioa.interrupts.len(), 1);
    assert_eq!(gpioa.interrupts[0].name, "EXTI0");
    assert_eq!(gpioa.interrupts[0].value, 6);

    let crl = gpioa.registers.iter().find(|r| r.name == "CRL").expect("CRL register");
    assert_eq!(crl.address_offset, 0);
    assert_eq!(crl.fields.len(), 2);
    assert_eq!(crl.fields[0].name, "MODE0");
    assert_eq!(crl.fields[0].bit_offset, 0);
    assert_eq!(crl.fields[0].bit_width, 2);
}

#[test]
fn test_svd_parsing_rp2040() {
    let svd_text = roze_svd::reference_rp2040_svd();
    let dev = roze_svd::parse_svd(svd_text).expect("parse rp2040 svd");

    assert_eq!(dev.name, "RP2040");
    assert_eq!(dev.cpu_name, "CM0PLUS");
    let sio = dev.peripherals.iter().find(|p| p.name == "SIO").expect("SIO peripheral");
    assert_eq!(sio.base_address, 0xD0000000);
}

#[test]
fn test_svd_parsing_esp32c3() {
    let svd_text = roze_svd::reference_esp32c3_svd();
    let dev = roze_svd::parse_svd(svd_text).expect("parse esp32c3 svd");

    assert_eq!(dev.name, "ESP32C3");
    assert_eq!(dev.cpu_name, "RISCV32IMC");
    let gpio = dev.peripherals.iter().find(|p| p.name == "GPIO").expect("GPIO peripheral");
    assert_eq!(gpio.base_address, 0x60004000);
}

#[test]
fn test_pac_generation() {
    let svd_text = roze_svd::reference_stm32f103_svd();
    let dev = roze_svd::parse_svd(svd_text).unwrap();
    let pac = roze_svd::generate_pac(&dev);

    assert!(pac.contains("func gpioa_base() -> int { return 1073809408; }"));
    assert!(pac.contains("func gpioa_crl_offset() -> int { return 0; }"));
    assert!(pac.contains("func gpioa_crl_read() -> int"));
    assert!(pac.contains("func gpioa_crl_write(val: int)"));
    assert!(pac.contains("func gpioa_crl_modify(clear_mask: int, set_mask: int)"));
    assert!(pac.contains("func gpioa_crl_mode0_get() -> int"));
    assert!(pac.contains("func gpioa_crl_mode0_set(val: int)"));
    assert!(pac.contains("func irq_exti0() -> int { return 6; }"));
    assert!(pac.contains("func pac_take_peripherals() -> DevicePeripherals"));
}

#[test]
fn test_process_svd_file() {
    let dir = scratch_dir("process");
    let svd_path = dir.join("test.svd");
    fs::write(&svd_path, roze_svd::reference_stm32f103_svd()).unwrap();

    let out_dir = dir.join("pac_out");
    roze_svd::process_svd_file(&svd_path, &out_dir).expect("process svd file");

    let pac_file = out_dir.join("stm32f103_pac.roze");
    assert!(pac_file.exists());
    assert!(out_dir.join("README.md").exists());

    let content = fs::read_to_string(&pac_file).unwrap();
    assert!(content.contains("Device: STM32F103"));
}
