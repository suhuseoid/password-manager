//! Inti Brankas: file brankas terenkripsi.
//!
//! Format file (JSON):
//!   { format, version, kdf: {alg, m_kib, t, p, salt}, nonce, ciphertext }
//! Kunci = Argon2id(master password, salt). Data dienkripsi XChaCha20-Poly1305
//! dengan header (format, versi, parameter KDF) sebagai associated data,
//! sehingga perubahan sekecil apa pun pada file akan terdeteksi.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, seq::SliceRandom, Rng, RngCore};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub const FORMAT: &str = "brankas-vault";
pub const VERSION: u32 = 1;
pub const MIN_MASTER_LEN: usize = 10;

#[derive(Debug)]
pub enum VaultError {
    WrongPassword,
    Corrupt(String),
    Io(String),
    Invalid(String),
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VaultError::WrongPassword => write!(f, "Master password salah, atau file brankas telah diubah."),
            VaultError::Corrupt(m) => write!(f, "File brankas rusak: {m}"),
            VaultError::Io(m) => write!(f, "Gagal membaca/menulis file: {m}"),
            VaultError::Invalid(m) => write!(f, "{m}"),
        }
    }
}

impl From<std::io::Error> for VaultError {
    fn from(e: std::io::Error) -> Self { VaultError::Io(e.to_string()) }
}

pub type Result<T> = std::result::Result<T, VaultError>;

/// Biaya Argon2id. Bawaan: 64 MiB memori, 3 iterasi.
#[derive(Clone, Copy)]
pub struct KdfCost { pub m_kib: u32, pub t: u32, pub p: u32 }

impl Default for KdfCost {
    fn default() -> Self { KdfCost { m_kib: 64 * 1024, t: 3, p: 1 } }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct KdfParams { pub alg: String, pub m_kib: u32, pub t: u32, pub p: u32, pub salt: String }

#[derive(Serialize, Deserialize)]
struct VaultFile { format: String, version: u32, kdf: KdfParams, nonce: String, ciphertext: String }

#[derive(Serialize, Deserialize, Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct Entry {
    pub id: String,
    /// "login" atau "note" (catatan aman, mis. kode backup)
    #[serde(default = "default_kind")]
    pub kind: String,
    pub title: String,
    pub url: String,
    pub username: String,
    pub password: String,
    pub notes: String,
    /// Kunci 2FA (Base32 atau tautan otpauth://)
    #[serde(default)]
    pub totp: String,
    pub created: i64,
    pub updated: i64,
}

fn default_kind() -> String { "login".into() }

