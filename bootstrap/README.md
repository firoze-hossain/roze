# 🌹 Roze Autonomous Bootstrap Subsystem (M6-T05)

## Overview

Roze is an autonomous, self-hosting programming language. Following the complete decommissioning of the legacy Rust host compiler (`M6-T05`), building Roze requires **only Roze** (or any standard C99 compiler to build the initial seed).

The Roze repository build pipeline is completely decoupled from `rustc` and `cargo`.

---

## Bootstrap Architecture

```
                  ┌──────────────────────────────┐
                  │   bootstrap/roze_seed.c      │ (Standard C99 Seed)
                  └──────────────┬───────────────┘
                                 │
                   gcc -O2 -w    │ (0.5s compile time)
                                 ▼
                  ┌──────────────────────────────┐
                  │    bin/roze_bootstrap        │ (Pure Roze Compiler Seed)
                  └──────────────┬───────────────┘
                                 │
                   compiles      │ src/roze_compiler.roze
                                 ▼
                  ┌──────────────────────────────┐
                  │    bin/roze_stage1           │ (Stage 1 Pure Roze Compiler)
                  └──────────────┬───────────────┘
                                 │
                   compiles      │ src/roze_compiler.roze
                                 ▼
                  ┌──────────────────────────────┐
                  │    bin/roze_stage2           │ (Stage 2 Pure Roze Compiler)
                  └──────────────┬───────────────┘
                                 │
                   compiles      │ src/roze_compiler.roze
                                 ▼
                  ┌──────────────────────────────┐
                  │    bin/roze_stage3           │ (Stage 3 Pure Roze Compiler)
                  └──────────────┬───────────────┘
                                 │
               diff -u roze_stage2.c roze_stage3.c == 0 bytes
                                 │
                                 ▼
                   ✅ Mathematical Fixpoint Verified
                   Copied to `bin/roze` (and `./roze`)
```

---

## Quick Start (Zero Rust Required)

To build Roze without Rust or Cargo:

```bash
# Build the self-hosted compiler and verify the 3-stage bootstrap fixpoint:
make

# Run the complete test suite:
make test

# Install Roze to /usr/local/bin:
sudo make install
```

Or using the portable build script:

```bash
./build.sh
./build.sh test
```

---

## Reproducing the Seed

To regenerate `bootstrap/roze_seed.c` from the latest pure Roze source code:

```bash
make seed
```
