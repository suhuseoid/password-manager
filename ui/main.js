// Brankas - antarmuka. Semua kriptografi ada di Rust; JS hanya tampilan.
const { invoke } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);
const AUTO_LOCK_MS = 5 * 60 * 1000;

let selectedId = null;
let editingId = null;
let idleTimer = null;

function show(view) {
  for (const v of ["view-setup", "view-unlock", "view-main"]) $(v).classList.toggle("hidden", v !== view);
}
function modal(id, on) { $(id).classList.toggle("hidden", !on); }
function toast(msg) {
  const t = $("toast"); t.textContent = msg; t.classList.remove("hidden");
  clearTimeout(toast.h); toast.h = setTimeout(() => t.classList.add("hidden"), 2600);
}
function el(tag, props = {}, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === "class") e.className = v; else if (k.startsWith("on")) e.addEventListener(k.slice(2), v); else e[k] = v;
  }
  for (const k of kids) e.append(k);   // append() memperlakukan string sebagai teks, aman dari HTML
  return e;
}
async function call(cmd, args) {
  try { return await invoke(cmd, args); }
  catch (e) { throw new Error(typeof e === "string" ? e : (e?.message || String(e))); }
}

// ---------- start ----------
async function boot() {
  const s = await call("status");
  if (s.unlocked) return enterMain();
  show(s.exists ? "view-unlock" : "view-setup");
  (s.exists ? $("unlock-pw") : $("setup-pw")).focus();
}

// ---------- buat & buka ----------
function strength(pw) {
  let score = Math.min(pw.length, 24) / 24 * 60;
  if (/[a-z]/.test(pw) && /[A-Z]/.test(pw)) score += 15;
  if (/\d/.test(pw)) score += 10;
  if (/[^A-Za-z0-9]/.test(pw)) score += 15;
  return Math.min(100, Math.round(score));
}
$("setup-pw").addEventListener("input", (e) => {
  const s = strength(e.target.value), bar = $("strength").firstElementChild;
  bar.style.width = s + "%";
  bar.style.background = s < 45 ? "#d33a3a" : s < 75 ? "#e0a000" : "#0f9d74";
});
$("btn-create").onclick = async () => {
  const a = $("setup-pw").value, b = $("setup-pw2").value;
  $("setup-err").textContent = "";
  if (a !== b) return ($("setup-err").textContent = "Kedua master password tidak sama.");
  $("btn-create").disabled = true; $("btn-create").textContent = "Membuat…";
  try { await call("create_vault", { master: a }); $("setup-pw").value = $("setup-pw2").value = ""; enterMain(); }
  catch (e) { $("setup-err").textContent = e.message; }
  finally { $("btn-create").disabled = false; $("btn-create").textContent = "Buat Brankas"; }
};
async function doUnlock() {
  $("unlock-err").textContent = "";
  $("btn-unlock").disabled = true; $("btn-unlock").textContent = "Membuka…";
  try { await call("unlock", { master: $("unlock-pw").value }); $("unlock-pw").value = ""; enterMain(); }
  catch (e) { $("unlock-err").textContent = e.message; $("unlock-pw").select(); }
  finally { $("btn-unlock").disabled = false; $("btn-unlock").textContent = "Buka"; }
}
$("btn-unlock").onclick = doUnlock;
$("unlock-pw").addEventListener("keydown", (e) => e.key === "Enter" && doUnlock());
$("setup-pw2").addEventListener("keydown", (e) => e.key === "Enter" && $("btn-create").click());

async function enterMain() { show("view-main"); selectedId = null; renderDetail(null); await refreshList(); resetIdle(); $("search").focus(); }

async function doLock() {
  await call("lock");
  selectedId = null; $("detail").replaceChildren(); $("list").replaceChildren();
  for (const m of ["modal-edit", "modal-gen", "modal-settings", "modal-confirm"]) modal(m, false);
  show("view-unlock"); $("unlock-pw").focus();
}
$("btn-lock").onclick = doLock;

// Kunci otomatis setelah 5 menit tidak ada aktivitas
function resetIdle() {
  clearTimeout(idleTimer);
  if (!$("view-main").classList.contains("hidden")) idleTimer = setTimeout(() => { doLock(); toast("Brankas dikunci otomatis"); }, AUTO_LOCK_MS);
}
for (const ev of ["mousemove", "keydown", "click", "scroll"]) document.addEventListener(ev, resetIdle, { passive: true });

