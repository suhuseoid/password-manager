// Brankas - antarmuka. Semua kriptografi ada di Rust; JS hanya tampilan.
const { invoke } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);
const AUTO_LOCK_MS = 5 * 60 * 1000;

let selectedId = null;
let editingId = null;
let idleTimer = null;
let totpTimer = null;
let editingNoteId = null;
function stopTotp() { clearInterval(totpTimer); totpTimer = null; }

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
  stopTotp();
  await call("lock");
  selectedId = null; $("detail").replaceChildren(); $("list").replaceChildren();
  for (const m of ["modal-edit", "modal-note", "modal-gen", "modal-settings", "modal-confirm"]) modal(m, false);
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
  const items = await call("list_entries", { query: $("search").value.trim(), filter: $("filter").value });
  const s = await call("status");
  $("count").textContent = `${items.length} dari ${s.count} data`;
  const ul = $("list"); ul.replaceChildren();
  for (const it of items) {
    const letter = (it.title || it.host || "?").trim().charAt(0).toUpperCase();
    const isNote = it.kind === "note";
    const name = el("div", {}, it.title || it.host);
    if (it.has_totp) name.append(el("span", { class: "badge" }, "2FA"));
    const li = el("li", { class: it.id === selectedId ? "sel" : "", onclick: () => select(it.id) },
      el("div", { class: "avatar" + (isNote ? " note" : "") }, isNote ? "✎" : letter),
      el("div", { class: "li-text" }, name, el("div", { class: "muted small" }, isNote ? "Catatan aman" : (it.username || it.host))));
    ul.append(li);
  }
  if (!items.length) ul.append(el("li", { class: "muted" }, s.count ? "Tidak ada yang cocok." : "Belum ada data. Klik + Tambah atau Impor CSV."));
}
$("search").addEventListener("input", () => { clearTimeout(refreshList.h); refreshList.h = setTimeout(refreshList, 120); });
$("filter").addEventListener("change", refreshList);

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

function secretBlock(label, value, field, id, pre) {
  const mask = "••••••••••••";
  const span = pre ? el("pre", { class: "secret" }, "•••••• (tersembunyi)") : el("span", { class: "mono" }, mask);
  let shown = false;
  const toggle = el("button", { onclick: () => {
    shown = !shown; span.textContent = shown ? value : (pre ? "•••••• (tersembunyi)" : mask);
    toggle.textContent = shown ? "Sembunyikan" : "Lihat";
  } }, "Lihat");
  return el("div", { class: "field" }, el("div", { class: "k" }, label),
    el("div", { class: "v" }, span, el("div", { class: "row" }, toggle, copyBtn(field, id))));
}

function totpBlock(id) {
  const code = el("span", { class: "code" }, "------");
  const C = 2 * Math.PI * 14;
  const svgNS = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(svgNS, "svg"); svg.setAttribute("viewBox", "0 0 34 34"); svg.classList.add("ring");
  const mk = (cls) => { const c = document.createElementNS(svgNS, "circle"); c.setAttribute("cx", 17); c.setAttribute("cy", 17); c.setAttribute("r", 14); c.setAttribute("class", cls); c.setAttribute("stroke-dasharray", C); return c; };
  const bg = mk("bg"), fg = mk("fg"); svg.append(bg, fg);
  const secs = el("span", { class: "muted small" }, "");
  const box = el("div", { class: "totp" }, svg, code, secs);
  const tick = async () => {
    try {
      const t = await call("totp_now", { id });
      code.textContent = t.code.length === 6 ? t.code.slice(0, 3) + " " + t.code.slice(3) : t.code;
      fg.setAttribute("stroke-dashoffset", C * (1 - t.remaining / t.period));
      secs.textContent = `${t.remaining} dtk`;
      box.classList.toggle("low", t.remaining <= 5);
    } catch (e) { code.textContent = "Error"; secs.textContent = e.message; stopTotp(); }
  };
  stopTotp(); tick(); totpTimer = setInterval(tick, 1000);
  return el("div", { class: "field" }, el("div", { class: "k" }, "Kode 2FA"),
    el("div", { class: "v" }, box, copyBtn("totp", id)));
}

