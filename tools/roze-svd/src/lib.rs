// tools/roze-svd/src/lib.rs
//
// 🌹 roze-svd: CMSIS-SVD XML Parser and Peripheral Access Crate (PAC) Generator for Roze
// Parses ARM & RISC-V SVD hardware description files and generates type-safe, zero-overhead
// Roze Peripheral Access Crates (PAC) with MMIO volatile register read/write/modify methods.

use anyhow::{Result, anyhow};
use colored::*;
use std::fs;
use std::path::Path;

// ==============================================================================
// 1. SVD Data Structures
// ==============================================================================

#[derive(Debug, Clone)]
pub struct SvdDevice {
    pub name: String,
    pub version: String,
    pub description: String,
    pub address_unit_bits: u32,
    pub width: u32,
    pub cpu_name: String,
    pub cpu_nvic_prio_bits: u32,
    pub peripherals: Vec<SvdPeripheral>,
}

#[derive(Debug, Clone)]
pub struct SvdPeripheral {
    pub name: String,
    pub description: String,
    pub group_name: String,
    pub base_address: u64,
    pub registers: Vec<SvdRegister>,
    pub interrupts: Vec<SvdInterrupt>,
}

#[derive(Debug, Clone)]
pub struct SvdRegister {
    pub name: String,
    pub description: String,
    pub address_offset: u32,
    pub size: u32,
    pub access: String,
    pub reset_value: u64,
    pub fields: Vec<SvdField>,
}

#[derive(Debug, Clone)]
pub struct SvdField {
    pub name: String,
    pub description: String,
    pub bit_offset: u32,
    pub bit_width: u32,
}

#[derive(Debug, Clone)]
pub struct SvdInterrupt {
    pub name: String,
    pub value: u32,
    pub description: String,
}

// ==============================================================================
// 2. Lightweight CMSIS-SVD XML Parser
// ==============================================================================

#[derive(Debug, Clone)]
enum XmlNode {
    Element {
        tag: String,
        children: Vec<XmlNode>,
    },
    Text(String),
}

impl XmlNode {
    fn child_tag(&self, name: &str) -> Option<&XmlNode> {
        if let XmlNode::Element { children, .. } = self {
            for child in children {
                if let XmlNode::Element { tag, .. } = child {
                    if tag == name {
                        return Some(child);
                    }
                }
            }
        }
        None
    }

    fn child_text(&self, name: &str) -> Option<String> {
        if let Some(node) = self.child_tag(name) {
            if let XmlNode::Element { children, .. } = node {
                for c in children {
                    if let XmlNode::Text(txt) = c {
                        return Some(txt.trim().to_string());
                    }
                }
            }
        }
        None
    }

    fn children_tags(&self, name: &str) -> Vec<&XmlNode> {
        let mut list = Vec::new();
        if let XmlNode::Element { children, .. } = self {
            for child in children {
                if let XmlNode::Element { tag, .. } = child {
                    if tag == name {
                        list.push(child);
                    }
                }
            }
        }
        list
    }
}

