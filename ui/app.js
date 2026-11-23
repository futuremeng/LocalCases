// Rendering rule: every value from the API goes through textContent. The corpus
// is the user's own data and can contain markup, so this file never builds HTML
// out of a field value.
const FACETS = [
  ["court", "法院"],
  ["year", "年份"],
  ["case_type", "案件类型"],
  ["procedure_group", "程序"],
];
const LABELS = {
  court: "法院",
  region: "地区",
  cause: "案由",
  case_type: "案件类型",
  procedure: "审理程序",
  procedure_group: "程序",
  procedure_kind: "程序类别",
  law: "法律",
  court_code: "法院代码",
  year_from: "年份起",
  year_to: "年份止",
  id_gt: "游标",
};

const q = { search: "", sort: "relevance", offset: 0, size: 8, filters: new Map() };
let api = null;
let lastId = null;
let total = 0;
let detailOpener = null;

// The traffic lights sit over the header only on macOS; the CSS leans on this
// class to leave them their corner.
if (/Macintosh|Mac OS X/.test(navigator.userAgent)) document.documentElement.classList.add("mac");

function params(extra) {
  const p = new URLSearchParams();
  if (q.search) p.set("search", q.search);
  p.set("sort", q.sort);
  p.set("offset", String(q.offset));
  p.set("size", String(q.size));
  for (const [k, v] of q.filters) p.set(k, v);
  for (const [k, v] of Object.entries(extra || {})) p.set(k, v);
  return p;
}

async function call(path, p) {
  const r = await fetch(api.addr + path + "?" + p, {
    headers: { "x-localcases-token": api.token },
  });
  if (!r.ok) throw new Error(path + " -> HTTP " + r.status);
  return r.json();
}

function el(tag, cls, text) {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text !== undefined && text !== null) n.textContent = String(text);
  return n;
}

// The corpus stores markup verbatim (SPEC 2); textContent keeps it inert, and
// this strips it for display so previews never show raw tags.
function plain(s) {
  return String(s || "").replace(/<[^>]*>/g, " ").replace(/\s+/g, " ").trim();
}

function setStatus(text, isError) {
  const status = document.getElementById("status");
  status.textContent = text;
  status.classList.toggle("error", !!isError);
}

function notice(text, actions) {
  const box = el("div", "notice");
  box.append(el("p", null, text));
  for (const [label, fn] of actions || []) {
    const b = el("button", null, label);
    b.type = "button";
    b.onclick = fn;
    box.append(b);
  }
  return box;
}

function renderChips() {
  const box = document.getElementById("chips");
  box.replaceChildren();
  for (const [k, v] of q.filters) {
    const chip = el("button", "chip", (LABELS[k] || k) + " " + v + " ×");
    chip.type = "button";
    chip.setAttribute("aria-label", "移除筛选：" + (LABELS[k] || k) + " " + v);
    chip.onclick = () => {
      q.filters.delete(k);
      q.offset = 0;
      run();
    };
    box.append(chip);
  }
}

function renderFacets(facets) {
  const box = document.getElementById("facets");
  box.replaceChildren();
  for (const [group, title] of FACETS) {
    const data = facets && facets[group];
    if (!data) continue;
    box.append(el("h3", null, title));
    const active = q.filters.get(group);
    for (const b of data.buckets) {
      const row = el("button", "bucket" + (active === b.key ? " on" : ""));
      row.type = "button";
      row.setAttribute("aria-pressed", String(active === b.key));
      row.append(el("span", "key", b.key), el("span", "count", b.doc_count));
      row.onclick = () => {
        if (active === b.key) q.filters.delete(group);
        else q.filters.set(group, b.key);
        q.offset = 0;
        run();
      };
      box.append(row);
    }
    if (data.sum_other_doc_count) {
      box.append(el("div", "other", "其余 " + data.sum_other_doc_count.toLocaleString()));
    }
  }
}

