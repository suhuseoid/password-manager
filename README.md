# Brankas

Penyimpan password offline untuk macOS, Windows, dan Linux.
Tanpa server, tanpa langganan, tanpa integrasi browser.

## Keamanan
- Master password diolah dengan **Argon2id** (64 MiB, 3 iterasi) menjadi kunci 256-bit.
- Isi brankas dienkripsi **XChaCha20-Poly1305**. Header file ikut diautentikasi,
  jadi perubahan sekecil apa pun pada file akan ditolak.
- Setiap simpan memakai nonce acak baru, ditulis secara atomik, dan versi sebelumnya disimpan sebagai `.vault.bak`.
- Clipboard dihapus otomatis 20 detik setelah menyalin.
- Brankas terkunci otomatis setelah 5 menit tidak ada aktivitas.
- **Master password tidak bisa dipulihkan.** Lupa = data tidak bisa dibuka.

## Lokasi file brankas
- macOS: `~/Library/Application Support/id.suhu.brankas/brankas.vault`
- Windows: `%APPDATA%\id.suhu.brankas\brankas.vault`
- Linux: `~/.local/share/id.suhu.brankas/brankas.vault`

File ini aman disalin ke SSD/flashdisk sebagai cadangan karena sudah terenkripsi.
Untuk pindah komputer: salin file ini ke lokasi yang sama di komputer baru.

## Impor dari browser
Ekspor password ke CSV, lalu klik **Impor CSV**. Hapus file CSV setelahnya.
- Chrome/Edge/Brave: Settings > Passwords (Password Manager) > Settings > Export passwords
- Firefox: about:logins > menu ••• > Export Logins
- Safari: File > Export > Passwords
- Bitwarden CSV juga didukung.

## Pengembangan
    npm install
    npx tauri dev                      # jalankan mode pengembangan
    cargo test --lib --manifest-path src-tauri/Cargo.toml
    npx tauri build                    # bangun untuk OS yang sedang dipakai

Windows dan Linux: lihat `.github/workflows/build.yml` (dibangun otomatis lewat GitHub Actions),
atau jalankan `npx tauri build` langsung di komputer Windows/Linux.