fn parse_xml(input: &str) -> Result<XmlNode> {
    let mut chars = input.chars().peekable();
    let mut stack: Vec<(String, Vec<XmlNode>)> = Vec::new();
    let mut root: Option<XmlNode> = None;

    while let Some(&ch) = chars.peek() {
        if ch == '<' {
            chars.next();
            if chars.peek() == Some(&'?') || chars.peek() == Some(&'!') {
                // Skip processing instruction or comment
                let is_comment = chars.peek() == Some(&'!');
                chars.next();
                if is_comment && chars.peek() == Some(&'-') {
                    chars.next();
                    if chars.peek() == Some(&'-') { chars.next(); }
                    // skip until -->
                    let mut prev2 = ' ';
                    let mut prev1 = ' ';
                    while let Some(c) = chars.next() {
                        if prev2 == '-' && prev1 == '-' && c == '>' {
                            break;
                        }
                        prev2 = prev1;
                        prev1 = c;
                    }
                } else {
                    // skip until >
                    while let Some(c) = chars.next() {
                        if c == '>' { break; }
                    }
                }
                continue;
            }

            if chars.peek() == Some(&'/') {
                // Closing tag
                chars.next();
                let mut tag_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '>' || c.is_whitespace() { break; }
                    tag_name.push(c);
                    chars.next();
                }
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c == '>' { break; }
                }

                if let Some((open_tag, children)) = stack.pop() {
                    let elem = XmlNode::Element {
                        tag: open_tag,
                        children,
                    };
                    if let Some((_, parent_children)) = stack.last_mut() {
                        parent_children.push(elem);
                    } else {
                        root = Some(elem);
                    }
                }
            } else {
                // Opening or self-closing tag
                let mut tag_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '>' || c == '/' || c.is_whitespace() { break; }
                    tag_name.push(c);
                    chars.next();
                }

                // Skip attributes until '>' or '/>'
                let mut self_closing = false;
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c == '/' && chars.peek() == Some(&'>') {
                        chars.next();
                        self_closing = true;
                        break;
                    }
                    if c == '>' { break; }
                }

                if self_closing {
                    let elem = XmlNode::Element { tag: tag_name, children: Vec::new() };
                    if let Some((_, parent_children)) = stack.last_mut() {
                        parent_children.push(elem);
                    } else {
                        root = Some(elem);
                    }
                } else {
                    stack.push((tag_name, Vec::new()));
                }
            }
        } else {
            // Text content
            let mut text = String::new();
            while let Some(&c) = chars.peek() {
                if c == '<' { break; }
                text.push(c);
                chars.next();
            }
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                if let Some((_, children)) = stack.last_mut() {
                    children.push(XmlNode::Text(trimmed.to_string()));
                }
            }
        }
    }

    root.ok_or_else(|| anyhow!("Failed to parse XML: empty or invalid document"))
}

pub fn parse_svd(content: &str) -> Result<SvdDevice> {
    let root = parse_xml(content)?;
    let dev_node = match &root {
        XmlNode::Element { tag, .. } if tag == "device" => &root,
        _ => root.child_tag("device").ok_or_else(|| anyhow!("Root <device> element not found"))?,
    };

    let name = dev_node.child_text("name").unwrap_or_else(|| "UnknownDevice".to_string());
    let version = dev_node.child_text("version").unwrap_or_else(|| "1.0".to_string());
    let description = dev_node.child_text("description").unwrap_or_else(|| "".to_string());
    let address_unit_bits = dev_node.child_text("addressUnitBits").and_then(|s| s.parse().ok()).unwrap_or(8);
    let width = dev_node.child_text("width").and_then(|s| s.parse().ok()).unwrap_or(32);

    let cpu_node = dev_node.child_tag("cpu");
    let cpu_name = cpu_node.and_then(|n| n.child_text("name")).unwrap_or_else(|| "CM3".to_string());
    let cpu_nvic_prio_bits = cpu_node.and_then(|n| n.child_text("nvicPrioBits")).and_then(|s| s.parse().ok()).unwrap_or(4);

    let mut peripherals = Vec::new();
    if let Some(periphs_node) = dev_node.child_tag("peripherals") {
        for p_elem in periphs_node.children_tags("peripheral") {
            let p_name = p_elem.child_text("name").unwrap_or_default();
            let p_desc = p_elem.child_text("description").unwrap_or_default();
            let p_group = p_elem.child_text("groupName").unwrap_or_default();
            let base_addr_str = p_elem.child_text("baseAddress").unwrap_or_else(|| "0".to_string());
            let base_address = parse_int_hex_or_dec(&base_addr_str);

            let mut registers = Vec::new();
            if let Some(regs_node) = p_elem.child_tag("registers") {
                for r_elem in regs_node.children_tags("register") {
                    let r_name = r_elem.child_text("name").unwrap_or_default();
                    let r_desc = r_elem.child_text("description").unwrap_or_default();
                    let off_str = r_elem.child_text("addressOffset").unwrap_or_else(|| "0".to_string());
                    let address_offset = parse_int_hex_or_dec(&off_str) as u32;
                    let size = r_elem.child_text("size").and_then(|s| s.parse().ok()).unwrap_or(32);
                    let access = r_elem.child_text("access").unwrap_or_else(|| "read-write".to_string());
                    let reset_str = r_elem.child_text("resetValue").unwrap_or_else(|| "0".to_string());
                    let reset_value = parse_int_hex_or_dec(&reset_str);

                    let mut fields = Vec::new();
                    if let Some(fields_node) = r_elem.child_tag("fields") {
                        for f_elem in fields_node.children_tags("field") {
                            let f_name = f_elem.child_text("name").unwrap_or_default();
                            let f_desc = f_elem.child_text("description").unwrap_or_default();

                            let mut bit_offset = 0;
                            let mut bit_width = 1;

                            if let Some(bo) = f_elem.child_text("bitOffset").and_then(|s| s.parse().ok()) {
                                bit_offset = bo;
                                if let Some(bw) = f_elem.child_text("bitWidth").and_then(|s| s.parse().ok()) {
                                    bit_width = bw;
                                }
                            } else if let Some(range) = f_elem.child_text("bitRange") {
                                // [msb:lsb] format
                                let clean = range.trim_matches(|c| c == '[' || c == ']');
                                if let Some((msb_s, lsb_s)) = clean.split_once(':') {
                                    if let (Ok(msb), Ok(lsb)) = (msb_s.parse::<u32>(), lsb_s.parse::<u32>()) {
                                        bit_offset = lsb;
                                        bit_width = msb.saturating_sub(lsb) + 1;
                                    }
                                }
                            }

                            fields.push(SvdField {
                                name: f_name,
                                description: f_desc,
                                bit_offset,
                                bit_width,
                            });
                        }
                    }

                    registers.push(SvdRegister {
                        name: r_name,
                        description: r_desc,
                        address_offset,
                        size,
                        access,
                        reset_value,
                        fields,
                    });
                }
            }

            let mut interrupts = Vec::new();
            for int_elem in p_elem.children_tags("interrupt") {
                let i_name = int_elem.child_text("name").unwrap_or_default();
                let i_val = int_elem.child_text("value").and_then(|s| s.parse().ok()).unwrap_or(0);
                let i_desc = int_elem.child_text("description").unwrap_or_default();
                interrupts.push(SvdInterrupt {
                    name: i_name,
                    value: i_val,
                    description: i_desc,
                });
            }

            peripherals.push(SvdPeripheral {
                name: p_name,
                description: p_desc,
                group_name: p_group,
                base_address,
                registers,
                interrupts,
            });
        }
    }

    Ok(SvdDevice {
        name,
        version,
        description,
        address_unit_bits,
        width,
        cpu_name,
        cpu_nvic_prio_bits,
        peripherals,
    })
}