function renderResults(cases, meta) {
  const box = document.getElementById("results");
  box.replaceChildren();
  total = meta.total;
  for (const c of cases) {
    lastId = c.id;
    const item = el("article", "hit");
    const h = el("h2");
    const open = el("button", "hit-title", c.case_name || c.case_id || "(无标题)");
    open.type = "button";
    open.onclick = () => showCase(c.id);
    h.append(open);
    const line = [c.case_id, c.court, c.judgment_date, c.case_type].filter(Boolean).join(" · ");
    item.append(h, el("div", "meta", line), el("p", "preview", plain(c.preview)));
    box.append(item);
  }
  if (!cases.length) {
    if (q.filters.size) {
      box.append(notice("这些筛选下没有文书。", [["清除全部筛选", () => {
        q.filters.clear();
        q.offset = 0;
        run();
      }]]));
    } else {
      box.append(notice("没有匹配的文书。减少关键词，或换用案号试试。"));
    }
  }
  setStatus(
    meta.total.toLocaleString() + " 篇" +
    (q.search ? " 匹配「" + q.search + "」" : "") +
    " · 本页 " + cases.length + " 篇" +
    (q.filters.has("id_gt") ? " · 正在按导入顺序浏览" : ""),
    false
  );
  document.getElementById("page").textContent =
    "第 " + (Math.floor(meta.offset / meta.limit) + 1) + " 页";
}

function renderError(e) {
  const box = document.getElementById("results");
  box.replaceChildren();
  box.append(notice("请求失败：" + e.message + "。本地索引可能未运行。", [["重试", () => run()]]));
  setStatus("检索请求失败", true);
}

async function showCase(id) {
  const d = await call("/api/case/" + id, new URLSearchParams());
  const box = document.getElementById("detailbody");
  box.replaceChildren();
  const row = d.record;
  const title = el("h2", "detail-title", row["案件名称"] || row["案号"] || "");
  title.id = "detail-title";
  box.append(title);
  const head = el("dl", "fields");
  const keys = ["案号", "法院", "所属地区", "案件类型", "审理程序", "裁判日期", "公开日期",
    "当事人", "案由", "法律依据"];
  for (const key of keys) {
    if (!row[key]) continue;
    head.append(el("dt", null, key), el("dd", null, row[key]));
  }
  if (d.derived) {
    const proc = (d.derived.procedure_group || "?") + " / " + (d.derived.procedure_kind || "?");
    head.append(el("dt", null, "程序分组"), el("dd", null, proc));
    head.append(el("dt", null, "正文长度"), el("dd", null, d.derived.text_len));
  }
  box.append(head);
  box.append(el("pre", "fulltext", row["全文"]));
  openDetail();
}

async function run() {
  renderChips();
  const p = params({ group_by: FACETS.map((f) => f[0]).join(",") });
  const box = document.getElementById("results");
  box.setAttribute("aria-busy", "true");
  try {
    const data = await call("/api/search", p);
    renderFacets(data.facets);
    renderResults(data.cases, data.search_meta);
  } catch (e) {
    renderError(e);
  } finally {
    box.removeAttribute("aria-busy");
    syncPager();
  }
}

function syncPager() {
  const cursorMode = q.filters.has("id_gt");
  document.getElementById("prev").disabled = cursorMode || q.offset === 0;
  document.getElementById("next").disabled = cursorMode || q.offset + q.size >= total;
}

// The detail panel is an inspector: it docks at the right edge, the result list
// stays live behind it, Esc or the close button dismisses it, and focus returns
// to whatever opened it.
function openDetail() {
  detailOpener = document.activeElement;
  const panel = document.getElementById("detail");
  panel.hidden = false;
  panel.scrollTop = 0;
  document.getElementById("close").focus();
}

function closeDetail() {
  document.getElementById("detail").hidden = true;
  if (detailOpener && document.contains(detailOpener)) detailOpener.focus();
  detailOpener = null;
}

