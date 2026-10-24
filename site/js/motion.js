// Mouvements de page : préchargement, découpage des titres, révélations, barre de
// progression, navigation, curseur, boutons magnétiques, inclinaison 3D, compteurs,
// manifeste mot à mot, pipeline 3D, feuille de route horizontale et terminal.

import { photoURL } from './screen.js';

const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
export const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const finePointer = matchMedia('(hover: hover) and (pointer: fine)').matches;

/** Avancement (0 → 1) d'une section collante pendant qu'elle traverse l'écran. */
function sticky(el) {
  const r = el.getBoundingClientRect();
  return clamp(-r.top / Math.max(1, r.height - window.innerHeight));
}

// Une seule boucle de défilement pour toutes les sections collantes.
const scrollers = [];
let ticking = false;
function onScroll() {
  if (ticking) return;
  ticking = true;
  requestAnimationFrame(() => { ticking = false; scrollers.forEach((f) => f()); });
}

export function loader(onDone) {
  const root = document.documentElement;
  const el = $('.loader');
  root.classList.add('loading');
  const g = $('.blades', el);
  const N = 6;
  for (let i = 0; i < N; i++) {
    const p = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    p.setAttribute('d', 'M0,-44 L38,-22 L12,-6 Z');
    p.setAttribute('transform', `rotate(${(360 / N) * i})`);
    p.style.transformOrigin = '0 0';
    g.appendChild(p);
  }
  const count = $('.loader-count span', el);
  const start = performance.now();
  const dur = reduced ? 200 : 1500;
  let ready = false, done = false;
  window.addEventListener('load', () => { ready = true; });
  if (document.readyState === 'complete') ready = true;
  (function tick(now) {
    const t = clamp((now - start) / dur);
    count.textContent = Math.round((ready ? t : Math.min(t, 0.86)) * 100);
    $$('path', g).forEach((p, i) => { p.style.transform = `rotate(${(360 / N) * i}deg) translate(${t * 9}px, ${-t * 16}px)`; });
    if ((t >= 1 && ready) || now - start > 4500) {
      if (done) return;
      done = true;
      root.classList.remove('loading');
      root.classList.add('loaded');
      onDone && onDone();
      return;
    }
    requestAnimationFrame(tick);
  })(start);
  el.addEventListener('click', () => { ready = true; });
}

export function splitTitles() {
  $$('[data-split]').forEach((el) => {
    const text = el.textContent;
    el.textContent = '';
    [...text].forEach((c, i) => {
      const outer = document.createElement('span');
      outer.className = 'ch';
      outer.setAttribute('aria-hidden', 'true');
      const inner = document.createElement('span');
      inner.textContent = c;
      inner.style.setProperty('--i', i);
      outer.appendChild(inner);
      el.appendChild(outer);
    });
  });
}

export function reveals() {
  // Décalage en cascade dans chaque groupe d'éléments voisins.
  $$('.reveal').forEach((el) => {
    const sibs = [...el.parentElement.children].filter((c) => c.classList.contains('reveal'));
    el.style.setProperty('--d', `${Math.min(sibs.indexOf(el), 6) * 0.08}s`);
  });
  const io = new IntersectionObserver((entries) => entries.forEach((e) => {
    if (e.isIntersecting) { e.target.classList.add('in'); io.unobserve(e.target); }
  }), { rootMargin: '0px 0px -8% 0px', threshold: 0.08 });
  $$('.reveal').forEach((el) => {
    const r = el.getBoundingClientRect();
    if (r.top < window.innerHeight && r.bottom > 0) el.classList.add('in');
    else io.observe(el);
  });
  document.documentElement.classList.add('hydrated');
}

export function chrome() {
  const bar = $('.progress');
  const nav = $('.nav');
  let lastY = window.scrollY;
  scrollers.push(() => {
    const y = window.scrollY;
    const max = document.documentElement.scrollHeight - window.innerHeight;
    bar.style.setProperty('--p', (y / Math.max(1, max)).toFixed(4));
    if (!document.documentElement.classList.contains('menu-open')) nav.classList.toggle('hide', y > lastY && y > 400);
    lastY = y;
  });
  // Lien actif selon la section affichée
  const links = $$('.links a');
  const io = new IntersectionObserver((entries) => entries.forEach((e) => {
    if (e.isIntersecting) links.forEach((a) => a.classList.toggle('active', a.getAttribute('href') === `#${e.target.id}`));
  }), { rootMargin: '-45% 0px -50% 0px' });
  links.forEach((a) => { const s = $(a.getAttribute('href')); s && io.observe(s); });

  const toggle = $('.menu-toggle');
  toggle.addEventListener('click', () => {
    const open = document.documentElement.classList.toggle('menu-open');
    toggle.setAttribute('aria-expanded', open);
  });
  links.forEach((a) => a.addEventListener('click', () => {
    document.documentElement.classList.remove('menu-open');
    toggle.setAttribute('aria-expanded', 'false');
  }));
}