fn parse_int_hex_or_dec(s: &str) -> u64 {
    let clean = s.trim();
    if clean.starts_with("0x") || clean.starts_with("0X") {
        u64::from_str_radix(&clean[2..], 16).unwrap_or(0)
    } else if clean.starts_with("#") {
        u64::from_str_radix(&clean[1..], 16).unwrap_or(0)
    } else {
        clean.parse().unwrap_or(0)
    }
}

// ==============================================================================
// 3. Peripheral Access Crate (PAC) Code Generator
// ==============================================================================

pub fn generate_pac(device: &SvdDevice) -> String {
    let mut out = String::new();
    out.push_str("// ==============================================================================\n");
    out.push_str(&format!("// 🌹 Roze Peripheral Access Crate (PAC) for {}\n", device.name));
    out.push_str("// Generated automatically by roze-svd from CMSIS-SVD hardware description.\n");
    out.push_str("// DO NOT EDIT DIRECTLY.\n");
    out.push_str("// ==============================================================================\n\n");
    out.push_str("import \"embedded_hal\";\n\n");

    // Device metadata
    out.push_str(&format!("// Device: {}\n", device.name));
    out.push_str(&format!("// Architecture: {}\n", device.cpu_name));
    out.push_str(&format!("// Description: {}\n\n", device.description.replace('\n', " ")));

    // 1. Interrupt Enum & Table
    out.push_str("// --- Peripheral Interrupts ---\n");
    let mut all_interrupts = Vec::new();
    for p in &device.peripherals {
        for i in &p.interrupts {
            all_interrupts.push(i.clone());
        }
    }
    all_interrupts.sort_by_key(|i| i.value);
    all_interrupts.dedup_by(|a, b| a.value == b.value);

    for i in &all_interrupts {
        out.push_str(&format!("func irq_{}() -> int {{ return {}; }}\n", i.name.to_lowercase(), i.value));
    }
    out.push_str("\n");

    // 2. Peripherals
    for p in &device.peripherals {
        let p_lower = p.name.to_lowercase();
        out.push_str("// ==============================================================================\n");
        out.push_str(&format!("// Peripheral: {} (Base: 0x{:08X})\n", p.name, p.base_address));
        if !p.description.is_empty() {
            out.push_str(&format!("// {}\n", p.description.replace('\n', " ")));
        }
        out.push_str("// ==============================================================================\n");

        out.push_str(&format!("func {}_base() -> int {{ return {}; }}\n\n", p_lower, p.base_address as i64));

        // Registers
        for r in &p.registers {
            let r_lower = r.name.to_lowercase();
            let reg_addr_name = format!("{}_{}", p_lower, r_lower);

            out.push_str(&format!("// Register: {} (Offset: 0x{:02X}, Reset: 0x{:08X})\n", r.name, r.address_offset, r.reset_value));
            out.push_str(&format!("func {}_offset() -> int {{ return {}; }}\n", reg_addr_name, r.address_offset));
            out.push_str(&format!("func {}_addr() -> int {{ return {}_base() + {}; }}\n", reg_addr_name, p_lower, r.address_offset));

            // Read
            out.push_str(&format!("func {}_read() -> int {{\n", reg_addr_name));
            out.push_str(&format!("    return mmio_read({}_addr());\n", reg_addr_name));
            out.push_str("}\n");

            // Write
            out.push_str(&format!("func {}_write(val: int) {{\n", reg_addr_name));
            out.push_str(&format!("    mmio_write({}_addr(), val);\n", reg_addr_name));
            out.push_str("}\n");

            // Modify
            out.push_str(&format!("func {}_modify(clear_mask: int, set_mask: int) {{\n", reg_addr_name));
            out.push_str(&format!("    mmio_modify({}_addr(), clear_mask, set_mask);\n", reg_addr_name));
            out.push_str("}\n");

            // Fields
            for f in &r.fields {
                let f_lower = f.name.to_lowercase();
                let field_name = format!("{}_{}", reg_addr_name, f_lower);
                let mask = (1u64 << f.bit_width) - 1;

                out.push_str(&format!("func {}_mask() -> int {{ return {}; }}\n", field_name, mask));
                out.push_str(&format!("func {}_shift() -> int {{ return {}; }}\n", field_name, f.bit_offset));

                // Get field
                out.push_str(&format!("func {}_get() -> int {{\n", field_name));
                out.push_str(&format!("    return bit_and(bit_shr({}_read(), {}), {});\n", reg_addr_name, f.bit_offset, mask));
                out.push_str("}\n");

                // Set field
                out.push_str(&format!("func {}_set(val: int) {{\n", field_name));
                let clear_mask = mask << f.bit_offset;
                out.push_str(&format!("    {}_modify({}, bit_shl(bit_and(val, {}), {}));\n", reg_addr_name, clear_mask, mask, f.bit_offset));
                out.push_str("}\n");
            }

            out.push_str("\n");
        }
    }

    // 3. Device Peripherals Structure
    out.push_str("// ==============================================================================\n");
    out.push_str("// Device Peripherals Handle\n");
    out.push_str("// ==============================================================================\n\n");

    out.push_str("class DevicePeripherals {\n");
    out.push_str("    name: string\n");
    out.push_str("}\n\n");

    out.push_str("func pac_take_peripherals() -> DevicePeripherals {\n");
    out.push_str(&format!("    let dp = new DevicePeripherals(\"{}\");\n", device.name));
    out.push_str("    return dp;\n");
    out.push_str("}\n");

    out
}