// A small context menu over the parts where copying and opening are the actions
// a Mac user reaches for; elsewhere the WKWebView default menu (Copy, Look Up)
// is left alone so inputs keep their Paste.
const ctx = el("div", "ctx-menu");
ctx.hidden = true;
document.body.append(ctx);

function hideCtx() {
  ctx.hidden = true;
}

function showCtx(x, y, items) {
  ctx.replaceChildren();
  for (const [label, fn] of items) {
    const b = el("button", null, label);
    b.type = "button";
    b.onclick = () => {
      hideCtx();
      fn();
    };
    ctx.append(b);
  }
  ctx.hidden = false;
  ctx.style.left = Math.min(x, innerWidth - ctx.offsetWidth - 8) + "px";
  ctx.style.top = Math.min(y, innerHeight - ctx.offsetHeight - 8) + "px";
}

function copyText(text) {
  navigator.clipboard.writeText(text).catch((e) => setStatus("复制失败：" + e.message, true));
}

function selectionText() {
  return String(document.getSelection() || "").trim();
}

function wireContext(id) {
  document.getElementById(id).addEventListener("contextmenu", (e) => {
    const items = [];
    const sel = selectionText();
    if (sel) items.push(["复制选中内容", () => copyText(sel)]);
    const hit = e.target.closest(".hit");
    if (hit) items.push(["打开文书", () => hit.querySelector(".hit-title").click()]);
    const pre = e.target.closest(".fulltext");
    if (pre && !sel) items.push(["复制全文", () => copyText(pre.textContent)]);
    if (!items.length) return;
    e.preventDefault();
    showCtx(e.clientX, e.clientY, items);
  });
}

// Mac muscle memory, answered here too so the UI works in a plain browser;
// inside the shell the menu bar consumes these first, so nothing double-fires.
function focusSearch() {
  const box = document.getElementById("q");
  box.focus();
  box.select();
}

function page(delta) {
  document.getElementById(delta < 0 ? "prev" : "next").click();
}

function wireChrome() {
  wireContext("results");
  wireContext("detailbody");
  document.addEventListener("click", hideCtx);
  window.addEventListener("blur", hideCtx);
  document.addEventListener("keydown", (e) => {
    if (!ctx.hidden && e.key === "Escape") {
      hideCtx();
      return;
    }
    if (document.getElementById("detail") && !document.getElementById("detail").hidden && e.key === "Escape") {
      closeDetail();
      return;
    }
    if (!(e.metaKey || e.ctrlKey)) return;
    const k = e.key.toLowerCase();
    if (k === "f" || k === "l") {
      e.preventDefault();
      focusSearch();
    } else if (k === "e") {
      e.preventDefault();
      exportCsv();
    } else if (k === "[") {
      e.preventDefault();
      page(-1);
    } else if (k === "]") {
      e.preventDefault();
      page(1);
    }
  });
  // Menu bar commands from the shell land here as events.
  if (window.__TAURI__ && window.__TAURI__.event) {
    window.__TAURI__.event.listen("menu", (m) => {
      if (m.payload === "export") exportCsv();
      else if (m.payload === "focus-search") focusSearch();
      else if (m.payload === "toggle-facets") document.body.classList.toggle("no-facets");
      else if (m.payload === "help") showHelp();
    });
  }
}