// ---------- daftar ----------
async function refreshList() {
  const items = await call("list_entries", { query: $("search").value.trim() });
  const s = await call("status");
  $("count").textContent = `${items.length} dari ${s.count} data`;
  const ul = $("list"); ul.replaceChildren();
  for (const it of items) {
    const letter = (it.title || it.host || "?").trim().charAt(0).toUpperCase();
    const li = el("li", { class: it.id === selectedId ? "sel" : "", onclick: () => select(it.id) },
      el("div", { class: "avatar" }, letter),
      el("div", { class: "li-text" }, el("div", {}, it.title || it.host), el("div", { class: "muted small" }, it.username || it.host)));
    ul.append(li);
  }
  if (!items.length) ul.append(el("li", { class: "muted" }, s.count ? "Tidak ada yang cocok." : "Belum ada data. Klik + Tambah atau Impor CSV."));
}
$("search").addEventListener("input", () => { clearTimeout(refreshList.h); refreshList.h = setTimeout(refreshList, 120); });

async function select(id) {
  selectedId = id;
  try { renderDetail(await call("get_entry", { id })); } catch (e) { toast(e.message); }
  refreshList();
}

function copyBtn(field, id) {
  return el("button", { onclick: async () => {
    try { await call("copy_field", { id, field }); toast("Disalin. Clipboard dihapus otomatis dalam 20 detik."); }
    catch (e) { toast(e.message); }
  } }, "Salin");
}

function renderDetail(e) {
  const d = $("detail"); d.replaceChildren();
  if (!e) { d.append(el("div", { class: "empty muted" }, "Pilih data di sebelah kiri, atau klik + Tambah.")); return; }
  d.append(el("h2", {}, e.title || "(tanpa judul)"));
  if (e.url) d.append(el("div", { class: "field" }, el("div", { class: "k" }, "Alamat situs"),
    el("div", { class: "v" }, el("span", {}, e.url), copyBtn("url", e.id))));
  d.append(el("div", { class: "field" }, el("div", { class: "k" }, "Username / email"),
    el("div", { class: "v" }, el("span", {}, e.username || "-"), copyBtn("username", e.id))));
  const pwSpan = el("span", { class: "mono" }, "••••••••••••");
  let shown = false;
  const toggle = el("button", { onclick: () => { shown = !shown; pwSpan.textContent = shown ? e.password : "••••••••••••"; toggle.textContent = shown ? "Sembunyikan" : "Lihat"; } }, "Lihat");
  d.append(el("div", { class: "field" }, el("div", { class: "k" }, "Password"),
    el("div", { class: "v" }, pwSpan, el("div", { class: "row" }, toggle, copyBtn("password", e.id)))));
  if (e.notes) d.append(el("div", { class: "field" }, el("div", { class: "k" }, "Catatan"), el("div", { class: "notes" }, e.notes)));
  const dt = (t) => t ? new Date(t * 1000).toLocaleString("id-ID") : "-";
  d.append(el("p", { class: "muted small" }, `Dibuat ${dt(e.created)} · Diubah ${dt(e.updated)}`));
  d.append(el("div", { class: "actions", style: "justify-content:flex-start" },
    el("button", { onclick: () => openEdit(e) }, "Ubah"),
    el("button", { class: "danger", onclick: () => askDelete(e) }, "Hapus")));
}