// ==============================================================================
// 4. Built-in Reference SVD Templates
// ==============================================================================

pub fn reference_stm32f103_svd() -> &'static str {
    r#"<?xml version="1.0" encoding="utf-8"?>
<device schemaVersion="1.1" xmlns:xs="http://www.w3.org/2001/XMLSchema-instance">
  <name>STM32F103</name>
  <version>1.0</version>
  <description>STM32F103 ARM Cortex-M3 High-Density Microcontroller</description>
  <addressUnitBits>8</addressUnitBits>
  <width>32</width>
  <cpu>
    <name>CM3</name>
    <revision>r1p1</revision>
    <endian>little</endian>
    <nvicPrioBits>4</nvicPrioBits>
  </cpu>
  <peripherals>
    <peripheral>
      <name>GPIOA</name>
      <description>General purpose I/O Port A</description>
      <groupName>GPIO</groupName>
      <baseAddress>0x40010800</baseAddress>
      <interrupt>
        <name>EXTI0</name>
        <value>6</value>
        <description>External Line 0 Interrupt</description>
      </interrupt>
      <registers>
        <register>
          <name>CRL</name>
          <description>Port configuration register low</description>
          <addressOffset>0x00</addressOffset>
          <size>32</size>
          <access>read-write</access>
          <resetValue>0x44444444</resetValue>
          <fields>
            <field>
              <name>MODE0</name>
              <description>Port 0 mode</description>
              <bitOffset>0</bitOffset>
              <bitWidth>2</bitWidth>
            </field>
            <field>
              <name>CNF0</name>
              <description>Port 0 configuration</description>
              <bitOffset>2</bitOffset>
              <bitWidth>2</bitWidth>
            </field>
          </fields>
        </register>
        <register>
          <name>ODR</name>
          <description>Port output data register</description>
          <addressOffset>0x0C</addressOffset>
          <size>32</size>
          <access>read-write</access>
          <resetValue>0x00000000</resetValue>
          <fields>
            <field>
              <name>ODR0</name>
              <description>Port output 0</description>
              <bitOffset>0</bitOffset>
              <bitWidth>1</bitWidth>
            </field>
          </fields>
        </register>
        <register>
          <name>BSRR</name>
          <description>Port bit set/reset register</description>
          <addressOffset>0x10</addressOffset>
          <size>32</size>
          <access>write-only</access>
          <resetValue>0x00000000</resetValue>
          <fields>
            <field>
              <name>BS0</name>
              <description>Set bit 0</description>
              <bitOffset>0</bitOffset>
              <bitWidth>1</bitWidth>
            </field>
            <field>
              <name>BR0</name>
              <description>Reset bit 0</description>
              <bitOffset>16</bitOffset>
              <bitWidth>1</bitWidth>
            </field>
          </fields>
        </register>
      </registers>
    </peripheral>
    <peripheral>
      <name>USART1</name>
      <description>Universal synchronous asynchronous receiver transmitter</description>
      <groupName>USART</groupName>
      <baseAddress>0x40013800</baseAddress>
      <interrupt>
        <name>USART1</name>
        <value>37</value>
        <description>USART1 global interrupt</description>
      </interrupt>
      <registers>
        <register>
          <name>SR</name>
          <description>Status register</description>
          <addressOffset>0x00</addressOffset>
          <size>32</size>
          <access>read-write</access>
          <resetValue>0x00C00000</resetValue>
          <fields>
            <field>
              <name>RXNE</name>
              <description>Read data register not empty</description>
              <bitOffset>5</bitOffset>
              <bitWidth>1</bitWidth>
            </field>
            <field>
              <name>TC</name>
              <description>Transmission complete</description>
              <bitOffset>6</bitOffset>
              <bitWidth>1</bitWidth>
            </field>
            <field>
              <name>TXE</name>
              <description>Transmit data register empty</description>
              <bitOffset>7</bitOffset>
              <bitWidth>1</bitWidth>
            </field>
          </fields>
        </register>
        <register>
          <name>DR</name>
          <description>Data register</description>
          <addressOffset>0x04</addressOffset>
          <size>32</size>
          <access>read-write</access>
          <resetValue>0x00000000</resetValue>
          <fields>
            <field>
              <name>DR</name>
              <description>Data value</description>
              <bitOffset>0</bitOffset>
              <bitWidth>9</bitWidth>
            </field>
          </fields>
        </register>
      </registers>
    </peripheral>
  </peripherals>
