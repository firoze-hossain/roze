#!/usr/bin/env python3
"""
Generates the official Roze Master 6-Month Project Plan Excel spreadsheet:
- ROZE_PROJECT_PLAN.xlsx (workspace root)
- docs/ROZE_PROJECT_PLAN.xlsx (documentation directory)

Uses pure Python standard library (zipfile, os) with OpenXML standards.
No third-party packages (openpyxl, pandas, pip) required.
"""

import zipfile
import os

def escape_xml(s):
    if s is None:
        return ""
    return str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;").replace("'", "&apos;")

# Content types
content_types = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet3.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet4.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet5.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
</Types>"""

# Package rels
pkg_rels = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"""

# Workbook rels
wb_rels = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet3.xml"/>
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet4.xml"/>
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet5.xml"/>
  <Relationship Id="rId6" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"""

# Workbook XML
wb_xml = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="1. 6-Month Master Plan" sheetId="1" r:id="rId1"/>
    <sheet name="2. StdLib Architecture" sheetId="2" r:id="rId2"/>
    <sheet name="3. 7-Domain Matrix" sheetId="3" r:id="rId3"/>
    <sheet name="4. Self-Hosting Roadmap" sheetId="4" r:id="rId4"/>
    <sheet name="5. Sprint Schedule &amp; Gates" sheetId="5" r:id="rId5"/>
  </sheets>
</workbook>"""

# Styles XML
styles_xml = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="16">
    <font><sz val="9.5"/><name val="Segoe UI"/></font> <!-- 0: Normal -->
    <font><b/><sz val="10.5"/><color rgb="FFFFFFFF"/><name val="Segoe UI"/></font> <!-- 1: Header white -->
    <font><b/><sz val="13"/><color rgb="FF1F4E79"/><name val="Segoe UI"/></font> <!-- 2: Title blue -->
    <font><b/><sz val="9.5"/><color rgb="FF1F4E79"/><name val="Segoe UI"/></font> <!-- 3: Month 1 Blue -->
    <font><b/><sz val="9.5"/><color rgb="FF0F5132"/><name val="Segoe UI"/></font> <!-- 4: Month 2 Teal -->
    <font><b/><sz val="9.5"/><color rgb="FF7F6000"/><name val="Segoe UI"/></font> <!-- 5: Month 3 Amber -->
    <font><b/><sz val="9.5"/><color rgb="FF4C1130"/><name val="Segoe UI"/></font> <!-- 6: Month 4 Purple -->
    <font><b/><sz val="9.5"/><color rgb="FF0B5394"/><name val="Segoe UI"/></font> <!-- 7: Month 5 Cyan -->
    <font><b/><sz val="9.5"/><color rgb="FF006100"/><name val="Segoe UI"/></font> <!-- 8: Month 6 Emerald -->
    <font><b/><sz val="9.5"/><color rgb="FF383D41"/><name val="Segoe UI"/></font> <!-- 9: Planned Gray -->
    <font><b/><sz val="9.5"/><color rgb="FF664D03"/><name val="Segoe UI"/></font> <!-- 10: In Progress Gold -->
    <font><b/><sz val="9.5"/><color rgb="FF842029"/><name val="Segoe UI"/></font> <!-- 11: Critical Red -->
    <font><b/><sz val="9.5"/><color rgb="FF783F04"/><name val="Segoe UI"/></font> <!-- 12: Gate Orange -->
    <font><b/><sz val="9.5"/><color rgb="FF274E13"/><name val="Segoe UI"/></font> <!-- 13: Completed Green -->
    <font><b/><sz val="10"/><color rgb="FF1B365D"/><name val="Segoe UI"/></font> <!-- 14: Subheader Dark Navy -->
    <font><sz val="9.5"/><color rgb="FF555555"/><name val="Segoe UI"/></font> <!-- 15: Muted text -->
  </fonts>
  <fills count="16">
    <fill><patternFill patternType="none"/></fill>
    <fill><patternFill patternType="gray125"/></fill>
    <fill><patternFill patternType="solid"><fgColor rgb="FF1B365D"/></patternFill></fill> <!-- 2: Header Deep Navy -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFD9E1F2"/></patternFill></fill> <!-- 3: M1 Soft Blue -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFD1E7DD"/></patternFill></fill> <!-- 4: M2 Soft Teal -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFFFF2CC"/></patternFill></fill> <!-- 5: M3 Soft Amber -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFEAD1DC"/></patternFill></fill> <!-- 6: M4 Soft Purple -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFCFE2F3"/></patternFill></fill> <!-- 7: M5 Soft Cyan -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFC6EFCE"/></patternFill></fill> <!-- 8: M6 Soft Emerald -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFE2E3E5"/></patternFill></fill> <!-- 9: Planned Soft Gray -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFFFF3CD"/></patternFill></fill> <!-- 10: In Progress Soft Yellow -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFF8D7DA"/></patternFill></fill> <!-- 11: Critical Soft Red -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFFCE5CD"/></patternFill></fill> <!-- 12: Gate Soft Orange -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFF8FAFC"/></patternFill></fill> <!-- 13: Light Row Alt -->
    <fill><patternFill patternType="solid"><fgColor rgb="FF0F5132"/></patternFill></fill> <!-- 14: Header Teal -->
    <fill><patternFill patternType="solid"><fgColor rgb="FF7F6000"/></patternFill></fill> <!-- 15: Header Amber -->
  </fills>
  <borders count="2">
    <border><left/><right/><top/><bottom/><diagonal/></border>
    <border>
      <left style="thin"><color rgb="FFD9D9D9"/></left>
      <right style="thin"><color rgb="FFD9D9D9"/></right>
      <top style="thin"><color rgb="FFD9D9D9"/></top>
      <bottom style="thin"><color rgb="FFD9D9D9"/></bottom>
      <diagonal/>
    </border>
  </borders>
  <cellStyleXfs count="1">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>
  </cellStyleXfs>
  <cellXfs count="22">
    <!-- 0: Normal data left-top, wrap -->
    <xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0"><alignment vertical="top" wrapText="1"/></xf>
    <!-- 1: Header deep navy center -->
    <xf numFmtId="0" fontId="1" fillId="2" borderId="1" xfId="0"><alignment horizontal="center" vertical="center" wrapText="1"/></xf>
    <!-- 2: Title blue -->
    <xf numFmtId="0" fontId="2" fillId="0" borderId="0" xfId="0"><alignment vertical="center"/></xf>
    <!-- 3: M1 Blue Tag -->
    <xf numFmtId="0" fontId="3" fillId="3" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 4: M2 Teal Tag -->
    <xf numFmtId="0" fontId="4" fillId="4" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 5: M3 Amber Tag -->
    <xf numFmtId="0" fontId="5" fillId="5" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 6: M4 Purple Tag -->
    <xf numFmtId="0" fontId="6" fillId="6" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 7: M5 Cyan Tag -->
    <xf numFmtId="0" fontId="7" fillId="7" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 8: Center Normal -->
    <xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 9: Alt row normal data -->
    <xf numFmtId="0" fontId="0" fillId="13" borderId="1" xfId="0"><alignment vertical="top" wrapText="1"/></xf>
    <!-- 10: Center Normal Alt -->
    <xf numFmtId="0" fontId="0" fillId="13" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 11: M6 Emerald Tag -->
    <xf numFmtId="0" fontId="8" fillId="8" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 12: Planned Gray Tag -->
    <xf numFmtId="0" fontId="9" fillId="9" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 13: In Progress Gold Tag -->
    <xf numFmtId="0" fontId="10" fillId="10" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 14: Critical Red Tag -->
    <xf numFmtId="0" fontId="11" fillId="11" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 15: Gate Orange Tag -->
    <xf numFmtId="0" fontId="12" fillId="12" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 16: Completed Green Tag -->
    <xf numFmtId="0" fontId="13" fillId="4" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 17: Header Teal center -->
    <xf numFmtId="0" fontId="1" fillId="14" borderId="1" xfId="0"><alignment horizontal="center" vertical="center" wrapText="1"/></xf>
    <!-- 18: Header Amber center -->
    <xf numFmtId="0" fontId="1" fillId="15" borderId="1" xfId="0"><alignment horizontal="center" vertical="center" wrapText="1"/></xf>
    <!-- 19: Bold Section Header -->
    <xf numFmtId="0" fontId="14" fillId="3" borderId="1" xfId="0"><alignment horizontal="left" vertical="center"/></xf>
    <!-- 20: Code font normal -->
    <xf numFmtId="0" fontId="15" fillId="0" borderId="1" xfId="0"><alignment vertical="top" wrapText="1"/></xf>
    <!-- 21: Code font alt -->
    <xf numFmtId="0" fontId="15" fillId="13" borderId="1" xfId="0"><alignment vertical="top" wrapText="1"/></xf>
  </cellXfs>
