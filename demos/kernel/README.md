# 🌹 Roze OS - Bootable 64-Bit Multiboot2 Operating System Kernel

A minimal, fully functional, bootable **64-bit x86_64 Operating System Kernel** written in **Roze**.

This showcase demo provides **indisputable proof** that Roze compiles directly to bare-metal machine code capable of bootstrapping hardware from reset with:
- **Zero OS dependencies** (no Linux, Windows, macOS, or POSIX underneath)
- **Zero libc / glibc** (zero runtime library dependencies)
- **Zero mandatory runtime overhead** (true bare-metal execution)

---

## 🌟 Architectural Features

### 1. Dual Bootloader Protocol Support
- **GRUB Multiboot2 Specification**: Standard compliant Multiboot2 headers (`0xE85250D6`). Boots on physical hardware and VMs from CD-ROM/USB ISO images created via `grub-mkrescue`. Returns magic `0x36D76289`.
- **QEMU Direct Multiboot1**: Multiboot1 headers (`0x1BADB002`) allowing direct headless and emulator booting via `qemu-system-x86_64 -kernel main.qemu`. Returns magic `0x2BADB002`.

### 2. 32-Bit to 64-Bit Long Mode Bootstrap Trampoline
Bootloaders hand off control to the kernel at physical address `0x100000` (1MB) in **32-bit Protected Mode** with paging disabled. The Roze freestanding runtime includes a bootstrap trampoline that:
1. Captures the bootloader magic (`%eax`) and Multiboot tag pointers (`%ebx`).
2. Configures a 64KB kernel stack.
3. Builds a 4-level page table setup (`PML4` -> `PDPT` -> `PD`) identity-mapping the entire first **1GB of physical RAM** using **2MB huge pages** (`0x83` bitmask).
4. Enables Physical Address Extension (`CR4.PAE = 1`).
5. Loads `CR3` with the `PML4` physical table base.
6. Enables Long Mode via the Extended Feature Enable Register (`EFER.LME = 1`).
7. Activates Paging and Protected Mode (`CR0.PG = 1`, `CR0.PE = 1`).
8. Loads a 64-bit Global Descriptor Table (`GDT64`) with 64-bit Code (`0x00209A0000000000`) and Data (`0x0000920000000000`) descriptors.
9. Performs a far jump (`ljmp $0x08, $entry64`) into genuine **64-bit Long Mode**.
10. Clears data segment registers (`DS`, `ES`, `FS`, `GS`, `SS`) and transfers execution to `__roze_main()`.

### 3. Physical Memory Frame Allocator
The kernel manages physical memory frames (4KB / 4096 bytes per page) starting from physical address `0x200000` (2MB):
- **`frame_allocator_init(total_frames)`**: Initializes page frame bitmap table.
- **`frame_alloc(total_frames) -> int`**: Locates an available page frame, zeros out all 4096 bytes, marks the frame as allocated, and returns the physical page address.
- **`frame_free(addr) -> bool`**: Releases the allocated frame back to the free pool for immediate reuse.
- **`check_4kb_alignment(addr) -> bool`**: Mathematically verifies `(addr / 4096) * 4096 == addr`.

### 4. Direct Hardware I/O & Dual Output
- **Serial COM1 UART (0x3F8)**: Configured at 115200 baud, 8 data bits, no parity, 1 stop bit (8N1). Allows headless automated testing, CI assertions, and serial debug logging.
- **VGA Text Mode Buffer (0xB8000)**: Color text mode console (80 columns x 25 rows) rendering status banners and diagnostic badges.
- **Dual Output Mirroring**: All `println` calls automatically mirror to both the VGA screen and COM1 serial.
- **QEMU Clean Exit Port (0xF4)**: Invokes `qemu_exit(1)` via `isa-debug-exit` for instantaneous automated test execution.

---

## 🚀 Quick Start & Running

### Prerequisites
- `qemu-system-x86_64` (QEMU emulator)
- `gcc` and `ld` (standard toolchain for linking)
- `grub-mkrescue` and `xorriso` (optional, for creating bootable ISOs)

### Building the Kernel
```bash
make build
```
This produces:
- `main`: 64-bit ELF kernel binary for GRUB and real hardware.
- `main.qemu`: 32-bit ELF multiboot image for direct QEMU boot.

### Booting in QEMU (Direct Boot)
```bash
make run
```

### Creating & Booting ISO (GRUB Multiboot2)
```bash
make iso
make run-iso
```

---

## 🧪 In-Kernel Verification Results

When booted, the kernel executes an automated diagnostic test suite verifying:
- [x] Multiboot handshake identifier (`0x36D76289` or `0x2BADB002`)
- [x] 4KB physical page alignment across all allocated frames
- [x] Direct memory byte pattern write and readback (`ROZEOS` signature)
- [x] Physical frame deallocation and page slot reuse
- [x] Clean exit with status code 3 (`(1 << 1) | 1`)