impl Entry {
    pub fn new(title: String, url: String, username: String, password: String, notes: String) -> Self {
        Entry { id: String::new(), kind: default_kind(), title, url, username, password, notes, totp: String::new(), created: 0, updated: 0 }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct VaultData { pub entries: Vec<Entry> }

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn aad(kdf: &KdfParams) -> Vec<u8> {
    format!("{FORMAT}|{VERSION}|{}|{}|{}|{}|{}", kdf.alg, kdf.m_kib, kdf.t, kdf.p, kdf.salt).into_bytes()
}

fn derive_key(master: &str, kdf: &KdfParams) -> Result<Zeroizing<[u8; 32]>> {
    if kdf.alg != "argon2id" { return Err(VaultError::Corrupt("algoritma KDF tidak dikenal".into())); }
    // Batas wajar agar file yang dimanipulasi tidak membuat aplikasi kehabisan memori
    if kdf.m_kib < 8 || kdf.m_kib > 4 * 1024 * 1024 || kdf.t == 0 || kdf.t > 100 || kdf.p == 0 || kdf.p > 16 {
        return Err(VaultError::Corrupt("parameter KDF di luar batas".into()));
    }
    let salt = B64.decode(&kdf.salt).map_err(|_| VaultError::Corrupt("salt tidak valid".into()))?;
    let params = Params::new(kdf.m_kib, kdf.t, kdf.p, Some(32)).map_err(|e| VaultError::Corrupt(e.to_string()))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(master.as_bytes(), &salt, key.as_mut())
        .map_err(|e| VaultError::Corrupt(e.to_string()))?;
    Ok(key)
}

fn new_kdf(cost: KdfCost) -> KdfParams {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    KdfParams { alg: "argon2id".into(), m_kib: cost.m_kib, t: cost.t, p: cost.p, salt: B64.encode(salt) }
}

pub struct Vault {
    path: PathBuf,
    key: Zeroizing<[u8; 32]>,
    kdf: KdfParams,
    pub data: VaultData,
}

impl Vault {
    pub fn path(&self) -> &Path { &self.path }

    pub fn create(path: &Path, master: &str, cost: KdfCost) -> Result<Self> {
        if path.exists() { return Err(VaultError::Invalid("File brankas sudah ada.".into())); }
        if master.chars().count() < MIN_MASTER_LEN {
            return Err(VaultError::Invalid(format!("Master password minimal {MIN_MASTER_LEN} karakter.")));
        }
        let kdf = new_kdf(cost);
        let key = derive_key(master, &kdf)?;
        let v = Vault { path: path.to_path_buf(), key, kdf, data: VaultData::default() };
        v.save()?;
        Ok(v)
    }

    pub fn open(path: &Path, master: &str) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let file: VaultFile = serde_json::from_str(&raw).map_err(|e| VaultError::Corrupt(e.to_string()))?;
        if file.format != FORMAT { return Err(VaultError::Corrupt("bukan file Brankas".into())); }
        if file.version != VERSION { return Err(VaultError::Corrupt(format!("versi {} tidak didukung", file.version))); }
        let key = derive_key(master, &file.kdf)?;
        let nonce = B64.decode(&file.nonce).map_err(|_| VaultError::Corrupt("nonce tidak valid".into()))?;
        if nonce.len() != 24 { return Err(VaultError::Corrupt("panjang nonce salah".into())); }
        let ct = B64.decode(&file.ciphertext).map_err(|_| VaultError::Corrupt("ciphertext tidak valid".into()))?;
        let cipher = XChaCha20Poly1305::new(key.as_ref().into());
        let plain = Zeroizing::new(
            cipher
                .decrypt(XNonce::from_slice(&nonce), Payload { msg: &ct, aad: &aad(&file.kdf) })
                .map_err(|_| VaultError::WrongPassword)?,
        );
        let data: VaultData = serde_json::from_slice(&plain).map_err(|e| VaultError::Corrupt(e.to_string()))?;
        Ok(Vault { path: path.to_path_buf(), key, kdf: file.kdf, data })
    }

    /// Simpan secara atomik: tulis ke file sementara, salin versi lama ke .bak, lalu ganti.
    pub fn save(&self) -> Result<()> {
        let plain = Zeroizing::new(serde_json::to_vec(&self.data).map_err(|e| VaultError::Io(e.to_string()))?);
        let mut nonce = [0u8; 24];
        OsRng.fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new(self.key.as_ref().into());
        let ct = cipher
            .encrypt(XNonce::from_slice(&nonce), Payload { msg: &plain, aad: &aad(&self.kdf) })
            .map_err(|_| VaultError::Io("enkripsi gagal".into()))?;
        let file = VaultFile {
            format: FORMAT.into(), version: VERSION, kdf: self.kdf.clone(),
            nonce: B64.encode(nonce), ciphertext: B64.encode(ct),
        };
        let json = serde_json::to_string_pretty(&file).map_err(|e| VaultError::Io(e.to_string()))?;
        if let Some(dir) = self.path.parent() { fs::create_dir_all(dir)?; }
        let tmp = self.path.with_extension("vault.tmp");
        fs::write(&tmp, json)?;
        if self.path.exists() { fs::copy(&self.path, self.path.with_extension("vault.bak"))?; }
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn change_master(&mut self, new_master: &str, cost: KdfCost) -> Result<()> {
        if new_master.chars().count() < MIN_MASTER_LEN {
            return Err(VaultError::Invalid(format!("Master password minimal {MIN_MASTER_LEN} karakter.")));
        }
        let kdf = new_kdf(cost);
        self.key = derive_key(new_master, &kdf)?;
        self.kdf = kdf;
        self.save()
    }

    pub fn upsert(&mut self, mut e: Entry) -> Result<String> {
        if e.kind != "login" && e.kind != "note" { e.kind = default_kind(); }
        if e.kind == "note" {
            if e.title.trim().is_empty() { return Err(VaultError::Invalid("Catatan harus punya judul.".into())); }
            e.url.clear(); e.username.clear(); e.password.clear(); e.totp.clear();
        } else if e.title.trim().is_empty() && e.url.trim().is_empty() {
            return Err(VaultError::Invalid("Isi minimal judul atau alamat situs.".into()));
        }
        e.totp = e.totp.trim().to_string();
        if !e.totp.is_empty() {
            crate::totp::parse(&e.totp).map_err(VaultError::Invalid)?;
        }
        let t = now();
        if let Some(existing) = self.data.entries.iter_mut().find(|x| !e.id.is_empty() && x.id == e.id) {
            e.created = existing.created;
            e.updated = t;
            *existing = e.clone();
        } else {
            e.id = uuid::Uuid::new_v4().to_string();
            e.created = t;
            e.updated = t;
            self.data.entries.push(e.clone());
        }
        self.save()?;
        Ok(e.id.clone())
    }

    pub fn delete(&mut self, id: &str) -> Result<()> {
        let before = self.data.entries.len();
        self.data.entries.retain(|x| x.id != id);
        if self.data.entries.len() == before { return Err(VaultError::Invalid("Data tidak ditemukan.".into())); }
        self.save()
    }

    pub fn get(&self, id: &str) -> Option<&Entry> { self.data.entries.iter().find(|x| x.id == id) }

    /// Tambahkan hasil impor; lewati yang persis sama (situs, username, password).
    pub fn import(&mut self, items: Vec<Entry>) -> Result<(usize, usize)> {
        let (mut added, mut dup) = (0, 0);
        let t = now();
        for mut e in items {
            let same = self.data.entries.iter().any(|x| {
                norm_url(&x.url) == norm_url(&e.url) && x.username == e.username && x.password == e.password
            });
            if same { dup += 1; continue; }
            e.id = uuid::Uuid::new_v4().to_string();
            e.created = t;
            e.updated = t;
            self.data.entries.push(e);
            added += 1;
        }
        if added > 0 { self.save()?; }
        Ok((added, dup))
    }
}

pub fn host_of(url: &str) -> String {
    let s = url.trim();
    let s = s.split("://").nth(1).unwrap_or(s);
    let s = s.split(['/', '?', '#']).next().unwrap_or("");
    let s = s.rsplit('@').next().unwrap_or(s);
    s.trim_start_matches("www.").to_lowercase()
}

fn norm_url(url: &str) -> String { url.trim().trim_end_matches('/').to_lowercase() }

/// Baca CSV ekspor Chrome/Edge/Brave, Firefox, Safari, atau Bitwarden.
pub fn parse_csv(text: &str) -> Result<(Vec<Entry>, usize, String)> {
    let text = text.trim_start_matches('\u{feff}');
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(text.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| VaultError::Invalid(format!("CSV tidak bisa dibaca: {e}")))?
        .iter()
        .map(|h| h.trim().to_lowercase())
        .collect();
    let col = |names: &[&str]| names.iter().find_map(|n| headers.iter().position(|h| h == n));
    let c_pass = col(&["password", "login_password"])
        .ok_or_else(|| VaultError::Invalid("Kolom password tidak ditemukan. Pastikan ini file ekspor password dari browser.".into()))?;
    let c_url = col(&["url", "login_uri", "website", "origin"]);
    let c_user = col(&["username", "login_username", "user", "email"]);
    let c_title = col(&["name", "title"]);
    let c_notes = col(&["note", "notes", "extra"]);
    let c_totp = col(&["otpauth", "login_totp", "totp"]);
    let has = |n: &str| headers.iter().any(|h| h == n);
    let source = if has("login_uri") { "Bitwarden" }
        else if has("httprealm") { "Firefox" }
        else if has("otpauth") || has("title") { "Safari" }
        else if has("name") && has("url") { "Chrome/Edge" }
        else { "CSV" }.to_string();

    let mut out = Vec::new();
    let mut skipped = 0;
    for rec in rdr.records() {
        let rec = match rec { Ok(r) => r, Err(_) => { skipped += 1; continue; } };
        let get = |c: Option<usize>| c.and_then(|i| rec.get(i)).unwrap_or("").trim().to_string();
        let password = rec.get(c_pass).unwrap_or("").to_string();
        if password.is_empty() { skipped += 1; continue; }
        let url = get(c_url);
        let username = get(c_user);
        let mut title = get(c_title);
        if title.is_empty() { title = host_of(&url); }
        if title.is_empty() { title = username.clone(); }
        let mut e = Entry::new(title, url, username, password, get(c_notes));
        let totp = get(c_totp);
        if !totp.is_empty() && crate::totp::parse(&totp).is_ok() { e.totp = totp; }
        out.push(e);
    }
    Ok((out, skipped, source))
}

/// Buat password acak kuat; minimal satu karakter dari tiap kelompok yang dipilih.
pub fn generate_password(len: usize, symbols: bool) -> String {
    let len = len.clamp(8, 128);
    let lower = b"abcdefghijkmnopqrstuvwxyz";
    let upper = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
    let digits = b"23456789";
    let syms = b"!@#$%^&*-_=+?";
    let mut groups: Vec<&[u8]> = vec![lower, upper, digits];
    if symbols { groups.push(syms); }
    let all: Vec<u8> = groups.iter().flat_map(|g| g.iter().copied()).collect();
    let mut rng = OsRng;
    let mut out: Vec<u8> = groups.iter().map(|g| g[rng.gen_range(0..g.len())]).collect();
    while out.len() < len { out.push(all[rng.gen_range(0..all.len())]); }
    out.shuffle(&mut rng);
    String::from_utf8(out).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    const FAST: KdfCost = KdfCost { m_kib: 1024, t: 1, p: 1 };

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("brankas-test-{}-{}", name, uuid::Uuid::new_v4()));
        fs::create_dir_all(&d).unwrap();
        d.join("brankas.vault")
    }