function renderDetail(e) {
  stopTotp();
  const d = $("detail"); d.replaceChildren();
  if (!e) { d.append(el("div", { class: "empty muted" }, "Pilih data di sebelah kiri, atau klik + Login / + Catatan.")); return; }
  d.append(el("h2", {}, e.title || "(tanpa judul)"));
  const dt = (t) => t ? new Date(t * 1000).toLocaleString("id-ID") : "-";
  if (e.kind === "note") {
    d.append(secretBlock("Isi catatan", e.notes, "notes", e.id, true));
    d.append(el("p", { class: "muted small" }, `Dibuat ${dt(e.created)} · Diubah ${dt(e.updated)}`));
    d.append(el("div", { class: "actions", style: "justify-content:flex-start" },
      el("button", { onclick: () => openNote(e) }, "Ubah"),
      el("button", { class: "danger", onclick: () => askDelete(e) }, "Hapus")));
    return;
  }
  if (e.url) d.append(el("div", { class: "field" }, el("div", { class: "k" }, "Alamat situs"),
    el("div", { class: "v" }, el("span", {}, e.url), copyBtn("url", e.id))));
  d.append(el("div", { class: "field" }, el("div", { class: "k" }, "Username / email"),
    el("div", { class: "v" }, el("span", {}, e.username || "-"), copyBtn("username", e.id))));
  const pwSpan = el("span", { class: "mono" }, "••••••••••••");
  let shown = false;
  const toggle = el("button", { onclick: () => { shown = !shown; pwSpan.textContent = shown ? e.password : "••••••••••••"; toggle.textContent = shown ? "Sembunyikan" : "Lihat"; } }, "Lihat");
  d.append(el("div", { class: "field" }, el("div", { class: "k" }, "Password"),
    el("div", { class: "v" }, pwSpan, el("div", { class: "row" }, toggle, copyBtn("password", e.id)))));
  if (e.totp) d.append(totpBlock(e.id));
  if (e.notes) d.append(el("div", { class: "field" }, el("div", { class: "k" }, "Catatan"), el("div", { class: "notes" }, e.notes)));
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
  $("f-totp").value = e?.totp || ""; $("f-totp").type = "password"; $("f-totp-show").textContent = "Lihat";
  $("f-pass").type = "password"; $("f-show").textContent = "Lihat"; $("edit-err").textContent = "";
  previewTotp();
  modal("modal-edit", true); $("f-title").focus();
}
$("btn-add").onclick = () => openEdit(null);
$("edit-cancel").onclick = () => modal("modal-edit", false);
$("f-show").onclick = () => { const p = $("f-pass"); p.type = p.type === "password" ? "text" : "password"; $("f-show").textContent = p.type === "password" ? "Lihat" : "Sembunyikan"; };
$("f-gen").onclick = async () => { $("f-pass").value = await call("generate", { len: 20, symbols: true }); $("f-pass").type = "text"; $("f-show").textContent = "Sembunyikan"; };
$("edit-save").onclick = async () => {
  const entry = { id: editingId || "", title: $("f-title").value.trim(), url: $("f-url").value.trim(),
    username: $("f-user").value.trim(), password: $("f-pass").value, notes: $("f-notes").value,
    totp: $("f-totp").value.trim(), kind: "login", created: 0, updated: 0 };
  try { const id = await call("save_entry", { entry }); modal("modal-edit", false); toast("Tersimpan"); await select(id); }
  catch (e) { $("edit-err").textContent = e.message; }
};

// ---------- 2FA di formulir ----------
async function previewTotp() {
  const v = $("f-totp").value.trim(), out = $("totp-preview");
  if (!v) { out.textContent = ""; return; }
  try { const t = await call("totp_preview", { secret: v });
        out.textContent = `Kunci valid${t.issuer ? " (" + t.issuer + ")" : ""}. Kode saat ini: ${t.code}`; out.style.color = ""; }
  catch (e) { out.textContent = e.message; out.style.color = "var(--danger)"; }
}
$("f-totp").addEventListener("input", () => { clearTimeout(previewTotp.h); previewTotp.h = setTimeout(previewTotp, 250); });
$("f-totp-show").onclick = () => { const p = $("f-totp"); p.type = p.type === "password" ? "text" : "password"; $("f-totp-show").textContent = p.type === "password" ? "Lihat" : "Sembunyikan"; };
$("f-qr").onclick = () => $("file-qr").click();
$("file-qr").addEventListener("change", async (ev) => {
  const f = ev.target.files[0]; ev.target.value = "";
  if (!f) return;
  try {
    const bytes = Array.from(new Uint8Array(await f.arrayBuffer()));
    $("f-totp").value = await call("qr_decode", { bytes });
    previewTotp(); toast("QR 2FA terbaca. Sebaiknya hapus gambar QR tersebut.");
  } catch (e) { $("totp-preview").textContent = e.message; $("totp-preview").style.color = "var(--danger)"; }
});

// ---------- catatan aman ----------
function openNote(e) {
  editingNoteId = e ? e.id : null;
  $("note-title-h").textContent = e ? "Ubah Catatan Aman" : "Catatan Aman Baru";
  $("n-title").value = e?.title || ""; $("n-body").value = e?.notes || ""; $("note-err").textContent = "";
  modal("modal-note", true); $("n-title").focus();
}
$("btn-add-note").onclick = () => openNote(null);
$("note-cancel").onclick = () => modal("modal-note", false);
$("n-load").onclick = () => $("file-txt").click();
$("file-txt").addEventListener("change", async (ev) => {
  const f = ev.target.files[0]; ev.target.value = "";
  if (!f) return;
  if (f.size > 1024 * 1024) return ($("note-err").textContent = "File terlalu besar (maks 1 MB).");
  const text = await f.text();
  $("n-body").value = $("n-body").value ? $("n-body").value.replace(/\s*$/, "\n") + text : text;
  if (!$("n-title").value) $("n-title").value = f.name.replace(/\.txt$/i, "");
  toast("File dimuat. Setelah disimpan, hapus file .txt aslinya.");
});
$("note-save").onclick = async () => {
  const entry = { id: editingNoteId || "", kind: "note", title: $("n-title").value.trim(), url: "", username: "", password: "",
    notes: $("n-body").value, totp: "", created: 0, updated: 0 };
  try { const id = await call("save_entry", { entry }); modal("modal-note", false); toast("Catatan tersimpan"); await select(id); }
  catch (e) { $("note-err").textContent = e.message; }
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
  if (e.key === "Escape") for (const m of ["modal-edit", "modal-note", "modal-gen", "modal-settings", "modal-confirm"]) modal(m, false);
});

boot();