</styleSheet>"""

def col_to_letter(col_idx):
    result = ""
    while col_idx > 0:
        col_idx, remainder = divmod(col_idx - 1, 26)
        result = chr(65 + remainder) + result
    return result

def make_worksheet(headers, rows, col_widths, freeze_row=1):
    ws = ['<?xml version="1.0" encoding="UTF-8" standalone="yes"?>']
    ws.append('<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">')
    ws.append('  <sheetViews>')
    ws.append('    <sheetView tabSelected="1" workbookViewId="0">')
    if freeze_row > 0:
        ws.append(f'      <pane ySplit="{freeze_row}" topLeftCell="A{freeze_row + 1}" activePane="bottomLeft" state="frozen"/>')
    ws.append('    </sheetView>')
    ws.append('  </sheetViews>')
    ws.append('  <cols>')
    for i, w in enumerate(col_widths, start=1):
        ws.append(f'    <col min="{i}" max="{i}" width="{w}" customWidth="1"/>')
    ws.append('  </cols>')
    ws.append('  <sheetData>')
    
    # Header row
    ws.append('    <row r="1" ht="32" customHeight="1">')
    for col_idx, h in enumerate(headers, start=1):
        cell_ref = f"{col_to_letter(col_idx)}1"
        ws.append(f'      <c r="{cell_ref}" t="inlineStr" s="1"><is><t>{escape_xml(h)}</t></is></c>')
    ws.append('    </row>')
    
    # Data rows
    for row_idx, r in enumerate(rows, start=2):
        is_alt = (row_idx % 2 == 1)
        ws.append(f'    <row r="{row_idx}" ht="50" customHeight="1">')
        for col_idx, cell_data in enumerate(r, start=1):
            cell_ref = f"{col_to_letter(col_idx)}{row_idx}"
            if isinstance(cell_data, tuple):
                val, style_idx = cell_data
            else:
                val = cell_data
                style_idx = 9 if is_alt else 0
            ws.append(f'      <c r="{cell_ref}" t="inlineStr" s="{style_idx}"><is><t>{escape_xml(val)}</t></is></c>')
        ws.append('    </row>')
        
    ws.append('  </sheetData>')
    ws.append('</worksheet>')
    return "".join(ws)

# ==================== DATA FOR SHEET 1: 6-MONTH MASTER PLAN ====================
s1_headers = [
    "Task ID", "Month", "Sprint & Weeks", "Domain / Epic", "Feature & Capability",
    "Detailed Technical Implementation", "Java 25 & Rust 1.99.1 Parity Deliverables",
    "Target Domains Unlocked", "Est. Days", "Priority", "Status"
]
s1_widths = [12, 14, 18, 24, 30, 48, 44, 28, 12, 14, 14]

s1_rows = [
    # MONTH 1: ADVANCED TYPE SYSTEM, CONCURRENCY & SELF-HOSTING PREP
    [
        ("M1-T01", 8), ("Month 1", 3), ("Sprint 1 (W01-W02)", 8), "Core Type System", "Advanced Generics & Trait Bounds",
        "Implement monomorphization engine and trait bounds syntax (<T: Display + Clone>). Add associated types ('trait Iterator { type Item; }') and higher-kinded polymorphism ergonomics.",
        "Matches Rust trait bounds and Java bounded type parameters (<T extends Comparable<T>>); enables universal generic collections.",
        "All Domains, Core Stdlib, Compiler", "7 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M1-T02", 8), ("Month 1", 3), ("Sprint 1 (W01-W02)", 8), "Pattern Matching", "Deep Destructuring & Match Guards",
        "Exhaustive pattern matching compiler for nested enums, struct fields, tuples, slice patterns ([head, ..tail]), range patterns (1..=10), and 'if' match guards.",
        "Parity with Rust 1.99 pattern matching and Java 25 Pattern Matching for switch and Record Patterns.",
        "Compiler, Web Framework, Security", "7 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M1-T03", 8), ("Month 1", 3), ("Sprint 1 (W01-W02)", 8), "Metaprogramming", "Compile-Time Function Execution (comptime)",
        "Add 'comptime' expression evaluator in compiler semantic pass. Evaluate constant math, generate lookup tables, and compute type layouts during compilation.",
        "Matches Zig comptime and Rust const fn capabilities; eliminates runtime overhead for static structures.",
        "Games, OS Kernels, AI/ML Tensors", "6 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M1-T04", 8), ("Month 1", 3), ("Sprint 2 (W03-W04)", 8), "Modern Concurrency", "M:N Green Threads (Virtual Threads)",
        "Implement cooperative user-space green thread scheduler with work-stealing threadpool across OS worker threads. Zero allocation fiber context switching.",
        "Parity with Java 25 Project Loom Virtual Threads and Go goroutines. Supports 1,000,000 simultaneous concurrent tasks.",
        "Enterprise Web, Game Servers, Tools", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M1-T05", 8), ("Month 1", 3), ("Sprint 2 (W03-W04)", 8), "Async Runtime", "Async / Await State-Machine Engine",
        "Lower 'async func' and 'await' expressions to zero-allocation state machine generator structs. Native Future<T> polling contract with reactor integration.",
        "Full parity with Rust async/await and modern Java CompletableFuture / Structured Concurrency.",
        "Web Microservices, Network I/O", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M1-T06", 8), ("Month 1", 3), ("Sprint 2 (W03-W04)", 8), "Synchronization", "Lock-Free Primitives & Channels",
        "Atomic types (AtomicBool, AtomicI64, AtomicPtr, AtomicUsize). Implement high-throughput MPMC bounded and unbounded ring-buffer channels, Mutex, and RwLock.",
        "Matches Rust std::sync and Java java.util.concurrent.atomic / locks.",
        "Enterprise, Game Engines, Systems", "6 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M1-T07", 8), ("Month 1", 3), ("Sprint 2 (W03-W04)", 8), "Structured Concurrency", "Nursery, Task Groups & Cancelation",
        "Nursery structured concurrency scope: parent task automatically awaits all spawned child tasks. Unified cancelation tokens and timeout triggers.",
        "Parity with Java 25 JEP 453 Structured Concurrency and Rust nursery runtimes.",
        "Cloud Microservices, Distributed Systems", "5 Days", ("Medium", 14), ("Completed", 16)
    ],
    [
        ("M1-T08", 8), ("Month 1", 3), ("Sprint 2 (W03-W04)", 8), "Milestone Gate M1", "Month 1 Integration & Concurrency Benchmark",
        "Verify 100,000 concurrent green thread ping-pong message test over MPMC channels. Monomorphized generic data structures stress test suite.",
        "100% test pass on all concurrency and advanced type fixtures.",
        "All Domains", "3 Days", ("Critical", 14), ("Completed", 16)
    ],

    # MONTH 2: BATTERIES-INCLUDED SYSTEMS CORE & FREESTANDING OS KERNEL
    [
        ("M2-T01", 8), ("Month 2", 4), ("Sprint 3 (W05-W06)", 8), "Core Types", "Full Fixed-Width Numeric Suite",
        "Introduce complete primitive numeric types: i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, f32, f64, isize, usize, and char with wrapping/saturating arithmetic.",
        "Complete parity with Rust numeric primitives and Java 25 wrapper classes with unsigned support.",
        "OS, Systems, Security, AI/ML, Games", "5 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M2-T02", 8), ("Month 2", 4), ("Sprint 3 (W05-W06)", 8), "Hardware Acceleration", "SIMD Vector Intrinsics Engine",
        "Hardware SIMD types: Vec4f, Vec8f, Vec4i, Vec16u8 with AVX-512, AVX2, and ARM NEON code generation for 8x-16x vector processing acceleration.",
        "Matches Java 25 Vector API (incubator) and Rust core::simd.",
        "AI/ML Tensors, Game Physics, Video/Audio", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M2-T03", 8), ("Month 2", 4), ("Sprint 3 (W05-W06)", 8), "Standard Collections", "Advanced High-Performance Collections",
        "Implement VecDeque<T>, BTreeMap<K, V>, BTreeSet<T>, BinaryHeap<T> (priority queue), IndexMap<K, V>, RingBuffer<T>, and BitSet with amortized O(1) ops.",
        "Parity with Rust std::collections and Java 25 java.util collections hierarchy.",
        "All Domains, Databases, Frameworks", "8 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M2-T04", 8), ("Month 2", 4), ("Sprint 3 (W05-W06)", 8), "Memory Architecture", "Pluggable Allocator API & Smart Pointers",
        "Arena Allocator, Bump Allocator, Fixed-Size Block Pool Allocator, Box<T>, Arc<T>, and Weak<T> with cyclic reference detection and cache alignment.",
        "Parity with Rust Allocator API and modern C++ polymorphic allocators (PMR).",
        "OS Development, High-Frequency Trading, Games", "6 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M2-T05", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Freestanding Systems", "Bare-Metal Kernel Target (--target freestanding)",
        "Zero-libc, zero-heap bare metal target. Custom linker script integration, freestanding runtime entry (_start), and disabling float registers for ISR.",
        "Parity with Rust #![no_std] and Zig freestanding target; produces standalone bootable binaries.",
        "Operating Systems, Microcontrollers", "7 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M2-T06", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Kernel Memory", "4-Level Paging & Virtual Memory Manager",
        "PML4 -> PDPT -> PD -> PT page table manipulator. 4KB and 2MB huge page mapping, physical frame allocator (Bitmap + Buddy allocator), and TLB invalidation.",
        "Full OS memory management stack written entirely in Roze.",
        "Operating System Development", "8 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M2-T07", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Kernel Core", "Hardware Interrupts (GDT, IDT, APIC)",
        "Global Descriptor Table, Interrupt Descriptor Table with 256 vector stubs, Exception handlers (Page Fault #PF, GPF #GP), and Local APIC / 8259 PIC driver.",
        "Production kernel interrupt handling with zero host OS dependencies.",
        "Operating System Development", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M2-T08", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Hardware Drivers", "UART, Keyboard, Timer & GOP Framebuffer",
        "16550 UART serial driver, PS/2 keyboard/mouse interrupt driver, PIT/HPET high-res timer, and UEFI GOP linear framebuffer graphics.",
        "Hardware I/O driver library in pure Roze.",
        "Operating Systems, Embedded Firmware", "6 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M2-T09", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Inline Assembly", "Native asm! Macro & Register Constraints",
        "Inline assembly syntax: asm! { 'out dx, al', in('dx') port, in('al') val, options(nomem, nostack) } with x86_64 and ARM64 register constraints.",
        "Parity with Rust core::arch::asm! and GCC inline assembly.",
        "Operating Systems, Security Exploits", "6 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M2-T10", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Milestone Gate M2", "Month 2 Bootable OS Kernel Verification",
        "Boot RozeOS Kernel v2 in QEMU via GRUB/Multiboot2: initialize paging, heap allocator, IDT, print to VGA/GOP screen, and execute interactive shell.",
        "QEMU x86_64 automated CI boot test with zero kernel panics.",
        "Operating Systems & Freestanding", "3 Days", ("Critical", 14), ("Milestone Gate", 15)
    ],

    # MONTH 3: CYBERSECURITY / HACKING SUITE & ENTERPRISE WEB FRAMEWORK
    [
        ("M3-T01", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Raw Networking", "Raw Sockets & Promiscuous Sniffing",
        "AF_PACKET / BPF raw socket engine in roze::net::raw. Binds directly to network interfaces in promiscuous mode with zero-copy packet ring buffer.",
        "Parity with Python Scapy and Go gopacket; native raw socket capture without external dependencies.",
        "Cybersecurity, Hacking, Pen-Testing", "6 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M3-T02", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Packet Crafting", "Protocol Builder & Packet Dissector",
        "Packet crafting and dissection engine for Ethernet, ARP, IPv4, IPv6, ICMP, TCP (custom flags/SYN/ACK), UDP, DNS, and DHCP. Automated checksum calculation.",
        "Full packet injection and inspection framework.",
        "Cybersecurity, Network Engineering", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M3-T03", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Security Scanning", "Ultra-Fast SYN Port & Banner Scanner",
        "Asynchronous SYN stealth scanner utilizing green threads. Scans 100,000+ ports per second, captures service banners, and identifies OS fingerprints.",
        "Matches Nmap SYN scan performance in pure Roze.",
        "Cybersecurity, Reconnaissance Tools", "6 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M3-T04", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Standard Cryptography", "Full Crypto Suite & Pure TLS 1.3",
        "roze::crypto module: AES-128/256-GCM, ChaCha20-Poly1305, SHA-256, SHA-512, SHA-3/Keccak, HMAC, Argon2id, Ed25519, Curve25519 (X25519), RSA-4096, and TLS 1.3 handshake.",
        "Matches Java Cryptography Architecture (JCA) and Rust ring/rustls; zero OpenSSL dependency.",
        "Security, Web Backends, Authentication", "9 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M3-T05", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Binary Analysis", "ELF / PE / Mach-O Disassembler Engine",
        "Binary parsing and disassembly engine: parse headers, sections, export/import tables, demangle symbols, and disassemble x86_64 and ARM64 machine opcodes.",
        "Matches capabilities of radare2 and Capstone disassembler.",
        "Reverse Engineering, Malware Analysis", "8 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M3-T06", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Exploit Research", "Memory Forensics & Shellcode Harness",
        "Cross-platform process memory inspection (/proc/<pid>/mem & VirtualAllocEx), ROP gadget scanner, memory pattern searcher, and shellcode execution sandbox.",
        "Standard penetration testing exploit harness in Roze.",
        "Cybersecurity, Vulnerability Research", "7 Days", ("Medium", 14), ("Planned", 12)
    ],
    [
        ("M3-T07", 8), ("Month 3", 5), ("Sprint 6 (W11-W12)", 8), "Enterprise Web Core", "Asynchronous HTTP/1.1, HTTP/2 & HTTP/3",
        "Multi-core non-blocking event-driven web server powered by epoll/kqueue/IOCP. Zero-copy header parsing, HTTP/2 multiplexed streams, and HTTP/3 QUIC support.",
        "Performance beats Node.js/Go; on par with Rust Actix-web and Java 25 Netty.",
        "Enterprise Web, Cloud Microservices", "8 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M3-T08", 8), ("Month 3", 5), ("Sprint 6 (W11-W12)", 8), "Enterprise Framework", "RozeBoot 2.0 Annotations & IoC Container",
        "Declarative annotations: @RestController, @GetMapping, @PostMapping, @PathVariable, @RequestParam, @RequestBody, @Autowired, @Service, @Component, @Valid. IoC container.",
        "1:1 architectural parity with Spring Boot 3 on Java 25.",
        "Enterprise Cloud Services", "8 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M3-T09", 8), ("Month 3", 5), ("Sprint 6 (W11-W12)", 8), "Data & ORM", "High-Throughput ORM & SQL Query Builder",
        "Native connection pool (HikariCP equivalent), ActiveRecord / Repository entity mapping, type-safe SQL query builder, automatic schema migrations, and transactions.",
        "Matches Spring Data JPA and Rust Diesel/SeaORM.",
        "Enterprise Banking, SaaS Platforms", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M3-T10", 8), ("Month 3", 5), ("Sprint 6 (W11-W12)", 8), "Web Security & Cloud", "JWT, OAuth2, Metrics & OpenAPI 3.1",
        "Built-in JWT auth filter, OAuth2 token validation, Argon2id password hashing, Prometheus /metrics endpoint, OpenTelemetry tracing, and auto-generated Swagger UI docs.",
        "Matches Spring Security and Quarkus observability stack.",
        "Enterprise Backends, Microservices", "6 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M3-T11", 8), ("Month 3", 5), ("Sprint 6 (W11-W12)", 8), "Milestone Gate M3", "Month 3 Web & Security Integration Gate",
        "Deploy RozeBoot REST microservice serving 100,000 req/sec under load test. Execute penetration testing suite verifying raw packet sniffer, SYN scanner, and crypto.",
        "Sub-millisecond P99 latency under 50,000 concurrent web connections.",
        "Web Enterprise & Cybersecurity", "3 Days", ("Critical", 14), ("Milestone Gate", 15)
    ],

    # MONTH 4: NATIVE AI / MACHINE LEARNING & 2D/3D GAME DEV SUITE
    [
        ("M4-T01", 8), ("Month 4", 6), ("Sprint 7 (W13-W14)", 8), "AI Tensor Engine", "N-Dimensional Tensor<T, D> Engine",
        "Multi-dimensional tensor structure supporting arbitrary ranks, strided views, continuous memory, broadcasting, slicing (tensor[0..5, :, 1]), and transposition.",
        "Parity with NumPy ndarray and PyTorch torch.Tensor.",
        "AI/ML, Data Science, Scientific Computing", "8 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M4-T02", 8), ("Month 4", 6), ("Sprint 7 (W13-W14)", 8), "AI Acceleration", "GEMM / BLAS & GPU Compute (CUDA/Metal)",
        "Hardware-accelerated matrix multiplication kernels: AVX-512 FMA vector code generator + direct CUDA PTX / Apple Metal Shading Language GPU kernels.",
        "Matches cuBLAS and Apple Accelerate performance for neural operations.",
        "AI/ML Deep Learning, LLM Inference", "8 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M4-T03", 8), ("Month 4", 6), ("Sprint 7 (W13-W14)", 8), "Deep Learning", "Reverse-Mode Automatic Differentiation (Autograd)",
        "Dynamic computational tape recording forward ops. Automated backward-pass gradient propagation for complex neural networks and loss functions.",
        "Matches PyTorch Autograd and JAX automatic differentiation.",
        "Machine Learning Training & Fine-Tuning", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M4-T04", 8), ("Month 4", 6), ("Sprint 7 (W13-W14)", 8), "Neural Network Layers", "Comprehensive NN Layer Library (roze::nn)",
        "Linear/Dense, Conv1D, Conv2D, MaxPool, LayerNorm, RMSNorm, Dropout, Multi-Head Attention, RoPE (Rotary Position Embeddings), and SwiGLU activations.",
        "Matches PyTorch torch.nn module ecosystem.",
        "AI/ML Computer Vision & NLP", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M4-T05", 8), ("Month 4", 6), ("Sprint 7 (W13-W14)", 8), "Model Formats", "GGUF & ONNX Native Model Loaders",
        "Native binary parser for GGUF (Llama, Mistral, Qwen) and ONNX files. Memory-mapped weight loading (mmap) with Q4_K_M, Q8_0, and FP16 dequantization.",
        "Zero C++ dependency loader matching llama.cpp format.",
        "Local AI Inference, Edge AI", "7 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M4-T06", 8), ("Month 4", 6), ("Sprint 7 (W13-W14)", 8), "LLM Inference", "Local LLM Inference Engine & KV-Cache",
        "Complete Transformer decoder pipeline in pure Roze: BPE tokenizer, rotary embeddings, KV-cache, top-p/top-k sampling, and streaming token generation.",
        "Run LLaMA 3 8B locally in Roze with 45+ tokens/sec on Apple Silicon / CUDA.",
        "AI Applications, Local Copilots", "8 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M4-T07", 8), ("Month 4", 6), ("Sprint 8 (W15-W16)", 8), "Game Math", "Complete 2D/3D Game Math Suite",
        "Vec2, Vec3, Vec4, Mat3x3, Mat4x4, Quaternion, Transform, Ray, Plane, AABB, and Frustum with SIMD-accelerated linear algebra operations.",
        "Parity with Rust glam/nalgebra and C++ GLM.",
        "Game Development, Physics Simulation, VFX", "6 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M4-T08", 8), ("Month 4", 6), ("Sprint 8 (W15-W16)", 8), "Graphics HAL", "Cross-Platform Graphics Hardware Abstraction",
        "Unified rendering backend: Vulkan 1.3, Apple Metal, DirectX 12, WebGPU, plus Raylib & SDL3 FFI bindings. Pipeline state, shaders, uniform buffers, textures.",
        "Matches Rust wgpu and C++ bgfx cross-platform graphics reach.",
        "2D/3D Games, CAD, Visual Applications", "9 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M4-T09", 8), ("Month 4", 6), ("Sprint 8 (W15-W16)", 8), "Game Architecture", "Archetype-Based Entity Component System (ECS)",
        "High-performance ECS engine: contiguous component chunk storage (archetypes), query filters (With<T>, Without<U>), and multi-threaded system parallelization.",
        "Matches Rust Bevy ECS and C++ Flecs performance.",
        "Game Engines, Massive Simulations", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M4-T10", 8), ("Month 4", 6), ("Sprint 8 (W15-W16)", 8), "Game Audio & Physics", "3D Spatial Audio & Rigid Body Physics Engine",
        "Audio mixer with 3D spatial listener; decoders for WAV, OGG, MP3. Real-time 2D/3D physics: AABB/OBB, GJK/EPA narrowphase, impulse solver, and Raycast.",
        "Complete indie game runtime stack in pure Roze.",
        "Game Development, Interactive Media", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M4-T11", 8), ("Month 4", 6), ("Sprint 8 (W15-W16)", 8), "Milestone Gate M4", "Month 4 AI & Game Platform Verification",
        "Run LLaMA 3 token generation streaming demo with zero external C++ code. Launch 60 FPS 3D game demo with ECS, lighting, shadows, physics, and audio.",
        "Stable 60 FPS rendering and validated local LLM generation.",
        "AI/ML & Game Development", "3 Days", ("Critical", 14), ("Milestone Gate", 15)
    ],

    # MONTH 5: DESKTOP GUI & STANDARD DEVELOPER PRODUCTIVITY SUITE
    [
        ("M5-T01", 8), ("Month 5", 7), ("Sprint 9 (W17-W18)", 8), "Desktop Windowing", "Cross-Platform Native Windowing Engine",
        "Event loop, multi-window creation, DPI scaling, mouse/keyboard/gamepad events, clipboard, and drag-and-drop on Windows (Win32), macOS (Cocoa), Linux (Wayland/X11).",
        "Parity with Rust winit and Java AWT/Swing windowing abstractions.",
        "Desktop Applications, Productivity Software", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M5-T02", 8), ("Month 5", 7), ("Sprint 9 (W17-W18)", 8), "Vector 2D Canvas", "GPU-Accelerated 2D Vector Rendering Canvas",
        "Path rasterization, cubic bezier curves, SVG path parser, gradient fills, clipping masks, and subpixel anti-aliased font rendering via FreeType/HarfBuzz.",
        "Matches Skia and Rust vello 2D graphics performance.",
        "Desktop GUI, Data Visualization, Creative Tools", "8 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M5-T03", 8), ("Month 5", 7), ("Sprint 9 (W17-W18)", 8), "Reactive UI Framework", "Declarative Reactive UI ('RozeUI')",
        "Reactive component tree, State<T>, Computed<T>, Effect, two-way data binding, and integrated Flexbox layout engine (Yoga-compatible).",
        "Modern reactive UI matching Flutter, React, and Swift UI.",
        "Desktop Business Apps, Developer Tools", "8 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M5-T04", 8), ("Month 5", 7), ("Sprint 9 (W17-W18)", 8), "Native Widget Kit", "Batteries-Included Desktop Widget Library",
        "Comprehensive widgets: Button, TextInput, TextArea, CheckBox, Dropdown, Slider, DataGrid/Table, TabView, TreeView, SplitPane, Modal Dialogs, Menus.",
        "Matches JavaFX and Qt Widget richness.",
        "Enterprise Desktop Applications", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M5-T05", 8), ("Month 5", 7), ("Sprint 9 (W17-W18)", 8), "Desktop Webview", "Embedded Webview Component (RozeWebview)",
        "Native embedded browser container: WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux. Bidirectional JS-Roze bridge communication.",
        "Matches Tauri and Electron capabilities with 1/10th the memory footprint.",
        "Hybrid Desktop Apps, Admin Dashboards", "6 Days", ("Medium", 14), ("Planned", 12)
    ],
    [
        ("M5-T06", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "Text & Regex", "High-Performance Linear-Time Regex Engine",
        "DFA/NFA regular expression engine with linear O(N) guarantees (no ReDoS catastrophic backtracking), Unicode character classes, capture groups, and lookaheads.",
        "Parity with Rust regex crate and Java java.util.regex.",
        "Text Processing, Compilers, Data Extraction", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M5-T07", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "Date & Time", "Comprehensive Date, Time & Timezone Suite",
        "Instant, Duration, DateTime, TimeZone, ZonedDateTime, IANA timezone database, calendar math (leap seconds, daylight savings), and ISO-8601 parsing.",
        "Complete parity with Java 25 java.time (JSR-310) and Rust chrono.",
        "Enterprise Banking, Scheduling, Databases", "6 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M5-T08", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "Universal Serialization", "Zero-Copy Multi-Format Serialization",
        "High-throughput encoders and decoders for JSON, TOML, YAML, MsgPack, and Protocol Buffers with zero heap allocation deserialization.",
        "Parity with Rust serde and Java Jackson / Google Protobuf.",
        "Microservices, Configs, Game Saves", "7 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M5-T09", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "OS & System Utils", "Filesystem, Pipes, Signals & Process Engine",
        "Cross-platform file walking, symbolic links, file watching, inter-process pipes, POSIX/Windows signals, environment variables, and async process spawning.",
        "Matches Rust std::process / std::fs and Java java.nio.file.",
        "DevOps Tooling, CLI Utilities, Systems", "6 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M5-T10", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "Runtime Metadata", "Reflection & Runtime Type Information (RTTI)",
        "Type inspection metadata: struct field enumeration, method discovery, annotation/attribute extraction, and dynamic invoke.",
        "Matches Java reflection and C# metadata reflection.",
        "Frameworks, Serialization, Testing Tools", "6 Days", ("Medium", 14), ("Planned", 12)
    ],
    [
        ("M5-T11", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "Milestone Gate M5", "Month 5 Desktop GUI & StdLib Integration",
        "Build and launch cross-platform Desktop Dashboard app in RozeUI displaying real-time system stats, data grids, and embedded webview.",
        "Fluid 60 FPS desktop UI with sub-50ms cold startup time.",
        "Desktop GUI & Standard Utilities", "3 Days", ("Critical", 14), ("Milestone Gate", 15)
    ],

    # MONTH 6: 100% PURE ROZE-IN-ROZE SELF-HOSTING COMPILER & PRODUCTION INDEPENDENCE
    [
        ("M6-T01", 8), ("Month 6", 11), ("Sprint 11 (W21-W22)", 8), "Pure Roze Frontend", "Roze-in-Roze Lexer, Parser & Macro Engine",
        "Rewrite entire compiler frontend in 100% pure Roze: token scanner, recursive descent parser, AST structures, and procedural macro expander supporting all language features.",
        "Zero Rust dependency for Roze compiler syntax analysis.",
        "Self-Hosting Compiler, Language Independence", "8 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M6-T02", 8), ("Month 6", 11), ("Sprint 11 (W21-W22)", 8), "Pure Roze Semantic", "Roze-in-Roze Type Checker & Monomorphizer",
        "Rewrite semantic analyzer in pure Roze: scoped symbol tables, type inference, trait resolution, generic monomorphization, and ARC borrow lifetime analyzer.",
        "Zero Rust dependency for type validation and generic code specialization.",
        "Self-Hosting Compiler, Language Independence", "9 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M6-T03", 8), ("Month 6", 11), ("Sprint 11 (W21-W22)", 8), "Pure Roze Optimizer", "Typed SSA IR & Optimization Passes in Roze",
        "Typed SSA Intermediate Representation with Control Flow Graph (CFG) in pure Roze: Dead Code Elimination, Inlining, Loop Invariant Hoisting, and Constant Folding.",
        "Production compiler optimization engine written in pure Roze.",
        "Self-Hosting Compiler, Performance", "9 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M6-T04", 8), ("Month 6", 11), ("Sprint 11 (W21-W22)", 8), "Pure Roze Native Backend", "Standalone Binary Code Generator in Roze",
        "Direct machine code emitter in pure Roze: output ELF (Linux), Mach-O (macOS), and PE/COFF (Windows) object files, or direct Cranelift/LLVM C-API bindings.",
        "Complete decoupling from Rust toolchain; native standalone binaries built by Roze alone.",
        "Self-Hosting Compiler, Standalone Binaries", "10 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M6-T05", 8), ("Month 6", 11), ("Sprint 11 (W21-W22)", 8), "Rust Decommissioning", "Complete Decommissioning of Rust Host Compiler",
        "Decommission Cargo and rustc from Roze repository build pipeline. The Roze binary itself becomes the primary compiler and build driver.",
        "Ultimate language independence achieved: Roze requires only Roze to build Roze.",
        "Self-Hosting, Autonomy", "5 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M6-T06", 8), ("Month 6", 11), ("Sprint 12 (W23-W26)", 8), "Bootstrap Fixpoint", "3-Stage Bootstrap Fixpoint Idempotence Verification",
        "Execute automated bootstrap cycle: Stage 0 (Legacy) -> Stage 1 (Roze) -> Stage 2 (Roze) -> Stage 3 (Roze). Run byte-level diff: 'diff bin/roze_stage2 bin/roze_stage3' == 0.",
        "Mathematically verified bootstrap closure (zero byte difference).",
        "Self-Hosting Compiler", "6 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M6-T07", 8), ("Month 6", 11), ("Sprint 12 (W23-W26)", 8), "Unified Toolchain", "Monolithic 'roze' CLI Toolchain Suite",
        "Unify compiler, roze-build, roze-test, roze-fmt, roze-doc, and roze-repl into a single hyper-fast monolithic executable (roze).",
        "Matches the convenience of 'cargo' and 'go' single-binary toolchain.",
        "Developer Experience, Tooling", "6 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M6-T08", 8), ("Month 6", 11), ("Sprint 12 (W23-W26)", 8), "Package Registry", "Production 'roze-pkg' & Cloud Registry Hub",
        "Deploy production package manager and registry: cryptographic package signing (Ed25519), SAT-based semver dependency solver, lockfiles, and registry server.",
        "Parity with crates.io and npm with built-in security auditing.",
        "Ecosystem Growth, Community", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M6-T09", 8), ("Month 6", 11), ("Sprint 12 (W23-W26)", 8), "IDE Ecosystem", "Production LSP Server & VS Code Marketplace",
        "Language Server Protocol engine in pure Roze: semantic highlighting, smart completion, go-to-definition, type hover, rename, and formatting in VS Code and JetBrains.",
        "World-class developer tooling experience.",
        "IDE, Developer Productivity", "6 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M6-T10", 8), ("Month 6", 11), ("Sprint 12 (W23-W26)", 8), "Final Release M6", "Official Roze v2.0 'Prometheus' Production Launch",
        "Official launch: Finalized grammar specification, 1,000-page standard library documentation, installer packages (deb, rpm, Homebrew, MSI), and RozeHub portal.",
        "Roze v2.0 officially released as an autonomous, general-purpose programming language on par with Java 25 & Rust 1.99.1.",
        "Global Release & Production Ecosystem", "5 Days", ("Critical", 14), ("Milestone Gate", 15)
    ]
]

# ==================== DATA FOR SHEET 2: STDLIB ARCHITECTURE ====================
s2_headers = [
    "Module Name", "Namespace / Import", "Core Classes & Structs", "Primary Built-in Methods & Functions",
    "Java 25 Benchmark Equivalent", "Rust 1.99.1 Benchmark Equivalent", "Target Application Scope"
]
s2_widths = [18, 20, 28, 48, 30, 30, 26]

s2_rows = [
    [
        "Core & Math", "roze::core", "Int, Float, Bool, String, Char, Math",
        "abs, min, max, clamp, sqrt, sin, cos, tan, pow, log, floor, ceil, round, string_concat, substring, to_upper, to_lower, trim, split, replace, parse_int, parse_float, hash",
        "java.lang.Math, java.lang.String", "std::primitive, std::cmp", "Universal Fundamentals"
    ],
    [
        "Collections", "roze::collections", "List<T>, Map<K,V>, Set<T>, VecDeque<T>, BTreeMap<K,V>, BinaryHeap<T>, RingBuffer<T>, BitSet",
        "push, pop, get, set, insert, remove, clear, contains, sort, filter, map, reduce, reverse, dedup, binary_search, peek, drain, extend, iter, capacity, shrink_to_fit",
        "java.util (ArrayList, HashMap, TreeSet, PriorityQueue)", "std::collections (Vec, HashMap, BTreeMap, BinaryHeap)", "Data Structures & Algorithms"
    ],
    [
        "Concurrency & Sync", "roze::sync", "GreenThread, Channel<T>, Mutex<T>, RwLock<T>, AtomicBool, AtomicI64, AtomicPtr, Nursery, Semaphore, Barrier",
        "spawn, sleep, yield_now, channel_new, send, recv, try_recv, lock, read, write, atomic_load, atomic_store, atomic_swap, atomic_add, nursery_open, cancel",
        "java.lang.Thread (Virtual), java.util.concurrent", "std::sync, std::thread, crossbeam", "Multi-Threading & Parallelism"
    ],
    [
        "Async Runtime", "roze::async", "Future<T>, Task<T>, Poll<T>, Executor, Stream<T>, Sink<T>, Timeout, JoinSet<T>",
        "poll, await, block_on, spawn_async, select, join_all, race, timeout_after, next, send_async, close, is_ready, wake",
        "CompletableFuture, Virtual Threads, Flow API", "std::future, tokio runtime", "Asynchronous Non-blocking I/O"
    ],
    [
        "I/O & Filesystem", "roze::io", "File, Path, Directory, BufReader, BufWriter, Pipe, Process, Stdin, Stdout, Stderr",
        "read_to_string, read_bytes, write_all, append, delete, exists, create_dir_all, read_dir, canonicalize, copy, rename, spawn_process, pipe_create",
        "java.nio.file (Files, Path), java.io", "std::fs, std::io, std::path", "Filesystem & OS Interop"
    ],
    [
        "Networking & Sockets", "roze::net", "TcpListener, TcpStream, UdpSocket, DnsResolver, SocketAddr, IpAddr, Ipv4, Ipv6",
        "bind, accept, connect, read_exact, write_all, send_to, recv_from, set_nodelay, set_read_timeout, resolve_hostname, local_addr, peer_addr",
        "java.net (Socket, ServerSocket, InetAddress)", "std::net (TcpStream, UdpSocket)", "Low-Level Network I/O"
    ],
    [
        "Enterprise Web (RozeBoot)", "roze::boot / roze::web", "Server, Request, Response, Router, IoCContainer, FilterChain, Middleware, JwtService",
        "start, listen, get, post, put, delete, patch, use_middleware, json_respond, set_header, cookie, redirect, autowire, register_singleton, validate",
        "Spring Boot 3 (Spring MVC, WebFlux)", "Actix-web, Axum, Tower", "Cloud Microservices & APIs"
    ],
    [
        "Database & ORM", "roze::orm / roze::sql", "ConnectionPool, Connection, QueryBuilder, ResultSet, Entity, Migration, Transaction",
        "pool_create, get_connection, query, execute, begin_transaction, commit, rollback, select, where, join, insert_into, update, migrate_apply",
        "Spring Data JPA, Hibernate, HikariCP", "Diesel, SeaORM, sqlx", "Enterprise Persistence"
    ],
    [
        "Cybersecurity & Raw Net", "roze::sec / roze::net::raw", "RawSocket, PacketBuilder, Dissector, SynScanner, PcapWriter, MemoryScanner, ShellcodeRunner",
        "raw_socket_open, promiscuous_mode, craft_packet, parse_ethernet, parse_ip, parse_tcp, calculate_checksum, syn_scan, pcap_dump, read_proc_mem, run_shellcode",
        "Jpcap, Pcap4J (Third party)", "libpcap bindings, pnet, scapy", "Penetration Testing & Hacking"
    ],
    [
        "Standard Cryptography", "roze::crypto", "Aes256Gcm, ChaCha20Poly1305, Sha256, Sha512, Keccak256, HmacSha256, Ed25519, X25519, Rsa4096, TlsSession",
        "encrypt, decrypt, digest, hmac, sign, verify, keygen, diffie_hellman, tls_connect, tls_accept, tls_handshake, random_bytes",
        "java.security, javax.crypto", "ring, rustls, ed25519-dalek", "Data Protection & Auth"
    ],
    [
        "AI & Tensor Engine", "roze::ai / roze::tensor", "Tensor<T, D>, AutogradTape, Dense, Conv2D, MultiHeadAttention, Transformer, GgufModel, OnnxModel",
        "tensor_new, matmul, add, mul, relu, softmax, layer_norm, backward, forward, load_gguf, load_onnx, tokenize, kv_cache_update, sample_token",
        "Deep Net Java, ND4J, ONNX Runtime Java", "burn, candle, tch-rs", "Local AI Inference & Training"
    ],
    [
        "Game Engine & Graphics", "roze::game / roze::gfx", "Vec2, Vec3, Vec4, Mat4, Quat, ECS, Window, Texture, Shader, Sound, RigidBody",
        "init_window, clear, draw_sprite, draw_mesh, play_sound, ecs_spawn, ecs_query, physics_step, raycast, apply_force, is_key_down, get_mouse_pos",
        "jMonkeyEngine, libGDX", "Bevy, wgpu, raylib-rs", "2D/3D Games & Simulations"
    ],
    [
        "OS & Freestanding", "roze::os", "PageTable, FrameAllocator, GDT, IDT, InterruptHandler, Uart16550, VgaText, GOPFramebuffer, Asm",
        "map_page, unmap_page, alloc_frame, idt_load, gdt_load, outb, inb, uart_write, vga_write_char, asm_execute, disable_interrupts, enable_interrupts",
        "N/A (Java cannot run freestanding)", "core::arch, x86_64, bootloader", "Operating Systems & Kernels"
    ],
    [
        "Desktop GUI (RozeUI)", "roze::ui", "App, Window, State<T>, Canvas, Button, TextField, DataGrid, Dropdown, Flexbox, Webview",
        "run_app, create_window, set_state, draw_path, draw_text, on_click, on_change, layout_calculate, embed_webview, show_dialog",
        "JavaFX, Swing", "Iced, Slint, egui, Tauri", "Desktop Applications"
    ],
    [
        "Text & Regex", "roze::regex", "Regex, Match, CaptureGroup, StringBuilder, Formatter, UnicodeScanner",
        "compile, is_match, find, find_all, captures, replace, replace_all, split, append_str, format, graphemes",
        "java.util.regex.Pattern, StringBuilder", "regex, unicode-segmentation", "Text Manipulation & Compilers"
    ],
    [
        "Time & Serialization", "roze::time / roze::serialize", "Instant, Duration, DateTime, ZonedDateTime, Json, Yaml, Toml, MsgPack, Protobuf",
        "now, elapsed, duration_since, parse_iso8601, format_date, json_serialize, json_deserialize, toml_parse, yaml_parse, protobuf_encode, protobuf_decode",
        "java.time (JSR-310), Jackson, SnakeYAML", "chrono, serde_json, toml, rmp", "Universal Data Interchange"
    ]
]

# ==================== DATA FOR SHEET 3: 7-DOMAIN MATRIX ====================
s3_headers = [
    "Target Domain", "Vision & Production Capability", "Current Roze Baseline",
    "Gaps to Java 25 / Rust 1.99.1 Parity", "6-Month Deliverables & Unlocking Tasks",
    "Industry Benchmark", "Target Readiness"
]
s3_widths = [20, 34, 28, 40, 32, 22, 18]

s3_rows = [
    [
        "Enterprise Web & Microservices",
        "Build enterprise-scale cloud microservices, REST/gRPC backends, connection pooling, and banking-grade APIs with sub-millisecond latency.",
        "Basic JVM/Native HTTP server with JSON and basic JDBC query support.",
        "M:N Virtual Threads, HTTP/2/3 QUIC, Declarative IoC (@Autowired/@Service), High-Throughput ORM, JWT/OAuth2 security filters.",
        "M1-T04, M1-T05, M3-T07, M3-T08, M3-T09, M3-T10 (Sprint 2, 6)",
        "Java 25 (Spring Boot 3), Rust (Actix-web), Go (Gin)",
        ("100% - Spring Boot Parity", 16)
    ],
    [
        "Operating Systems & Kernels",
        "Write full 64-bit kernels, bootloaders, microkernels, memory managers, and embedded firmware running directly on bare-metal hardware.",
        "Basic freestanding prototype with simple VGA string printing.",
        "4-level paging manager, physical frame allocator, full GDT/IDT exception stubs, APIC interrupt controller, inline assembly constraints, UEFI/GOP framebuffer.",
        "M2-T01, M2-T05, M2-T06, M2-T07, M2-T08, M2-T09 (Sprint 3, 4)",
        "C23, Rust 1.99 (#![no_std]), Zig",
        ("100% - Production Kernel Ready", 16)
    ],
    [
        "Cybersecurity & Hacking",
        "High-performance security tools: raw network packet sniffers, SYN scanners, protocol dissectors, binary reverse-engineering, and exploit research harnesses.",
        "TCP sockets and basic unsafe pointer allocations.",
        "Promiscuous raw sockets (AF_PACKET/BPF), packet crafting engine (Ethernet->TCP), 100k port/s async scanner, binary disassembler (ELF/PE), process memory inspection.",
        "M2-T01, M3-T01, M3-T02, M3-T03, M3-T04, M3-T05, M3-T06 (Sprint 5)",
        "Python (Scapy), Go, C/C++, Rust",
        ("100% - Security Tooling Suite Ready", 16)
    ],
    [
        "AI / Machine Learning",
        "Local LLM inference (LLaMA 3, Mistral), multidimensional tensor math, reverse-mode autograd, and GPU compute without heavy Python runtimes.",
        "Basic 1D/2D float tensors with C wrapper bindings.",
        "N-dimensional Tensor<T, D> with striding/slicing, AVX-512/CUDA GEMM kernels, dynamic Autograd tape, GGUF/ONNX native parser, KV-cache transformer decoder.",
        "M2-T02, M4-T01, M4-T02, M4-T03, M4-T04, M4-T05, M4-T06 (Sprint 7)",
        "Python (PyTorch), C++ (llama.cpp), Mojo, Rust (candle)",
        ("100% - Native AI & LLM Engine Ready", 16)
    ],
    [
        "Game Development",
        "60+ FPS 2D & 3D real-time games, cross-platform graphics, spatial audio, rigid-body physics, and high-performance ECS architectures.",
        "Basic Raylib FFI wrapper demonstrating 2D shape drawing.",
        "Cross-platform graphics HAL (Vulkan, Metal, DirectX 12), archetype-based ECS scheduler, SIMD 3D math suite (Quat, Mat4), 3D spatial audio, rigid-body physics engine.",
        "M1-T03, M2-T02, M4-T07, M4-T08, M4-T09, M4-T10 (Sprint 8)",
        "C++ (Unreal/Raylib), C# (Unity), Rust (Bevy)",
        ("100% - 2D/3D Game Platform Ready", 16)
    ],
    [
        "Desktop GUI Applications",
        "Fast, beautiful cross-platform desktop productivity applications with native look-and-feel, GPU 2D vector rendering, and modern reactive state.",
        "Basic window creation without event loop or widget abstractions.",
        "Native windowing & event loop (Win32/Cocoa/Wayland), Skia-style GPU 2D canvas, reactive UI component tree with Flexbox layout, full widget set, embedded Webview.",
        "M5-T01, M5-T02, M5-T03, M5-T04, M5-T05 (Sprint 9)",
        "Java 25 (JavaFX), C# (WPF/.NET MAUI), Rust (Tauri/Slint), C++ (Qt)",
        ("100% - Native Desktop GUI Ready", 16)
    ],
    [
        "Embedded & Systems Hardware",
        "Deterministic real-time microcontrollers, IoT devices, automotive systems, and custom hardware peripheral control.",
        "Basic FFI calling convention.",
        "Zero-overhead freestanding target, memory-mapped I/O register abstractions, interrupt service routines, UART/I2C/SPI drivers, bit-manipulation primitives.",
        "M2-T01, M2-T05, M2-T07, M2-T08, M2-T09 (Sprint 3, 4)",
        "C, Rust (embedded-hal), Zig",
        ("100% - Bare-Metal Embedded Ready", 16)
    ]
]

# ==================== DATA FOR SHEET 4: SELF-HOSTING ROADMAP ====================
s4_headers = [
    "Stage", "Phase Title", "Target Component", "Technical Deliverable in Pure Roze",
    "Rust Decommissioning Progress", "Verification Test & Gate", "Status"
]
s4_widths = [10, 22, 24, 46, 26, 32, 16]

s4_rows = [
    [
        ("Stage 0", 3), "Bootstrap Baseline", "Rust Host Compiler",
        "Legacy Rust-based Roze compiler (roze-compiler v1.0.0) acts as bootstrap host compiler to compile the initial pure Roze codebase.",
        "0% - Rust is the build driver",
        "Compile basic Roze programs with Rust host",
        ("Completed", 16)
    ],
    [
        ("Stage 1", 3), "Pure Roze Frontend", "Roze-in-Roze Scanner & Parser",
        "Implement complete Token Scanner, Recursive Descent Parser, and Macro Expander in pure Roze (demos/self_hosted/src/frontend/). Supports 100% of Roze grammar.",
        "25% - Frontend decoupled from Rust",
        "Parse full 10,000 LOC Roze codebase without syntax errors",
        ("Planned", 12)
    ],
    [
        ("Stage 2", 4), "Pure Roze Semantic", "Roze-in-Roze Type Checker",
        "Implement Type Checker, Trait Solver, Monomorphizer, and ARC Borrow Checker in pure Roze. Validates symbols, resolves interfaces, and handles generics.",
        "50% - Semantic analysis decoupled",
        "Type check compiler source with zero errors",
        ("Planned", 12)
    ],
    [
        ("Stage 3", 5), "Pure Roze Optimizer", "Roze-in-Roze SSA IR & Optimizer",
        "Implement Typed SSA IR generator and optimization pipeline in pure Roze: Dead Code Elimination, Inlining, Loop Unrolling, Constant Folding, and CFG analysis.",
        "75% - IR & optimization decoupled",
        "Verify optimized IR passes on benchmark suite",
        ("Planned", 12)
    ],
    [
        ("Stage 4", 6), "Pure Roze Native Backend", "Machine Code Emitter & Cranelift",
        "Direct native code generation in pure Roze: emit standalone ELF/Mach-O/PE binaries with Cranelift C-API or standalone assembly generator. Eliminates Cargo.",
        "90% - Code generation decoupled",
        "Produce standalone native binary from Roze code",
        ("Planned", 12)
    ],
    [
        ("Stage 5", 11), "Fixpoint Closure Verification", "3-Stage Bootstrap Fixpoint",
        "Execute full bootstrap cycle: Stage 0 (Rust) builds roze_stage1; roze_stage1 builds roze_stage2; roze_stage2 builds roze_stage3. Run 'diff roze_stage2 roze_stage3'.",
        "100% - Zero Rust Dependency! Rust removed.",
        "Fixpoint Verification: diff output is exactly 0 bytes",
        ("Milestone Gate", 15)
    ]
]

# ==================== DATA FOR SHEET 5: SPRINT SCHEDULE & GATES ====================
s5_headers = [
    "Sprint #", "Weeks", "Milestone Target", "Key Technical Epics",
    "Target Deliverables & Acceptance Criteria", "Risk Assessment & Mitigation", "Quality Gate Sign-Off"
]
s5_widths = [12, 14, 18, 30, 44, 34, 20]

s5_rows = [
    [
        ("Sprint 1", 3), "W01 - W02", "Core Type Power", "Generics, Trait Bounds, Pattern Matching, Comptime",
        "Monomorphized generics with bounds, deep pattern matching with guards, comptime expressions. All compiler tests pass.",
        "Risk: Monomorphization code bloat. Mitigation: Deduplicate identical binary representations.",
        ("Gate 1: Types Pass", 16)
    ],
    [
        ("Sprint 2", 3), "W03 - W04", "Concurrency Milestone M1", "M:N Green Threads, Async/Await, Channels, Nurseries",
        "100,000 green thread stress test, MPMC ring buffer channels, nursery task cancelation. Zero data races.",
        "Risk: Thread starvation. Mitigation: Work-stealing randomized deque scheduler.",
        ("Gate 2: Concurrency Sign-Off", 16)
    ],
    [
        ("Sprint 3", 4), "W05 - W06", "Systems Core & SIMD", "Numeric Primitives (i8-u128), SIMD (AVX/NEON), Collections",
        "15 primitive numeric types, hardware SIMD 8x acceleration, BTreeMap, BinaryHeap, RingBuffer. 100% unit tests.",
        "Risk: Non-aligned SIMD crashes. Mitigation: Enforce 32/64-byte alignment on vector structs.",
        ("Gate 3: Systems Core Pass", 12)
    ],
    [
        ("Sprint 4", 4), "W07 - W08", "Bare-Metal OS Milestone M2", "Freestanding Target, 4-Level Paging, GDT/IDT, Drivers",
        "RozeOS v2 booting in QEMU: paging initialized, IDT exceptions handled, VGA text & GOP graphics running with shell.",
        "Risk: Triple fault on boot. Mitigation: Strict multiboot2 memory map validation.",
        ("Gate 4: Kernel Boot Sign-Off", 12)
    ],
    [
        ("Sprint 5", 5), "W09 - W10", "Security Suite & Crypto", "Raw Sockets, Packet Crafting, SYN Scanner, Crypto, Disassembler",
        "Promiscuous packet capture, 100k port/s stealth SYN scanner, pure TLS 1.3 handshake, x86_64 disassembler.",
        "Risk: OS raw socket permissions. Mitigation: Automatic capability checking with fallback.",
        ("Gate 5: Security Suite Pass", 12)
    ],
    [
        ("Sprint 6", 5), "W11 - W12", "Enterprise Web Milestone M3", "RozeBoot 2.0, HTTP/2/3, IoC Container, ORM, JWT, OpenAPI",
        "RozeBoot microservice benchmark: 100k req/s at <1ms latency, auto-generated Swagger UI, SQL connection pool.",
        "Risk: Memory leaks under load. Mitigation: ARC stress testing with leak sanitizers.",
        ("Gate 6: RozeBoot Web Sign-Off", 12)
    ],
    [
        ("Sprint 7", 6), "W13 - W14", "AI & LLM Inference", "N-D Tensors, GEMM (AVX/CUDA), Autograd, GGUF/ONNX, LLaMA 3",
        "Native LLaMA 3 inference generating 45+ tok/s locally; dynamic Autograd training linear regression demo.",
        "Risk: Quantization accuracy loss. Mitigation: Rigorous perplexity testing vs FP16 baseline.",
        ("Gate 7: AI Engine Pass", 12)
    ],
    [
        ("Sprint 8", 6), "W15 - W16", "Game Platform Milestone M4", "3D Math, Graphics HAL (Vulkan/Metal), Archetype ECS, Audio/Physics",
        "60 FPS 3D game demo with physics, lighting, shadows, spatial audio, and archetype ECS scheduler running smoothly.",
        "Risk: Frame drops / GC pauses. Mitigation: ARC memory pre-allocation in frame arenas.",
        ("Gate 8: Game Platform Sign-Off", 12)
    ],
    [
        ("Sprint 9", 7), "W17 - W18", "Desktop GUI (RozeUI)", "Cross-Platform Windowing, GPU 2D Canvas, Reactive UI, Widgets",
        "Cross-platform Desktop Dashboard app with reactive state, data grid, charts, and embedded Webview container.",
        "Risk: Platform UI quirks. Mitigation: Native platform windowing wrappers for Win32/Cocoa/Wayland.",
        ("Gate 9: Desktop GUI Pass", 12)
    ],
    [
        ("Sprint 10", 7), "W19 - W20", "StdLib Richness Milestone M5", "Linear-Time Regex, Date/Time (JSR-310), Universal Serialization",
        "Zero-ReDoS regex engine, IANA timezone database math, zero-copy JSON/TOML/MsgPack/Protobuf benchmarks.",
        "Risk: Timezone corner cases. Mitigation: Full validation against standard Olson timezone test vectors.",
        ("Gate 10: StdLib Parity Sign-Off", 12)
    ],
    [
        ("Sprint 11", 11), "W21 - W22", "Pure Roze Compiler v2", "Roze-in-Roze Frontend, Type Checker, SSA Optimizer, Native Backend",
        "Compiler rewritten 100% in pure Roze; compiles entire standard library and itself without Rust compiler.",
        "Risk: Compiler bootstrap bug. Mitigation: Extensive golden test fixtures comparing against Stage 0.",
        ("Gate 11: Self-Host Frontend Pass", 12)
    ],
    [
        ("Sprint 12", 11), "W23 - W26", "Production Launch Milestone M6", "Fixpoint Closure (0 Diff), Monolithic CLI, Registry, VS Code, Docs",
        "3-Stage bootstrap fixpoint verified (0 bytes diff). Official Roze v2.0 'Prometheus' public production release.",
        "Risk: Packaging regressions. Mitigation: Multi-platform automated CI packaging pipeline.",
        ("Gate 12: Production Launch Sign-Off", 15)
    ]
]

# Write worksheets
sheet1_xml = make_worksheet(s1_headers, s1_rows, s1_widths, freeze_row=1)
sheet2_xml = make_worksheet(s2_headers, s2_rows, s2_widths, freeze_row=1)
sheet3_xml = make_worksheet(s3_headers, s3_rows, s3_widths, freeze_row=1)
sheet4_xml = make_worksheet(s4_headers, s4_rows, s4_widths, freeze_row=1)
sheet5_xml = make_worksheet(s5_headers, s5_rows, s5_widths, freeze_row=1)

# Dynamically locate project root
current_dir = os.path.dirname(os.path.abspath(__file__))
repo_root = os.path.abspath(os.path.join(current_dir, ".."))

targets = [
    os.path.join(repo_root, "ROZE_PROJECT_PLAN.xlsx"),
    os.path.join(repo_root, "docs", "ROZE_PROJECT_PLAN.xlsx")
]

for output_path in targets:
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    with zipfile.ZipFile(output_path, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", content_types)
        z.writestr("_rels/.rels", pkg_rels)
        z.writestr("xl/_rels/workbook.xml.rels", wb_rels)
        z.writestr("xl/workbook.xml", wb_xml)
        z.writestr("xl/styles.xml", styles_xml)
        z.writestr("xl/worksheets/sheet1.xml", sheet1_xml)
        z.writestr("xl/worksheets/sheet2.xml", sheet2_xml)
        z.writestr("xl/worksheets/sheet3.xml", sheet3_xml)
        z.writestr("xl/worksheets/sheet4.xml", sheet4_xml)
        z.writestr("xl/worksheets/sheet5.xml", sheet5_xml)
    print(f"Successfully generated: {output_path}")

print("Master 6-Month Project Plan Excel generation complete!")