export function cursor() {
  if (!finePointer || reduced) return;
  document.documentElement.classList.add('has-cursor');
  const ring = $('.cursor'), dot = $('.cursor-dot');
  let x = innerWidth / 2, y = innerHeight / 2, rx = x, ry = y;
  window.addEventListener('pointermove', (e) => {
    x = e.clientX; y = e.clientY;
    dot.style.transform = `translate3d(${x}px, ${y}px, 0)`;
  }, { passive: true });
  (function loop() {
    rx += (x - rx) * 0.18; ry += (y - ry) * 0.18;
    ring.style.transform = `translate3d(${rx}px, ${ry}px, 0)`;
    requestAnimationFrame(loop);
  })();
  document.addEventListener('pointerover', (e) => {
    ring.classList.toggle('hover', !!e.target.closest('a, button, input, .lab-split'));
  });
}

export function magnetic() {
  if (!finePointer || reduced) return;
  $$('.magnetic').forEach((el) => {
    el.addEventListener('pointermove', (e) => {
      const r = el.getBoundingClientRect();
      const dx = e.clientX - (r.left + r.width / 2), dy = e.clientY - (r.top + r.height / 2);
      el.style.transform = `translate(${dx * 0.25}px, ${dy * 0.35}px)`;
      el.firstElementChild.style.transform = `translate(${dx * 0.12}px, ${dy * 0.15}px)`;
    });
    el.addEventListener('pointerleave', () => {
      el.style.transform = '';
      el.firstElementChild.style.transform = '';
    });
  });
}

export function tilt(scope = document) {
  if (!finePointer || reduced) return;
  $$('.tilt', scope).forEach((el) => {
    el.addEventListener('pointermove', (e) => {
      const r = el.getBoundingClientRect();
      const px = (e.clientX - r.left) / r.width, py = (e.clientY - r.top) / r.height;
      el.style.setProperty('--mx', `${px * 100}%`);
      el.style.setProperty('--my', `${py * 100}%`);
      el.style.transform = `perspective(900px) rotateX(${(0.5 - py) * 10}deg) rotateY(${(px - 0.5) * 12}deg) translateZ(10px)`;
    });
    el.addEventListener('pointerleave', () => { el.style.transform = ''; });
  });
}

export function counters() {
  const io = new IntersectionObserver((entries) => entries.forEach((e) => {
    if (!e.isIntersecting) return;
    io.unobserve(e.target);
    const el = e.target, end = parseFloat(el.dataset.count), t0 = performance.now(), dur = reduced ? 1 : 1600;
    (function step(now) {
      const t = clamp((now - t0) / dur);
      el.textContent = Math.round(end * (1 - Math.pow(1 - t, 4))).toLocaleString('fr-FR');
      if (t < 1) requestAnimationFrame(step);
    })(t0);
  }), { threshold: 0.6 });
  $$('[data-count]').forEach((el) => io.observe(el));
}

export function manifesto() {
  const el = $('[data-words]');
  const highlight = new Set(['RAW,', 'couleur', 'vitesse', 'prouver.']);
  const words = el.textContent.trim().split(/\s+/);
  el.innerHTML = words.map((w) => `<span class="w${highlight.has(w) ? ' hl' : ''}">${w}</span>`).join(' ');
  const spans = $$('.w', el);
  const section = el.closest('.manifesto');
  scrollers.push(() => {
    const p = sticky(section);
    const n = Math.floor(clamp(p * 1.25) * spans.length);
    spans.forEach((s, i) => s.classList.toggle('on', i < n));
  });
}

