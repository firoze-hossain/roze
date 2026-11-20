#!/usr/bin/env python3
"""
Generates the official Roze Master 12-Month Project Plan Excel spreadsheet:
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
    <sheet name="1. 12-Month Master Plan" sheetId="1" r:id="rId1"/>
    <sheet name="2. StdLib Architecture" sheetId="2" r:id="rId2"/>
    <sheet name="3. 10-Domain Universal Matrix" sheetId="3" r:id="rId3"/>
    <sheet name="4. Self-Hosting Roadmap" sheetId="4" r:id="rId4"/>
    <sheet name="5. Sprint Schedule &amp; Gates" sheetId="5" r:id="rId5"/>
  </sheets>
</workbook>"""

# Styles XML
styles_xml = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="22">
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
    <font><b/><sz val="9.5"/><color rgb="FF283593"/><name val="Segoe UI"/></font> <!-- 16: Month 7 Indigo -->
    <font><b/><sz val="9.5"/><color rgb="FFAD1457"/><name val="Segoe UI"/></font> <!-- 17: Month 8 Crimson -->
    <font><b/><sz val="9.5"/><color rgb="FFC62828"/><name val="Segoe UI"/></font> <!-- 18: Month 9 Deep Red -->
    <font><b/><sz val="9.5"/><color rgb="FF1565C0"/><name val="Segoe UI"/></font> <!-- 19: Month 10 Deep Navy -->
    <font><b/><sz val="9.5"/><color rgb="FF6A1B9A"/><name val="Segoe UI"/></font> <!-- 20: Month 11 Violet -->
    <font><b/><sz val="9.5"/><color rgb="FFE65100"/><name val="Segoe UI"/></font> <!-- 21: Month 12 Bronze -->
  </fonts>
  <fills count="22">
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
    <fill><patternFill patternType="solid"><fgColor rgb="FFE8EAF6"/></patternFill></fill> <!-- 16: M7 Soft Indigo -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFFCE4EC"/></patternFill></fill> <!-- 17: M8 Soft Crimson -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFFFEBEE"/></patternFill></fill> <!-- 18: M9 Soft Red -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFE3F2FD"/></patternFill></fill> <!-- 19: M10 Soft Blue -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFF3E5F5"/></patternFill></fill> <!-- 20: M11 Soft Violet -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFFFF8E1"/></patternFill></fill> <!-- 21: M12 Soft Gold -->
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
  <cellXfs count="28">
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
    <!-- 22: M7 Indigo Tag -->
    <xf numFmtId="0" fontId="16" fillId="16" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 23: M8 Crimson Tag -->
    <xf numFmtId="0" fontId="17" fillId="17" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 24: M9 Deep Red Tag -->
    <xf numFmtId="0" fontId="18" fillId="18" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 25: M10 Deep Navy Tag -->
    <xf numFmtId="0" fontId="19" fillId="19" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 26: M11 Violet Tag -->
    <xf numFmtId="0" fontId="20" fillId="20" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 27: M12 Bronze Tag -->
    <xf numFmtId="0" fontId="21" fillId="21" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
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
        ws.append(f'    <row r="{row_idx}" ht="28" customHeight="1">')
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

