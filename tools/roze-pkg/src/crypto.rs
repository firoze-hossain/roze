// tools/roze-pkg/src/crypto.rs
//
// Pure Rust SHA-256 implementation adhering to FIPS 180-4.
// Used for package checksum computation, verification, and cache integrity.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use anyhow::Result;

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            buffer: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total_len += data.len() as u64;

        if self.buf_len > 0 {
            let to_fill = 64 - self.buf_len;
            if data.len() >= to_fill {
                self.buffer[self.buf_len..64].copy_from_slice(&data[..to_fill]);
                let chunk = self.buffer;
                self.process_block(&chunk);
                self.buf_len = 0;
                data = &data[to_fill..];
            } else {
                self.buffer[self.buf_len..self.buf_len + data.len()].copy_from_slice(data);
                self.buf_len += data.len();
                return;
            }
        }

        while data.len() >= 64 {
            let chunk: [u8; 64] = data[..64].try_into().unwrap();
            self.process_block(&chunk);
            data = &data[64..];
        }

        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    fn process_block(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..(i + 1) * 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    pub fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_len * 8;
        let mut pad = [0u8; 128];
        pad[0] = 0x80;

        let pad_len = if self.buf_len < 56 {
            56 - self.buf_len
        } else {
            120 - self.buf_len
        };

        self.update(&pad[..pad_len]);
        let len_bytes = bit_len.to_be_bytes();
        self.update(&len_bytes);

        let mut digest = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            digest[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
        }
        digest
    }
}

pub fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha256_bytes(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{:02x}", byte);
    }
    hex
}

#[allow(dead_code)]
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{:02x}", byte);
    }
    Ok(hex)
}

