//! TOTP (RFC 6238): kode 2FA yang berganti setiap periode, kompatibel dengan
//! Google Authenticator, Authy, Microsoft Authenticator, dll.

use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Algo { Sha1, Sha256, Sha512 }

#[derive(Debug, Clone)]
pub struct Totp { pub secret: Vec<u8>, pub digits: u32, pub period: u64, pub algo: Algo, pub issuer: String, pub account: String }

fn base32_decode(s: &str) -> Result<Vec<u8>, String> {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let (mut buf, mut bits, mut out) = (0u64, 0u32, Vec::new());
    for c in s.chars().filter(|c| !c.is_whitespace() && *c != '-' && *c != '=') {
        let v = A.iter().position(|&a| a as char == c.to_ascii_uppercase())
            .ok_or_else(|| format!("Karakter '{c}' tidak valid untuk kunci 2FA (Base32)."))?;
        buf = (buf << 5) | v as u64;
        bits += 5;
        if bits >= 8 { bits -= 8; out.push((buf >> bits) as u8); buf &= (1 << bits) - 1; }
    }
    if out.is_empty() { return Err("Kunci 2FA kosong.".into()); }
    Ok(out)
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v); i += 3; continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Terima kunci Base32 biasa ("JBSW Y3DP ...") atau tautan otpauth://totp/...
pub fn parse(input: &str) -> Result<Totp, String> {
    let input = input.trim();
    if input.is_empty() { return Err("Kunci 2FA kosong.".into()); }
    let mut t = Totp { secret: vec![], digits: 6, period: 30, algo: Algo::Sha1, issuer: String::new(), account: String::new() };
    if input.to_lowercase().starts_with("otpauth://") {
        let rest = &input["otpauth://".len()..];
        let (kind, rest) = rest.split_once('/').ok_or("Tautan otpauth tidak valid.")?;
        if !kind.eq_ignore_ascii_case("totp") { return Err("Hanya 2FA berbasis waktu (TOTP) yang didukung.".into()); }
        let (label, query) = rest.split_once('?').unwrap_or((rest, ""));
        let label = pct_decode(label);
        match label.split_once(':') {
            Some((i, a)) => { t.issuer = i.trim().into(); t.account = a.trim().into(); }
            None => t.account = label.trim().into(),
        }
        let mut secret = None;
        for kv in query.split('&') {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            let v = pct_decode(v);
            match k.to_lowercase().as_str() {
                "secret" => secret = Some(v),
                "issuer" => t.issuer = v,
                "digits" => t.digits = v.parse().map_err(|_| "Nilai digits tidak valid.")?,
                "period" => t.period = v.parse().map_err(|_| "Nilai period tidak valid.")?,
                "algorithm" => t.algo = match v.to_uppercase().as_str() {
                    "SHA1" => Algo::Sha1, "SHA256" => Algo::Sha256, "SHA512" => Algo::Sha512,
                    _ => return Err("Algoritma 2FA tidak didukung.".into()),
                },
                _ => {}
            }
        }
        t.secret = base32_decode(&secret.ok_or("Tautan otpauth tidak berisi secret.")?)?;
    } else {
        t.secret = base32_decode(input)?;
    }
    if !(6..=8).contains(&t.digits) { return Err("Jumlah digit 2FA harus 6 sampai 8.".into()); }
    if !(10..=300).contains(&t.period) { return Err("Periode 2FA tidak wajar.".into()); }
    Ok(t)
}