# ==================== DATA FOR SHEET 1: 12-MONTH MASTER PLAN ====================
s1_headers = [
    "Task ID", "Month", "Sprint & Weeks", "Domain / Epic", "Feature & Capability",
    "Detailed Technical Implementation", "Java 25, Rust 1.99.1 & Python 3.15 Parity Deliverables",
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
        "Cooperative user-space green thread scheduler with multi-core work-stealing threadpool across CPU worker threads. Zero-allocation fiber lifecycle via slab pools.",
        "Matches Java 25 Loom Virtual Threads (Thread.ofVirtual().start(...)) and Go goroutines. Supports 1,000,000 simultaneous concurrent tasks.",
        "Enterprise Web, Game Servers, Tools", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M1-T05", 8), ("Month 1", 3), ("Sprint 2 (W03-W04)", 8), "Async Runtime", "Async / Await State-Machine Engine",
        "Lower 'async func' and 'await' expressions to zero-allocation state machine tasks. Native Future<T> polling contract, timer reactors, and top-level await.",
        "Full parity with Rust async/await and Java 25 CompletableFuture / Loom Virtual Threads.",
        "Web Microservices, Network I/O", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M1-T06", 8), ("Month 1", 3), ("Sprint 2 (W03-W04)", 8), "Synchronization", "Lock-Free Primitives & Channels",
        "Atomic types (AtomicBool, AtomicI64, AtomicPtr, AtomicUsize) with CAS, swap, add, sub. High-throughput MPMC bounded and unbounded ring-buffer channels, Mutex, RwLock.",
        "Parity with Rust crossbeam / std::sync and Java 25 java.util.concurrent.atomic / locks.",
        "Enterprise, Game Engines, Systems", "6 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M1-T07", 8), ("Month 1", 3), ("Sprint 2 (W03-W04)", 8), "Structured Concurrency", "Nursery, Task Groups & Cancelation",
        "Lifetime-bounded scopes (TaskGroup / Nursery) awaiting all child tasks without leaks. Policies: ALL, SHUTDOWN_ON_FAILURE, SHUTDOWN_ON_SUCCESS, timeout deadlines.",
        "Matches Python Trio/AnyIO nurseries and Java 25 StructuredTaskScope incubator.",
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
        "Complete primitive numeric types (i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, f32, f64, isize, usize, char), type-suffixed literals, explicit casts, wrapping/saturating arithmetic.",
        "Full parity with Rust numeric primitives and Java 25 wrapper classes with unsigned support.",
        "Systems, Embedded, Cryptography, Math", "6 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M2-T02", 8), ("Month 2", 4), ("Sprint 3 (W05-W06)", 8), "Hardware Acceleration", "SIMD Vector Intrinsics Engine",
        "First-class hardware SIMD vector types (Vec4f, Vec8f, Vec4i, Vec16u8), component-wise/splat/zero constructors, arithmetic operators (+, -, *, /), dot, sum, sqrt, abs, fma.",
        "AVX-512/AVX2/NEON code generation parity with Java 25 Vector API and Rust core::simd.",
        "Games, AI/ML, Video/Audio Processing", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M2-T03", 8), ("Month 2", 4), ("Sprint 3 (W05-W06)", 8), "Standard Collections", "Advanced High-Performance Collections",
        "Implement VecDeque<T>, BTreeMap<K, V>, BTreeSet<T>, BinaryHeap<T> (priority queue), IndexMap<K, V>, RingBuffer<T>, and BitSet with amortized O(1) ops.",
        "Parity with Rust std::collections and Java 25 java.util collections hierarchy.",
        "All Domains, Databases, Compilers", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M2-T04", 8), ("Month 2", 4), ("Sprint 3 (W05-W06)", 8), "Memory Architecture", "Pluggable Allocator API & Smart Pointers",
        "Arena Allocator, Bump Allocator, Fixed-Size Block Pool Allocator, Box<T>, Arc<T>, and Weak<T> with cyclic reference detection and cache alignment.",
        "Full parity with Rust Allocator API and modern C++ polymorphic allocators (PMR).",
        "OS Kernels, Game Engines, Embedded", "7 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M2-T05", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Freestanding Systems", "Bare-Metal Kernel Target (--target freestanding)",
        "Zero-libc, zero-heap bare metal target (--no-heap), custom linker script integration (-T / --linker-script), freestanding runtime entry (_start, --entry), and float register disabling.",
        "Full parity with Rust #![no_std] and Zig freestanding target; produces standalone bootable ELF64 and QEMU multiboot images.",
        "Operating Systems, Microcontrollers, Bootloaders", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M2-T06", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Kernel Memory", "4-Level Paging & Virtual Memory Manager",
        "PML4 -> PDPT -> PD -> PT page table manipulator. 4KB standard and 2MB huge page mapping, unmapping, page table walks. Physical frame allocator (Bitmap + Buddy).",
        "Full OS memory management stack written entirely in Roze; 1:1 parity with Linux/Rust OS architectures.",
        "OS Kernels, Hypervisors, Microkernels", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M2-T07", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Kernel Core", "Hardware Interrupts (GDT, IDT, APIC)",
        "Global Descriptor Table (GDT), TSS with RSP0/IST1..7 tables, Interrupt Descriptor Table (IDT) with 256 vector gates, exception handlers #PF and #GP. 8259 PIC remap and Local APIC.",
        "Complete hardware interrupt and exception architecture matching x86_64 system specifications.",
        "Operating Systems, Systems Engineering", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M2-T08", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Hardware Drivers", "Hardware Drivers (UART, Keyboard, Timer & GOP)",
        "16550 UART serial driver, PS/2 keyboard/mouse interrupt driver, 8253/8254 PIT timer, HPET MMIO driver, and UEFI GOP linear framebuffer graphics driver with 8x16 font rasterizer.",
        "Zero external dependencies hardware driver suite running directly on bare-metal hardware and QEMU.",
        "OS Kernels, Bare-Metal UI, Diagnostics", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M2-T09", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Inline Assembly", "Native asm! Macro & Register Constraints",
        "Inline assembly macro supporting x86_64, ARM64 registers, generic register classes, GCC constraints, in/out/inout operands, and memory clobbers.",
        "Parity with Rust core::arch::asm! and GCC inline assembly.",
        "Systems Programming, Drivers, Cryptography", "7 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M2-T10", 8), ("Month 2", 4), ("Sprint 4 (W07-W08)", 8), "Milestone Gate M2", "Month 2 Bootable OS Kernel Verification",
        "Boot RozeOS Kernel v2 in QEMU via GRUB/Multiboot2: initialize paging, heap allocator, IDT, print to VGA/GOP screen, and execute interactive shell.",
        "QEMU x86_64 automated CI boot test with zero kernel panics.",
        "Operating Systems & Freestanding", "3 Days", ("Critical", 14), ("Completed", 16)
    ],

    # MONTH 3: CYBERSECURITY / HACKING SUITE & ENTERPRISE WEB FRAMEWORK
    [
        ("M3-T01", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Raw Networking", "Raw Sockets & Promiscuous Sniffing",
        "AF_PACKET / BPF raw socket engine in roze::net::raw. Promiscuous network interface binding, zero-copy packet ring buffer, RFC 1071 checksum engine, and PCAP 2.4 capture file export/import.",
        "Parity with Python Scapy and Go gopacket with zero external dependencies.",
        "Cybersecurity, Network Engineering", "7 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M3-T02", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Packet Crafting", "Protocol Builder & Packet Dissector",
        "Full packet crafting and dissection engine for Ethernet, ARP, IPv4, IPv6, ICMP, ICMPv6, TCP, UDP, DNS, and DHCP. Scapy-style fluent PacketBuilder layer stacking.",
        "Parity with Python Scapy layer composition with native compiled execution speed.",
        "Penetration Testing, Security Auditing", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M3-T03", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Security Scanning", "Ultra-Fast SYN Port & Banner Scanner",
        "High-performance asynchronous SYN stealth scanner and network diagnostic engine utilizing green threads. Passive OS fingerprinting by TTL/window size, service banner parser, 100k+ probes/sec.",
        "Parity with Nmap SYN scan performance in pure Roze with zero external dependencies.",
        "Red Teaming, Reconnaissance, DevOps", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M3-T04", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Standard Cryptography", "Full Crypto Suite & Pure TLS 1.3",
        "Constant-time word arithmetic, SHA-256, SHA-512, SHA-3/Keccak, HMAC, HKDF, Argon2id, AES-128/256 GCM with GHASH, ChaCha20-Poly1305, Curve25519/X25519, Ed25519, RSA-4096, pure TLS 1.3.",
        "Parity with Java Cryptography Architecture (JCA) and Rust ring/rustls with zero OpenSSL dependency.",
        "Security, Web, Blockchain, Cloud", "9 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M3-T05", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Binary Analysis", "ELF / PE / Mach-O Disassembler Engine",
        "Binary parsing and disassembly in roze::binary: ELF 32/64-bit, PE/COFF, Mach-O 64-bit, symbol demangling (Itanium C++, Rust v0, Roze), x86_64 opcode disassembler, ARM64 opcode disassembler.",
        "Matches capabilities of radare2 and Capstone disassembler with zero external dependencies.",
        "Reverse Engineering, Malware Analysis", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M3-T06", 8), ("Month 3", 5), ("Sprint 5 (W09-W10)", 8), "Memory Forensics", "Memory Forensics & Defensive Analysis Harness",
        "Cross-platform process memory inspection (/proc/<pid>/maps, VirtualQueryEx), memory dump and ELF core parsing, wildcard pattern matching, Shannon entropy calculation, RWX violation auditing, sandboxed CPU emulator.",
        "Meets and exceeds capabilities of Volatility and memory analysis frameworks.",
        "Blue Teaming, Incident Response, Forensics", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M3-T07", 8), ("Month 3", 5), ("Sprint 6 (W11-W12)", 8), "Enterprise Web Core", "Asynchronous HTTP/1.1, HTTP/2 & HTTP/3 Engine",
        "Production-grade event-driven web server: zero-copy HTTP/1.1 chunked parser, HTTP/2 binary framing (RFC 9113) with HPACK compression, HTTP/3 over QUIC with QPACK compression, 185,000+ req/sec throughput.",
        "Matches Actix-web/Netty performance in pure Roze with zero external dependencies.",
        "Cloud Web Services, High-Load APIs", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M3-T08", 8), ("Month 3", 5), ("Sprint 6 (W11-W12)", 8), "Enterprise Framework", "RozeBoot 2.0 Annotations & IoC Container",
        "Spring Boot 3 parity: first-class declarative annotations (@RestController, @GetMapping, @PostMapping, @Autowired, @Service, @Valid), IoC container, dependency injection with cycle detection, URI template routing.",
        "1:1 architectural parity with Spring Boot 3 on Java 25.",
        "Enterprise Web, Microservices", "9 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M3-T09", 8), ("Month 3", 5), ("Sprint 6 (W11-W12)", 8), "Data & ORM", "High-Throughput ORM & SQL Query Builder",
        "Declarative entity annotations (@Entity, @Table, @Id, @Column), HikariCP-grade connection pool, type-safe AST SQL Query Builder, parameterized queries, schema migrations, Spring Data JPA repository pattern, in-memory ACID database.",
        "1:1 architectural parity with Spring Data JPA and Rust Diesel / SeaORM.",
        "Databases, Financial Services, Enterprise", "9 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M3-T10", 8), ("Month 3", 5), ("Sprint 6 (W11-W12)", 8), "Web Security & Cloud", "JWT, OAuth2, Metrics & OpenAPI 3.1",
        "JWT Authentication Filter and RBAC, OAuth 2.0 / OIDC token introspection, Argon2id password hashing, Prometheus / OpenMetrics exposition, OpenTelemetry distributed tracing, auto-generated OpenAPI 3.1 and Swagger UI.",
        "Full parity with Spring Security and Quarkus observability stack.",
        "Cloud Security, DevOps, Observability", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M3-T11", 8), ("Month 3", 5), ("Sprint 6 (W11-W12)", 8), "Milestone Gate M3", "Month 3 Web & Security Integration Gate",
        "Deploy RozeBoot REST microservice serving 100,000 req/sec under load test. Execute penetration testing suite verifying raw packet sniffer, SYN scanner, and crypto.",
        "Sub-millisecond P99 latency under 50,000 concurrent web connections.",
        "Web Enterprise & Cybersecurity", "3 Days", ("Critical", 14), ("Completed", 16)
    ],

    # MONTH 4: UNIVERSAL C FFI, ATOMIC THREAD-SAFETY & CORE OS STANDARD LIBRARY
    [
        ("M4-T01", 8), ("Month 4", 6), ("Sprint 7 (W13-W14)", 8), "Universal C FFI", "C-ABI Foreign Function Interface Engine",
        "Universal C FFI supporting extern 'C' declarations, dynamic library loading (dlopen/dlsym/LoadLibrary), calling conventions ('C', 'sysv64', 'win64'), and dynamic symbol invocation.",
        "Full 1:1 parity with Rust extern 'C' FFI and Java 25 Project Panama (Foreign Function & Memory API).",
        "Systems Interop, Native Acceleration", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M4-T02", 8), ("Month 4", 6), ("Sprint 7 (W13-W14)", 8), "Memory Layout & Types", "#[repr(C)] Layout & Raw Pointer Primitives",
        "C-compatible struct memory layout (#[repr(C)], #[repr(packed)], #[repr(align(N))]), field padding, alignment rules, and pointer primitives (*const T, *mut T, *raw u8) with zero-copy C-string conversion.",
        "Matches C ABI memory guarantees and Rust std::ptr / core::ffi.",
        "Systems Programming, OS Drivers, Graphics", "7 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M4-T03", 8), ("Month 4", 6), ("Sprint 7 (W13-W14)", 8), "Memory Architecture", "Thread-Safe Atomic ARC & Weak References",
        "Multi-threaded atomic retain/release (atomic_fetch_add/sub with acquire-release semantics), full Weak<T> cyclic reference detection, and thread-local single-threaded Rc<T> distinction.",
        "100% thread-safe memory management matching Rust Arc<T>/Weak<T> and C++ std::shared_ptr.",
        "Concurrency, Multi-Threading, Enterprise", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M4-T04", 8), ("Month 4", 6), ("Sprint 7 (W13-W14)", 8), "Standard Filesystem", "Cross-Platform Filesystem & Path Library",
        "Object-oriented Path abstraction, synchronous and asynchronous file I/O (File, BufReader, BufWriter), directory traversal (WalkDir), file metadata (size, permissions, timestamps), and atomic file updates.",
        "Parity with Rust std::fs / std::path and Java 25 java.nio.file.",
        "All Domains, CLI Tools, Cloud Services", "7 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M4-T05", 8), ("Month 4", 6), ("Sprint 7 (W13-W14)", 8), "Operating System Interop", "Process Spawning, Environment & Pipes",
        "Process execution (Command, Process, Child), stdin/stdout/stderr piping, exit status codes, signal handling (SIGINT, SIGTERM), environment variables, and argument parsing.",
        "Matches Rust std::process / std::env and Java 25 java.lang.ProcessBuilder.",
        "CLI Utilities, Build Systems, DevOps", "7 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M4-T06", 8), ("Month 4", 6), ("Sprint 8 (W15-W16)", 8), "Temporal Systems", "Monotonic Time & High-Precision Clocks",
        "Monotonic Instant, Duration with nanosecond precision, SystemTime (UNIX timestamp), sleep timers, and ISO-8601 string formatting/parsing.",
        "Parity with Rust std::time and Java 25 java.time (Instant, Duration).",
        "All Domains, Benchmarking, Distributed Systems", "6 Days", ("Medium", 14), ("Completed", 16)
    ],
    [
        ("M4-T07", 8), ("Month 4", 6), ("Sprint 8 (W15-W16)", 8), "Text Processing", "Linear-Time Regular Expressions Engine",
        "DFA/NFA-based regular expression engine with guaranteed linear execution time (zero catastrophic backtracking), submatch capture groups, pattern compilation caching, and replacement.",
        "Parity with Rust regex crate and Python re / Java java.util.regex.",
        "Compilers, Log Parsers, Security", "7 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M4-T08", 8), ("Month 4", 6), ("Sprint 8 (W15-W16)", 8), "Data Interchange", "Universal Serialization (JSON, CBOR, TOML)",
        "Zero-copy serialization and deserialization framework (Serializer, Deserializer, Value) supporting JSON, binary CBOR, and TOML. Compile-time derived encoders.",
        "Parity with Rust serde and Java 25 Jackson/Gson.",
        "Web APIs, Config Management, Network RPC", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M4-T09", 8), ("Month 4", 6), ("Sprint 8 (W15-W16)", 8), "Metaprogramming", "Runtime Reflection & Introspection",
        "Type descriptor inspection (TypeId, TypeInfo, FieldInfo, MethodInfo), dynamic value boxing/unboxing, struct field enumeration, and method invocation.",
        "Parity with Java 25 java.lang.reflect and Rust std::any.",
        "Frameworks, ORMs, Serialization", "7 Days", ("Medium", 14), ("Completed", 16)
    ],
    [
        ("M4-T10", 8), ("Month 4", 6), ("Sprint 8 (W15-W16)", 8), "Ecosystem Bridges", "Native C FFI Bridges (Raylib & SQLite)",
        "Production-grade C-FFI bridges: Raylib 2D/3D hardware-accelerated graphics bridge, SQLite3 embedded database driver with prepared statement caching, and POSIX libc wrappers.",
        "Demonstrates real-world FFI interoperability and unlocks rich game and database ecosystems.",
        "Game Development, Embedded DBs, Systems", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M4-T11", 8), ("Month 4", 6), ("Sprint 8 (W15-W16)", 8), "Milestone Gate M4", "Universal Runtime & FFI Verification Gate",
        "Comprehensive verification gate: call native C libraries via FFI, spawn multi-process pipelines, serialize complex data, and run multi-threaded atomic ARC stress test.",
        "Zero memory leaks, zero data races under Valgrind/TSan, 100% C ABI parity.",
        "Core Systems & Language Runtime", "3 Days", ("Critical", 14), ("Completed", 16)
    ],

    # MONTH 5: BASIR ('bsr') PACKAGE MANAGER, MANIFEST ECOSYSTEM & DEVELOPER TOOLING
    [
        ("M5-T01", 8), ("Month 5", 7), ("Sprint 9 (W17-W18)", 8), "Package Manager Core", "Basir ('bsr') Package Manager Architecture",
        "Standalone bsr binary with subcommands: new, build, run, test, add, remove, update, lock, cache, publish, bench. Built-in SemVer parser and project manifest validator.",
        "1:1 architectural parity with Rust Cargo and Go modules.",
        "Developer Experience, Ecosystem, Tooling", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M5-T02", 8), ("Month 5", 7), ("Sprint 9 (W17-W18)", 8), "Manifest System", "Proprietary 'bsr.mfz' (Mohammad Firoze) Manifest",
        "Proprietary manifest format bsr.mfz (with backward compatibility for roze.mfz and roze.toml): [package], [dependencies], [dev-dependencies], [build], [features], [profile].",
        "Distinctive branding and syntax; full parity with Cargo.toml and go.mod.",
        "Build Engineering, Configuration", "7 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M5-T03", 8), ("Month 5", 7), ("Sprint 9 (W17-W18)", 8), "Deterministic Builds", "Cryptographic Lockfile ('bsr.lock')",
        "Deterministic lockfile bsr.lock recording exact package versions, cryptographic SHA-256 package checksums, and resolved dependency graphs. Zero non-deterministic build drift.",
        "Parity with Cargo.lock, poetry.lock, and package-lock.json.",
        "Security, Reproducible Builds, CI/CD", "7 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M5-T04", 8), ("Month 5", 7), ("Sprint 9 (W17-W18)", 8), "Dependency Resolution", "SAT-Based SemVer Dependency Solver",
        "NP-complete dependency graph solver resolving transitive dependencies, diamond dependencies, version range constraints (^, ~, >=, *), and conflict diagnostics.",
        "Parity with Cargo PubGrub algorithm and Dart pub solver.",
        "Package Resolution, Enterprise Tooling", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M5-T05", 8), ("Month 5", 7), ("Sprint 9 (W17-W18)", 8), "Package Distribution", "Global Package Cache & Offline Vendoring",
        "Global immutable package cache in ~/.roze/bsr_cache/, content-addressable storage, bsr vendor offline mode, and decentralized Git repository dependency resolution.",
        "Parity with cargo vendor and go mod vendor for air-gapped enterprise environments.",
        "Enterprise Infrastructure, Air-Gapped Deployments", "7 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M5-T06", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "Ecosystem Decoupling", "Modular Framework Decoupling",
        "Decouple frameworks into standalone bsr packages: packages/rozeboot, packages/roze-orm, packages/roze-net, packages/roze-crypto. Keep compiler core lean and modular.",
        "Matches modern language design principles; eliminates monolithic compiler coupling.",
        "Framework Architecture, Clean Code", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M5-T07", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "Registry Client", "Cryptographic Registry Client & Publishing",
        "bsr publish and bsr search CLI, Ed25519 digital signature signing of published package archives, index protocol compatible with Go module proxies, and package registry client.",
        "Parity with crates.io and npm publish workflows with built-in cryptographic signing.",
        "Ecosystem, Open Source, Security", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M5-T08", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "IDE Tooling", "Language Server Protocol ('roze-lsp') Engine",
        "Full LSP 3.17 server daemon: semantic tokens, hover type inspection, go-to-definition, document formatting, diagnostic error squiggles, and auto-completion.",
        "Matches rust-analyzer and Java Eclipse JDT LS experience in VS Code, IntelliJ, and Vim.",
        "Developer Productivity, IDEs, Editors", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M5-T09", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "Documentation Tooling", "Automated Documentation Generator ('rozedoc')",
        "CLI tool rozedoc parsing doc comments (///), extracting types/methods/traits, and compiling interactive searchable HTML documentation with offline search and syntax highlighting.",
        "Parity with rustdoc and Javadoc.",
        "Documentation, API Reference, Developer UX", "7 Days", ("Medium", 14), ("Completed", 16)
    ],
    [
        ("M5-T10", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "Performance Tooling", "Standard Benchmark Harness ('bsr bench')",
        "Built-in micro-benchmarking engine (bsr bench): statistical outlier removal, warmup cycles, nanosecond throughput metrics, and linear regression analysis.",
        "Parity with cargo bench and Criterion.",
        "Performance Engineering, Profiling", "6 Days", ("Medium", 14), ("Completed", 16)
    ],
    [
        ("M5-T11", 8), ("Month 5", 7), ("Sprint 10 (W19-W20)", 8), "Milestone Gate M5", "Package Ecosystem & Developer Experience Gate",
        "Build, resolve, fetch, compile, test, benchmark, and document multi-package modular projects using 'bsr' and 'bsr.mfz' with zero compiler interventions.",
        "Full package manager autonomy and verified developer tooling workflow.",
        "Package Ecosystem & Developer Tooling", "3 Days", ("Critical", 14), ("Completed", 16)
    ],

    # MONTH 6: 100% PURE ROZE-IN-ROZE SELF-HOSTING COMPILER & PRODUCTION INDEPENDENCE
    [
        ("M6-T01", 8), ("Month 6", 11), ("Sprint 11 (W21-W22)", 8), "Self-Hosting Scanner", "Pure Roze Lexer & Scanner",
        "Pure Roze lexer module (demos/self_hosted/src/lexer.roze): tokenizes all Roze syntax, Unicode identifiers, string interpolation, and keyword tables without Rust.",
        "100% pure Roze source code; zero foreign dependencies.",
        "Compiler Engineering, Self-Hosting", "7 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M6-T02", 8), ("Month 6", 11), ("Sprint 11 (W21-W22)", 8), "Self-Hosting Parser", "Pure Roze Recursive Descent Parser",
        "Pure Roze parser module (demos/self_hosted/src/parser.roze): constructs typed AST nodes for functions, classes, enums, traits, pattern matching, comptime, and asm!.",
        "100% pure Roze source code; complete grammar coverage.",
        "Compiler Engineering, AST", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M6-T03", 8), ("Month 6", 11), ("Sprint 11 (W21-W22)", 8), "Self-Hosting Semantic", "Pure Roze Type Checker & Monomorphizer",
        "Pure Roze semantic analysis module (demos/self_hosted/src/checker.roze): symbol tables, type resolution, trait constraint verification, pattern exhaustiveness, and monomorphization.",
        "100% pure Roze source code; mathematically sound type system.",
        "Type Safety, Compiler", "9 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M6-T04", 8), ("Month 6", 11), ("Sprint 11 (W21-W22)", 8), "Self-Hosting Codegen", "Pure Roze C99 & Native Emitter Backend",
        "Pure Roze code emission engine (demos/self_hosted/src/emitter.roze): generates optimized ISO C99 code for native compilation or direct Cranelift IR without Rust compiler.",
        "100% pure Roze source code; standalone native compilation.",
        "Code Generation, Backend", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M6-T05", 8), ("Month 6", 11), ("Sprint 11 (W21-W22)", 8), "Rust Decommissioning", "Complete Decommissioning of Rust Host Compiler",
        "Complete decommissioning of Cargo and rustc from Roze repository primary build pipeline. The Roze binary itself (bin/roze / ./roze) becomes the primary compiler and build driver.",
        "Root Makefile/build.sh orchestrates 3-stage bootstrap fixpoint closure; zero Rust dependency.",
        "Language Independence, Systems", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M6-T06", 8), ("Month 6", 11), ("Sprint 12 (W23-W26)", 8), "Bootstrap Fixpoint", "3-Stage Bootstrap Fixpoint Verification",
        "Automated bootstrap cycle: Stage 0 Seed -> Stage 1 (Roze) -> Stage 2 (Roze) -> Stage 3 (Roze). Proves 0-byte difference between roze_stage2 and roze_stage3.",
        "Mathematically verified bootstrap closure (zero byte difference, identical SHA-256 digests ed6a2704... / 6602e60e...).",
        "Language Sovereignty, Formal Verification", "6 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M6-T07", 8), ("Month 6", 11), ("Sprint 12 (W23-W26)", 8), "WebAssembly Target", "WebAssembly Compiler Target (--target wasm32 / wasi) & 'roze-bindgen'",
        "Direct WebAssembly 1.0 (MVP) and WASI snapshot-preview1 binary code generator in pure Roze. Standalone roze-bindgen tool generating JS/TS bindings, npm manifests, and ESM modules.",
        "Parity with Rust wasm32-wasi / wasm-bindgen and Go WebAssembly target.",
        "WebAssembly, Browser Apps, Edge Serverless", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M6-T08", 8), ("Month 6", 11), ("Sprint 12 (W23-W26)", 8), "Microcontroller IoT", "Microcontroller IoT Targets & 'embedded-hal' Architecture",
        "ARM Cortex-M and RISC-V targets. Pure Roze embedded_hal.roze library, tools/roze-svd/ CMSIS-SVD PAC generator, bare-metal linker scripts, and direct roze flash CLI.",
        "Parity with Rust embedded-hal and C CMSIS/HAL micro-firmware architectures; direct flash programming on STM32, ESP32, and RP2040.",
        "Microcontrollers, IoT Edge, Automotive, Robotics", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M6-T09", 8), ("Month 6", 11), ("Sprint 12 (W23-W26)", 8), "Unified Toolchain", "Monolithic 'roze' CLI Toolchain Suite",
        "Unify compiler, roze-build, roze-test, roze-fmt, roze-doc, and roze-repl into a single hyper-fast monolithic executable (roze) integrating with 'bsr'.",
        "Matches the convenience of 'cargo' and 'go' single-binary toolchain.",
        "Developer Experience, Tooling", "6 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M6-T10", 8), ("Month 6", 11), ("Sprint 12 (W23-W26)", 8), "Ecosystem Seed Packages", "Community Package Registry & Reference Seed Ecosystem",
        "Deploy official package registry with seed packages: rozeboot (web), roze-orm (database), roze-torch (tensors/AI via CUDA FFI), roze-game (games via Raylib FFI), roze-ui (desktop GUI).",
        "Empowers community to install and build frameworks using 'bsr add <package>'.",
        "Ecosystem, Community, Frameworks", "7 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M6-T11", 8), ("Month 6", 11), ("Sprint 12 (W23-W26)", 8), "Final Release M6", "Official Roze v2.0 'Prometheus' Production Launch",
        "Official launch: Finalized grammar specification, 1,000-page standard library documentation, installer packages (deb, rpm, Homebrew, MSI), and RozeHub portal.",
        "Roze v2.0 officially released as an autonomous, resourceful general-purpose programming language on par with Java 25 & Rust 1.99.1.",
        "Global Release & Production Ecosystem", "5 Days", ("Critical", 14), ("Completed", 16)
    ],

    # =========================================================================
    # ROZE v3.0 'TITAN' - THE UNIVERSAL ENTERPRISE LANGUAGE ERA
    # =========================================================================

    # MONTH 7: UNIVERSAL ENTERPRISE STANDARD LIBRARY & ADVANCED MATHEMATICS (JAVA 25 & PYTHON 3.15 PARITY)
    [
        ("M7-T01", 8), ("Month 7", 22), ("Sprint 13 (W27-W28)", 8), "Universal Math", "Arbitrary-Precision BigInt & BigDecimal Engine (roze.math)",
        "Pure Roze implementation of arbitrary precision integers (BigInt with Karatsuba multiplication, modular exponentiation, Miller-Rabin prime test) and arbitrary precision fixed-point decimals (BigDecimal with precision & rounding modes: HALF_UP, HALF_EVEN, FLOOR, CEILING).",
        "Direct mathematical parity with Java 25 java.math.BigInteger / java.math.BigDecimal and Python 3.15 decimal.Decimal.",
        "Fintech, Cryptography, Scientific Computing", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M7-T02", 8), ("Month 7", 22), ("Sprint 13 (W27-W28)", 8), "Enterprise Collections", "Complete Java 25 Collections Hierarchy (roze.util)",
        "Comprehensive enterprise collections in pure Roze: ArrayList<T>, LinkedList<T>, HashMap<K,V>, LinkedHashMap<K,V>, HashSet<T>, TreeSet<T>, TreeMap<K,V>, ArrayDeque<T>, PriorityQueue<T>, BitSet. Built-in Collections static algorithm suite: sort, binary_search, shuffle, reverse, unmodifiable_view.",
        "1:1 parity with Java 25 java.util.* Collections Framework and Rust 1.99 std::collections.",
        "All Domains, Data Structures, Algorithms", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M7-T03", 8), ("Month 7", 22), ("Sprint 13 (W27-W28)", 8), "Temporal Engine", "JSR-310 Temporal Engine & High-Precision Clocks (roze.time)",
        "Pure Roze timezone-aware temporal engine: Instant (nanosecond epoch precision), Duration, Period, LocalDate, LocalTime, LocalDateTime, ZonedDateTime, ZoneId, ZoneOffset. Comprehensive ISO-8601 and RFC 2822 formatting and custom DateTimeFormatter.",
        "Parity with Java 25 java.time.* (JSR-310) and Python 3.15 datetime / zoneinfo.",
        "Enterprise Systems, Calendaring, Telemetry", "7 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M7-T04", 8), ("Month 7", 22), ("Sprint 14 (W29-W30)", 8), "Python Ergonomics", "Functional Itertools, Statistics & Pathlib (roze.data)",
        "Python-class functional data manipulation in pure Roze: roze.itertools (permutations, combinations, product, cycle, repeat, chain, groupby), roze.functools (reduce, partial, lru_cache, memoize), roze.statistics (mean, median, mode, variance, stdev, linear_regression), roze.pathlib (Path), roze.csv, roze.argparse.",
        "Direct functional parity with Python 3.15 itertools, functools, statistics, pathlib, and argparse standard libraries.",
        "Data Science, Numerical Analysis, Scripting", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M7-T05", 8), ("Month 7", 22), ("Sprint 14 (W29-W30)", 8), "Non-Blocking I/O", "Zero-Copy Buffers & Memory-Mapped Channels (roze.nio)",
        "Zero-copy I/O engine in pure Roze: ByteBuffer (heap and direct off-heap native memory), byte order endianness switching, slicing, duplicate views, memory-mapped files (mmap with PAGE_READONLY, PAGE_READWRITE), I/O multiplexing event selectors (epoll, kqueue, IOCP).",
        "Direct parity with Java 25 java.nio.* (ByteBuffers, Channels, Selectors) and Rust mio / tokio-core.",
        "High-Performance I/O, Storage, Databases", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M7-T06", 8), ("Month 7", 22), ("Sprint 14 (W29-W30)", 8), "Enterprise Concurrency", "CompletableFuture & Work-Stealing Pools (roze.concurrent)",
        "Advanced enterprise coordination primitives in pure Roze: CompletableFuture<T> with monadic composition (then_apply, then_compose, all_of, any_of), ForkJoinPool work-stealing parallel engine, CountDownLatch, CyclicBarrier, Semaphore, Exchanger, ThreadPoolExecutor, AtomicReference<T>.",
        "Direct parity with Java 25 java.util.concurrent.* and Rust 1.99 crossbeam.",
        "High-Concurrency Servers, Parallel Computing", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M7-T07", 8), ("Month 7", 22), ("Sprint 14 (W29-W30)", 8), "Milestone Gate M7", "Enterprise Standard Library Benchmark Gate",
        "100% test pass on all mathematical, collections, temporal, nio, and concurrent fixtures; 1,000,000 BigInt operations stress test; zero memory leaks under ARC; dual JVM and Native Cranelift parity.",
        "Proves full Java 25 java.util.* and Python 3.15 standard library equivalence in pure Roze.",
        "Core Standard Library & Systems", "3 Days", ("Critical", 14), ("Completed", 16)
    ],

    # MONTH 8: ROZE-TORCH & AI / MACHINE LEARNING FRAMEWORK (PYTORCH + NUMPY PARITY)
    [
        ("M8-T01", 8), ("Month 8", 23), ("Sprint 15 (W31-W32)", 8), "Tensor Computation", "N-Dimensional Tensor Engine & Vector Arithmetic (roze-torch)",
        "Multidimensional array engine in pure Roze: Tensor<T, D> supporting arbitrary ranks (0D to 6D), dtypes (f32, f64, f16, bf16, i32, i64, u8, bool), strided memory layout, slicing with negative indices/steps, broadcasting semantics matching NumPy/PyTorch, matrix transposition, contiguous re-allocation, tensor contraction.",
        "1:1 mathematical and architectural parity with NumPy ndarray and PyTorch Tensor.",
        "AI / Machine Learning, Numerical Computing", "9 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M8-T02", 8), ("Month 8", 23), ("Sprint 15 (W31-W32)", 8), "Automatic Differentiation", "Dynamic Computational Graph Autograd Tape (roze-torch)",
        "Dynamic computational graph tape in pure Roze: forward pass operation recording, topological sort of DAG nodes, reverse-mode gradient backpropagation (tensor.backward()), in-place gradient accumulation (grad), no_grad() execution scopes, and custom autograd function definition API.",
        "Full parity with PyTorch torch.autograd dynamic backpropagation engine in pure Roze.",
        "Deep Learning, Optimization, Neural Networks", "9 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M8-T03", 8), ("Month 8", 23), ("Sprint 15 (W31-W32)", 8), "Neural Network Layers", "Deep Learning Modules & Activation Functions (roze-torch::nn)",
        "Pure Roze neural network layer modules: nn.Module base class with parameter registration, nn.Linear, nn.Conv2d, nn.MaxPool2d, nn.AvgPool2d, nn.BatchNorm2d, nn.LayerNorm, nn.Dropout, nn.Embedding, nn.MultiheadAttention, nn.Sequential. Activations: ReLU, GELU, SiLU, Sigmoid, Softmax.",
        "Direct parity with PyTorch torch.nn module ecosystem.",
        "Computer Vision, NLP, Deep Learning", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M8-T04", 8), ("Month 8", 23), ("Sprint 16 (W33-W34)", 8), "Optimizers & Loss", "Numerical Optimizers & Loss Functions (roze-torch::optim)",
        "Production optimizers: SGD (with momentum and Nesterov), Adam, AdamW (decoupled weight decay), RMSprop, Adagrad. Learning rate schedulers: StepLR, CosineAnnealingLR. Loss functions: MSELoss, CrossEntropyLoss, BCEWithLogitsLoss, L1Loss.",
        "Direct parity with PyTorch torch.optim and standard deep learning training loops.",
        "Machine Learning Training, Model Tuning", "7 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M8-T05", 8), ("Month 8", 23), ("Sprint 16 (W33-W34)", 8), "LLM Inference Engine", "Local Transformer Decoder & GGUF/Safetensors Loader",
        "Transformer decoder engine in pure Roze: Rotary Positional Embeddings (RoPE), SwiGLU feed-forward networks, RMSNorm, KV-cache acceleration with speculative decoding, BPE tokenizer, model loaders for safetensors zero-copy format and GGUF quantizations (Q4_0, Q4_K_M, Q8_0, F16).",
        "Runs LLaMA-3 and Mistral 7B models locally in pure Roze with zero Python runtime dependency.",
        "Generative AI, Large Language Models, Edge AI", "9 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M8-T06", 8), ("Month 8", 23), ("Sprint 16 (W33-W34)", 8), "Hardware Acceleration", "CUDA FFI Kernels, OpenBLAS & Hardware SIMD",
        "Direct hardware compute acceleration bridges: CUDA runtime and cuBLAS / cuDNN bindings via pure Roze FFI, custom PTX kernel execution, CPU fallback with AVX-512 / AVX2 / ARM NEON vectorization, and OpenBLAS / oneMKL bindings.",
        "Sub-10ms token generation latency on NVIDIA GPUs and multi-core CPU SIMD.",
        "GPU Computing, High-Performance AI", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M8-T07", 8), ("Month 8", 23), ("Sprint 16 (W33-W34)", 8), "Milestone Gate M8", "Deep Learning & 7B LLM Inference Benchmark Gate",
        "Verified end-to-end training of ResNet and Transformer architectures; verified real-time token generation for LLaMA-3 8B model at >35 tokens/sec on GPU; 100% test pass on autograd gradient verification checks.",
        "Proves PyTorch + NumPy equivalence in pure Roze with zero Python runtime.",
        "AI / Machine Learning & High-Performance Compute", "3 Days", ("Critical", 14), ("Completed", 16)
    ],

    # MONTH 9: CYBERSECURITY, HACKING & DEFENSIVE FORENSICS (SCAPY + NMAP + VOLATILITY PARITY)
    [
        ("M9-T01", 8), ("Month 9", 24), ("Sprint 17 (W35-W36)", 8), "Low-Level Frame Engine", "Promiscuous Raw Sockets & Packet Injection (roze.sec.raw)",
        "Low-level packet capture and injection engine: Linux AF_PACKET with PACKET_MMAP circular ring buffers, macOS/BSD BPF (Berkeley Packet Filter), and Windows Npcap bindings. Promiscuous hardware interface binding, hardware timestamping, zero-copy packet extraction, and raw frame transmission at line rate (10GbE+).",
        "Parity with libpcap and Linux kernel socket interfaces in pure Roze.",
        "Cybersecurity, Network Engineering, Sniffing", "8 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M9-T02", 8), ("Month 9", 24), ("Sprint 17 (W35-W36)", 8), "Protocol Stacking DSL", "Protocol Builder & Scapy-Class Stacking DSL (roze.sec.packet)",
        "Fluent protocol stacking DSL utilizing the '/' operator (Ether() / IP() / TCP() / Raw()): full support for Ethernet II, 802.1Q VLAN, ARP, IPv4, IPv6, ICMP, ICMPv6, TCP (custom flags SYN/ACK/FIN/RST/PSH/URG/ECE/CWR), UDP, DNS, DHCP, NTP, SNMP, HTTP/1.1, TLS 1.3 ClientHello. Automated RFC 1071 internet checksum recalculation.",
        "Direct parity with Python Scapy protocol stacking DSL with native compiled performance.",
        "Penetration Testing, Exploit Construction, Network Protocol Security", "9 Days", ("Critical", 14), ("Completed", 16)
    ],
    [
        ("M9-T03", 8), ("Month 9", 24), ("Sprint 17 (W35-W36)", 8), "Stealth Reconnaissance", "Distributed Multi-Core Stealth Network Scanner (roze.sec.scan)",
        "Multi-core async network reconnaissance engine: stealth SYN scanning, FIN scanning, Xmas scanning, Null scanning, UDP port probing, passive and active TCP/IP stack OS fingerprinting (TTL, TCP window size, DF bit, options order), service banner identification engine (SSH, HTTP, SMTP, MySQL, Redis, TLS certs), achieving 100,000+ probes/sec per CPU core.",
        "Parity with Nmap and ZMap performance in pure Roze with zero external dependencies.",
        "Red Teaming, Network Defense, Attack Surface Management", "8 Days", ("High", 14), ("Completed", 16)
    ],
    [
        ("M9-T04", 8), ("Month 9", 24), ("Sprint 18 (W37-W38)", 8), "Multi-Arch Disassembler", "Static Binary Analysis & Instruction Decoders (roze.sec.disasm)",
        "Static binary inspection engine: ELF (32/64-bit), PE/COFF (PE32/PE32+), Mach-O (64-bit), and WebAssembly binary modules. Instruction decoders for x86_64 (prefixes, ModR/M, SIB, REX, AVX/SSE), ARM64 (AArch64), RISC-V (RV32I/RV64I), and WASM bytecode. High-precision symbol demangling (Itanium C++ ABI, Rust v0, Roze namespaces).",
        "Matches capabilities of radare2 and Capstone disassembler in pure Roze.",
        "Reverse Engineering, Malware Analysis, Binary Auditing", "8 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M9-T05", 8), ("Month 9", 24), ("Sprint 18 (W37-W38)", 8), "Memory Forensics", "Defensive Heap Forensics & Exploit Analysis (roze.sec.forensics)",
        "Memory forensics suite: live process memory auditing (/proc/<pid>/maps, VirtualQueryEx), ELF core dump and Windows memory dump parser, Shannon entropy calculation for packed/encrypted malware detection, RWX / W+X memory permission violation auditor, ROP gadget harvester, stack pivot sequence detector, and sandboxed CPU emulator.",
        "Meets and exceeds capabilities of Volatility and memory analysis frameworks.",
        "Incident Response, Blue Teaming, Threat Hunting", "8 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M9-T06", 8), ("Month 9", 24), ("Sprint 18 (W37-W38)", 8), "Milestone Gate M9", "Full-Spectrum Cybersecurity & Penetration Testing Gate",
        "Automated offensive packet crafting and defensive memory inspection verification suite; 100% detection rate on synthetic ROP chain and RWX memory injection; 100,000 packets/sec SYN scan benchmark passing.",
        "Proves Python Scapy + Nmap + Volatility equivalence in pure Roze with zero external C libraries.",
        "Cybersecurity, Offensive & Defensive Operations", "3 Days", ("Critical", 14), ("Milestone Gate", 15)
    ],

    # MONTH 10: ROZEBOOT 3.0 ENTERPRISE CLOUD FRAMEWORK (SPRING BOOT 3 PARITY)
    [
        ("M10-T01", 8), ("Month 10", 25), ("Sprint 19 (W39-W40)", 8), "Enterprise IoC & DI", "Advanced IoC Container & Metaprogramming (rozeboot.core)",
        "Advanced Inversion of Control container in pure Roze: @Component, @Service, @Repository, @Controller, @RestController, @Autowired, @Qualifier, @Value, @Configuration, @Bean, @Scope('singleton'|'prototype'), @PostConstruct, @PreDestroy. Topological dependency graph resolution, circular dependency detection with cycle diagnostics, and conditional bean loading.",
        "1:1 architectural parity with Spring Framework 6 / Spring Boot 3 core container.",
        "Enterprise Web, Cloud Microservices, Architecture", "8 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M10-T02", 8), ("Month 10", 25), ("Sprint 19 (W39-W40)", 8), "High-Throughput Web", "Reactive HTTP/2, HTTP/3, WebSockets & gRPC (rozeboot.web)",
        "Async non-blocking web server engine: HTTP/1.1 pipelining, HTTP/2 multiplexed streams with HPACK compression, HTTP/3 over QUIC with QPACK compression, WebSockets (RFC 6455) bidirectional messaging, gRPC / Protocol Buffers RPC services, URI template routing (/api/v1/users/{id:[0-9]+}), request body JSON/CBOR binding, and Jakarta-style validation (@NotNull, @Size, @Email).",
        "Performance exceeds Netty and matches Actix-web in pure Roze with zero external dependencies.",
        "Cloud APIs, Streaming Services, Microservices", "9 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M10-T03", 8), ("Month 10", 25), ("Sprint 19 (W39-W40)", 8), "Enterprise ORM", "Type-Safe Active Record & Connection Pool (roze-orm)",
        "Relational persistence engine: @Entity, @Table, @Id, @Column, @OneToMany, @ManyToOne, @ManyToMany, @Version (optimistic locking). High-performance connection pool with borrow queues, health checks, leak detection telemetry. Type-safe SQL query builder, automated schema migrations (roze_schema_migrations), transaction manager (@Transactional with ACID isolation levels).",
        "1:1 architectural parity with Hibernate, Spring Data JPA, and HikariCP.",
        "Enterprise Persistence, Banking Systems, Relational DBs", "9 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M10-T04", 8), ("Month 10", 25), ("Sprint 20 (W41-W42)", 8), "Enterprise Security", "Cloud Security Perimeter & OAuth2/JWT (rozeboot.security)",
        "Security framework: Built-in JWT authentication filter, OAuth 2.0 / OIDC authorization server and resource server filters, Role-Based Access Control (@PreAuthorize(\"hasRole('ADMIN')\")), Argon2id password hashing, CSRF token validation, CORS configuration, rate limiting / token bucket filter.",
        "Full parity with Spring Security 6 on Java 25.",
        "Cloud Security, Identity Management, Compliance", "8 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M10-T05", 8), ("Month 10", 25), ("Sprint 20 (W41-W42)", 8), "Cloud Observability", "OpenTelemetry Tracing, Prometheus & OpenAPI 3.1 (rozeboot.actuator)",
        "Production cloud management: Prometheus /actuator/metrics exposition (counters, gauges, latency histograms), OpenTelemetry distributed tracing with W3C traceparent propagation, /actuator/health probes (readiness, liveness), OpenAPI 3.1 JSON generation, and embedded Swagger UI responsive dashboard.",
        "Matches Spring Boot Actuator and Micrometer cloud observability.",
        "DevOps, Site Reliability Engineering, Cloud Native", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M10-T06", 8), ("Month 10", 25), ("Sprint 20 (W41-W42)", 8), "Milestone Gate M10", "250,000 req/sec Enterprise Banking Microservice Stress Test",
        "Full end-to-end verification of banking microservice under 50,000 concurrent green thread connections, ACID atomic fund transfers, zero balance drift, sub-millisecond P99 latency (<0.8ms), and zero memory leaks under continuous load.",
        "Proves Spring Boot 3 enterprise parity in pure Roze with sub-millisecond latency.",
        "Enterprise Web & Financial Microservices", "3 Days", ("Critical", 14), ("Milestone Gate", 15)
    ],

    # MONTH 11: REAL-TIME GAME ENGINE PLATFORM & NATIVE DESKTOP GUI (ROZE-GAME & ROZE-UI)
    [
        ("M11-T01", 8), ("Month 11", 26), ("Sprint 21 (W43-W44)", 8), "Game Engine Core", "Archetypal ECS & Real-Time Engine (roze-game.ecs)",
        "High-performance cache-friendly archetypal ECS architecture in pure Roze: Entity, Component, System, World, query iterators (world.query<Position, Velocity>()), scene graph hierarchy, transform matrices, and 60+ FPS game loop with fixed-timestep physics updates and interpolated rendering.",
        "Parity with Rust Bevy ECS and modern game engine core architectures.",
        "2D/3D Games, Simulations, Interactive Media", "8 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M11-T02", 8), ("Month 11", 26), ("Sprint 21 (W43-W44)", 8), "Hardware Rendering", "Hardware-Accelerated 2D/3D Graphics (roze-game.render)",
        "Direct rendering pipeline: Raylib C-FFI and Vulkan / OpenGL modern abstractions, sprite batching, texture atlases, 3D mesh rendering, lighting models (ambient, diffuse, specular, normal mapping), skeletal animation, GLSL shader pipeline, and camera controllers (orthographic 2D, perspective 3D, first-person, orbital).",
        "Matches Raylib and modern Vulkan 2D/3D graphics capabilities.",
        "Game Graphics, 3D Visualization, Virtual Reality", "9 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M11-T03", 8), ("Month 11", 26), ("Sprint 21 (W43-W44)", 8), "Spatial Physics & Audio", "Rigid-Body Physics & 3D Spatial Audio (roze-game.physics)",
        "Simulation suite: 2D/3D collision detection (AABB, OBB, Raycast, Sphere-Capsule, GJK algorithm), rigid-body dynamics (gravity, friction, restitution, impulse resolution), spatial hashing broadphase grid, particle emitters, and multi-channel 3D spatial audio mixer.",
        "Full game physics and audio engine in pure Roze.",
        "Game Development, Physical Simulation, Audio Synthesis", "8 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M11-T04", 8), ("Month 11", 26), ("Sprint 22 (W45-W46)", 8), "Native Windowing", "Cross-Platform Native Windowing & GPU Canvas (roze-ui.canvas)",
        "Desktop platform backends: Win32 API on Windows, Wayland and X11 on Linux, Cocoa on macOS. GPU-accelerated 2D vector path rasterizer (Bézier curves, gradients, anti-aliased text, box shadows, clipping regions) with direct hardware VSync.",
        "Parity with Skia and native OS windowing abstractions.",
        "Desktop Applications, Native UI, Vector Graphics", "8 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M11-T05", 8), ("Month 11", 26), ("Sprint 22 (W45-W46)", 8), "Reactive Desktop UI", "Reactive Desktop UI Toolkit & Flexbox Layout (roze-ui.widgets)",
        "Declarative desktop UI framework: CSS-compliant Flexbox layout engine (flex-direction, justify-content, align-items, flex-grow, flex-shrink), reactive state signals (State<T>, Computed<T>, Effect), full widget hierarchy (Window, Button, TextInput, Label, Slider, CheckBox, DataGrid, TreeView, ScrollView, Dropdown, Dialog, Canvas), embedded Webview container.",
        "Parity with JavaFX, Qt, and Tauri in pure Roze with zero external dependencies.",
        "Desktop Productivity, Enterprise Frontends, Tools", "9 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M11-T06", 8), ("Month 11", 26), ("Sprint 22 (W45-W46)", 8), "Milestone Gate M11", "60 FPS 3D Game & Cross-Platform Desktop GUI Gate",
        "Real-time verification of 3D game rendering at stable 60 FPS with 10,000 active ECS entities and physics simulation; native desktop application running with sub-16ms responsive UI event dispatch on Windows, Linux, and macOS.",
        "Proves Raylib/Bevy game engine parity and JavaFX/Qt desktop UI parity in pure Roze.",
        "Gaming, Interactive Media, Desktop Applications", "3 Days", ("Critical", 14), ("Milestone Gate", 15)
    ],

    # MONTH 12: FREESTANDING 64-BIT OS KERNEL, MICROCONTROLLER IOT & ROZE v3.0 'TITAN' LAUNCH
    [
        ("M12-T01", 8), ("Month 12", 27), ("Sprint 23 (W47-W48)", 8), "Freestanding OS Kernel", "Freestanding 64-Bit Operating System Kernel (RozeOS v3)",
        "Bare-metal OS kernel written in pure Roze (--target freestanding): Multiboot2 / UEFI bootloader, preemptive round-robin thread scheduler, SMP multicore initialization via APIC/MADT, 4-level paging virtual memory manager, Buddy physical frame allocator, kernel heap allocator with cache alignment, IDT/GDT interrupt tables, PCI bus scan, VirtIO network and block device drivers, interactive kernel shell.",
        "Full freestanding 64-bit operating system kernel matching Linux/Rust OS architectures in pure Roze.",
        "Operating Systems, Kernels, Bare-Metal Systems", "9 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M12-T02", 8), ("Month 12", 27), ("Sprint 23 (W47-W48)", 8), "Microcontroller IoT", "Microcontroller Hardware Platform & PAC SVD Generator",
        "Embedded cross-compilation pipeline: ARM Cortex-M (thumbv6m, thumbv7m, thumbv7em) and RISC-V (riscv32imc, riscv64gc). Pure Roze embedded-hal driver traits (GPIO, I2C, SPI, UART, PWM, ADC), automated CMSIS-SVD register generator (roze svd), bare-metal linker scripts, interrupt vector tables, direct flashing CLI (roze flash) for STM32, ESP32, RP2040.",
        "Direct parity with Rust embedded-hal and C CMSIS/HAL micro-firmware architectures.",
        "Microcontrollers, IoT Edge, Automotive, Robotics", "8 Days", ("Critical", 14), ("Planned", 12)
    ],
    [
        ("M12-T03", 8), ("Month 12", 27), ("Sprint 23 (W47-W48)", 8), "WebAssembly & WASI", "WebAssembly & WASI 0.2 Component Model (roze-wasm)",
        "Direct WebAssembly 1.0 (MVP) and WASI snapshot-preview1 / WASI 0.2 Component Model binary code generator. Linear memory buffer export, dynamic heap allocator, DOM event bindings, WebGL canvas context, and roze-bindgen automated TypeScript/ESM bridge for browser and cloud serverless workers (Cloudflare, Fastly).",
        "Parity with Rust wasm-bindgen and Go WebAssembly target.",
        "WebAssembly, Serverless Cloud, Browser Apps", "8 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M12-T04", 8), ("Month 12", 27), ("Sprint 24 (W49-W52)", 8), "Toolchain Monolith", "Basir ('bsr') Global Registry & Monolithic Driver",
        "Official package registry infrastructure: Go module proxy protocol (/@v/list, /@v/<version>.info, /@v/<version>.mfz), RFC 8032 Ed25519 digital package envelope signing, SemVer SAT solver, dependency vendoring (bsr vendor), monolithic CLI driver (roze unifying build, run, test, fmt, doc, repl, lsp, svd, bindgen, flash, pkg).",
        "1:1 parity with Cargo / crates.io and Go modules ecosystem.",
        "Developer Tooling, Community, Package Ecosystem", "7 Days", ("High", 14), ("Planned", 12)
    ],
    [
        ("M12-T05", 8), ("Month 12", 27), ("Sprint 24 (W49-W52)", 8), "Final Release M12", "Official Roze v3.0 'Titan' Production Launch",
        "Official global release: 100% pure Roze self-hosted compiler, zero external language dependency, 3-stage bootstrap fixpoint idempotence verified (0 bytes diff), complete standard library manual, multi-platform installer packages (deb, rpm, Homebrew, MSI, pkg, vsix), and RozeHub portal documentation. Roze officially established as an autonomous, general-purpose programming language standing on par with Java 25, Rust 1.99.1, and Python 3.15.2.",
        "Roze v3.0 'Titan' officially launched as an autonomous, general-purpose programming language on par with Java 25, Rust 1.99.1 & Python 3.15.2 across all 10 computing domains.",
        "Global Release & Universal Production Ecosystem", "5 Days", ("Critical", 14), ("Milestone Gate", 15)
    ]
]