    fn sample() -> Entry {
        Entry::new("Bank".into(), "https://bank.example".into(), "budi".into(), "rahasia123".into(), String::new())
    }

    #[test]
    fn create_save_open_roundtrip() {
        let p = tmp("rt");
        let mut v = Vault::create(&p, "master-password-kuat", FAST).unwrap();
        v.upsert(sample()).unwrap();
        let v2 = Vault::open(&p, "master-password-kuat").unwrap();
        assert_eq!(v2.data.entries.len(), 1);
        assert_eq!(v2.data.entries[0].password, "rahasia123");
    }

    #[test]
    fn file_does_not_contain_plaintext() {
        let p = tmp("plain");
        let mut v = Vault::create(&p, "master-password-kuat", FAST).unwrap();
        v.upsert(sample()).unwrap();
        let raw = fs::read_to_string(&p).unwrap();
        assert!(!raw.contains("rahasia123") && !raw.contains("budi") && !raw.contains("bank.example"));
    }

    #[test]
    fn wrong_password_rejected() {
        let p = tmp("wrong");
        Vault::create(&p, "master-password-kuat", FAST).unwrap();
        assert!(matches!(Vault::open(&p, "salah-password-123"), Err(VaultError::WrongPassword)));
    }

    #[test]
    fn tampering_detected() {
        let p = tmp("tamper");
        let mut v = Vault::create(&p, "master-password-kuat", FAST).unwrap();
        v.upsert(sample()).unwrap();
        let mut f: serde_json::Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        // Ubah satu byte ciphertext
        let mut ct = B64.decode(f["ciphertext"].as_str().unwrap()).unwrap();
        ct[5] ^= 1;
        f["ciphertext"] = B64.encode(ct).into();
        fs::write(&p, f.to_string()).unwrap();
        assert!(Vault::open(&p, "master-password-kuat").is_err());
    }