// Help doubles as the settings screen a Mac app would have: the shell is
// configured through the environment, so say so where a user will look.
function showHelp() {
  const box = document.getElementById("detailbody");
  box.replaceChildren();
  const title = el("h2", "detail-title", "配置与使用说明");
  title.id = "detail-title";
  box.append(title);
  box.append(el("p", null, "本应用只检索本机上的文库。文库位置用环境变量指定：LOCALCASES_CONFIG 指向包含 db 与 index_path 的配置文件（参考仓库中的 config_sample.toml）；LOCALCASES_REMOTE_API 指向另一台机器上的 LocalCases 服务，此时本机不打开文库，只通过回环地址转发。"));
  box.append(el("h3", null, "快捷键"));
  const dl = el("dl", "fields");
  for (const [k, v] of [
    ["⌘F / ⌘L", "聚焦搜索框"],
    ["⌘E", "导出当前筛选结果为 CSV"],
    ["⌘[ / ⌘]", "上一页 / 下一页"],
    ["⌃⌘S", "显示/隐藏筛选侧栏"],
    ["Esc", "关闭本面板或文书详情"],
  ]) {
    dl.append(el("dt", null, k), el("dd", null, v));
  }
  box.append(dl);
  openDetail();
}

async function exportCsv() {
  try {
    const r = await fetch(api.addr + "/api/export.csv?" + params(), {
      headers: { "x-localcases-token": api.token },
    });
    if (!r.ok) throw new Error("导出 -> HTTP " + r.status);
    const buf = new Uint8Array(await r.arrayBuffer());
    if (window.__TAURI__ && window.__TAURI__.core) {
      // A WKWebView drops blob downloads; the shell writes through the native
      // save panel instead.
      const saved = await window.__TAURI__.core.invoke("save_csv", {
        name: "localcases.csv",
        data: Array.from(buf),
      });
      setStatus(saved ? "已导出：" + saved : "已取消导出", false);
    } else {
      const a = el("a");
      a.href = URL.createObjectURL(new Blob([buf], { type: "text/csv" }));
      a.download = "localcases.csv";
      a.click();
      URL.revokeObjectURL(a.href);
    }
  } catch (e) {
    setStatus("导出失败：" + e.message, true);
  }
}

function wire() {
  wireChrome();
  document.getElementById("close").onclick = closeDetail;
  document.getElementById("searchform").onsubmit = (e) => {
    e.preventDefault();
    q.search = document.getElementById("q").value.trim();
    q.sort = document.getElementById("sort").value;
    q.filters.delete("id_gt");
    q.offset = 0;
    run();
  };
  document.getElementById("sort").onchange = () => {
    q.sort = document.getElementById("sort").value;
    q.offset = 0;
    run();
  };
  document.getElementById("prev").onclick = () => {
    q.offset = Math.max(0, q.offset - q.size);
    run();
  };
  document.getElementById("next").onclick = () => {
    q.offset += q.size;
    run();
  };
  // id is a fast field, so walking with id_gt stays cheap however deep the page
  // is. The switch is visible: sort flips to id and the status line says so.
  document.getElementById("cursor").onclick = () => {
    if (lastId === null) return;
    q.filters.set("id_gt", String(lastId));
    q.offset = 0;
    q.sort = "id";
    document.getElementById("sort").value = "id";
    run();
  };
  document.getElementById("export").onclick = () => exportCsv();
}

async function boot() {
  api = await window.__TAURI__.core.invoke("api_info");
  wire();
  for (let attempt = 0; attempt < 20; attempt += 1) {
    try {
      await run();
      await showLibState();
      return;
    } catch (e) {
      setStatus("正在启动本地索引…", false);
      await new Promise((r) => setTimeout(r, 250));
    }
  }
  setStatus("本地索引打不开：检查配置文件里的 db 与 index_path，并先运行 convert / index。", true);
}

// Say what the window is actually looking at, so a fresh install reads as an
// empty library — true, and with the version on screen — rather than as a
// blank page. The digest is the corpus identity `/api/health` reports; a
// library predating it answers null, and null is shown as 未记录.
async function showLibState() {
  const line = document.getElementById("libstate");
  try {
    const h = await call("/api/health", new URLSearchParams());
    line.textContent =
      `程序版本 v${h.version} · 本机已收录 ${h.indexed_documents} 篇 · 语料摘要 ${h.corpus_digest || "未记录"}`;
  } catch (e) {
    line.textContent = "本机库状态未知";
  }
}

boot();