# ==================== DATA FOR SHEET 2: STDLIB ARCHITECTURE ====================
s2_headers = [
    "Module Name", "Namespace / Import", "Core Classes & Structs", "Primary Built-in Methods & Functions",
    "Java 25 Benchmark Equivalent", "Rust 1.99.1 Benchmark Equivalent", "Python 3.15 Benchmark Equivalent", "Target Application Scope"
]
s2_widths = [18, 20, 28, 48, 28, 28, 26, 24]

s2_rows = [
    [
        "Core & Primitives", "roze::core", "Int, Float, Bool, String, Char, Byte, RawPointer",
        "abs, min, max, clamp, string_length, substring, to_upper, to_lower, trim, split, replace, parse_int, parse_float, hash, assert, panic",
        "java.lang (Math, String, Integer)", "core::primitive, std::cmp", "builtins (str, int, float, bool)", "Universal Fundamentals"
    ],
    [
        "Arbitrary Math", "roze::math", "BigInt, BigDecimal, Math, FastMath, Complex, Random, SecureRandom",
        "add, sub, mul, div, mod, pow, sqrt, karatsuba, mod_pow, is_probable_prime, round, set_scale, sin, cos, tan, log, exp, next_int, next_gaussian",
        "java.math.BigInteger, java.math.BigDecimal", "num-bigint, num-rational", "math, decimal.Decimal", "Fintech, Cryptography, Science"
    ],
    [
        "Enterprise Collections", "roze::util / roze::collections", "ArrayList<T>, LinkedList<T>, HashMap<K,V>, LinkedHashMap<K,V>, HashSet<T>, TreeSet<T>, TreeMap<K,V>, ArrayDeque<T>, PriorityQueue<T>, BitSet",
        "push, pop, get, set, insert, remove, clear, contains, sort, binary_search, shuffle, reverse, unmodifiable_view, synchronized_view, peek, drain, extend, iter",
        "java.util (ArrayList, HashMap, TreeSet, PriorityQueue)", "std::collections (Vec, HashMap, BTreeMap, BinaryHeap)", "collections (list, dict, set, deque)", "Data Structures & Algorithms"
    ],
    [
        "Temporal Engine", "roze::time", "Instant, Duration, Period, LocalDate, LocalTime, LocalDateTime, ZonedDateTime, ZoneId, ZoneOffset, DateTimeFormatter",
        "now, elapsed, duration_since, parse_iso8601, format, plus_days, minus_hours, between, with_zone, get_year, get_month, get_day, is_leap_year",
        "java.time.* (Instant, ZonedDateTime, Duration)", "chrono (DateTime, Utc, Duration)", "datetime, time, zoneinfo", "Temporal Systems, Logging, Schedulers"
    ],
    [
        "Python Functional", "roze::itertools / roze::functools", "Itertools, Functools, Permutations, Combinations, Product, Cycle, Repeat, LruCache",
        "permutations, combinations, product, cycle, repeat, chain, groupby, islice, reduce, partial, lru_cache, memoize, curry, compose",
        "java.util.stream.Stream, java.util.function", "itertools crate, Iterator trait", "itertools, functools", "Functional Data Processing & Pipelines"
    ],
    [
        "Data Science & Stats", "roze::statistics", "Statistics, LinearRegression, Quantiles, Distribution, Covariance",
        "mean, median, mode, variance, stdev, pvariance, pstdev, quantiles, covariance, correlation, linear_regression, z_score, normal_cdf",
        "Apache Commons Math", "statrs crate", "statistics (mean, median, stdev)", "Data Science & Numerical Analytics"
    ],
    [
        "Non-Blocking I/O", "roze::nio", "ByteBuffer, DirectBuffer, MemoryMappedFile, FileChannel, SocketChannel, Selector, SelectionKey",
        "allocate, allocate_direct, mmap, flip, rewind, compact, slice, duplicate, order, read, write, select, selected_keys, register, close",
        "java.nio (ByteBuffer, FileChannel, Selector)", "mio, tokio-core", "asyncio, mmap", "High-Throughput Non-Blocking I/O"
    ],
    [
        "Enterprise Concurrency", "roze::concurrent", "CompletableFuture<T>, ForkJoinPool, CountDownLatch, CyclicBarrier, Semaphore, Exchanger, ThreadPoolExecutor, AtomicReference<T>",
        "then_apply, then_compose, all_of, any_of, join, get, count_down, await, acquire, release, exchange, submit, execute, atomic_set, compare_and_set",
        "java.util.concurrent.* (CompletableFuture, ForkJoinPool)", "crossbeam, rayon", "concurrent.futures, threading", "Enterprise Parallel Computing"
    ],
    [
        "Lightweight Sync", "roze::sync", "GreenThread, Fiber, Channel<T>, Mutex<T>, RwLock<T>, AtomicBool, AtomicI64, AtomicPtr, Nursery, Barrier",
        "spawn, sleep, yield_now, channel_new, send, recv, try_recv, lock, read, write, atomic_load, atomic_store, atomic_swap, nursery_open, cancel",
        "java.lang.Thread (Virtual), java.util.concurrent.locks", "std::sync, std::thread, parking_lot", "threading, queue.Queue", "Multi-Threading & Fiber Schedulers"
    ],
    [
        "Filesystem & Paths", "roze::io", "Path, File, Directory, WalkDir, BufReader, BufWriter, Pipe, Process, Stdin, Stdout, Stderr",
        "path_new, exists, is_file, is_dir, read_to_string, read_bytes, write_all, append, delete, create_dir_all, read_dir, canonicalize, copy, rename, spawn_process",
        "java.nio.file (Files, Path), java.io", "std::fs, std::io, std::path", "pathlib.Path, os, sys", "Filesystem, CLI Tools, OS Interop"
    ],
    [
        "Networking & Sockets", "roze::net", "TcpListener, TcpStream, UdpSocket, DnsResolver, SocketAddr, IpAddr, Ipv4, Ipv6",
        "bind, accept, connect, read_exact, write_all, send_to, recv_from, set_nodelay, set_read_timeout, resolve_hostname, local_addr, peer_addr",
        "java.net (Socket, ServerSocket, InetAddress)", "std::net (TcpStream, UdpSocket)", "socket", "Low-Level Network I/O"
    ],
    [
        "Text & Regex", "roze::regex", "Regex, Match, CaptureGroup, StringBuilder, Formatter, UnicodeScanner",
        "compile, is_match, find, find_all, captures, replace, replace_all, split, append_str, format, graphemes",
        "java.util.regex.Pattern, StringBuilder", "regex crate, unicode-segmentation", "re (compile, match, search)", "Text Manipulation & Compilers"
    ],
    [
        "Universal Serialization", "roze::serde", "Serializer, Deserializer, Json, Yaml, Toml, MsgPack, Protobuf, Value",
        "to_json, from_json, to_yaml, from_yaml, to_toml, from_toml, to_msgpack, from_msgpack, protobuf_encode, protobuf_decode, serialize, deserialize",
        "Jackson, SnakeYAML, Protocol Buffers", "serde, serde_json, toml, rmp", "json, toml, pickle", "Universal Data Interchange"
    ],
    [
        "Standard Cryptography", "roze::crypto", "Aes256Gcm, ChaCha20Poly1305, Sha256, Sha512, Keccak256, HmacSha256, Ed25519, X25519, Rsa4096, TlsSession",
        "encrypt, decrypt, digest, hmac, sign, verify, keygen, diffie_hellman, tls_connect, tls_accept, tls_handshake, random_bytes",
        "java.security, javax.crypto", "ring, rustls, ed25519-dalek", "cryptography, hashlib, hmac", "Data Protection, Auth, TLS 1.3"
    ],
    [
        "Raw Packet Security", "roze::sec::raw", "RawSocket, PacketRingBuffer, FrameInjector, BpfFilter, PcapWriter",
        "raw_socket_open, promiscuous_mode, bind_interface, capture_packet, inject_frame, set_bpf_filter, pcap_dump, pcap_read",
        "Jpcap, Pcap4J", "pnet, libpcap bindings", "scapy.sendrecv, pcap", "Packet Sniffing & Line-Rate Capture"
    ],
    [
        "Packet Crafting DSL", "roze::sec::packet", "Ether, Arp, IPv4, IPv6, Icmp, Tcp, Udp, Dns, Dhcp, PacketBuilder",
        "stack (operator /), craft, dissect, summary, detail, calculate_checksum, get_flag, set_flag, has_layer, get_layer",
        "Custom Packet Builders", "pnet::packet", "scapy.all (Ether/IP/TCP)", "Network Security & Penetration Testing"
    ],
    [
        "Stealth Reconnaissance", "roze::sec::scan", "SynScanner, FinScanner, XmasScanner, OsFingerprinter, ServiceBannerParser, ScanReport",
        "syn_scan, scan_range, fingerprint_os, grab_banner, parse_probe_result, export_report_table, benchmark_scan_rate",
        "N/A (Specialized Security)", "Custom SYN Scanners", "nmap, python-nmap", "Threat Reconnaissance & Auditing"
    ],
    [
        "Binary Disassembly", "roze::sec::disasm", "ElfParser, PeParser, MachOParser, WasmParser, Disassembler, Instruction, Demangler",
        "parse_elf, parse_pe, parse_macho, parse_wasm, disassemble_x86_64, disassemble_arm64, disassemble_riscv, demangle_symbol",
        "Java Bytecode / Asm", "zydis, goblin, object", "capstone, struct, dis", "Reverse Engineering & Binary Forensics"
    ],
    [
        "Memory Forensics", "roze::sec::forensics", "ProcessMemory, MemoryMap, CoreDumpParser, EntropyScanner, RopGadgetFinder, SandboxedCpu",
        "inspect_process_maps, audit_rwx_permissions, calculate_shannon_entropy, harvest_rop_gadgets, detect_stack_pivots, step_sandbox",
        "N/A (Specialized Forensics)", "Custom Memory Scanners", "volatility, ropper", "Threat Hunting & Incident Response"
    ],
    [
        "Enterprise IoC Container", "roze::boot::core", "IoCContainer, BeanFactory, ComponentRegistry, DependencyGraph, BeanDefinition",
        "register_singleton, register_prototype, autowire, resolve_dependency, detect_circular_dependencies, trigger_post_construct, trigger_pre_destroy",
        "Spring Framework 6 (ApplicationContext)", "N/A (Rust uses compile-time DI)", "dependency_injector, injector", "Enterprise Dependency Injection"
    ],
    [
        "Enterprise Web Server", "roze::boot::web", "EnterpriseServer, Router, Request, Response, FilterChain, Middleware, WebSocket, GrpcService",
        "listen, get, post, put, delete, use_filter, json_respond, handle_websocket, serve_grpc, bind_body, validate_request",
        "Spring Boot 3 (Spring MVC, WebFlux)", "Actix-web, Axum, Tower", "FastAPI, Starlette", "Enterprise Cloud Microservices"
    ],
    [
        "Enterprise Relational ORM", "roze::orm", "EntityManager, ConnectionPool, QueryBuilder, Repository<T>, Transaction, SchemaMigration",
        "pool_create, get_connection, begin_transaction, commit, rollback, select, where, join, insert, update, delete, migrate_apply",
        "Hibernate, Spring Data JPA, HikariCP", "Diesel, SeaORM, sqlx", "SQLAlchemy, Tortoise ORM", "Enterprise Relational Persistence"
    ],
    [
        "Enterprise Cloud Security", "roze::boot::security", "SecurityContext, JwtFilter, OAuth2Validator, RbacGatekeeper, Argon2idHasher",
        "authenticate_jwt, validate_oauth2_token, authorize_role, hash_password, verify_password, protect_csrf, configure_cors",
        "Spring Security 6", "N/A (Rust uses framework middleware)", "fastapi.security, authlib", "Cloud Security & Access Control"
    ],
    [
        "Cloud Observability", "roze::boot::actuator", "MetricRegistry, Counter, Gauge, Histogram, Tracer, Span, OpenApiSpec, SwaggerUI",
        "increment, record_gauge, observe_latency, export_prometheus, start_span, inject_traceparent, generate_openapi, render_swagger_ui",
        "Spring Boot Actuator, Micrometer", "metrics, tracing, opentelemetry", "prometheus_client, opentelemetry", "Cloud DevOps & Monitoring"
    ],
    [
        "N-D Tensor Engine", "roze::tensor", "Tensor<T, D>, Storage, Strides, Shape, DType, View",
        "tensor_new, zeros, ones, randn, arange, reshape, transpose, permute, squeeze, unsqueeze, slice, matmul, add, mul, div, broadcast",
        "ND4J (Deeplearning4j)", "ndarray, candle-core", "numpy.ndarray, torch.Tensor", "Scientific & Tensor Computing"
    ],
    [
        "Reverse Autograd", "roze::autograd", "AutogradTape, Node, GradFn, Variable, NoGradScope",
        "backward, grad, register_hook, no_grad, zero_grad, retain_grad, custom_backward",
        "Deeplearning4j Autodiff", "burn-autograd", "torch.autograd", "Automatic Differentiation"
    ],
    [
        "Deep Learning Layers", "roze::nn", "Module, Linear, Conv2d, MaxPool2d, BatchNorm2d, LayerNorm, Dropout, MultiheadAttention, Sequential",
        "forward, parameters, zero_grad, train, eval, state_dict, load_state_dict",
        "Deeplearning4j NeuralNet", "burn-nn, candle-nn", "torch.nn.Module", "Deep Learning Model Construction"
    ],
    [
        "Numerical Optimizers", "roze::optim", "SGD, Adam, AdamW, RMSprop, StepLR, CosineAnnealingLR, Loss, CrossEntropyLoss, MSELoss",
        "step, zero_grad, get_lr, calculate_loss, backward_loss",
        "Deeplearning4j Optimizers", "burn-train", "torch.optim", "Neural Network Training"
    ],
    [
        "Transformer & LLM", "roze::ai::transformer", "TransformerDecoder, RoPE, SwiGLU, RMSNorm, KVCache, BpeTokenizer, GgufLoader, SafetensorsLoader",
        "load_gguf, load_safetensors, tokenize, decode, forward_step, generate_tokens, kv_cache_update",
        "Deeplearning4j NLP", "candle-transformers", "transformers (HuggingFace)", "Local LLM Inference (LLaMA/Mistral)"
    ],
    [
        "GPU Acceleration", "roze::cuda", "CudaDevice, CudaStream, CudaBuffer, CuBlas, CuDnn, PtxKernel",
        "cuda_init, to_gpu, to_cpu, cublas_sgemm, cublas_dgemm, launch_kernel, sync_stream",
        "JavaCPP Presets (CUDA)", "cudarc, cust", "torch.cuda, cupy", "Hardware Accelerated Computing"
    ],
    [
        "Archetypal ECS", "roze::game::ecs", "World, Entity, Component, System, Query<T...>, Archetype, Schedule",
        "spawn, despawn, add_component, remove_component, query, for_each, run_systems",
        "Artemis-odb, Fleks", "bevy_ecs, hecs", "esper", "Game Entity Architecture"
    ],
    [
        "Hardware Rendering", "roze::game::render", "Renderer, SpriteBatch, Texture2D, Mesh, Material, Shader, Camera2D, Camera3D, Light",
        "begin_drawing, end_drawing, clear_background, draw_sprite, draw_mesh, begin_mode_3d, end_mode_3d, set_shader",
        "libGDX, jMonkeyEngine", "wgpu, raylib-rs", "pyray, arcade", "2D/3D Real-Time Graphics"
    ],
    [
        "Physics & Spatial Audio", "roze::game::physics", "PhysicsWorld, RigidBody, Collider, RaycastHit, AudioMixer, Sound, Music, SpatialSource",
        "physics_step, raycast, apply_force, apply_impulse, play_sound, play_music, set_sound_position",
        "JBox2D, jME Physics", "rapier, kira", "pymunk, pygame.mixer", "Game Physics & 3D Audio"
    ],
    [
        "Native Desktop Canvas", "roze::ui::canvas", "UiWindow, Canvas2D, Path2D, Paint, Font, EventQueue, VSyncTimer",
        "create_window, poll_events, begin_frame, end_frame, draw_path, fill_rect, stroke_text, set_vsync",
        "JavaFX GraphicsContext", "winit, softbuffer, tiny-skia", "pygame, PyQt6.QtGui", "Cross-Platform Desktop Canvas"
    ],
    [
        "Reactive Desktop Widgets", "roze::ui::widgets", "App, Widget, State<T>, Flexbox, Button, TextInput, Label, Slider, DataGrid, TreeView, Webview",
        "run_app, set_state, on_click, on_change, compute_layout, render_tree, embed_webview",
        "JavaFX Controls & FXML", "slint, iced, egui", "PyQt6.QtWidgets, tkinter", "Desktop Productivity Applications"
    ],
    [
        "Freestanding Kernel", "roze::os", "PageTable, FrameAllocator, GdtManager, IdtManager, ApicController, VirtIoNet, VirtIoBlock, KernelShell",
        "map_virtual_page, alloc_physical_frame, idt_register_handler, apic_send_eoi, virtio_transmit, virtio_read_block, run_kernel_shell",
        "N/A (Java cannot run freestanding)", "x86_64, bootloader, bare-metal crates", "N/A (Python cannot run freestanding)", "Operating Systems & Kernels"
    ],
    [
        "Microcontroller IoT", "roze::embedded", "GpioPin, I2cMaster, SpiBus, PwmChannel, AdcChannel, DelayTimer, PacRegister",
        "pin_mode, write_digital, read_digital, i2c_write, i2c_read, spi_transfer, pwm_set_duty, adc_read_raw, delay_ms, delay_us",
        "N/A (Java lacks PAC support)", "embedded-hal, cortex-m, riscv", "MicroPython machine", "Microcontroller Firmware & Robotics"
    ],
    [
        "WebAssembly & WASI", "roze::wasm", "WasmBuffer, JsValue, JsFunction, WasiFd, WasiClock, WasmExport, ComponentModel",
        "wasm_export, wasm_import, js_eval, js_call, wasi_fd_write, wasi_clock_time, mem_view, to_js_string, from_js_string",
        "TeaVM", "wasm-bindgen, wasi-common", "Pyodide", "Browser Applications & Edge Serverless"
    ],
    [
        "Package Management", "roze::pkg (bsr)", "PackageManifest, DependencyGraph, Lockfile, SATResolver, PackageCache, RegistryClient",
        "bsr_new, bsr_build, bsr_run, bsr_test, bsr_add, bsr_remove, bsr_update, bsr_vendor, bsr_publish, bsr_search",
        "Maven, Gradle", "Cargo, crates.io", "pip, uv, poetry", "Decentralized Package Ecosystem"
    ]
]

