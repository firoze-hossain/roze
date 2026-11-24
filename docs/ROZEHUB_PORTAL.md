# 🌹 RozeHub Ecosystem & Global Registry Portal (`ROZEHUB_PORTAL.md`)

**The Official Package Registry, Distribution Infrastructure, and Developer Portal for Roze v3.0 'Titan'**

---

## 1. Executive Portal Architecture

**RozeHub** (`https://rozehub.roze-lang.org`) serves as the central hub for the global Roze developer ecosystem, integrating:
1. **Basir (`bsr`) Global Package Registry**: Highly available CDN distribution of immutable, digitally signed packages.
2. **Go Module Proxy Protocol Gateway**: Universal, caching-friendly endpoints serving package versions and manifests.
3. **Cryptographic Supply Chain Security**: 100% RFC 8032 Ed25519 digital envelope signing, package hash pinning, and tamper resistance.
4. **Autonomous Package Documentation**: Automated documentation generation (`rozedoc`) deployed on every package release.
5. **Interactive Playground & Code Examples**: Live browser execution via WebAssembly MVP & WASI components.

```
                             ┌──────────────────────────────────────┐
                             │       ROZEHUB DEVELOPER PORTAL       │
                             │   https://rozehub.roze-lang.org      │
                             └──────────────────┬───────────────────┘
                                                │
             ┌──────────────────────────────────┼──────────────────────────────────┐
             ▼                                  ▼                                  ▼
   [Basir Package Registry]           [Cryptographic Security]           [Autonomous Documentation]
   • CDN Proxy (pkg.roze-lang.org)    • RFC 8032 Ed25519 Envelope Signing• rozedoc API Browser
   • Go Module Proxy Protocol         • SHA-256 Content Digest Checksum  • Interactive Code Snippets
   • SemVer 2.0.0 SAT Resolver        • Air-Gapped Offline Vendoring     • Live Playground (WASM)
             │                                  │                                  │
             └──────────────────────────────────┼──────────────────────────────────┘
                                                │
                             ┌──────────────────┴───────────────────┐
                             │    MONOLITHIC DEVELOPER TOOLCHAIN    │
                             │       roze / bsr CLI Driver          │
                             └──────────────────────────────────────┘
```

---

## 2. Basir Registry & Go Module Proxy Protocol

RozeHub adopts the high-performance, caching-friendly Go module proxy specification for package distribution:

### Protocol Endpoints:
- `GET /@v/list`: Returns a newline-delimited list of all published versions for a given package.
  ```
  v1.0.0
  v2.0.0
  v3.0.0
  ```
- `GET /@v/<version>.info`: Returns a JSON metadata object describing the version:
  ```json
  {
    "version": "3.0.0",
    "time": "2026-11-24T18:00:00Z",
    "origin": "https://github.com/roze-lang/roze-release",
    "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  }
  ```
- `GET /@v/<version>.mfz`: Returns the canonical package manifest and bundle archive in `bsr.mfz` format.

---

## 3. Cryptographic Supply Chain Security

Every package published to RozeHub is protected against tampering, man-in-the-middle attacks, and malicious substitutions:

1. **RFC 8032 Ed25519 Digital Envelopes**:
   The package publisher signs a canonical envelope payload:
   `roze-pkg:signed-envelope:<package-name>:<version>:<sha256-checksum>`
2. **Public Key Pinning**:
   Clients pin author and registry public keys in `~/.roze/keys/known_publishers.json`.
3. **Deterministic Dependency Vendoring (`bsr vendor`)**:
   Enterprise users can vendor all transitive dependencies into `vendor/` along with `.bsr-checksum.json` files for air-gapped, zero-network production builds.

---

## 4. Developer Publishing & Dependency Management Workflow

### Creating a Package:
```bash
# Initialize a new package with bsr.mfz manifest
roze pkg new my-service
cd my-service
```

### Adding Dependencies:
```bash
# Add a SemVer compatible dependency from RozeHub
roze add rozeboot@^3.0.0
roze add roze-orm@^3.0.0
```

### Publishing to RozeHub:
```bash
# Authenticate with RozeHub API token
bsr login --token <ROZEHUB_TOKEN>

# Sign package envelope and publish
bsr publish
```

### Air-Gapped Offline Vendoring:
```bash
# Vendor all dependencies into local vendor/ tree
bsr vendor --verify

# Build completely offline without network access
roze build --offline
```

---

## 5. Portal Navigation & Developer Resources

- **Home Portal**: `https://rozehub.roze-lang.org`
- **Documentation**: `https://rozehub.roze-lang.org/docs`
- **Standard Library Manual**: `docs/ROZE_TITAN_MANUAL.md`
- **Package Search**: `https://rozehub.roze-lang.org/packages`
- **Community Forum & Discord**: `https://chat.roze-lang.org`
- **Security Vulnerability Advisory**: `https://rozehub.roze-lang.org/security`