impl Totp {
    pub fn code_at(&self, unix: u64) -> String {
        let msg = (unix / self.period).to_be_bytes();
        let h: Vec<u8> = match self.algo {
            Algo::Sha1 => { let mut m = Hmac::<Sha1>::new_from_slice(&self.secret).unwrap(); m.update(&msg); m.finalize().into_bytes().to_vec() }
            Algo::Sha256 => { let mut m = Hmac::<Sha256>::new_from_slice(&self.secret).unwrap(); m.update(&msg); m.finalize().into_bytes().to_vec() }
            Algo::Sha512 => { let mut m = Hmac::<Sha512>::new_from_slice(&self.secret).unwrap(); m.update(&msg); m.finalize().into_bytes().to_vec() }
        };
        let o = (h[h.len() - 1] & 0x0f) as usize;
        let bin = ((h[o] as u32 & 0x7f) << 24) | ((h[o + 1] as u32) << 16) | ((h[o + 2] as u32) << 8) | h[o + 3] as u32;
        format!("{:0width$}", bin % 10u32.pow(self.digits), width = self.digits as usize)
    }
    pub fn remaining(&self, unix: u64) -> u64 { self.period - unix % self.period }
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Baca QR dari gambar (PNG/JPEG/WebP), mis. screenshot halaman pengaturan 2FA.
pub fn decode_qr(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > 15 * 1024 * 1024 { return Err("Gambar terlalu besar (maks 15 MB).".into()); }
    let img = image::load_from_memory(bytes).map_err(|_| "File bukan gambar yang didukung (PNG/JPEG/WebP).")?.to_luma8();
    let (w, h) = img.dimensions();
    let mut prep = rqrr::PreparedImage::prepare_from_greyscale(w as usize, h as usize, |x, y| img.get_pixel(x as u32, y as u32)[0]);
    for g in prep.detect_grids() {
        if let Ok((_, content)) = g.decode() { return Ok(content); }
    }
    Err("QR code tidak ditemukan di gambar. Coba screenshot yang lebih jelas dan hanya berisi QR.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    // Vektor uji resmi RFC 6238 (kunci ASCII "12345678901234567890", 8 digit)
    const SEC1: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    #[test]
    fn rfc6238_sha1_vectors() {
        let mut t = parse(SEC1).unwrap(); t.digits = 8;
        assert_eq!(t.code_at(59), "94287082");
        assert_eq!(t.code_at(1111111109), "07081804");
        assert_eq!(t.code_at(1234567890), "89005924");
        assert_eq!(t.code_at(20000000000), "65353130");
    }

    #[test]
    fn rfc6238_sha256_sha512_vectors() {
        let s256 = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZA";
        let t = parse(&format!("otpauth://totp/x?secret={s256}&algorithm=SHA256&digits=8")).unwrap();
        assert_eq!(t.code_at(59), "46119246");
        let s512 = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNA";
        let t = parse(&format!("otpauth://totp/x?secret={s512}&algorithm=SHA512&digits=8")).unwrap();
        assert_eq!(t.code_at(59), "90693936");
    }

    #[test]
    fn parses_otpauth_uri() {
        let t = parse("otpauth://totp/Google%3Abudi%40gmail.com?secret=JBSWY3DPEHPK3PXP&issuer=Google").unwrap();
        assert_eq!((t.issuer.as_str(), t.account.as_str(), t.digits, t.period), ("Google", "budi@gmail.com", 6, 30));
        assert_eq!(t.code_at(59).len(), 6);
    }

    #[test]
    fn accepts_spaced_lowercase_secret() {
        assert_eq!(parse("jbsw y3dp ehpk 3pxp").unwrap().secret, parse("JBSWY3DPEHPK3PXP").unwrap().secret);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("").is_err());
        assert!(parse("ABC!@#").is_err());
        assert!(parse("otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP").is_err());
        assert!(parse("otpauth://totp/x?issuer=a").is_err());
    }

    #[test]
    fn decodes_qr_from_image() {
        let png = include_bytes!("../tests/fixtures/totp-qr.png");
        let text = decode_qr(png).unwrap();
        assert_eq!(text, "otpauth://totp/Contoh:budi@contoh.id?secret=JBSWY3DPEHPK3PXP&issuer=Contoh");
        assert!(decode_qr(b"bukan gambar").is_err());
    }

    #[test]
    fn pct_decode_handles_edge_cases() {
        assert_eq!(pct_decode("a%40b"), "a@b");
        assert_eq!(pct_decode("50%"), "50%");
        assert_eq!(pct_decode("%zz%E2%9C%93"), "%zz✓");
    }

    #[test]
    fn remaining_seconds() {
        let t = parse("JBSWY3DPEHPK3PXP").unwrap();
        assert_eq!(t.remaining(60), 30);
        assert_eq!(t.remaining(59), 1);
    }
}
