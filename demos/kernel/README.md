# 🌹 Roze OS - Bootable 64-Bit Multiboot2 Operating System Kernel v2

A production-grade, fully functional, bootable **64-bit x86_64 Operating System Kernel** written entirely in **Roze**.

This milestone demo provides **indisputable proof** that Roze compiles directly to bare-metal machine code capable of bootstrapping hardware from reset with:
- **Zero OS dependencies** (no Linux, Windows, macOS, or POSIX underneath)
- **Zero libc / glibc** (zero runtime library dependencies)
- **Zero mandatory runtime overhead** (true bare-metal execution)

---

## 🌟 Architectural Features

### 1. Dual Bootloader Protocol Support
- **GRUB Multiboot2 Specification**: Standard compliant Multiboot2 headers (`0xE85250D6`). Boots on physical hardware and VMs from CD-ROM/USB ISO images created via `grub-mkrescue`. Returns magic `0x36D76289` (`920085129`).
- **QEMU Direct Multiboot1**: Multiboot1 headers (`0x1BADB002`) allowing direct headless and emulator booting via `qemu-system-x86_64 -kernel main.qemu`. Returns magic `0x2BADB002` (`732803074`).

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

### 3. 4-Level Paging & Virtual Memory Manager (VMM)
- **PML4 -> PDPT -> PD -> PT**: Full 4-level x86_64 hierarchical paging system with 48-bit canonical virtual address decomposition.
- **`vmm_map_page(pml4, vaddr, paddr, flags, ...)`**: Maps 4KB pages with custom permissions (`PAGE_PRESENT`, `PAGE_WRITABLE`, `PAGE_USER`).
- **`vmm_translate(pml4, vaddr) -> int`**: Translates virtual addresses to physical frames.
- **`vmm_get_page_flags(pml4, vaddr) -> int`**: Extracts PTE permission and status bits.
- **TLB Invalidation**: Direct `invlpg` hardware instruction emission.

### 4. Physical Memory Frame Allocator
The kernel manages physical memory frames (4KB / 4096 bytes per page) starting from physical address `0x400000` (4MB):
- **`BitmapAllocator`**: Bitmap-backed frame tracking pool managing 128 frames (512 KB pool) at `0x400000`.
- **`bitmap_alloc_frame(alloc) -> int`**: Locates an available page frame, zeros out all 4096 bytes, marks the frame as allocated, and returns the physical page address.
- **`bitmap_free_frame(alloc, addr) -> bool`**: Releases the allocated frame back to the free pool for immediate reuse.
- **`is_4kb_aligned(addr) -> bool`**: Mathematically verifies 4096-byte alignment.

### 5. Pluggable Kernel Heap Allocator (`KernelHeap`)
- **Configurable Base & Capacity**: 1MB kernel heap pool starting at `0x600000` (6MB physical RAM).
- **Cache-Line Alignment**: Strict 64-byte CPU cache-line and 8-byte word alignment via `kheap_alloc(heap, size, align)`.
- **Memory Integrity**: Automated test pattern write and readback verification.
- **Freestanding Heap Query**: Integration with compiler BSS heap pool metrics (`freestanding_get_heap_capacity()`, `freestanding_get_heap_used()`).

