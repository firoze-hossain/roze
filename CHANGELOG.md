# 🌹 Changelog - Roze Programming Language

All notable changes to the Roze programming language, compiler, runtime, package manager, and tooling ecosystem are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] - 2026-10-27 - Official Stable Production Release 🚀

### Milestones Completed (18 / 18 Steps)
This landmark release completes all 18 milestones from `ROZE_PROJECT_PLAN.xlsx`:

- **Phase 1: Core Language Maturity & Safety (CORE-01 .. CORE-05)**
  - `CORE-01`: Explicit `Result<T, E>` and `Option<T>` with `?` try operator.
  - `CORE-02`: Algebraic Data Types (`enum`) with exhaustive pattern matching (`match`).
  - `CORE-03`: First-class closures/lambdas (`(x) => x * 2`) and function types.
  - `CORE-04`: Parametric polymorphism (Generics `<T>`) across classes, functions, and enums.
  - `CORE-05`: Traits and polymorphic dispatch (`trait`, `impl Trait for Type`).

- **Phase 2: Universal Interop & Concurrency (MED-01 .. MED-05)**
  - `MED-01`: C-ABI Foreign Function Interface (`extern "C"`).
  - `MED-02`: Thread-safe Automatic Reference Counting (`atomic_rmw` ARC).
  - `MED-03`: Native OS threads (`thread_spawn`) and thread-safe channels (`channel_new`, `channel_send`, `channel_recv`).
  - `MED-04`: High-throughput concurrent asynchronous HTTP web server (`http_server_start`).
  - `MED-05`: Production package manager (`roze-pkg`) with semver resolver, lockfile, and registry client.

- **Phase 3: Low-Level Systems & Freestanding (ADV-01 .. ADV-02)**
  - `ADV-01`: Unsafe blocks and raw memory intrinsics (`alloc`, `free_ptr`, `write_bytes`, `read_bytes`).
  - `ADV-02`: Bare-metal `--target freestanding` mode; multiboot2 x86_64 OS kernel demo booting via GRUB in QEMU.

- **Phase 4: Multi-Domain Proof Points (ADV-03 .. ADV-05)**
  - `ADV-03`: 'RozeBoot' Spring-Boot-style web framework with declarative controllers, connection pooling, and SQL query builder.
  - `ADV-04`: 60 FPS real-time 2D/3D game platform demo via Raylib FFI with sprite rendering and physics loop.
  - `ADV-05`: Local AI/ML inference wrapper with multidimensional tensor mathematical engine, activations, and LLaMA-style decoder transformer.

- **Phase 5: Self-Hosting & Ecosystem (ADV-06 .. ADV-07)**
  - `ADV-06`: Pure Roze self-hosted compiler (`roze-in-roze`); 3-stage bootstrap fixpoint verification (zero diff between Stage 2 and Stage 3).
  - `ADV-07`: Official v1.0.0 release engineering, formal grammar specification (`docs/GRAMMAR.md`), language reference specification (`docs/SPECIFICATION.md`), VS Code extension (`ide/vscode/`), Debian (`.deb`), RPM (`.rpm`), Homebrew, and Windows MSI installer packages, and RozeHub documentation integration.

---

## [0.8.0] - 2026-10-19
- Added pattern matching syntax and improved compiler diagnostics.

## [0.5.0-beta] - 2026-09-15
- C-ABI FFI and native multithreading support.

## [0.2.0-alpha] - 2026-08-01
- Core error handling with Result and Option.

## [0.1.0] - 2026-07-28
- Initial public prototype release of Roze.