    #[test]
    fn header_tampering_detected() {
        let p = tmp("hdr");
        Vault::create(&p, "master-password-kuat", FAST).unwrap();
        let mut f: serde_json::Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        f["kdf"]["t"] = 2.into();
        fs::write(&p, f.to_string()).unwrap();
        assert!(Vault::open(&p, "master-password-kuat").is_err());
    }

    #[test]
    fn short_master_rejected() {
        assert!(Vault::create(&tmp("short"), "pendek", FAST).is_err());
    }

    #[test]
    fn change_master_works() {
        let p = tmp("chg");
        let mut v = Vault::create(&p, "master-password-lama", FAST).unwrap();
        v.upsert(sample()).unwrap();
        v.change_master("master-password-baru", FAST).unwrap();
        assert!(Vault::open(&p, "master-password-lama").is_err());
        assert_eq!(Vault::open(&p, "master-password-baru").unwrap().data.entries.len(), 1);
    }

    #[test]
    fn backup_file_written() {
        let p = tmp("bak");
        let mut v = Vault::create(&p, "master-password-kuat", FAST).unwrap();
        v.upsert(sample()).unwrap();
        assert!(p.with_extension("vault.bak").exists());
    }

    #[test]
    fn import_chrome_csv() {
        let csv = "name,url,username,password,note\nbank.example,https://bank.example/login,budi,p4ss,\nkosong,https://x.example,a,,\n";
        let (items, skipped, src) = parse_csv(csv).unwrap();
        assert_eq!((items.len(), skipped, src.as_str()), (1, 1, "Chrome/Edge"));
        assert_eq!(items[0].password, "p4ss");
    }