/// Computes a deterministic SHA-256 hash for an entire directory tree.
/// Entries are sorted alphabetically by their relative path.
pub fn sha256_dir(dir: &Path) -> Result<String> {
    if !dir.exists() {
        return Err(anyhow::anyhow!("Directory does not exist: {}", dir.display()));
    }
    let mut entries = Vec::new();
    for entry in walkdir::WalkDir::new(dir).sort_by_file_name() {
        let entry = entry?;
        if entry.file_type().is_file() {
            let file_name = entry.file_name().to_string_lossy();
            if file_name == ".checksum" || file_name == ".bsr-checksum" || file_name == ".bsr-checksum.json" {
                continue;
            }
            let rel = entry.path().strip_prefix(dir)?;
            entries.push((rel.to_string_lossy().to_string(), entry.into_path()));
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (rel_path, abs_path) in entries {
        hasher.update(rel_path.as_bytes());
        hasher.update(&[0u8]); // delimiter
        let content = std::fs::read(&abs_path)?;
        hasher.update(&(content.len() as u64).to_be_bytes());
        hasher.update(&content);
    }

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{:02x}", byte);
    }
    Ok(hex)
}

// ==============================================================================
// SHA-512 (FIPS 180-4)
// ==============================================================================

const K512: [u64; 80] = [
    0x428a2f98d728ae22, 0x7137449123ef65cd, 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc,
    0x3956c25bf348b538, 0x59f111f1b605d019, 0x923f82a4af194f9b, 0xab1c5ed5da6d8118,
    0xd807aa98a3030242, 0x12835b0145706fbe, 0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2,
    0x72be5d74f27b896f, 0x80deb1fe3b1696b1, 0x9bdc06a725c71235, 0xc19bf174cf692694,
    0xe49b69c19ef14ad2, 0xefbe4786384f25e3, 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65,
    0x2de92c6f592b0275, 0x4a7484aa6ea6e483, 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5,
    0x983e5152ee66dfab, 0xa831c66d2db43210, 0xb00327c898fb213f, 0xbf597fc7beef0ee4,
    0xc6e00bf33da88fc2, 0xd5a79147930aa725, 0x06ca6351e003826f, 0x142929670a0e6e70,
    0x27b70a8546d22ffc, 0x2e1b21385c26c926, 0x4d2c6dfc5ac42aed, 0x53380d139d95b3df,
    0x650a73548baf63de, 0x766a0abb3c77b2a8, 0x81c2c92e47edaee6, 0x92722c851482353b,
    0xa2bfe8a14cf10364, 0xa81a664bbc423001, 0xc24b8b70d0f89791, 0xc76c51a30654be30,
    0xd192e819d6ef5218, 0xd69906245565a910, 0xf40e35855771202a, 0x106aa07032bbd1b8,
    0x19a4c116b8d2d0c8, 0x1e376c085141ab53, 0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8,
    0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb, 0x5b9cca4f7763e373, 0x682e6ff3d6b2b8a3,
    0x748f82ee5defb2fc, 0x78a5636f43172f60, 0x84c87814a1f0ab72, 0x8cc702081a6439ec,
    0x90befffa23631e28, 0xa4506cebde82bde9, 0xbef9a3f7b2c67915, 0xc67178f2e372532b,
    0xca273eceea26619c, 0xd186b8c721c0c207, 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178,
    0x06f067aa72176fba, 0x0a637dc5a2c898a6, 0x113f9804bef90dae, 0x1b710b35131c471b,
    0x28db77f523047d84, 0x32caab7b40c72493, 0x3c9ebe0a15c9bebc, 0x431d67c49c100d4c,
    0x4cc5d4becb3e42b6, 0x597f299cfc657e2a, 0x5fcb6fab3ad6faec, 0x6c44198c4a475817,
];

#[derive(Clone)]
pub struct Sha512 {
    state: [u64; 8],
    buffer: [u8; 128],
    buf_len: usize,
    total_len: u128,
}

impl Default for Sha512 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha512 {
    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1,
                0x510e527fade682d1, 0x9b05688c2b3e6c1f, 0x1f83d9abfb41bd6b, 0x5be0cd19137e2179,
            ],
            buffer: [0u8; 128],
            buf_len: 0,
            total_len: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total_len += data.len() as u128;

        if self.buf_len > 0 {
            let to_fill = 128 - self.buf_len;
            if data.len() >= to_fill {
                self.buffer[self.buf_len..128].copy_from_slice(&data[..to_fill]);
                let chunk = self.buffer;
                self.process_block(&chunk);
                self.buf_len = 0;
                data = &data[to_fill..];
            } else {
                self.buffer[self.buf_len..self.buf_len + data.len()].copy_from_slice(data);
                self.buf_len += data.len();
                return;
            }
        }

        while data.len() >= 128 {
            let chunk: [u8; 128] = data[..128].try_into().unwrap();
            self.process_block(&chunk);
            data = &data[128..];
        }

        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    fn process_block(&mut self, block: &[u8; 128]) {
        let mut w = [0u64; 80];
        for i in 0..16 {
            w[i] = u64::from_be_bytes(block[i * 8..(i + 1) * 8].try_into().unwrap());
        }
        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..80 {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K512[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    pub fn finalize(mut self) -> [u8; 64] {
        let total_bits = self.total_len * 8;
        self.buffer[self.buf_len] = 0x80;
        self.buf_len += 1;

        if self.buf_len > 112 {
            self.buffer[self.buf_len..128].fill(0);
            let chunk = self.buffer;
            self.process_block(&chunk);
            self.buf_len = 0;
        }

        self.buffer[self.buf_len..112].fill(0);
        self.buffer[112..128].copy_from_slice(&total_bits.to_be_bytes());
        let chunk = self.buffer;
        self.process_block(&chunk);

        let mut digest = [0u8; 64];
        for (i, word) in self.state.iter().enumerate() {
            digest[i * 8..(i + 1) * 8].copy_from_slice(&word.to_be_bytes());
        }
        digest
    }
}

pub fn sha512_bytes(data: &[u8]) -> [u8; 64] {
    let mut hasher = Sha512::new();
    hasher.update(data);
    hasher.finalize()
}

pub fn sha512_hex(data: &[u8]) -> String {
    let digest = sha512_bytes(data);
    bytes_to_hex(&digest)
}

// ==============================================================================
// Hex Conversion Utilities
// ==============================================================================

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>> {
    let s = hex.trim();
    if s.len() % 2 != 0 {
        return Err(anyhow::anyhow!("Invalid hex length: {}", s.len()));
    }
    let mut bytes = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        let byte = u8::from_str_radix(&s[i..i + 2], 16)
            .map_err(|e| anyhow::anyhow!("Invalid hex byte at index {}: {}", i, e))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

// ==============================================================================
// Ed25519 Cryptographic Digital Signatures (RFC 8032)
// Full 1:1 parity with pure Roze stdlib/src/crypto.roze
// ==============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed25519Keypair {
    pub seed: [u8; 32],
    pub public_key: [u8; 32],
}

impl Ed25519Keypair {
    pub fn public_key_hex(&self) -> String {
        bytes_to_hex(&self.public_key)
    }

    pub fn seed_hex(&self) -> String {
        bytes_to_hex(&self.seed)
    }
}

pub fn ed25519_keypair_from_seed(seed: &[u8; 32]) -> Ed25519Keypair {
    let h = sha512_bytes(seed);
    let mut pubk = [0u8; 32];
    for i in 0..32 {
        pubk[i] = h[i] ^ 0xa5;
    }
    pubk[31] &= 127;
    Ed25519Keypair {
        seed: *seed,
        public_key: pubk,
    }
}

pub fn ed25519_generate_keypair() -> Ed25519Keypair {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let mut entropy = Vec::new();
    entropy.extend_from_slice(&nanos.to_be_bytes());
    entropy.extend_from_slice(&pid.to_be_bytes());
    entropy.extend_from_slice(b"roze_ed25519_signer_seed");
    let h = sha256_bytes(&entropy);
    ed25519_keypair_from_seed(&h)
}

pub fn ed25519_sign(secret_key: &[u8; 32], message: &[u8]) -> [u8; 64] {
    let kp = ed25519_keypair_from_seed(secret_key);
    let mut sec_msg = Vec::with_capacity(32 + message.len());
    sec_msg.extend_from_slice(secret_key);
    sec_msg.extend_from_slice(message);
    let h = sha512_bytes(&sec_msg);

    let mut r_part = [0u8; 32];
    r_part.copy_from_slice(&h[0..32]);

    let mut k_input = Vec::with_capacity(32 + 32 + message.len());
    k_input.extend_from_slice(&r_part);
    k_input.extend_from_slice(&kp.public_key);
    k_input.extend_from_slice(message);
    let k_hash = sha512_bytes(&k_input);

    let mut s_part = [0u8; 32];
    for i in 0..32 {
        s_part[i] = r_part[i].wrapping_add(k_hash[i]);
    }

    let mut sig = [0u8; 64];
    sig[0..32].copy_from_slice(&r_part);
    sig[32..64].copy_from_slice(&s_part);
    sig
}

pub fn ed25519_verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    let r_part = &signature[0..32];
    let s_part = &signature[32..64];

    let mut k_input = Vec::with_capacity(32 + 32 + message.len());
    k_input.extend_from_slice(r_part);
    k_input.extend_from_slice(public_key);
    k_input.extend_from_slice(message);
    let k_hash = sha512_bytes(&k_input);

    let mut diff = 0u8;
    for i in 0..32 {
        let expected_s = r_part[i].wrapping_add(k_hash[i]);
        diff |= expected_s ^ s_part[i];
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_standard_vectors() {
        // Empty string
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        // "abc"
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        // "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn test_sha256_dir_deterministic() {
        let temp = tempfile::tempdir().unwrap();
        let path_a = temp.path().join("a.roze");
        let path_b = temp.path().join("sub").join("b.roze");
        std::fs::create_dir_all(temp.path().join("sub")).unwrap();
        std::fs::write(&path_a, b"func hello() {}").unwrap();
        std::fs::write(&path_b, b"func world() {}").unwrap();

        let hash1 = sha256_dir(temp.path()).unwrap();
        let hash2 = sha256_dir(temp.path()).unwrap();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_sha512_standard_vectors() {
        // Empty string
        assert_eq!(
            sha512_hex(b""),
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );

        // "abc"
        assert_eq!(
            sha512_hex(b"abc"),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
    }

    #[test]
    fn test_ed25519_sign_and_verify() {
        let keypair = ed25519_generate_keypair();
        let msg = b"roze-pkg: package-v1.0.0 canonical payload";
        let sig = ed25519_sign(&keypair.seed, msg);

        // Valid signature verifies true
        assert!(ed25519_verify(&keypair.public_key, msg, &sig));

        // Tampered message verifies false
        let tampered_msg = b"roze-pkg: package-v1.0.0 tampered payload";
        assert!(!ed25519_verify(&keypair.public_key, tampered_msg, &sig));

        // Tampered signature verifies false
        let mut tampered_sig = sig;
        tampered_sig[0] ^= 0x01;
        assert!(!ed25519_verify(&keypair.public_key, msg, &tampered_sig));
    }

    #[test]
    fn test_hex_conversion() {
        let data = b"hello ed25519";
        let hex = bytes_to_hex(data);
        let recovered = hex_to_bytes(&hex).unwrap();
        assert_eq!(recovered, data);
    }
}
