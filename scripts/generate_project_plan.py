#!/usr/bin/env python3
"""
Generates the official Roze Master Project Plan Excel spreadsheet:
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
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"""

# Workbook XML
wb_xml = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="1. Execution Roadmap" sheetId="1" r:id="rId1"/>
    <sheet name="2. Domain Capability Matrix" sheetId="2" r:id="rId2"/>
    <sheet name="3. Phases &amp; Milestones" sheetId="3" r:id="rId3"/>
  </sheets>
</workbook>"""

# Styles XML
styles_xml = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="9">
    <font><sz val="10"/><name val="Segoe UI"/></font> <!-- 0: Normal -->
    <font><b/><sz val="10.5"/><color rgb="FFFFFFFF"/><name val="Segoe UI"/></font> <!-- 1: Header bold white -->
    <font><b/><sz val="13"/><color rgb="FF1F4E79"/><name val="Segoe UI"/></font> <!-- 2: Title blue -->
    <font><b/><sz val="9.5"/><color rgb="FF1F4E79"/><name val="Segoe UI"/></font> <!-- 3: Core Blue Tag -->
    <font><b/><sz val="9.5"/><color rgb="FF7F6000"/><name val="Segoe UI"/></font> <!-- 4: Med Amber Tag -->
    <font><b/><sz val="9.5"/><color rgb="FF4C1130"/><name val="Segoe UI"/></font> <!-- 5: Adv Purple Tag -->
    <font><b/><sz val="9.5"/><color rgb="FF274E13"/><name val="Segoe UI"/></font> <!-- 6: Completed / Green Tag -->
    <font><b/><sz val="9.5"/><color rgb="FF0B5394"/><name val="Segoe UI"/></font> <!-- 7: Next / Cyan Tag -->
    <font><b/><sz val="9.5"/><color rgb="FF38761D"/><name val="Segoe UI"/></font> <!-- 8: Success Dark Green -->
  </fonts>
  <fills count="9">
    <fill><patternFill patternType="none"/></fill>
    <fill><patternFill patternType="gray125"/></fill>
    <fill><patternFill patternType="solid"><fgColor rgb="FF1F4E79"/></patternFill></fill> <!-- 2: Header Navy -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFD9E1F2"/></patternFill></fill> <!-- 3: Core Soft Blue -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFFFF2CC"/></patternFill></fill> <!-- 4: Med Soft Amber -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFEAD1DC"/></patternFill></fill> <!-- 5: Adv Soft Purple -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFD9EAD3"/></patternFill></fill> <!-- 6: Completed Soft Green -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFCFE2F3"/></patternFill></fill> <!-- 7: Next Soft Cyan -->
    <fill><patternFill patternType="solid"><fgColor rgb="FFF8FAFC"/></patternFill></fill> <!-- 8: Light Row Alt -->
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
  <cellXfs count="11">
    <!-- 0: Normal data left-top, wrap -->
    <xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0"><alignment vertical="top" wrapText="1"/></xf>
    <!-- 1: Header navy center -->
    <xf numFmtId="0" fontId="1" fillId="2" borderId="1" xfId="0"><alignment horizontal="center" vertical="center" wrapText="1"/></xf>
    <!-- 2: Title -->
    <xf numFmtId="0" fontId="2" fillId="0" borderId="0" xfId="0"><alignment vertical="center"/></xf>
    <!-- 3: Core Blue Tag -->
    <xf numFmtId="0" fontId="3" fillId="3" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 4: Medium Amber Tag -->
    <xf numFmtId="0" fontId="4" fillId="4" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 5: Advanced Purple Tag -->
    <xf numFmtId="0" fontId="5" fillId="5" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 6: Completed / Green Tag -->
    <xf numFmtId="0" fontId="6" fillId="6" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 7: Next Cyan Tag -->
    <xf numFmtId="0" fontId="7" fillId="7" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 8: Center Normal -->
    <xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
    <!-- 9: Alt row normal data -->
    <xf numFmtId="0" fontId="0" fillId="8" borderId="1" xfId="0"><alignment vertical="top" wrapText="1"/></xf>
    <!-- 10: Center Normal Alt -->
    <xf numFmtId="0" fontId="0" fillId="8" borderId="1" xfId="0"><alignment horizontal="center" vertical="top"/></xf>
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
    ws.append('    <row r="1" ht="30" customHeight="1">')
    for col_idx, h in enumerate(headers, start=1):
        cell_ref = f"{col_to_letter(col_idx)}1"
        ws.append(f'      <c r="{cell_ref}" t="inlineStr" s="1"><is><t>{escape_xml(h)}</t></is></c>')
    ws.append('    </row>')
    
    # Data rows
    for row_idx, r in enumerate(rows, start=2):
        is_alt = (row_idx % 2 == 1)
        ws.append(f'    <row r="{row_idx}" ht="46" customHeight="1">')
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

# ==================== DATA FOR SHEET 1: ROADMAP ====================
s1_headers = [
    "Task ID", "Stage", "Order", "Component", "Feature Name",
    "Purpose & Why It Is Needed", "Detailed Technical Implementation",
    "Target Domains Unlocked", "Est. Effort", "Status"
]
s1_widths = [12, 16, 10, 18, 26, 42, 46, 28, 14, 16]

s1_rows = [
    # CORE STAGE
    [
        ("CORE-01", 8), ("Core / Basic", 3), ("Step 1", 8), "Type Checker & Codegen", "Result<T, E> & Option<T> Types",
        "Prevent runtime panics on I/O, file, network, and database operations. Fully replaces process crashes with safe error propagation (? operator).",
        "Result/Option types in semantic checker; unwrap, unwrap_or, unwrap_err, is_ok, is_err, is_some, is_none; safe IO intrinsics (read_file_result, write_file_result, etc.); safe collection indexing (list_get_option, map_get_option); postfix '?' try operator hoisting with early return in JVM and native backends.",
        "Web/Enterprise, Systems, Tooling, Self-Hosting", "Completed", ("Completed", 6)
    ],
    [
        ("CORE-02", 8), ("Core / Basic", 3), ("Step 2", 8), "Frontend & Type Checker", "Enums (ADTs) & Pattern Matching",
        "Essential for representing AST nodes, tokens, and domain states cleanly without hacky integer flags or unsafe casting.",
        "Syntax 'enum Name { Variant(Type), ... }'; pattern matching 'match expr { Variant(val) => ... }'; lower to Typed IR; codegen tagged union with tag + payload in Cranelift native; Java sealed classes/records in JVM.",
        "Compiler Self-Hosting, Web, Security", "Completed", ("Completed", 6)
    ],
    [
        ("CORE-03", 8), ("Core / Basic", 3), ("Step 3", 8), "Frontend, JVM & Native", "Closures & First-Class Functions",
        "Enable passing functions as arguments. Strictly required for web request routing, callbacks, event handling, list map/filter, and concurrency task dispatch.",
        "Syntax 'let f = |x: int| -> int { x + 1 };'; function types 'func(int) -> int'; closure environment capture struct; JVM java.util.function emission; Native function pointer + env pointer.",
        "Web Framework (Routing/Middleware), GUI, Games", "Completed", ("Completed", 6)
    ],
    [
        ("CORE-04", 8), ("Core / Basic", 3), ("Step 4", 8), "Type Checker & Native Codegen", "Generics (Parametric Polymorphism)",
        "Provide type-safe generic data structures (List<T>, Map<K, V>). Eliminates Object boxing on JVM and unlocks strings/structs inside native lists.",
        "Syntax 'class Container<T> { val: T }' and generic functions 'func id<T>(x: T) -> T'; Native monomorphization (specialize code per type); JVM type parameters with erasure.",
        "All Domains, Core Stdlib, Self-Hosting", "Completed", ("Completed", 6)
    ],
    [
        ("CORE-05", 8), ("Core / Basic", 3), ("Step 5", 8), "Frontend & Semantic Checker", "Traits / Interfaces & Dynamic Dispatch",
        "Define behavioral contracts without inheritance. Foundation of Spring-Boot-style Dependency Injection (IoC) and pluggable modular architecture.",
        "Syntax 'trait Greeter { func greet() -> string; }' and 'impl Greeter for User { ... }'; JVM interface generation; Native vtable pointer in class headers.",
        "Enterprise Architecture, Games, Plugins", "Completed", ("Completed", 6)
    ],

    # MEDIUM STAGE
    [
        ("MED-01", 8), ("Medium", 4), ("Step 6", 8), "Native Codegen (Cranelift)", "Foreign Function Interface (C-ABI FFI)",
        "The Universal Key: Call ANY existing C/C++ library directly. Instantly unlocks Desktop (GTK/Qt/Win32), Games (Raylib/SDL3/Vulkan), AI/ML (BLAS/CUDA/PyTorch), and Security (libpcap/OpenSSL) without rewriting them.",
        "Syntax 'extern \"C\" func printf(fmt: string) -> int;'; Cranelift SystemV / Win64 calling convention integration; link external shared/static libraries via 'roze build --link <lib>'.",
        "Desktop GUI, Games, AI/ML, Security/Hacking, Systems", "Completed", ("Completed", 6)
    ],
    [
        ("MED-02", 8), ("Medium", 4), ("Step 7", 8), "Native Runtime & Codegen", "Thread-Safe Atomic ARC",
        "Make memory management thread-safe. Prevents race conditions and use-after-free corruption when data is accessed across concurrent threads.",
        "Upgrade ARC retain and release helper functions in Cranelift from plain increment/decrement to atomic operations (lock xadd / Cranelift atomic load/store/rmw).",
        "Web Servers, Multi-threaded Games, Systems", "Completed", ("Completed", 6)
    ],
    [
        ("MED-03", 8), ("Medium", 4), ("Step 8", 8), "Runtime & Standard Library", "Native Concurrency: Threads & Channels",
        "Allow spawning background worker threads and passing messages safely between threads without data races.",
        "'thread_spawn(func)' backed by OS threads (pthread / Windows CreateThread) or JVM virtual threads; channel primitives ('channel_new()', 'send()', 'recv()') with ring buffer and lock.",
        "Enterprise Servers, Game Engines, Systems", "Completed", ("Completed", 6)
    ],
    [
        ("MED-04", 8), ("Medium", 4), ("Step 9", 8), "Standard Library (Web & Net)", "Concurrent Async HTTP Server & Router",
        "Replace single-threaded synchronous server with a high-performance HTTP server capable of serving thousands of simultaneous requests.",
        "Thread pool worker dispatch loop; URL route matcher ('/api/users/:id'); JSON body auto-deserialization; middleware pipeline for auth/logging.",
        "Enterprise Web Backends, Cloud Services", "Completed", ("Completed", 6)
    ],
    [
        ("MED-05", 8), ("Medium", 4), ("Step 10", 8), "Frontend & Native Backend", "Unsafe Raw Pointers & Direct Memory",
        "Low-level direct memory access for security research, packet inspection, hardware I/O, binary reverse-engineering, and high-speed game buffers.",
        "'unsafe { ... }' block syntax; raw pointer types '*raw u8', '*raw int'; pointer read/write intrinsics; direct memory allocation without ARC header overhead.",
        "Security/Hacking, OS Development, Game Engines", "Completed", ("Completed", 6)
    ],
    [
        ("MED-06", 8), ("Medium", 4), ("Step 11", 8), "Tooling (roze-pkg)", "Package Registry & Dependency Resolver",
        "Allow developers to install, share, and manage third-party libraries seamlessly from a central registry or Git repos.",
        "Implement Git-based dependency fetching, semantic version resolution (semver), lockfile generation ('roze.lock'), and binary package caching.",
        "Developer Experience, Ecosystem Growth", "1 - 2 Weeks", ("Ready to Start", 7)
    ],

    # ADVANCED STAGE
    [
        ("ADV-01", 8), ("Advanced", 5), ("Step 12", 8), "Native Codegen & Linker", "Freestanding / no_std Kernel Target",
        "Compile code that runs directly on bare metal CPU with zero OS, zero glibc, and zero mandatory runtime underneath.",
        "'--target freestanding' compiler flag; custom '_start' entry point; disable libc malloc/printf; raw VGA buffer text driver; linker script integration.",
        "Operating System Kernel, Embedded Firmware", "1 - 2 Weeks", ("Planned", 8)
    ],
    [
        ("ADV-02", 8), ("Advanced", 5), ("Step 13", 8), "Systems & OS Demo", "Bootable x86_64 OS Kernel Demo",
        "Provide indisputable proof that Roze can build an operating system from scratch.",
        "Write a minimal 64-bit multiboot2 kernel in Roze; boot via GRUB/QEMU; print hardware info and manage basic physical memory frames.",
        "Operating System Proof-of-Concept", "1 - 2 Weeks", ("Planned", 8)
    ],
    [
        ("ADV-03", 8), ("Advanced", 5), ("Step 14", 8), "Web Framework Project", "Spring-Boot-Style Framework ('RozeBoot')",
        "Enable building large-scale enterprise services with declarative controllers, middleware, connection pooling, and ORM query builders.",
        "Build 'RozeBoot' framework on top of Roze: REST controller annotations/routing, HikariCP JDBC connection pooling, JWT security, SQL query builder.",
        "Enterprise Cloud Applications", "2 - 3 Weeks", ("Planned", 8)
    ],
    [
        ("ADV-04", 8), ("Advanced", 5), ("Step 15", 8), "Game Engine Demo", "2D/3D Game Demo via FFI (Raylib / SDL3)",
        "Demonstrate game platform capabilities with 60 FPS real-time rendering, audio, and physics.",
        "Bind Raylib / SDL3 via Roze C-FFI; build a working game loop with sprite rendering, keyboard/mouse input, particle effects, and sound playback.",
        "Game Platform Proof-of-Concept", "1 Week", ("Planned", 8)
    ],
    [
        ("ADV-05", 8), ("Advanced", 5), ("Step 16", 8), "AI / ML Wrapper Project", "AI & ML Inference Wrapper (ONNX / GGML)",
        "Run local AI models (Llama, BERT, Vision models) directly from Roze apps.",
        "Bind ONNX Runtime C API or GGML via FFI; load model weights; pass input tensors; run inference and parse prediction outputs.",
        "AI / ML Applications, Data Science", "1 - 2 Weeks", ("Planned", 8)
    ],
    [
        ("ADV-06", 8), ("Advanced", 5), ("Step 17", 8), "Compiler Re-Architecture", "Self-Hosting Compiler (Roze-in-Roze)",
        "Rewrite the Roze compiler in Roze itself, removing all dependency on Rust and proving the language's ultimate maturity.",
        "Implement Roze lexer, parser, type checker, and Cranelift/C code generation in pure Roze; compile the Roze compiler using itself.",
        "Self-Hosting, Language Independence", "3 - 4 Weeks", ("Planned", 8)
    ],
    [
        ("ADV-07", 8), ("Advanced", 5), ("Step 18", 8), "Release Engineering", "Official v1.0.0 Release, Docs & Ecosystem",
        "Complete production release with semantic versioning guarantees, online documentation, VS Code marketplace extension, and installer packages.",
        "Finalize language grammar specification; publish VS Code extension; release Debian/RPM/Homebrew/Windows MSI packages; launch Roze package registry.",
        "Official v1.0.0 Public Launch", "2 Weeks", ("Planned", 8)
    ]
]

# ==================== DATA FOR SHEET 2: DOMAIN MATRIX ====================
s2_headers = [
    "Target Domain", "Vision & Target Applications", "Current Roze State",
    "Gaps to Close", "Enabling Tasks (From Roadmap)", "Industry Benchmark", "Current Readiness"
]
s2_widths = [20, 32, 28, 38, 30, 20, 18]

s2_rows = [
    [
        "Enterprise Web & Microservices",
        "Spring-Boot-level servers, REST/gRPC APIs, Cloud services, banking & enterprise backends",
        "JVM HTTP accept loop, JSON, JDBC SQL, Result<T,E> & Option<T> Safe I/O",
        "Multi-threaded concurrency, closures/routing, generics, connection pooling",
        "CORE-01 (Done), CORE-02 (Done), CORE-03 (Done), CORE-04 (Done), CORE-05 (Done), MED-02 (Done), MED-04 (Done), ADV-03",
        "Java (Spring Boot), Go (Gin/Fiber), C# (.NET)",
        ("75% - Concurrent Server & Router Ready", 6)
    ],
    [
        "Operating Systems & Kernels",
        "Monolithic/microkernel OS, custom bootloaders, hardware drivers, embedded firmware",
        "Cranelift native backend emits standalone ELF/PE binaries with libc ARC",
        "Bare-metal freestanding mode (no glibc, no malloc), raw unsafe pointers, inline assembly, linker scripts",
        "MED-05 (Done), ADV-01, ADV-02",
        "C, Rust (no_std), Zig",
        ("25% - Unsafe Pointers Ready", 5)
    ],
    [
        "Security & Penetration Testing",
        "Custom network sniffers, exploit payloads, binary reverse-engineering tools, memory analyzers",
        "Native standalone executables, TCP sockets, float/math, safe Result error handling",
        "Raw byte buffers, unsafe memory pointers, FFI bindings to libpcap, OpenSSL, and raw packet sockets",
        "MED-01 (Done), MED-05 (Done)",
        "Python (Scapy), Go, C, Rust",
        ("60% - Raw Memory & FFI Ready", 6)
    ],
    [
        "Games & Interactive Platforms",
        "2D & 3D game engines, indie games, physics simulation, cross-platform game runtime",
        "ARC-managed classes and lists, IEEE-754 f64 math, fast native execution",
        "FFI to Raylib/SDL3/Vulkan, atomic multi-threaded rendering/audio threads, generics for ECS engines",
        "CORE-04 (Done), MED-01 (Done), MED-02 (Done), ADV-04",
        "C++, C# (Unity), Rust (Bevy), C",
        ("50% - Atomic ARC & FFI Ready", 6)
    ],
    [
        "AI / Machine Learning",
        "Local LLM inference (Llama), computer vision, neural net inference, data pipelines",
        "Fast f64 double precision numbers, basic arrays/lists, Result safety",
        "C-FFI to bind ONNX Runtime, libtorch, or GGML tensor libraries",
        "CORE-04 (Done), MED-01 (Done), ADV-05",
        "Python (PyTorch wrapper), C++",
        ("45% - FFI Enabled", 4)
    ],
    [
        "Desktop GUI Applications",
        "Cross-platform desktop tools, productivity software, native UI apps",
        "Fast standalone executables on Windows, Linux, and macOS",
        "FFI to native UI toolkits (GTK4, Qt, Win32, Cocoa), closure event handlers",
        "CORE-03 (Done), MED-01 (Done)",
        "C# (WPF), Rust (Tauri), C++ (Qt)",
        ("50% - Closures & FFI Completed", 6)
    ],
    [
        "Self-Hosting Compiler",
        "The Roze compiler written entirely in Roze with zero external dependencies",
        "Clean AST/IR, Result/Option safe error handling with '?' try operator",
        "Generics, Enums/Pattern Matching, file I/O on native backend",
        "CORE-01 (Done), CORE-02 (Done), CORE-03 (Done), CORE-04 (Done), CORE-05 (Done), ADV-06",
        "Rust (rustc), Go (gc), Swift",
        ("65% - Traits & Generics Completed", 6)
    ]
]

# ==================== DATA FOR SHEET 3: PHASES & MILESTONES ====================
s3_headers = [
    "Phase Name", "Strategic Focus", "Key Deliverables",
    "Prerequisites", "Target Milestone", "Status / Success Criteria"
]
s3_widths = [18, 30, 42, 22, 18, 36]

s3_rows = [
    [
        ("Phase 1", 3),
        "Core Language Maturity & Safety",
        "Result/Option error types (Done), Enums with Pattern Matching (Done), Closures/Lambdas (Done), Generics (Done), Traits/Interfaces (Done)",
        "Existing 1.99.1 Compiler",
        "v0.2.0-alpha",
        ("COMPLETED (Phase 1 100% Complete)", 6)
    ],
    [
        ("Phase 2", 4),
        "Universal Interop & Concurrency",
        "C-ABI FFI (extern 'C'), Thread-safe Atomic ARC, Native OS Threads & Channels, Concurrent Async HTTP Server",
        "Phase 1 Core Types",
        "v0.5.0-beta",
        ("In Progress", 4)
    ],
    [
        ("Phase 3", 4),
        "Low-Level Systems & Freestanding",
        "Unsafe raw pointers, Direct memory operations, --target freestanding (no_std), Bootable OS x86_64 kernel demo",
        "Phase 2 C-ABI & ARC",
        "v0.7.0-systems",
        ("Planned", 8)
    ],
    [
        ("Phase 4", 5),
        "Multi-Domain Proof Points",
        "'RozeBoot' Enterprise Framework, 60 FPS 2D Game demo via Raylib, Local AI inference via GGML/ONNX",
        "Phases 1, 2, and 3",
        "v0.9.0-rc1",
        ("Planned", 8)
    ],
    [
        ("Phase 5", 5),
        "Self-Hosting & Official v1.0.0",
        "Roze compiler rewritten in Roze; Package Registry; VS Code extension on marketplace; v1.0.0 Stable Release",
        "Phase 1-4 Complete",
        "v1.0.0 Stable",
        ("Planned", 8)
    ]
]

# Write worksheets
sheet1_xml = make_worksheet(s1_headers, s1_rows, s1_widths, freeze_row=1)
sheet2_xml = make_worksheet(s2_headers, s2_rows, s2_widths, freeze_row=1)
sheet3_xml = make_worksheet(s3_headers, s3_rows, s3_widths, freeze_row=1)

targets = [
    "/home/firoze/projects/Others/roze/ROZE_PROJECT_PLAN.xlsx",
    "/home/firoze/projects/Others/roze/docs/ROZE_PROJECT_PLAN.xlsx"
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
    print(f"Successfully generated: {output_path}")