    #[test]
    fn import_firefox_csv() {
        let csv = "\"url\",\"username\",\"password\",\"httpRealm\",\"formActionOrigin\",\"guid\",\"timeCreated\",\"timeLastUsed\",\"timePasswordChanged\"\n\"https://www.mail.example\",\"ani\",\"p,a\"\"ss\",,\"https://www.mail.example\",\"{1}\",\"1\",\"1\",\"1\"\n";
        let (items, _, src) = parse_csv(csv).unwrap();
        assert_eq!(src, "Firefox");
        assert_eq!(items[0].password, "p,a\"ss");
        assert_eq!(items[0].title, "mail.example");
    }

    #[test]
    fn import_safari_csv_with_bom() {
        let csv = "\u{feff}Title,URL,Username,Password,Notes,OTPAuth\nToko (tok@x.id),https://toko.example/,tok@x.id,abc123,catatan,\n";
        let (items, _, src) = parse_csv(csv).unwrap();
        assert_eq!(src, "Safari");
        assert_eq!(items[0].notes, "catatan");
    }

    #[test]
    fn import_skips_duplicates() {
        let p = tmp("dup");
        let mut v = Vault::create(&p, "master-password-kuat", FAST).unwrap();
        let csv = "name,url,username,password,note\na,https://a.example,u,p,\n";
        let (items, _, _) = parse_csv(csv).unwrap();
        assert_eq!(v.import(items).unwrap(), (1, 0));
        let (items, _, _) = parse_csv(csv).unwrap();
        assert_eq!(v.import(items).unwrap(), (0, 1));
    }

    #[test]
    fn safari_csv_imports_totp() {
        let csv = "Title,URL,Username,Password,Notes,OTPAuth\nGitHub,https://github.com,budi,pw,,otpauth://totp/GitHub:budi?secret=JBSWY3DPEHPK3PXP&issuer=GitHub\n";
        let (items, _, _) = parse_csv(csv).unwrap();
        assert!(items[0].totp.starts_with("otpauth://"));
    }

    #[test]
    fn invalid_totp_rejected_on_save() {
        let p = tmp("badtotp");
        let mut v = Vault::create(&p, "master-password-kuat", FAST).unwrap();
        let mut e = sample(); e.totp = "bukan-kunci!".into();
        assert!(v.upsert(e).is_err());
    }

    #[test]
    fn secure_note_roundtrip_and_cleanup() {
        let p = tmp("note");
        let mut v = Vault::create(&p, "master-password-kuat", FAST).unwrap();
        let mut n = Entry::new("Kode backup Google".into(), "x".into(), "u".into(), "p".into(), "1111 2222\n3333 4444".into());
        n.kind = "note".into();
        let id = v.upsert(n).unwrap();
        let v2 = Vault::open(&p, "master-password-kuat").unwrap();
        let got = v2.get(&id).unwrap();
        assert_eq!((got.kind.as_str(), got.notes.as_str(), got.password.as_str()), ("note", "1111 2222\n3333 4444", ""));
        assert!(!fs::read_to_string(&p).unwrap().contains("3333"));
    }

    #[test]
    fn old_vault_without_new_fields_loads() {
        let d: VaultData = serde_json::from_str(r#"{"entries":[{"id":"a","title":"t","url":"","username":"","password":"p","notes":"","created":1,"updated":1}]}"#).unwrap();
        assert_eq!((d.entries[0].kind.as_str(), d.entries[0].totp.as_str()), ("login", ""));
    }

    #[test]
    fn csv_without_password_column_rejected() {
        assert!(parse_csv("a,b,c\n1,2,3\n").is_err());
    }

    #[test]
    fn generator_rules() {
        for _ in 0..200 {
            let p = generate_password(20, true);
            assert_eq!(p.len(), 20);
            assert!(p.chars().any(|c| c.is_ascii_lowercase()));
            assert!(p.chars().any(|c| c.is_ascii_uppercase()));
            assert!(p.chars().any(|c| c.is_ascii_digit()));
            assert!(p.chars().any(|c| "!@#$%^&*-_=+?".contains(c)));
        }
        assert!(generate_password(16, false).chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