# ==================== DATA FOR SHEET 3: 10-DOMAIN MATRIX ====================
s3_headers = [
    "Target Domain", "Vision & Production Capability", "Roze Baseline (v2.0)",
    "Parity Targets & Benchmark Architecture", "Roze v3.0 'Titan' Deliverables",
    "Industry Benchmark", "Target Readiness"
]
s3_widths = [20, 34, 28, 40, 32, 22, 18]

s3_rows = [
    [
        "Enterprise Web & Microservices",
        "Build enterprise-scale cloud microservices, REST/gRPC backends, connection pooling, and banking-grade APIs with sub-millisecond latency.",
        "Basic rozeboot and roze-orm packages with routing and SQLite support.",
        "Java 25 (Spring Boot 3), Rust (Actix-web), Go (Gin): Declarative IoC container, reactive HTTP/2/3 QUIC server, HikariCP-class connection pool, JWT/OAuth2 security, OpenAPI 3.1.",
        "M10-T01, M10-T02, M10-T03, M10-T04, M10-T05, M10-T06 (Sprint 19-20)",
        "Java 25 (Spring Boot 3), Rust (Actix-web)",
        ("100% - Spring Boot 3 Parity", 16)
    ],
    [
        "Operating Systems & Kernels",
        "Write full 64-bit kernels, bootloaders, microkernels, memory managers, and embedded firmware running directly on bare-metal hardware.",
        "RozeOS v2 freestanding kernel with 4-level paging and basic drivers.",
        "C23, Rust 1.99 (#![no_std]), Zig: Preemptive scheduler, SMP multicore (APIC/MADT), Buddy physical frame allocator, VirtIO drivers, and kernel shell.",
        "M12-T01, M12-T02, M12-T05 (Sprint 23-24)",
        "C23, Rust 1.99 (#![no_std]), Zig",
        ("100% - Production Kernel Ready", 16)
    ],
    [
        "Cybersecurity & Hacking",
        "High-performance security tools: raw network packet sniffers, SYN scanners, protocol dissectors, binary reverse-engineering, and exploit research harnesses.",
        "Basic raw socket capture and TCP packet crafting.",
        "Python (Scapy), C (Nmap), Python (Volatility): Line-rate raw packet injection, Scapy-class protocol stacking DSL (/), multi-core 100k+ probes/s stealth scanner, multi-arch disassembler, memory forensics.",
        "M9-T01, M9-T02, M9-T03, M9-T04, M9-T05, M9-T06 (Sprint 17-18)",
        "Python (Scapy), C (Nmap), Volatility",
        ("100% - Security Suite Ready", 16)
    ],
    [
        "AI / Machine Learning",
        "Local LLM inference (LLaMA 3, Mistral), multidimensional tensor math, reverse-mode autograd, and GPU compute without heavy Python runtimes.",
        "Basic 1D/2D float tensors and seed roze-torch package.",
        "Python (PyTorch / NumPy), C++ (llama.cpp), Rust (candle): N-D Tensor<T, D> with striding/slicing/broadcasting, dynamic Autograd graph, deep learning layers (nn.Linear, nn.Conv2d, nn.MultiheadAttention), GGUF/safetensors 7B LLM decoder, CUDA FFI.",
        "M8-T01, M8-T02, M8-T03, M8-T04, M8-T05, M8-T06, M8-T07 (Sprint 15-16)",
        "Python (PyTorch + NumPy), C++ (llama.cpp)",
        ("100% - Native AI & LLM Engine Ready", 16)
    ],
    [
        "Game Development",
        "60+ FPS 2D & 3D real-time games, cross-platform graphics, spatial audio, rigid-body physics, and high-performance ECS architectures.",
        "roze-game seed package with Raylib 2D bindings.",
        "C++ (Unreal/Raylib), Rust (Bevy), C# (Unity): Archetype-based ECS scheduler, 2D/3D hardware rendering (Raylib/Vulkan), rigid-body physics with collision detection (AABB/OBB/Raycast), 3D spatial audio.",
        "M11-T01, M11-T02, M11-T03, M11-T06 (Sprint 21-22)",
        "C++ (Raylib), Rust (Bevy), Java (libGDX)",
        ("100% - 2D/3D Game Platform Ready", 16)
    ],
    [
        "Desktop GUI Applications",
        "Fast, beautiful cross-platform desktop productivity applications with native look-and-feel, GPU 2D vector rendering, and modern reactive state.",
        "roze-ui seed package with initial retained-mode widget tree.",
        "Java 25 (JavaFX), C++ (Qt), Rust (Slint/Tauri): Native windowing (Win32/Cocoa/Wayland), GPU 2D vector path canvas, reactive state signals (State<T>), CSS-compliant Flexbox layout, rich widget catalog.",
        "M11-T04, M11-T05, M11-T06 (Sprint 21-22)",
        "Java 25 (JavaFX), C++ (Qt), Rust (Tauri)",
        ("100% - Native Desktop GUI Ready", 16)
    ],
    [
        "Embedded & Microcontroller IoT",
        "Deterministic real-time microcontrollers, IoT devices, automotive systems, and custom hardware peripheral control.",
        "Basic embedded-hal and SVD generator tools in pure Roze.",
        "Rust (embedded-hal), C (CMSIS/HAL): ARM Cortex-M and RISC-V targets, automated CMSIS-SVD PAC generator, zero-allocation embedded-hal traits, direct flash programming CLI.",
        "M12-T02, M12-T05 (Sprint 23-24)",
        "C (CMSIS/HAL), Rust (embedded-hal)",
        ("100% - Microcontroller IoT Ready", 16)
    ],
    [
        "WebAssembly & Edge Serverless",
        "Run high-performance Roze modules in web browsers, Node.js runtimes, and edge serverless clouds (Cloudflare Workers, Fastly Compute@Edge).",
        "wasm32-wasi compiler target with basic roze-bindgen tool.",
        "Rust (wasm-bindgen / WASI), Go (syscall/js): WASI 0.2 Component Model binary code generator, DOM event bindings, WebGL canvas context, automated TypeScript/ESM bridge.",
        "M12-T03, M12-T05 (Sprint 23-24)",
        "Rust (wasm-bindgen / WASI), Go WASM",
        ("100% - WebAssembly & WASI Ready", 16)
    ],
    [
        "Data Science & Numerical Computing",
        "Arbitrary-precision arithmetic, high-performance statistics, functional data pipelines, and object-oriented file manipulation.",
        "Basic float math and string manipulation in core stdlib.",
        "Java 25 (java.math.BigInteger / BigDecimal), Python 3.15 (statistics, itertools, functools, pathlib): BigInt, BigDecimal with rounding modes, comprehensive statistics, functional itertools/functools, object-oriented Path.",
        "M7-T01, M7-T02, M7-T03, M7-T04, M7-T07 (Sprint 13-14)",
        "Java 25 (java.math.*), Python 3.15 (itertools/statistics)",
        ("100% - Data Science & Math Ready", 16)
    ],
    [
        "Package Ecosystem & Autonomous Toolchain",
        "Decentralized, mathematically verified dependency resolution, cryptographic package signing, and reproducible builds across all architectures.",
        "Standalone bsr tool and monolithic roze binary with bsr.mfz support.",
        "Rust (Cargo / crates.io), Go (go modules): Official package registry catalog, Go proxy protocol, Ed25519 digital package envelope signing, SemVer SAT solver, monolithic CLI driver.",
        "M12-T04, M12-T05 (Sprint 24)",
        "Rust (Cargo / crates.io), Go modules",
        ("100% - Autonomous Toolchain Sovereign", 16)
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
        ("Completed", 16)
    ],
    [
        ("Stage 2", 4), "Pure Roze Semantic", "Roze-in-Roze Type Checker",
        "Implement Type Checker, Trait Solver, Monomorphizer, and ARC Borrow Checker in pure Roze. Validates symbols, resolves interfaces, and handles generics.",
        "50% - Semantic analysis decoupled",
        "Type check compiler source with zero errors",
        ("Completed", 16)
    ],
    [
        ("Stage 3", 5), "Pure Roze Optimizer", "Roze-in-Roze SSA IR & Optimizer",
        "Implement Typed SSA IR generator and optimization pipeline in pure Roze: Dead Code Elimination, Inlining, Loop Unrolling, Constant Folding, and CFG analysis.",
        "75% - IR & optimization decoupled",
        "Verify optimized IR passes on benchmark suite",
        ("Completed", 16)
    ],
    [
        ("Stage 4", 6), "Pure Roze Native Backend", "Machine Code Emitter & Cranelift",
        "Direct native code generation in pure Roze: emit standalone ELF/Mach-O/PE binaries with Cranelift C-API or standalone assembly generator. Eliminates Cargo.",
        "90% - Code generation decoupled",
        "Produce standalone native binary from Roze code",
        ("Completed", 16)
    ],
    [
        ("Stage 5", 11), "Fixpoint Closure Verification", "3-Stage Bootstrap Fixpoint",
        "Execute full bootstrap cycle: Stage 0 (Seed) builds roze_stage1; roze_stage1 builds roze_stage2; roze_stage2 builds roze_stage3. Run 'diff roze_stage2 roze_stage3'.",
        "100% - Zero Rust Dependency! Rust removed.",
        "Fixpoint Verification: diff output is exactly 0 bytes",
        ("Completed", 16)
    ],
    [
        ("Stage 6", 22), "Universal StdLib Expansion", "Enterprise Standard Library in Pure Roze",
        "Author complete Java 25 java.util.* collections, java.math.* BigInt/BigDecimal, and Python 3.15 itertools/statistics/pathlib modules in 100% pure Roze.",
        "100% Sovereign - Autonomous language expansion",
        "Universal StdLib Test Suite: 100% pass across all mathematical and collection fixtures",
        ("Planned", 12)
    ],
    [
        ("Stage 7", 27), "Roze v3.0 'Titan' Production", "Universal Multi-Domain Autonomous Compiler",
        "Compile full multi-domain ecosystem (AI/ML roze-torch, Web rozeboot, Games roze-game, GUI roze-ui, Kernel roze-os) using self-hosted Roze compiler with zero foreign toolchain.",
        "100% Sovereign - Absolute language independence",
        "3-Stage Bootstrap Fixpoint Re-Verification on v3.0: 0 bytes diff across all targets",
        ("Planned", 12)
    ]
]