</device>
"#
}

pub fn reference_rp2040_svd() -> &'static str {
    r#"<?xml version="1.0" encoding="utf-8"?>
<device schemaVersion="1.1" xmlns:xs="http://www.w3.org/2001/XMLSchema-instance">
  <name>RP2040</name>
  <version>1.0</version>
  <description>Raspberry Pi RP2040 Dual ARM Cortex-M0+ Microcontroller</description>
  <addressUnitBits>8</addressUnitBits>
  <width>32</width>
  <cpu>
    <name>CM0PLUS</name>
    <revision>r0p1</revision>
    <endian>little</endian>
    <nvicPrioBits>2</nvicPrioBits>
  </cpu>
  <peripherals>
    <peripheral>
      <name>SIO</name>
      <description>Single-cycle IO block</description>
      <groupName>SIO</groupName>
      <baseAddress>0xD0000000</baseAddress>
      <registers>
        <register>
          <name>GPIO_IN</name>
          <description>Input value for GPIO pins</description>
          <addressOffset>0x04</addressOffset>
          <size>32</size>
          <access>read-only</access>
          <resetValue>0x00000000</resetValue>
        </register>
        <register>
          <name>GPIO_OUT</name>
          <description>GPIO output value</description>
          <addressOffset>0x10</addressOffset>
          <size>32</size>
          <access>read-write</access>
          <resetValue>0x00000000</resetValue>
        </register>
        <register>
          <name>GPIO_OUT_SET</name>
          <description>Atomic GPIO output set</description>
          <addressOffset>0x14</addressOffset>
          <size>32</size>
          <access>write-only</access>
          <resetValue>0x00000000</resetValue>
        </register>
        <register>
          <name>GPIO_OUT_CLR</name>
          <description>Atomic GPIO output clear</description>
          <addressOffset>0x18</addressOffset>
          <size>32</size>
          <access>write-only</access>
          <resetValue>0x00000000</resetValue>
        </register>
      </registers>
    </peripheral>
  </peripherals>
