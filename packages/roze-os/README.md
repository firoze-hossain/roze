# 🌹 RozeOS v3: Freestanding 64-Bit Operating System Kernel

**RozeOS v3 ('Titan')** is a bare-metal, 64-bit operating system kernel written in 100% pure Roze (`--target freestanding`). It delivers complete architectural parity with modern Linux and Rust `#![no_std]` operating systems with zero external C/assembly toolchain dependencies.

---

## 🏛️ Kernel Architecture & Subsystems

RozeOS v3 is engineered as an enterprise-grade monolithic modular kernel with 10 primary subsystems:

1. **Multiboot2 & UEFI Bootloader Handover**:
   - Multiboot2 magic verification (`0x36D76289`) and tag stream parser (Cmdline, Bootloader, Memory Map, Framebuffer, ACPI RSDP).
   - UEFI GOP Linear Framebuffer and memory descriptor protocol parser.
2. **4-Level Paging Virtual Memory Manager (VMM)**:
   - Full 4-level PML4 $\to$ PDPT $\to$ PD $\to$ PT page table walks and translation.
   - 4KB standard and 2MB huge page mapping and unmapping with canonical address verification.
   - Hardware TLB invalidation (`invlpg`, `tlb_flush_all`) and Page Fault `#PF` exception handler.
3. **Buddy Physical Frame Allocator**:
   - Binary power-of-two frame allocation (Orders 0..10 from 4KB to 4MB).
   - Recursive block splitting and coalescing with buddy neighbors on free.
   - Low-fragmentation physical frame management.
4. **Kernel Heap Allocator with Cache Alignment**:
   - Hardware cache line alignment (64-byte alignment) and slab caches for power-of-two allocations.
   - Guard page boundaries and heap usage telemetry.
5. **IDT & GDT Interrupt Tables**:
   - 64-bit Long Mode Global Descriptor Table (Kernel Code `0x08`, Kernel Data `0x10`, User Data `0x1B`, User Code `0x23`, TSS `0x28`).
   - Interrupt Descriptor Table with 256 vector gates and `CpuContext` state saving.
6. **SMP Multicore Initialization via APIC / MADT**:
   - ACPI Multiple APIC Description Table (MADT) parser discovering Local APICs and I/O APICs.
   - Inter-Processor Interrupts (INIT, SIPI, Fixed Reschedule) and symmetric multiprocessing core management.
7. **Preemptive Round-Robin Thread Scheduler**:
   - Process Control Blocks (PCB) and Thread Control Blocks (TCB) with multi-level priorities.
   - Preemptive round-robin time-slicing and low-latency voluntary yielding.
8. **PCI Bus Scan & Hardware Enumeration**:
   - PCI configuration space accessor scanning Busses 0..255, Devices 0..31, Functions 0..7.
   - Identification of storage, display, networking, and VirtIO devices.
9. **VirtIO Network & Block Device Drivers**:
   - VirtQueue descriptor rings (`VirtqDesc`, `VirtqAvail`, `VirtqUsed`).
   - VirtIO-Net driver for Ethernet packet TX/RX.
   - VirtIO-Blk driver for 512-byte sector block storage reading and writing.
10. **Interactive Kernel Shell (`roze_sh v3`)**:
    - Console output with ANSI/VGA color support.
    - Interactive commands: `help`, `uname`, `mem`, `ps`, `smp`, `pci`, `lsblk`, `net`, `readblk`, `writeblk`, `ping`, `clear`, `reboot`.
    - In-kernel diagnostic verification engine (`roze_os_run_diagnostics`).
