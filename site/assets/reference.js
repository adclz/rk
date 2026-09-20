(() => {
const search = document.getElementById('search');
const links = [...document.querySelectorAll('nav.side a[data-code]')];
const entries = [...document.querySelectorAll('section.entry')];
const groups = [...document.querySelectorAll('nav.side details')];
const headings = [...document.querySelectorAll('.category-heading')];

search.addEventListener('input', () => {
  const q = search.value.trim().toLowerCase();
  const shown = new Set();
  for (const e of entries) {
    const hit = !q || e.dataset.text.includes(q);
    e.classList.toggle('hidden', !hit);
    if (hit) shown.add(e.dataset.cat);
  }
  for (const a of links) a.classList.toggle('hidden', q && !document.getElementById(a.dataset.code)?.matches(':not(.hidden)'));
  for (const g of groups) { const on = !q || shown.has(g.dataset.cat); g.classList.toggle('hidden', !on); if (q && on) g.open = true; }
  for (const h of headings) h.classList.toggle('hidden', q && !shown.has(h.dataset.cat));
});
document.addEventListener('keydown', e => {
  if (e.key === '/' && document.activeElement !== search) { e.preventDefault(); search.focus(); }
});

// Collapsed until the reader is on a section; the section they are in opens.
for (const g of groups) g.open = false;
let active = null;
const byCode = Object.fromEntries(links.map(a => [a.dataset.code, a]));
const seen = new IntersectionObserver(items => {
  for (const it of items) {
    if (!it.isIntersecting) continue;
    const a = byCode[it.target.id];
    if (!a) continue;
    active?.classList.remove('active');
    a.classList.add('active');
    active = a;
    if (!search.value) { for (const g of groups) g.open = g.contains(a); }
    // Only when the sidebar is on screen and scrolls itself; where it is
    // dropped (a phone) this would drag the page back up to the index.
    if (a.offsetParent) a.scrollIntoView({ block: 'nearest' });
  }
}, { rootMargin: '-10% 0px -70% 0px' });
entries.forEach(e => seen.observe(e));

for (const m of document.querySelectorAll('mark.diag')) {
  const pop = m.querySelector('.diag-popup');
  if (!pop) continue;
  m.removeAttribute('title');
  m.addEventListener('mouseenter', () => {
    const r = m.getBoundingClientRect();
    pop.classList.add('open');
    const w = pop.offsetWidth, h = pop.offsetHeight;
    pop.style.left = Math.max(8, Math.min(r.left, innerWidth - w - 8)) + 'px';
    // Below the mark when it fits, above it otherwise, and never off a short
    // screen: a tall popup on a phone used to land past the top edge.
    const top = r.bottom + h + 8 <= innerHeight ? r.bottom + 4 : r.top - h - 4;
    pop.style.top = Math.max(8, Math.min(top, innerHeight - h - 8)) + 'px';
  });
  m.addEventListener('mouseleave', () => pop.classList.remove('open'));
}
addEventListener('scroll', () => { for (const p of document.querySelectorAll('.diag-popup.open')) p.classList.remove('open'); }, { passive: true });

})();
