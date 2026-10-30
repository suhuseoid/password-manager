pub mod totp;
pub mod vault;

use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};
use vault::{Entry, KdfCost, Vault};
use zeroize::Zeroizing;

#[derive(Default)]
struct AppState { vault: Mutex<Option<Vault>>, path: Mutex<PathBuf> }

#[derive(Serialize)]
struct Status { exists: bool, unlocked: bool, path: String, count: usize }

#[derive(Serialize)]
struct Summary { id: String, kind: String, title: String, url: String, username: String, host: String, has_totp: bool, updated: i64 }

#[derive(Serialize)]
struct TotpCode { code: String, remaining: u64, period: u64, issuer: String, account: String }

#[derive(Serialize)]
struct ImportReport { source: String, imported: usize, duplicates: usize, skipped: usize }

type R<T> = Result<T, String>;

fn with_vault<T>(st: &State<AppState>, f: impl FnOnce(&mut Vault) -> R<T>) -> R<T> {
    let mut g = st.vault.lock().map_err(|_| "kunci internal rusak")?;
    match g.as_mut() { Some(v) => f(v), None => Err("Brankas terkunci.".into()) }
}

#[tauri::command]
fn status(st: State<AppState>) -> Status {
    let path = st.path.lock().unwrap().clone();
    let g = st.vault.lock().unwrap();
    Status {
        exists: path.exists(),
        unlocked: g.is_some(),
        path: path.display().to_string(),
        count: g.as_ref().map(|v| v.data.entries.len()).unwrap_or(0),
    }
}

#[tauri::command]
fn create_vault(st: State<AppState>, master: String) -> R<()> {
    let master = Zeroizing::new(master);
    let path = st.path.lock().unwrap().clone();
    let v = Vault::create(&path, &master, KdfCost::default()).map_err(|e| e.to_string())?;
    *st.vault.lock().unwrap() = Some(v);
    Ok(())
}

#[tauri::command]
fn unlock(st: State<AppState>, master: String) -> R<()> {
    let master = Zeroizing::new(master);
    let path = st.path.lock().unwrap().clone();
    let v = Vault::open(&path, &master).map_err(|e| e.to_string())?;
    *st.vault.lock().unwrap() = Some(v);
    Ok(())
}

#[tauri::command]
fn lock(st: State<AppState>) { *st.vault.lock().unwrap() = None; }

#[tauri::command]
fn list_entries(st: State<AppState>, query: String, filter: String) -> R<Vec<Summary>> {
    let q = query.to_lowercase();
    with_vault(&st, |v| {
        let mut out: Vec<Summary> = v.data.entries.iter()
            .filter(|e| match filter.as_str() {
                "login" => e.kind == "login", "note" => e.kind == "note", "totp" => !e.totp.is_empty(), _ => true })
            .filter(|e| q.is_empty() || e.title.to_lowercase().contains(&q) || e.url.to_lowercase().contains(&q)
                || e.username.to_lowercase().contains(&q) || e.notes.to_lowercase().contains(&q))
            .map(|e| Summary { id: e.id.clone(), kind: e.kind.clone(), title: e.title.clone(), url: e.url.clone(),
                               username: e.username.clone(), host: vault::host_of(&e.url), has_totp: !e.totp.is_empty(), updated: e.updated })
            .collect();
        out.sort_by_key(|s| s.title.to_lowercase());
        Ok(out)
    })
}

#[tauri::command]
fn get_entry(st: State<AppState>, id: String) -> R<Entry> {
    with_vault(&st, |v| v.get(&id).cloned().ok_or_else(|| "Data tidak ditemukan.".into()))
}

#[tauri::command]
fn save_entry(st: State<AppState>, entry: Entry) -> R<String> {
    with_vault(&st, |v| v.upsert(entry).map_err(|e| e.to_string()))
}

#[tauri::command]
fn delete_entry(st: State<AppState>, id: String) -> R<()> {
    with_vault(&st, |v| v.delete(&id).map_err(|e| e.to_string()))
}

#[tauri::command]
fn import_csv(st: State<AppState>, text: String) -> R<ImportReport> {
    let text = Zeroizing::new(text);
    let (items, skipped, source) = vault::parse_csv(&text).map_err(|e| e.to_string())?;
    with_vault(&st, |v| {
        let (imported, duplicates) = v.import(items).map_err(|e| e.to_string())?;
        Ok(ImportReport { source, imported, duplicates, skipped })
    })
}

