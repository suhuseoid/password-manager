# Panduan Instalasi Brankas

Brankas adalah penyimpan password **offline**: semua data tersimpan terenkripsi di komputer Anda sendiri,
tanpa server, tanpa akun, dan tanpa langganan.

Daftar isi
1. [Mengunduh](#1-mengunduh)
2. [macOS](#2-macos)
3. [Windows](#3-windows)
4. [Kali Linux, Debian, Ubuntu](#4-kali-linux-debian-ubuntu)
5. [Pemakaian pertama](#5-pemakaian-pertama)
6. [Impor password dari browser](#6-impor-password-dari-browser)
7. [Cadangan dan pindah komputer](#7-cadangan-dan-pindah-komputer)
8. [Memperbarui Brankas](#8-memperbarui-brankas)
9. [Menghapus Brankas](#9-menghapus-brankas)
10. [Membangun dari kode sumber](#10-membangun-dari-kode-sumber)
11. [Mengatasi masalah](#11-mengatasi-masalah)

---

## 1. Mengunduh

Buka halaman **[Releases](../../releases)** dan unduh file sesuai sistem Anda:

| Sistem | File | Keterangan |
|---|---|---|
| macOS (Apple Silicon & Intel) | `Brankas_x.y.z_universal.dmg` | Satu file untuk semua Mac |
| Windows 10/11 | `Brankas_x.y.z_x64-setup.exe` | Disarankan |
| Windows 10/11 | `Brankas_x.y.z_x64_en-US.msi` | Alternatif, untuk instalasi terkelola |
| Kali / Debian / Ubuntu (64-bit Intel/AMD) | `Brankas_x.y.z_amd64.deb` | Disarankan |
| Linux lain (64-bit Intel/AMD) | `Brankas_x.y.z_amd64.AppImage` | Tanpa instalasi |

> **Belum ada di Releases?** Buka tab **Actions** > **Build Brankas** > klik proses terakhir yang berhasil (centang hijau)
> > unduh dari bagian **Artifacts** di bawah halaman. File yang diunduh berupa `.zip`, ekstrak dulu.

> **Linux ARM64** (Kali di Raspberry Pi, atau mesin virtual di Mac Apple Silicon) belum ada paket siap pakai.
> Lihat [bagian 10](#10-membangun-dari-kode-sumber) untuk membangun sendiri.

---

## 2. macOS

1. Klik dua kali file `.dmg`.
2. Seret **Brankas** ke folder **Applications**.
3. Buka **Applications**, lalu **klik kanan Brankas > Open > Open**.

Langkah 3 hanya perlu sekali. Brankas belum ditandatangani sertifikat Apple berbayar, jadi macOS
akan memperingatkan saat pertama kali dibuka.

**Jika muncul pesan "Brankas is damaged and can't be opened"**, buka Terminal lalu jalankan:

```bash
xattr -dr com.apple.quarantine /Applications/Brankas.app
```

Lalu buka Brankas seperti biasa.

---

## 3. Windows

1. Klik dua kali `Brankas_x.y.z_x64-setup.exe`.
2. Jika muncul layar biru **"Windows protected your PC"**: klik **More info** > **Run anyway**.
   Ini muncul karena Brankas belum memiliki sertifikat penerbit berbayar.
3. Ikuti langkah pemasangan sampai selesai. Brankas muncul di menu Start.

Brankas memakai **Microsoft Edge WebView2**, yang sudah terpasang di Windows 10/11 modern.
Jika belum ada, pemasang akan mengunduhnya otomatis (butuh internet saat instalasi saja).

---

## 4. Kali Linux, Debian, Ubuntu

### Cara A: paket `.deb` (disarankan)

Buka terminal di folder tempat file diunduh:

```bash
cd ~/Downloads
sudo apt update
sudo apt install ./Brankas_*_amd64.deb
```

Gunakan `apt install ./nama-file.deb` (dengan `./`), **bukan** `dpkg -i`, agar pustaka pendukung
seperti WebKitGTK ikut terpasang otomatis.

Setelah selesai, Brankas ada di menu aplikasi (Kali: menu **Accessories** atau ketik "Brankas" di pencarian),
atau jalankan dari terminal:

```bash
brankas
```

### Cara B: AppImage (tanpa instalasi)

```bash
cd ~/Downloads
chmod +x Brankas_*_amd64.AppImage
./Brankas_*_amd64.AppImage
```

Jika muncul error tentang **FUSE**, pasang dulu:

```bash
sudo apt install libfuse2t64     # Kali/Debian terbaru
# atau, pada sistem yang lebih lama:
sudo apt install libfuse2
```

Atau jalankan tanpa FUSE:

```bash
./Brankas_*_amd64.AppImage --appimage-extract-and-run
```

---

## 5. Pemakaian pertama

1. Buka Brankas, buat **master password** minimal 10 karakter.
2. **Catat master password di kertas** dan simpan di tempat aman.
   Master password **tidak bisa dipulihkan**. Jika lupa, isi brankas tidak bisa dibuka oleh siapa pun,
   termasuk pembuat aplikasi.
3. Klik **+ Tambah** untuk menyimpan password pertama, atau **Impor CSV** untuk memindahkan dari browser.

### Menambahkan 2FA ke sebuah login
1. Di situs (mis. Google, GitHub, Instagram), aktifkan **2-Step Verification / Authenticator app**.
2. Situs menampilkan **QR code**. Ambil screenshot QR tersebut, atau klik "can't scan?" untuk melihat kunci teksnya.
3. Di Brankas, buka login terkait > **Ubah** > kolom **Kunci 2FA**:
   - klik **Pindai QR** lalu pilih screenshot tadi, **atau**
   - tempel kunci teks (mis. `JBSW Y3DP EHPK 3PXP`).
4. Simpan. Kode 6 digit akan tampil di halaman login tersebut dan berganti tiap 30 detik.
5. Masukkan kode itu di situs untuk menyelesaikan aktivasi. **Hapus screenshot QR** setelahnya.

> Kunci 2FA dan password yang disimpan di tempat yang sama berarti satu master password membuka keduanya.
> Untuk akun paling penting (mis. email utama), pertimbangkan menyimpan 2FA di aplikasi authenticator terpisah di HP.

### Menyimpan kode backup keamanan
Klik **+ Catatan**, beri judul (mis. "Kode backup Google"), lalu tempel kodenya
atau klik **Muat dari file .txt**. Isi catatan disembunyikan sampai Anda klik **Lihat**.
Hapus file `.txt` asli setelah disimpan.

Fitur keamanan yang aktif otomatis:
- Brankas terkunci sendiri setelah **5 menit** tidak ada aktivitas.
- Password yang disalin **dihapus dari clipboard setelah 20 detik**.

---

## 6. Impor password dari browser

Ekspor password dari browser menjadi file CSV, lalu di Brankas klik **Impor CSV** dan pilih file tersebut.
Data yang persis sama (situs, username, password) otomatis dilewati.

| Browser | Cara ekspor |
|---|---|
| Chrome / Edge / Brave | Settings > Passwords (Password Manager) > Settings > **Export passwords** |
| Firefox | Ketik `about:logins` di address bar > menu **•••** > **Export Logins** |
| Safari (macOS) | Menu **File > Export > Passwords** |
| Bitwarden | Tools > Export vault > format **.csv** |

> **Penting:** file CSV hasil ekspor **tidak terenkripsi**. Setelah impor berhasil,
> hapus file CSV tersebut dan kosongkan Trash/Recycle Bin.

Setelah yakin semua password sudah pindah, Anda bisa menghapus password tersimpan di browser
agar tidak ada salinan yang kurang aman.

---

## 7. Cadangan dan pindah komputer

Seluruh isi Brankas ada dalam **satu file**: `brankas.vault`. File ini sudah terenkripsi,
jadi aman disalin ke SSD, flashdisk, atau penyimpanan cloud.

| Sistem | Lokasi file |
|---|---|
| macOS | `~/Library/Application Support/id.suhu.brankas/brankas.vault` |
| Windows | `%APPDATA%\id.suhu.brankas\brankas.vault` |
| Kali / Linux | `~/.local/share/id.suhu.brankas/brankas.vault` |

Cara cepat membuka foldernya: **Pengaturan > Buka folder**.
Tombol **Buat cadangan sekarang** menyimpan salinan bertanggal di subfolder `backups`.

Di folder yang sama juga ada `brankas.vault.bak`, yaitu versi sebelum perubahan terakhir.

**Pindah ke komputer lain (termasuk beda sistem, mis. Mac ke Kali):**
1. Pasang Brankas di komputer baru, buka sekali lalu tutup (tidak perlu membuat brankas).
2. Salin `brankas.vault` dari komputer lama ke lokasi di tabel di atas pada komputer baru.
3. Buka Brankas dan masukkan master password yang sama.

**Saran:** simpan cadangan di minimal **dua tempat berbeda**, misalnya SSD dan flashdisk.

---

## 8. Memperbarui Brankas

Unduh versi baru dari Releases dan pasang seperti biasa, menimpa versi lama.
File `brankas.vault` tidak tersentuh oleh proses pembaruan. Sebelum memperbarui,
tetap disarankan membuat cadangan lewat **Pengaturan > Buat cadangan sekarang**.

---

## 9. Menghapus Brankas

> Menghapus aplikasi **tidak** menghapus file brankas. Hapus foldernya secara manual
> hanya jika Anda yakin tidak membutuhkan datanya lagi.

**macOS:** seret `Brankas.app` dari Applications ke Trash. Data: hapus folder
`~/Library/Application Support/id.suhu.brankas`.

**Windows:** Settings > Apps > Installed apps > **Brankas** > Uninstall. Data: hapus folder
`%APPDATA%\id.suhu.brankas`.

**Kali / Debian / Ubuntu:**
```bash
sudo apt remove brankas
rm -rf ~/.local/share/id.suhu.brankas    # hanya jika ingin menghapus data juga
```

---

## 10. Membangun dari kode sumber

Dibutuhkan **Rust** (stable) dan **Node.js 18+**.

### Kali Linux / Debian / Ubuntu (termasuk ARM64)

```bash
sudo apt update
sudo apt install -y build-essential curl git file pkg-config libssl-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev nodejs npm
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

git clone https://github.com/suhuseoid/password-manager.git
cd password-manager
npm install
npx tauri build --bundles deb
sudo apt install ./src-tauri/target/release/bundle/deb/*.deb
```

### macOS

```bash
xcode-select --install          # jika belum ada
brew install rust node
git clone https://github.com/suhuseoid/password-manager.git
cd password-manager
npm install
npx tauri build --bundles app dmg
```
Hasil: `src-tauri/target/release/bundle/macos/Brankas.app`

### Windows

1. Pasang [Rust](https://rustup.rs), [Node.js LTS](https://nodejs.org), dan
   **Visual Studio Build Tools** dengan workload "Desktop development with C++".
2. Di PowerShell:
   ```powershell
   git clone https://github.com/suhuseoid/password-manager.git
   cd password-manager
   npm install
   npx tauri build
   ```
Hasil: `src-tauri\target\release\bundle\nsis\*.exe`

### Menjalankan tes

```bash
cargo test --lib --manifest-path src-tauri/Cargo.toml
```

---

## 11. Mengatasi masalah

**Lupa master password**
Tidak ada cara memulihkannya. Ini sengaja, agar tidak ada "pintu belakang".
Jika Anda pernah membuat cadangan saat masih memakai password lama yang Anda ingat,
cadangan itu bisa dibuka dengan password lama tersebut.

**"Master password salah, atau file brankas telah diubah"**
- Periksa Caps Lock dan tata letak keyboard.
- Jika yakin password benar, file mungkin rusak. Coba pakai `brankas.vault.bak`
  atau cadangan di folder `backups`: tutup Brankas, ganti nama file rusak,
  lalu salin cadangan menjadi `brankas.vault`.

**Linux: jendela kosong/putih, atau aplikasi crash saat dibuka (sering di mesin virtual)**
```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 brankas
# jika masih bermasalah:
WEBKIT_DISABLE_COMPOSITING_MODE=1 brankas
```

**Linux: tombol Salin tidak berfungsi**
Pastikan sesi desktop berjalan normal (bukan lewat SSH). Pada Wayland, pasang `wl-clipboard`:
```bash
sudo apt install wl-clipboard
```

**macOS: "Brankas is damaged and can't be opened"**
Lihat solusi di [bagian 2](#2-macos).

**Windows: SmartScreen memblokir**
Klik **More info > Run anyway**. Lihat [bagian 3](#3-windows).
