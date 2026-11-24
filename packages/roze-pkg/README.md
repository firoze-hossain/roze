# Basir (`bsr`) Global Package Registry & Monolithic Driver (`roze`)

Official package manager, registry infrastructure, and monolithic developer toolchain for the Roze programming language (`roze-pkg`).

## Architectural Highlights

1. **Go Module Proxy Protocol Parity**:
   - `/@v/list`: Newline-delimited list of version tags (`v1.0.0\nv1.1.0\n`).
   - `/@v/<version>.info`: JSON object providing version metadata, RFC 3339 timestamps, and repository origin.
   - `/@v/<version>.mfz`: Downloadable canonical package manifest in `bsr.mfz` format.

2. **RFC 8032 Ed25519 Digital Package Envelope Signing**:
   - `SignedPackageEnvelope` encapsulating package name, version, SHA-256 content checksum, public key, and 64-byte Ed25519 signature.
   - Canonical payload construction: `roze-pkg:signed-envelope:<name>:<version>:<checksum>`.
   - Complete digital signature creation and verification ensuring zero tampering across network transit.

3. **SemVer 2.0.0 & PubGrub / CDCL SAT-Based Dependency Resolution**:
   - Exact SemVer 2.0.0 comparison (`major`, `minor`, `patch`).
   - Full constraint parsing (`^`, `~`, `>=`, `<=`, `>`, `<`, `=`, `*`).
   - Conflict-Driven Clause Learning (CDCL) resolution reconciling diamond dependencies and pinpointing version conflicts with causal tracebacks.
   - Cyclic dependency detection preventing infinite recursion.

4. **Dependency Vendoring Engine (`bsr vendor`)**:
   - Copies and formats all direct and transitive dependencies into `vendor/`.
   - Generates per-package `.bsr-checksum.json` manifests tracking cryptographic file hashes (parity with Cargo's `.cargo-checksum.json`).
   - Emits `vendor/bsr-vendor.mfz` manifest enabling 100% offline air-gapped enterprise builds.
   - Cryptographic integrity verification ensuring offline sources match their original registry hashes.

5. **Monolithic Developer Toolchain CLI Driver (`roze`)**:
   - Single unified monolithic binary orchestrating the entire developer lifecycle:
     - `roze build`: Multi-backend compilation (JVM Bytecode, Native Cranelift, Freestanding bare-metal).
     - `roze run`: Immediate execution of applications.
     - `roze test`: Integrated unit and integration test runner.
     - `roze fmt`: Automated code formatter with idempotence verification.
     - `roze doc`: Static HTML documentation generator with interactive playground.
     - `roze repl`: Interactive Read-Eval-Print Loop.
     - `roze lsp`: Language Server Protocol (v3.17) daemon for IDE integration (VS Code, Neovim, Zed).
     - `roze svd`: Automated CMSIS-SVD register generator for microcontroller PACs.
     - `roze bindgen`: Automated WebAssembly TypeScript/ESM bridge generator.
     - `roze flash`: Microcontroller flash programmer for STM32, RP2040, ESP32.
     - `roze pkg`: Basir package manager (`new`, `init`, `add`, `remove`, `install`, `update`, `lock`, `vendor`, `publish`, `search`).

6. **Decentralized Multi-Registry Federation**:
   - Custom registry endpoints with public key pinning.
   - Publishing with Ed25519 signatures and automated verification.
   - Version yanking and immutable release guarantees.
