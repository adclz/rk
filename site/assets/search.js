// The search field in the top bar. It looks through the documentation's
// sections (search.json, written by the generator) and the diagnostics
// (diagnostics.json), both fetched the first time the field is used. Every
// word typed must appear; a heading weighs more than a page, a page more than
// the text. `/` opens it, or Ctrl+K where the page has a search of its own.
(() => {
const input = document.getElementById('site-search');
const list = document.getElementById('site-search-results');
if (!input || !list) return;
let data = null;
let loading = null;
let items = [];
let active = -1;

const load = () => loading ??= Promise.all([
  fetch('/search.json').then(r => r.json()).catch(() => []),
  fetch('/diagnostics.json').then(r => r.json()).catch(() => []),
]).then(([docs, diags]) => {
  data = docs.map(d => ({
    title: d.h ? `${d.p} › ${d.h}` : d.p, url: d.u, text: d.x,
    head: d.h.toLowerCase(), page: d.p.toLowerCase(), body: d.x.toLowerCase(),
  })).concat(diags.map(d => {
    // A description is Markdown: its backticks are marks, not words.
    const text = d.description.replace(/`/g, '');
    return {
      title: `${d.code} · ${d.title}`, url: `/diagnostics/#${d.code}`, text,
      head: `${d.code} ${d.title}`.toLowerCase(), page: 'diagnostics', body: text.toLowerCase(),
    };
  }));
});

const esc = s => s.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
const words = () => input.value.trim().toLowerCase().split(/\s+/).filter(Boolean);

function search(ws) {
  const hits = [];
  for (const e of data) {
    let score = 0;
    let all = true;
    for (const w of ws) {
      const h = e.head.includes(w), p = e.page.includes(w), b = e.body.includes(w);
      if (!h && !p && !b) { all = false; break; }
      score += (h ? 6 : 0) + (e.head.startsWith(w) ? 4 : 0) + (p ? 3 : 0) + (b ? 1 : 0);
    }
    if (all) hits.push([score, e]);
  }
  return hits.sort((a, b) => b[0] - a[0]).slice(0, 8).map(h => h[1]);
}

// A line of the section's text around the first word found, the words marked.
function snippet(text, ws) {
  const low = text.toLowerCase();
  const found = ws.map(w => low.indexOf(w)).filter(i => i >= 0);
  const start = found.length ? Math.max(0, Math.min(...found) - 40) : 0;
  let s = esc(text.slice(start, start + 150));
  for (const w of ws) {
    const re = new RegExp(esc(w).replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi');
    s = s.replace(re, m => `<mark>${m}</mark>`);
  }
  return (start > 0 ? '…' : '') + s + (start + 150 < text.length ? '…' : '');
}

function close() {
  list.hidden = true;
  active = -1;
  input.setAttribute('aria-expanded', 'false');
  input.removeAttribute('aria-activedescendant');
}

function render() {
  const ws = words();
  if (!data || !ws.length) { close(); return; }
  items = search(ws);
  if (active >= items.length) active = items.length - 1;
  list.innerHTML = items.length
    ? items.map((e, i) => `<li role="option" id="ss-${i}"${i === active ? ' aria-selected="true"' : ''}><a href="${esc(e.url)}"><span class="ss-title">${esc(e.title)}</span><span class="ss-text">${snippet(e.text, ws)}</span></a></li>`).join('')
    : '<li class="ss-none">Nothing found</li>';
  list.hidden = false;
  input.setAttribute('aria-expanded', 'true');
  if (active >= 0) input.setAttribute('aria-activedescendant', `ss-${active}`);
}

input.addEventListener('focus', () => load().then(render));
input.addEventListener('input', () => load().then(() => { active = 0; render(); }));
input.addEventListener('keydown', e => {
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    if (!items.length) return;
    e.preventDefault();
    active = (active + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
    render();
    list.children[active]?.scrollIntoView({ block: 'nearest' });
  } else if (e.key === 'Enter') {
    const hit = items[active] ?? items[0];
    if (hit) { e.preventDefault(); close(); location.href = hit.url; }
  } else if (e.key === 'Escape') {
    close();
    input.blur();
  }
});
list.addEventListener('click', () => close());
document.addEventListener('click', e => { if (!e.target.closest('.site-search')) close(); });
document.addEventListener('keydown', e => {
  const el = document.activeElement;
  const typing = el && (/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) || el.isContentEditable);
  const own = document.getElementById('search');
  if ((e.key === 'k' && (e.metaKey || e.ctrlKey)) || (e.key === '/' && !typing && !own)) {
    e.preventDefault();
    input.focus();
    input.select();
  }
});
})();
