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

function renderChips() {
  const box = document.getElementById("chips");
  box.replaceChildren();
  for (const [k, v] of q.filters) {
    const chip = el("span", "chip", (LABELS[k] || k) + " " + v + " ×");
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
      const row = el("div", "bucket" + (active === b.key ? " on" : ""));
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
  for (const c of cases) {
    lastId = c.id;
    const item = el("article", "hit");
    item.append(el("h2", null, c.case_name || c.case_id || "(无标题)"));
    const line = [c.case_id, c.court, c.judgment_date, c.case_type].filter(Boolean).join(" · ");
    item.append(el("div", "meta", line));
    item.append(el("p", "preview", c.preview));
    item.onclick = () => showCase(c.id);
    box.append(item);
  }
  const status = document.getElementById("status");
  status.textContent =
    meta.total.toLocaleString() + " 篇" +
    (q.search ? " 匹配「" + q.search + "」" : "") +
    " · 本页 " + cases.length + " 篇";
  document.getElementById("page").textContent =
    "第 " + (Math.floor(meta.offset / meta.limit) + 1) + " 页";
}

async function showCase(id) {
  const d = await call("/api/case/" + id, new URLSearchParams());
  const box = document.getElementById("detailbody");
  box.replaceChildren();
  const row = d.record;
  box.append(el("h2", null, row["案件名称"] || row["案号"] || ""));
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
  document.getElementById("detail").hidden = false;
}

async function run() {
  renderChips();
  const p = params({ group_by: FACETS.map((f) => f[0]).join(",") });
  const data = await call("/api/search", p);
  renderFacets(data.facets);
  renderResults(data.cases, data.search_meta);
}

function wire() {
  document.getElementById("searchform").onsubmit = (e) => {
    e.preventDefault();
    q.search = document.getElementById("q").value.trim();
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
  // id is a fast field, so walking with id_gt stays cheap however deep the page is.
  document.getElementById("cursor").onclick = () => {
    if (lastId === null) return;
    q.filters.set("id_gt", String(lastId));
    q.offset = 0;
    q.sort = "id";
    document.getElementById("sort").value = "id";
    run();
  };
  document.getElementById("close").onclick = () => {
    document.getElementById("detail").hidden = true;
  };
  document.getElementById("export").onclick = async () => {
    const r = await fetch(api.addr + "/api/export.csv?" + params(), {
      headers: { "x-localcases-token": api.token },
    });
    const a = el("a");
    a.href = URL.createObjectURL(await r.blob());
    a.download = "localcases.csv";
    a.click();
    URL.revokeObjectURL(a.href);
  };
}

async function boot() {
  api = await window.__TAURI__.core.invoke("api_info");
  wire();
  for (let attempt = 0; attempt < 20; attempt += 1) {
    try {
      await run();
      return;
    } catch (e) {
      await new Promise((r) => setTimeout(r, 250));
    }
  }
  document.getElementById("status").textContent =
    "本地索引打不开：检查配置文件里的 db 与 index_path，并先运行 convert / index。";
}

boot();