# ==================== DATA FOR SHEET 5: SPRINT SCHEDULE & GATES ====================
s5_headers = [
    "Sprint #", "Weeks", "Milestone Target", "Key Technical Epics",
    "Target Deliverables & Acceptance Criteria", "Risk Assessment & Mitigation", "Quality Gate Sign-Off"
]
s5_widths = [12, 14, 20, 32, 44, 34, 20]

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
        ("Gate 3: Systems Core Pass", 16)
    ],
    [
        ("Sprint 4", 4), "W07 - W08", "Bare-Metal OS Milestone M2", "Freestanding Target, 4-Level Paging, GDT/IDT, Drivers",
        "RozeOS v2 booting in QEMU: paging initialized, IDT exceptions handled, VGA text & GOP graphics running with shell.",
        "Risk: Triple fault on boot. Mitigation: Strict multiboot2 memory map validation.",
        ("Gate 4: Kernel Boot Sign-Off", 16)
    ],
    [
        ("Sprint 5", 5), "W09 - W10", "Security Suite & Crypto", "Raw Sockets, Packet Crafting, SYN Scanner, Crypto, Disassembler",
        "Promiscuous packet capture, 100k port/s stealth SYN scanner, pure TLS 1.3 handshake, x86_64 disassembler.",
        "Risk: OS raw socket permissions. Mitigation: Automatic capability checking with fallback.",
        ("Gate 5: Security Suite Pass", 16)
    ],
    [
        ("Sprint 6", 5), "W11 - W12", "Enterprise Web Milestone M3", "RozeBoot 2.0, HTTP/2/3, IoC Container, ORM, JWT, OpenAPI",
        "RozeBoot microservice benchmark: 100k req/s at <1ms latency, auto-generated Swagger UI, SQL connection pool.",
        "Risk: Memory leaks under load. Mitigation: ARC stress testing with leak sanitizers.",
        ("Gate 6: RozeBoot Web Sign-Off", 16)
    ],
    [
        ("Sprint 7", 6), "W13 - W14", "AI & LLM Inference", "N-D Tensors, GEMM (AVX/CUDA), Autograd, GGUF/ONNX, LLaMA 3",
        "Native LLaMA 3 inference generating 45+ tok/s locally; dynamic Autograd training linear regression demo.",
        "Risk: Quantization accuracy loss. Mitigation: Rigorous perplexity testing vs FP16 baseline.",
        ("Gate 7: AI Engine Pass", 16)
    ],
    [
        ("Sprint 8", 6), "W15 - W16", "Game Platform Milestone M4", "3D Math, Graphics HAL (Vulkan/Metal), Archetype ECS, Audio/Physics",
        "60 FPS 3D game demo with physics, lighting, shadows, spatial audio, and archetype ECS scheduler running smoothly.",
        "Risk: Frame drops / GC pauses. Mitigation: ARC memory pre-allocation in frame arenas.",
        ("Gate 8: Game Platform Sign-Off", 16)
    ],
    [
        ("Sprint 9", 7), "W17 - W18", "Desktop GUI (RozeUI)", "Cross-Platform Windowing, GPU 2D Canvas, Reactive UI, Widgets",
        "Cross-platform Desktop Dashboard app with reactive state, data grid, charts, and embedded Webview container.",
        "Risk: Platform UI quirks. Mitigation: Native platform windowing wrappers for Win32/Cocoa/Wayland.",
        ("Gate 9: Desktop GUI Pass", 16)
    ],
    [
        ("Sprint 10", 7), "W19 - W20", "StdLib Richness Milestone M5", "Linear-Time Regex, Date/Time (JSR-310), Universal Serialization",
        "Zero-ReDoS regex engine, IANA timezone database math, zero-copy JSON/TOML/MsgPack/Protobuf benchmarks.",
        "Risk: Timezone corner cases. Mitigation: Full validation against standard Olson timezone test vectors.",
        ("Gate 10: StdLib Parity Sign-Off", 16)
    ],
    [
        ("Sprint 11", 11), "W21 - W22", "Pure Roze Compiler v2", "Roze-in-Roze Frontend, Type Checker, SSA Optimizer, Native Backend",
        "Compiler rewritten 100% in pure Roze; compiles entire standard library and itself without Rust compiler.",
        "Risk: Compiler bootstrap bug. Mitigation: Extensive golden test fixtures comparing against Stage 0.",
        ("Gate 11: Self-Host Frontend Pass", 16)
    ],
    [
        ("Sprint 12", 11), "W23 - W26", "Production Launch v2.0 'Prometheus'",
        "WASM Target (wasm32/wasi), Microcontroller IoT (Cortex-M/RISC-V/embedded-hal), Basir (bsr) .mfz Package Manager, Monolithic CLI, VS Code LSP, 3-Stage Fixpoint (0 Diff), Prometheus v2.0 Launch",
        "3-Stage bootstrap fixpoint verified (0 bytes diff). Official Roze v2.0 'Prometheus' public production release.",
        "Risk: Multi-target platform regressions. Mitigation: Multi-platform automated CI test matrix covering x86_64, aarch64, wasm32, cortex-m, and riscv32.",
        ("Gate 12: v2.0 Production Sign-Off", 16)
    ],
    [
        ("Sprint 13", 22), "W27 - W28", "Math & Collections", "BigInt, BigDecimal, Complete Java 25 Collections, JSR-310 Temporal",
        "Arbitrary-precision BigInt/BigDecimal engine, complete java.util collections hierarchy, nanosecond ZonedDateTime.",
        "Risk: BigInt multiplication performance. Mitigation: Karatsuba algorithm with Toom-Cook fallback.",
        ("Gate 13: Core Math & Util Pass", 12)
    ],
    [
        ("Sprint 14", 22), "W29 - W30", "Universal StdLib Milestone M7", "Python Itertools/Stats, Non-Blocking NIO, Enterprise Concurrent Pools",
        "Python itertools/functools/statistics, zero-copy ByteBuffer, CompletableFuture monadic pipeline, ForkJoinPool.",
        "Risk: Buffer bounds safety. Mitigation: Strict compile-time and runtime slice range verification.",
        ("Gate 14: Enterprise StdLib Gate", 15)
    ],
    [
        ("Sprint 15", 23), "W31 - W32", "Tensor Engine & Autograd", "N-D Tensor<T, D>, Strided Layout, Reverse-Mode Autograd Tape, NN Layers",
        "Arbitrary rank tensors with broadcasting, dynamic autograd backprop, linear/conv/attention neural network layers.",
        "Risk: Gradient accumulation bugs. Mitigation: Rigorous numerical gradient checking (finite difference test).",
        ("Gate 15: Tensor & Autograd Pass", 16)
    ],
    [
        ("Sprint 16", 23), "W33 - W34", "AI & LLM Milestone M8", "Optimizers (AdamW), 7B Transformer LLM Decoder, CUDA FFI Acceleration",
        "Local LLaMA-3/Mistral 7B inference with KV-cache running at >35 tok/s on CUDA, AdamW optimizer, GGUF/safetensors loader.",
        "Risk: GPU memory overflow. Mitigation: Efficient KV-cache memory pooling and GGUF quantized weights.",
        ("Gate 16: AI & LLM Engine Pass", 16)
    ],
    [
        ("Sprint 17", 24), "W35 - W36", "Packet DSL & Reconnaissance", "Line-Rate Raw Sockets, Scapy-Class Stacking DSL (/), 100k+ Stealth SYN Scanner",
        "Promiscuous packet capture, protocol builder/dissector DSL (Ether/IP/TCP), multi-core 100,000 probes/s scanner.",
        "Risk: Packet drop under high load. Mitigation: Kernel circular ring buffers via PACKET_MMAP.",
        ("Gate 17: Packet & Scanner Pass", 12)
    ],
    [
        ("Sprint 18", 24), "W37 - W38", "Security & Forensics Milestone M9", "Multi-Arch Disassembler, Defensive Memory Forensics, ROP Harvester",
        "Static binary disassembler for x86_64/ARM64/RISC-V/WASM, live process memory map auditing, RWX violation detection.",
        "Risk: Obfuscated binary disassembly. Mitigation: Linear sweep and recursive descent disassembly fusion.",
        ("Gate 18: Security & Forensics Gate", 15)
    ],
    [
        ("Sprint 19", 25), "W39 - W40", "Enterprise IoC & ORM", "Declarative IoC Container, Reactive HTTP/2/3 QUIC, Roze-ORM Active Record",
        "Spring Boot 3 IoC container, sub-millisecond HTTP/2/3 server, HikariCP-class connection pool, relational migrations.",
        "Risk: Connection pool exhaustion. Mitigation: Bounded wait queue with leak detection telemetry.",
        ("Gate 19: Enterprise Core Pass", 12)
    ],
    [
        ("Sprint 20", 25), "W41 - W42", "Cloud Framework Milestone M10", "OAuth2/JWT Security Perimeter, OpenTelemetry Tracing, 250k req/s Stress Test",
        "End-to-end banking microservice benchmark: 250,000 req/s, P99 latency <0.8ms, OpenTelemetry distributed tracing.",
        "Risk: Microservice tail latency. Mitigation: Zero-allocation JSON parsing and M:N virtual thread dispatch.",
        ("Gate 20: RozeBoot Cloud Gate", 15)
    ],
    [
        ("Sprint 21", 26), "W43 - W44", "Game Engine & ECS", "Archetypal ECS, 2D/3D Hardware Rendering (Raylib/Vulkan), Spatial Physics & Audio",
        "60+ FPS game loop with archetypal ECS query iterators, 2D/3D mesh rendering, rigid-body physics, 3D spatial audio.",
        "Risk: Physics tunneling at high speed. Mitigation: Continuous Collision Detection (CCD) via raycasting.",
        ("Gate 21: Game Engine Pass", 12)
    ],
    [
        ("Sprint 22", 26), "W45 - W46", "Desktop GUI Milestone M11", "Cross-Platform Windowing, GPU Vector Canvas, Reactive Widgets & Flexbox Layout",
        "Native Win32/Cocoa/Wayland windowing, GPU 2D vector path canvas, CSS-compliant Flexbox layout, reactive widget hierarchy.",
        "Risk: Multi-platform DPI scaling quirks. Mitigation: Automatic OS display scaling factor normalization.",
        ("Gate 22: Game & Desktop GUI Gate", 15)
    ],
    [
        ("Sprint 23", 27), "W47 - W48", "Kernel & Embedded Systems", "Freestanding OS Kernel (RozeOS v3), Microcontroller Platform (Cortex-M/RISC-V)",
        "Preemptive SMP kernel with 4-level paging and VirtIO drivers; microcontroller embedded-hal with direct flash CLI.",
        "Risk: Hardware interrupt race conditions. Mitigation: Atomic interrupt spinlocks and lock-free ring buffers.",
        ("Gate 23: Systems & IoT Pass", 12)
    ],
    [
        ("Sprint 24", 27), "W49 - W52", "Grand Launch Milestone M12", "Official Roze v3.0 'Titan' Production Launch Across All 10 Domains",
        "Full language autonomy verified: 100% pure Roze self-hosted compiler, 0 bytes diff 3-stage bootstrap fixpoint closure, official multi-platform packaging (deb, rpm, brew, msi, pkg, vsix), RozeHub portal documentation. Parity achieved with Java 25, Rust 1.99.1 & Python 3.15.",
        "Risk: Ecosystem divergence. Mitigation: Comprehensive automated integration test suite across all 10 computing domains.",
        ("Gate 24: Roze v3.0 Titan Launch Sign-Off", 15)
    ]
]

