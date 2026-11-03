# Roze Memory Forensics & Defensive Analysis Harness (M3-T06)

**Roze Memory Forensics** is a production-grade, modular defensive memory analysis toolkit and incident response engine designed for authorized cybersecurity operations on Linux and Windows. It integrates directly with the Roze platform and provides both standalone CLI execution and programmatic forensic auditing APIs.

---

## Key Features

1. **Cross-Platform Process Memory Inspection**:
   - **Linux**: Safe read-only inspection of virtual address space via `/proc/<pid>/maps` and `/proc/<pid>/mem`.
   - **Windows**: Memory region querying and committed memory enumeration via `VirtualQueryEx` and `ReadProcessMemory`.

2. **Offline Memory Dump & Core Analysis**:
   - Parses flat binary memory images and ELF core dumps (`PT_LOAD` segments).
   - Maps virtual addresses to file offsets with strict bounds and size validation.

3. **Memory Permission & W+X Auditing**:
   - Flags `RWX` (simultaneously writable and executable) memory regions.
   - Detects executable stack (`[stack]`) and executable heap (`[heap]`) violations.
   - Detects unbacked anonymous executable memory (injected code / unlinked stagers).

4. **In-Memory Binary Header Recognition**:
   - Identifies in-memory ELF (32/64-bit) headers and section boundaries.
   - Identifies in-memory PE (`MZ` / `PE\0\0`) headers and section tables.
   - Flags unmapped or injected binary headers residing in anonymous/writable space.

5. **Shannon Entropy Analysis**:
   - Calculates information entropy ($0.0$ to $8.0$ bits per byte) across buffers and blocks.
   - Flags executable memory regions exceeding configurable thresholds (default: $7.2$), indicating compressed, encrypted, or packed code.

6. **Byte Pattern & Signature Scanning**:
   - Wildcard hexadecimal pattern matching (e.g., `48 89 ?? ?? c3`).
   - Printable ASCII and UTF-16LE wide string extraction.
   - Optional YARA rule compilation and scanning integration.

7. **Defensive Control-Flow & Disassembly Analysis**:
   - Multi-architecture disassembly integration (Capstone or pure-Python fallback).
   - Identifies suspicious API string references (`VirtualAlloc`, `mprotect`, `WriteProcessMemory`).
   - Flags anomalous control-flow characteristics (high-density branch clusters, stack pivot sequences).

8. **Sandboxed CPU State Emulation**:
   - Pure software x86_64 CPU state emulator (`CpuState`, `SandboxedCpuEmulator`).
   - Safe instruction execution with strict memory bounds and step limits.
   - **Zero host execution**: Never executes untrusted machine instructions natively on the host machine.

---

## Directory Architecture

```
tools/roze_forensics/
├── README.md                      # Comprehensive documentation and usage guide
├── requirements.txt               # Optional dependencies (yara-python, capstone, pytest)
├── roze_forensics/
│   ├── __init__.py                # Package exports
│   ├── config.py                  # Forensics configuration and threshold dataclasses
│   ├── logger.py                  # Structured audit logging with timestamps
│   ├── memory_reader.py           # Linux /proc/pid and Windows API memory readers
│   ├── dump_parser.py             # Offline dump loader and ELF core segment parser
│   ├── entropy.py                 # Shannon entropy calculation & block distribution
│   ├── patterns.py                # Hex pattern with wildcards, strings, YARA engine
│   ├── binary_parser.py           # In-memory PE and ELF header parsing
│   ├── auditor.py                 # Defensive anomaly detection (RWX, unbacked exec)
│   ├── disasm.py                  # Instruction decoding and static classification
│   ├── control_flow.py            # Defensive control-flow anomaly detection
│   ├── emulator.py                # Pure-software sandboxed CPU state emulator
│   ├── reporter.py                # Structured JSON and aligned ASCII table reporter
│   └── cli.py                     # Command-line interface
└── tests/
    ├── __init__.py
    ├── test_entropy.py            # Unit tests for Shannon entropy
    ├── test_patterns.py           # Unit tests for wildcard hex matching & strings
    ├── test_binary_parser.py      # Unit tests for in-memory ELF and PE detection
    ├── test_auditor.py            # Unit tests for RWX and unbacked exec detection
    ├── test_control_flow.py       # Unit tests for control flow anomaly detection
    ├── test_emulator.py           # Unit tests for sandboxed CPU emulation
    └── test_integration.py        # End-to-end synthetic memory dump pipeline test
```

---

## Installation & Setup

The toolkit requires Python 3.8+ and is 100% operational using standard library modules alone.

### Optional Performance Enhancements
To enable native YARA scanning and Capstone disassembly:
```bash
pip install -r tools/roze_forensics/requirements.txt
```

---

## CLI Usage Examples

### 1. Inspect Live Process (Read-Only)
```bash
python3 -m roze_forensics.cli --pid <PID>
```

### 2. Inspect Offline Memory Dump
```bash
python3 -m roze_forensics.cli --dump /path/to/memory.dmp
```

### 3. Scan for Hexadecimal Wildcard Pattern
```bash
python3 -m roze_forensics.cli --pid <PID> --pattern "48 89 ?? ?? c3"
```

### 4. Generate Machine-Readable JSON Report
```bash
python3 -m roze_forensics.cli --pid <PID> --json --output report.json
```

### 5. Append Forensic Audit Log
```bash
python3 -m roze_forensics.cli --pid <PID> --audit-log /var/log/roze_forensics.log
```

---

## Programmatic Python API

```python
from roze_forensics import (
    ProcessMemoryReader,
    MemoryAuditor,
    ForensicsConfig,
    calculate_shannon_entropy,
    SandboxedCpuEmulator,
)

# 1. Enumerate memory regions
with ProcessMemoryReader(pid=1234) as reader:
    regions = reader.enumerate_regions()
    
    # 2. Audit regions for RWX / unbacked executable anomalies
    config = ForensicsConfig(entropy_threshold=7.2)
    auditor = MemoryAuditor(config)
    anomalies = auditor.audit_all_regions(regions, reader=reader)
    
    for anomaly in anomalies:
        print(f"[{anomaly.severity}] {anomaly.anomaly_type} at 0x{anomaly.start_address:x}")

# 3. Safe Sandboxed Emulation (Zero host execution)
emu = SandboxedCpuEmulator(memory_size=4096, base_address=0x400000)
code = b"\xb8\x2a\x00\x00\x00\xc3"  # mov eax, 42; ret
emu.load_bytes(code, target_address=0x400000)
emu.run(max_steps=10)
print(f"Final RAX: {emu.cpu.rax}")
```

---

## Integration with the Roze Language

The Roze language compiler and runtime provides native access to defensive memory forensics via `import forensics` or `import roze::forensics`. See `stdlib/src/forensics.roze` for the pure Roze standard library implementation.