export function pipeline() {
  const section = $('#pipeline');
  const layers = $$('.layer', section);
  const steps = $$('.pipeline-steps li', section);
  const stage = $('.pipeline-stage', section);
  // Fonds des couches : la même photo, à différents stades du développement.
  const photo = photoURL(640, 480, { seed: 3 }).toDataURL('image/jpeg', 0.85);
  const gray = photoURL(640, 480, { seed: 3, mono: true }).toDataURL('image/jpeg', 0.85);
  layers[1].style.backgroundImage = `url(${gray})`;
  layers[1].style.filter = 'contrast(.75) brightness(.8)';
  layers[2].style.backgroundImage = `url(${photo})`;
  layers[3].style.backgroundImage = `radial-gradient(ellipse at 62% 30%, rgba(139,123,255,.75), rgba(139,123,255,.15) 45%, transparent 60%), url(${photo})`;
  layers[4].style.backgroundImage = `linear-gradient(120deg, rgba(255,255,255,.18), transparent 40%), url(${photo})`;
  layers[4].style.filter = 'saturate(1.25) contrast(1.08)';

  scrollers.push(() => {
    const p = sticky(section);
    const spread = clamp(p * 1.6);
    const mobile = innerWidth < 1000;
    const gap = (mobile ? 46 : 92) * spread;
    layers.forEach((l, i) => l.style.setProperty('--z', ((i - 2) * gap).toFixed(1)));
    stage.style.setProperty('--rx', `${58 - p * 10}deg`);
    stage.style.setProperty('--rz', `${-36 + p * 22}deg`);
    const active = Math.min(4, Math.floor(clamp((p - 0.12) / 0.8) * 5));
    layers.forEach((l, i) => {
      l.classList.toggle('active', i === active && p > 0.1);
      l.style.setProperty('--o', p > 0.1 && i > active ? 0.35 : 1);
    });
    steps.forEach((s, i) => s.classList.toggle('on', i === active && p > 0.1));
  });
}

export function roadmap() {
  const section = $('#roadmap');
  const track = $('.roadmap-track', section);
  const line = $('.roadmap-line', section);
  const phases = $$('.phase', section);
  scrollers.push(() => {
    const p = sticky(section);
    const max = Math.max(0, track.scrollWidth - innerWidth);
    track.style.setProperty('--tx', `${(-p * max).toFixed(1)}px`);
    line.style.setProperty('--p', p.toFixed(4));
    const lit = Math.floor(p * phases.length + 0.6);
    phases.forEach((ph, i) => ph.classList.toggle('lit', i < lit));
  });
}

export function frames() {
  const el = $('.frames');
  for (let k = 0; k < 40; k++) {
    const i = document.createElement('i');
    i.style.setProperty('--k', k);
    el.appendChild(i);
  }
}

const DENY = [
  ['c', '# deny.toml — vérifié à chaque build'],
  ['k', '[bans]'],
  ['', 'deny = ['],
  ['', '  { name = <s>"openssl-sys"</s> },'],
  ['', '  { name = <s>"libraw-sys"</s> },'],
  ['', '  { name = <s>"ffmpeg-sys-next"</s> },'],
  ['', ']'],
  ['', ''],
  ['k', '[sources]'],
  ['', 'unknown-registry = <s>"deny"</s>'],
  ['', 'unknown-git      = <s>"deny"</s>'],
  ['', ''],
  ['k', '[licenses]'],
  ['', 'allow = [<s>"MIT"</s>, <s>"Apache-2.0"</s>, <s>"BSD-3-Clause"</s>, <s>"ISC"</s>, <s>"Zlib"</s>]'],
  ['', ''],
  ['c', '# cibles prioritaires'],
  ['', '<n>$</n> cargo deny --target aarch64-apple-darwin check'],
  ['', '<n>$</n> cargo deny --target aarch64-apple-ios check'],
];

export function terminal() {
  const el = $('#term');
  const html = DENY.map(([cls, line]) => {
    const body = line.replace(/<s>(.*?)<\/s>/g, '<span class="s">$1</span>').replace(/<n>(.*?)<\/n>/g, '<span class="n">$1</span>');
    return cls ? `<span class="${cls}">${body}</span>` : body;
  });
  if (reduced) { el.innerHTML = html.join('\n'); return; }
  const io = new IntersectionObserver(([e]) => {
    if (!e.isIntersecting) return;
    io.disconnect();
    let i = 0;
    (function next() {
      el.innerHTML = html.slice(0, i).join('\n') + (i < html.length ? '\n' : ' ') + '<span class="caret"></span>';
      if (i++ < html.length) setTimeout(next, DENY[i - 1]?.[1] === '' ? 60 : 110);
    })();
  }, { threshold: 0.4 });
  io.observe(el);
}

export function startScroll() {
  window.addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('resize', onScroll);
  scrollers.forEach((f) => f());
}