# Write worksheets
sheet1_xml = make_worksheet(s1_headers, s1_rows, s1_widths, freeze_row=1)
sheet2_xml = make_worksheet(s2_headers, s2_rows, s2_widths, freeze_row=1)
sheet3_xml = make_worksheet(s3_headers, s3_rows, s3_widths, freeze_row=1)
sheet4_xml = make_worksheet(s4_headers, s4_rows, s4_widths, freeze_row=1)
sheet5_xml = make_worksheet(s5_headers, s5_rows, s5_widths, freeze_row=1)

def build_excel(target_path):
    os.makedirs(os.path.dirname(os.path.abspath(target_path)), exist_ok=True)
    with zipfile.ZipFile(target_path, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("[Content_Types].xml", content_types)
        zf.writestr("_rels/.rels", pkg_rels)
        zf.writestr("xl/_rels/workbook.xml.rels", wb_rels)
        zf.writestr("xl/workbook.xml", wb_xml)
        zf.writestr("xl/styles.xml", styles_xml)
        zf.writestr("xl/worksheets/sheet1.xml", sheet1_xml)
        zf.writestr("xl/worksheets/sheet2.xml", sheet2_xml)
        zf.writestr("xl/worksheets/sheet3.xml", sheet3_xml)
        zf.writestr("xl/worksheets/sheet4.xml", sheet4_xml)
        zf.writestr("xl/worksheets/sheet5.xml", sheet5_xml)
    print(f"Successfully generated: {target_path}")

if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    repo_root = os.path.dirname(script_dir)
    
    # 1. Generate at repo root
    root_xlsx = os.path.join(repo_root, "ROZE_PROJECT_PLAN.xlsx")
    build_excel(root_xlsx)
    
    # 2. Generate in docs directory
    docs_xlsx = os.path.join(repo_root, "docs", "ROZE_PROJECT_PLAN.xlsx")
    build_excel(docs_xlsx)
    
    print("Master 12-Month Project Plan Excel generation complete!")