/// Salin username/password ke clipboard, lalu hapus otomatis setelah 20 detik.
#[tauri::command]
fn copy_field(st: State<AppState>, id: String, field: String) -> R<()> {
    let value = with_vault(&st, |v| {
        let e = v.get(&id).ok_or("Data tidak ditemukan.")?;
        Ok(Zeroizing::new(match field.as_str() {
            "password" => e.password.clone(),
            "username" => e.username.clone(),
            "url" => e.url.clone(),
            "notes" => e.notes.clone(),
            "totp" => {
                let t = totp::parse(&e.totp).map_err(|_| "Data ini tidak punya 2FA.")?;
                t.code_at(totp::unix_now())
            }
            _ => return Err("Kolom tidak dikenal.".into()),
        }))
    })?;
    copy_and_clear(value)
}

#[tauri::command]
fn copy_text(text: String) -> R<()> { copy_and_clear(Zeroizing::new(text)) }

fn copy_and_clear(value: Zeroizing<String>) -> R<()> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(value.as_str()).map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(20));
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if cb.get_text().map(|t| t == *value).unwrap_or(false) { let _ = cb.clear(); }
        }
    });
    Ok(())
}

/// Kode 2FA saat ini untuk sebuah data (kuncinya tidak pernah dikirim ke tampilan).
#[tauri::command]
fn totp_now(st: State<AppState>, id: String) -> R<TotpCode> {
    with_vault(&st, |v| {
        let e = v.get(&id).ok_or("Data tidak ditemukan.")?;
        let t = totp::parse(&e.totp)?;
        let now = totp::unix_now();
        Ok(TotpCode { code: t.code_at(now), remaining: t.remaining(now), period: t.period, issuer: t.issuer, account: t.account })
    })
}

/// Pratinjau saat mengisi kunci 2FA di formulir.
#[tauri::command]
fn totp_preview(secret: String) -> R<TotpCode> {
    let secret = Zeroizing::new(secret);
    let t = totp::parse(&secret)?;
    let now = totp::unix_now();
    Ok(TotpCode { code: t.code_at(now), remaining: t.remaining(now), period: t.period, issuer: t.issuer, account: t.account })
}

#[tauri::command]
fn qr_decode(bytes: Vec<u8>) -> R<String> {
    let text = totp::decode_qr(&bytes)?;
    if !text.to_lowercase().starts_with("otpauth://") {
        return Err("QR terbaca, tapi bukan QR untuk 2FA.".into());
    }
    totp::parse(&text)?;
    Ok(text)
}

#[tauri::command]
fn generate(len: usize, symbols: bool) -> String { vault::generate_password(len, symbols) }

#[tauri::command]
fn change_master(st: State<AppState>, old: String, new: String) -> R<()> {
    let (old, new) = (Zeroizing::new(old), Zeroizing::new(new));
    let path = st.path.lock().unwrap().clone();
    Vault::open(&path, &old).map_err(|_| "Master password lama salah.".to_string())?;
    with_vault(&st, |v| v.change_master(&new, KdfCost::default()).map_err(|e| e.to_string()))
}

/// Salin file brankas (tetap terenkripsi) ke folder backups dengan tanggal.
#[tauri::command]
fn backup_now(st: State<AppState>) -> R<String> {
    let path = st.path.lock().unwrap().clone();
    if !path.exists() { return Err("Belum ada brankas.".into()); }
    let dir = path.parent().unwrap().join("backups");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(format!("brankas-{}.vault", vault::now()));
    std::fs::copy(&path, &dest).map_err(|e| e.to_string())?;
    Ok(dest.display().to_string())
}

#[tauri::command]
fn reveal_folder(st: State<AppState>) -> R<()> {
    let dir = st.path.lock().unwrap().parent().unwrap().to_path_buf();
    #[cfg(target_os = "macos")] let cmd = "open";
    #[cfg(target_os = "windows")] let cmd = "explorer";
    #[cfg(all(unix, not(target_os = "macos")))] let cmd = "xdg-open";
    std::process::Command::new(cmd).arg(dir).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .setup(|app| {
            let dir = app.path().app_data_dir().expect("folder data aplikasi");
            std::fs::create_dir_all(&dir).ok();
            *app.state::<AppState>().path.lock().unwrap() = dir.join("brankas.vault");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            status, create_vault, unlock, lock, list_entries, get_entry, save_entry, delete_entry,
            import_csv, copy_field, copy_text, generate, change_master, backup_now, reveal_folder,
            totp_now, totp_preview, qr_decode
        ])
        .run(tauri::generate_context!())
        .expect("gagal menjalankan Brankas");
}