</device>
"#
}

pub fn reference_esp32c3_svd() -> &'static str {
    r#"<?xml version="1.0" encoding="utf-8"?>
<device schemaVersion="1.1" xmlns:xs="http://www.w3.org/2001/XMLSchema-instance">
  <name>ESP32C3</name>
  <version>1.0</version>
  <description>Espressif ESP32-C3 Single-Core 32-bit RISC-V Microcontroller</description>
  <addressUnitBits>8</addressUnitBits>
  <width>32</width>
  <cpu>
    <name>RISCV32IMC</name>
    <revision>r1p0</revision>
    <endian>little</endian>
    <nvicPrioBits>4</nvicPrioBits>
  </cpu>
  <peripherals>
    <peripheral>
      <name>GPIO</name>
      <description>General Purpose Input/Output</description>
      <groupName>GPIO</groupName>
      <baseAddress>0x60004000</baseAddress>
      <registers>
        <register>
          <name>OUT</name>
          <description>GPIO output register</description>
          <addressOffset>0x04</addressOffset>
          <size>32</size>
          <access>read-write</access>
          <resetValue>0x00000000</resetValue>
        </register>
        <register>
          <name>ENABLE</name>
          <description>GPIO output enable register</description>
          <addressOffset>0x20</addressOffset>
          <size>32</size>
          <access>read-write</access>
          <resetValue>0x00000000</resetValue>
        </register>
      </registers>
    </peripheral>
  </peripherals>
</device>
"#
}

// ==============================================================================
// 5. Entry point for Tool Runner
// ==============================================================================

pub fn process_svd_file(svd_path: &Path, out_dir: &Path) -> Result<()> {
    let content = if svd_path.to_string_lossy() == "stm32f103" {
        reference_stm32f103_svd().to_string()
    } else if svd_path.to_string_lossy() == "rp2040" {
        reference_rp2040_svd().to_string()
    } else if svd_path.to_string_lossy() == "esp32c3" {
        reference_esp32c3_svd().to_string()
    } else {
        fs::read_to_string(svd_path)
            .map_err(|e| anyhow!("Failed to read SVD file '{}': {}", svd_path.display(), e))?
    };

    let device = parse_svd(&content)?;
    let pac_code = generate_pac(&device);

    fs::create_dir_all(out_dir)?;
    let out_file = out_dir.join(format!("{}_pac.roze", device.name.to_lowercase()));
    fs::write(&out_file, &pac_code)?;

    let readme_content = format!(
        "# 🌹 {} Peripheral Access Crate (PAC)\n\n\
        Auto-generated by `roze-svd` for {}.\n\n\
        - Architecture: `{}`\n\
        - Total Peripherals: {}\n\n\
        ## Usage in Roze\n\n\
        ```roze\n\
        import \"{}_pac\";\n\n\
        func main() {{\n\
            let dp = pac_take_peripherals();\n\
            println(\"Initialized \" + dp.name);\n\
        }}\n\
        ```\n",
        device.name,
        device.description,
        device.cpu_name,
        device.peripherals.len(),
        device.name.to_lowercase()
    );
    fs::write(out_dir.join("README.md"), readme_content)?;

    println!("{} Generated PAC for {} in {}", "✅ [roze-svd]".bright_green().bold(), device.name.bold(), out_file.display());
    println!("   ├── {} (Peripheral Register Definitions)", out_file.file_name().unwrap().to_string_lossy());
    println!("   └── README.md (Documentation)");

    Ok(())
}
