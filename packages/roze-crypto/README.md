# roze-crypto

Pure Roze cryptographic suite and TLS 1.3 engine with zero external dependencies and 1:1 parity across JVM and Native Cranelift backends.

## Features

- **Hash Functions**: FIPS 180-4 SHA-256, SHA-512, FIPS 202 SHA-3 (Keccak-256 / Keccak-512)
- **Message Authentication**: RFC 2104 HMAC-SHA256, HMAC-SHA512, RFC 5869 HKDF key derivation
- **Symmetric Encryption**: NIST SP 800-38D AES-128-GCM, AES-256-GCM, RFC 8439 ChaCha20-Poly1305 AEAD
- **Asymmetric Cryptography**: RFC 7748 X25519 Diffie-Hellman, RFC 8032 Ed25519 digital signatures, PKCS#1 v1.5 / OAEP RSA-4096
- **Password Hashing**: RFC 9106 Argon2id memory-hard key derivation
- **Transport Security**: RFC 8446 pure TLS 1.3 handshake and record layer

## Installation

Add to your `bsr.mfz`:

```toml
[dependencies]
roze-crypto = "^1.0.0"
```

## Usage

```roze
import "roze-crypto";

func main() {
    let hash = sha256_hex("Hello Roze");
    println("SHA-256: " + hash);

    let key = "secret-key-32-bytes-length-here!";
    let hmac = hmac_sha256_hex(key, "authenticated-message");
    println("HMAC: " + hmac);
}
```