// ---------- tambah / ubah ----------
function openEdit(e) {
  editingId = e ? e.id : null;
  $("edit-title").textContent = e ? "Ubah Data" : "Tambah Data";
  $("f-title").value = e?.title || ""; $("f-url").value = e?.url || "";
  $("f-user").value = e?.username || ""; $("f-pass").value = e?.password || ""; $("f-notes").value = e?.notes || "";
  $("f-pass").type = "password"; $("f-show").textContent = "Lihat"; $("edit-err").textContent = "";
  modal("modal-edit", true); $("f-title").focus();
}
$("btn-add").onclick = () => openEdit(null);
$("edit-cancel").onclick = () => modal("modal-edit", false);
$("f-show").onclick = () => { const p = $("f-pass"); p.type = p.type === "password" ? "text" : "password"; $("f-show").textContent = p.type === "password" ? "Lihat" : "Sembunyikan"; };
$("f-gen").onclick = async () => { $("f-pass").value = await call("generate", { len: 20, symbols: true }); $("f-pass").type = "text"; $("f-show").textContent = "Sembunyikan"; };
$("edit-save").onclick = async () => {
  const entry = { id: editingId || "", title: $("f-title").value.trim(), url: $("f-url").value.trim(),
    username: $("f-user").value.trim(), password: $("f-pass").value, notes: $("f-notes").value, created: 0, updated: 0 };
  try { const id = await call("save_entry", { entry }); modal("modal-edit", false); toast("Tersimpan"); await select(id); }
  catch (e) { $("edit-err").textContent = e.message; }
};

// ---------- hapus ----------
function askDelete(e) {
  $("confirm-title").textContent = "Hapus data ini?";
  $("confirm-text").textContent = `"${e.title || e.url}" akan dihapus permanen dari brankas.`;
  $("confirm-yes").onclick = async () => {
    modal("modal-confirm", false);
    try { await call("delete_entry", { id: e.id }); selectedId = null; renderDetail(null); refreshList(); toast("Dihapus"); }
    catch (err) { toast(err.message); }
  };
  modal("modal-confirm", true);
}
$("confirm-no").onclick = () => modal("modal-confirm", false);

// ---------- impor CSV ----------
$("btn-import").onclick = () => $("file-csv").click();
$("file-csv").addEventListener("change", async (ev) => {
  const f = ev.target.files[0]; ev.target.value = "";
  if (!f) return;
  try {
    const r = await call("import_csv", { text: await f.text() });
    await refreshList();
    toast(`Impor ${r.source}: ${r.imported} baru, ${r.duplicates} duplikat dilewati${r.skipped ? `, ${r.skipped} baris kosong` : ""}`);
    if (r.imported) setTimeout(() => toast("Selesai impor. Sebaiknya hapus file CSV aslinya, karena isinya tidak terenkripsi."), 3000);
  } catch (e) { toast(e.message); }
});

// ---------- pembuat password ----------
async function regen() { $("gen-out").textContent = await call("generate", { len: +$("gen-len").value, symbols: $("gen-sym").checked }); }
$("btn-gen").onclick = () => { modal("modal-gen", true); regen(); };
$("gen-len").addEventListener("input", () => { $("gen-len-val").textContent = $("gen-len").value; regen(); });
$("gen-sym").addEventListener("change", regen);
$("gen-again").onclick = regen;
$("gen-close").onclick = () => modal("modal-gen", false);
$("gen-copy").onclick = async () => { await call("copy_text", { text: $("gen-out").textContent }); toast("Disalin. Clipboard dihapus otomatis dalam 20 detik."); };

// ---------- pengaturan ----------
$("btn-settings").onclick = async () => {
  $("vault-path").textContent = (await call("status")).path;
  for (const i of ["cm-old", "cm-new", "cm-new2"]) $(i).value = "";
  $("settings-err").textContent = ""; modal("modal-settings", true);
};
$("settings-close").onclick = () => modal("modal-settings", false);
$("btn-reveal").onclick = () => call("reveal_folder").catch((e) => toast(e.message));
$("btn-backup").onclick = async () => {
  try { const p = await call("backup_now"); toast("Cadangan dibuat di folder backups"); $("settings-err").textContent = ""; }
  catch (e) { $("settings-err").textContent = e.message; }
};
$("btn-change").onclick = async () => {
  $("settings-err").textContent = "";
  if ($("cm-new").value !== $("cm-new2").value) return ($("settings-err").textContent = "Password baru tidak sama.");
  $("btn-change").disabled = true;
  try { await call("change_master", { old: $("cm-old").value, new: $("cm-new").value }); modal("modal-settings", false); toast("Master password diganti"); }
  catch (e) { $("settings-err").textContent = e.message; }
  finally { $("btn-change").disabled = false; }
};

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") for (const m of ["modal-edit", "modal-gen", "modal-settings", "modal-confirm"]) modal(m, false);
});

boot();