### 6. Hardware Interrupts, IDT & Core Architecture
- **64-Bit GDT & TSS**: Long-mode Code (`0x08`), Data (`0x10`), and Task State Segment descriptors.
- **IDT 256 Vectors**: Interrupt Descriptor Table with 256 vector gates (`0x8E` Ring 0 interrupt gates).
- **CPU Exception Handlers**: Page Fault (#PF 14), General Protection (#GP 13), Double Fault (#DF 8).
- **8259 PIC Driver**: Remapped to hardware IRQs 32..47 (Master 0x20, Slave 0x28).
- **Local APIC**: Hardware MMIO base detection at `0xFEE00000`.
- **Software Interrupt Dispatch**: Verified via simulated Page Fault dispatch telemetry.

### 7. Hardware Device Drivers
- **16550 UART Serial (0x3F8)**: COM1 port running at 115200 baud, 8N1 mode, FIFO enabled, scratch register verified.
- **PS/2 Keyboard Driver**: Scancode Set 1 make/break decoder with ASCII translation.
- **PS/2 Mouse Driver**: 3-byte packet decoder with button states and relative motion tracking.
- **Timers**: 8253/8254 PIT running at 100 Hz (divisor 11931) and HPET 64-bit main counter driver (period 10,000,000 fs).
- **UEFI GOP Linear Framebuffer**: 32-bpp BGRA graphics primitives (rectangles, outlines, Bresenham lines, Midpoint circles) and built-in 8x16 monospace ASCII font rasterizer.

### 8. Interactive Kernel Shell (`roze_sh v2`)
The kernel includes an interactive diagnostic shell supporting:
- **`help`**: Displays list of available kernel commands.
- **`info`**: Displays kernel version, CPU mode, bootloader protocol, and console devices.
- **`paging`**: Dumps 4-level paging status, PML4 base, virtual-to-physical translation, and TLB invalidation.
- **`heap`**: Dumps kernel heap stats (base, capacity, used, available, allocation count).
- **`frames`**: Dumps physical memory frame allocator pool statistics.
- **`idt`**: Dumps IDT gates (256 vectors), PIC remapping, and Local APIC base.
- **`drivers`**: Dumps hardware status of UART, Keyboard, Mouse, PIT, HPET, and GOP framebuffer.
- **`gop`**: Renders test graphical primitives and 8x16 font banner to GOP framebuffer.
- **`echo <msg>`**: Echoes messages to VGA and COM1 UART.
- **`clear`**: Clears VGA console buffer and resets cursor.
- **`exit`**: Cleanly terminates kernel and emulator via `qemu_exit(1)`.

---

## 🚀 Quick Start & Running

### Prerequisites
- `qemu-system-x86_64` (QEMU emulator)
- `gcc` and `ld` (standard toolchain for linking)
- `grub-mkrescue` and `xorriso` (for creating bootable ISOs)

### Building the Kernel
```bash
make build
```
This produces:
- `main`: 64-bit ELF kernel binary for GRUB and real hardware.
- `main.qemu`: 32-bit ELF multiboot image for direct QEMU boot.

### Booting in QEMU (Direct Multiboot Boot)
```bash
make run
```

### Creating & Booting ISO (GRUB Multiboot2 CD-ROM)
```bash
make iso
make run-iso
```

### Running Automated CI Boot Test Suite
```bash
make test
```
Executes headless automated boot tests for both direct Multiboot1 and GRUB Multiboot2 ISO boot in QEMU, asserting zero kernel panics and verifying all 11 shell commands.

---

## 🧪 In-Kernel Verification Results

When booted, the kernel executes an automated diagnostic test suite verifying:
- [x] Multiboot handshake identifier (`0x36D76289` GRUB or `0x2BADB002` QEMU)
- [x] 4KB physical page alignment across all allocated frames
- [x] Direct memory byte pattern write and readback (`ROZEOS` signature)
- [x] Physical frame deallocation and page slot reuse
- [x] 4-level paging virtual memory mapping, translation, and TLB invalidation
- [x] Pluggable kernel heap allocation, 64-byte cache alignment, and read/write integrity
- [x] IDT 256 vector gates and exception dispatch telemetry
- [x] 8259 PIC remapping (32..47) and Local APIC base detection
- [x] 16550 UART serial COM1 driver at 115200 8N1
- [x] PS/2 keyboard & mouse interrupt decoding
- [x] PIT & HPET high-resolution timers
- [x] UEFI GOP linear framebuffer primitives and 8x16 font rendering
- [x] Interactive kernel shell (`roze_sh v2`) execution across all 11 commands
- [x] Clean exit with status code 3 (`(1 << 1) | 1`) and zero panics
