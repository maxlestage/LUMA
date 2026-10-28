// Les illustrations des nouveautés récentes et de la galerie avant / après : la photo de
// paysage de `screen.js` (`drawPhoto`), retouchée au canvas 2D. Aucune image à charger.
//
// - chaque nouveauté a sa petite scène qui tourne en boucle, seulement quand elle est à
//   l'écran ; avec « réduire les animations », une seule image, fixe ;
// - chaque photo de la galerie a sa barre avant / après (un vrai curseur, au clavier
//   aussi) : l'image brute à gauche, la photo développée et son film à droite (le filtre
//   CSS posé par la page).

import { drawPhoto, rawOf, macScreen } from './screen.js';

const W = 640, H = 400;
const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const smooth = (t) => { t = clamp(t); return t * t * (3 - 2 * t); };
const lerp = (a, b, k) => a + (b - a) * k;

function canvasOf(w, h) {
  const c = document.createElement('canvas');
  c.width = w; c.height = h;
  return c;
}

function photo(w, h, opts) {
  const c = canvasOf(w, h);
  drawPhoto(c.getContext('2d', { willReadFrequently: true }), 0, 0, w, h, opts);
  return c;
}

function round(ctx, x, y, w, h, r) {
  ctx.beginPath(); ctx.roundRect ? ctx.roundRect(x, y, w, h, r) : ctx.rect(x, y, w, h);
}

/** Le cercle du pinceau, avec son contour progressif. */
function brush(ctx, x, y, r, mark) {
  ctx.save();
  ctx.lineWidth = 2; ctx.strokeStyle = 'rgba(255,255,255,.95)';
  ctx.shadowColor = 'rgba(0,0,0,.5)'; ctx.shadowBlur = 6;
  ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2); ctx.stroke();
  ctx.setLineDash([4, 5]); ctx.lineWidth = 1.2; ctx.strokeStyle = 'rgba(255,255,255,.6)';
  ctx.beginPath(); ctx.arc(x, y, r * 0.6, 0, Math.PI * 2); ctx.stroke();
  ctx.setLineDash([]);
  if (mark) {
    ctx.lineWidth = 2.4; ctx.strokeStyle = '#fff'; ctx.lineCap = 'round';
    ctx.beginPath(); mark(ctx, x, y, r * 0.22); ctx.stroke();
  }
  ctx.restore();
}

// ----- Les scènes des nouveautés ------------------------------------------------------

/** Fluidité : le pinceau gonfle, dégonfle, puis pousse la photo sous lui. */
function liquify(ctx) {
  const base = photo(W, H, { seed: 4, hue: -8 });
  const src = base.getContext('2d').getImageData(0, 0, W, H).data;
  const R = 92;
  const out = ctx.createImageData(2 * R, 2 * R);
  const plus = (c, x, y, d) => { c.moveTo(x - d, y); c.lineTo(x + d, y); c.moveTo(x, y - d); c.lineTo(x, y + d); };
  const minus = (c, x, y, d) => { c.moveTo(x - d, y); c.lineTo(x + d, y); };
  const arrow = (c, x, y, d) => { c.moveTo(x - d, y); c.lineTo(x + d, y); c.moveTo(x + d * 0.3, y - d * 0.7); c.lineTo(x + d, y); c.lineTo(x + d * 0.3, y + d * 0.7); };
  return (t) => {
    // Trois temps de quatre secondes : gonfler, dégonfler, pousser.
    const phase = Math.floor(t / 4) % 3, u = (t % 4) / 4;
    const k = Math.sin(Math.PI * smooth(u));
    const cx = W * (0.36 + 0.28 * (phase === 2 ? smooth(u) : 0.5 + 0.12 * Math.sin(t * 0.8))), cy = H * 0.5;
    ctx.drawImage(base, 0, 0);
    const x0 = Math.round(cx - R), y0 = Math.round(cy - R), d = out.data;
    for (let j = 0; j < 2 * R; j++) {
      for (let i = 0; i < 2 * R; i++) {
        const dx = i - R, dy = j - R, r2 = (dx * dx + dy * dy) / (R * R);
        let sx = x0 + i, sy = y0 + j;
        if (r2 < 1) {
          const f = (1 - r2) * (1 - r2) * k;
          if (phase === 0) { sx = cx + dx * (1 - 0.55 * f); sy = cy + dy * (1 - 0.55 * f); }
          else if (phase === 1) { sx = cx + dx * (1 + 0.5 * f); sy = cy + dy * (1 + 0.5 * f); }
          else { sx -= 46 * f; }
        }
        sx = clamp(Math.round(sx), 0, W - 1); sy = clamp(Math.round(sy), 0, H - 1);
        const a = (sy * W + sx) * 4, b = (j * 2 * R + i) * 4;
        d[b] = src[a]; d[b + 1] = src[a + 1]; d[b + 2] = src[a + 2]; d[b + 3] = 255;
      }
    }
    // Seulement le disque du pinceau : le reste de la photo ne bouge pas.
    const tmp = liquify.tmp || (liquify.tmp = canvasOf(2 * R, 2 * R));
    tmp.getContext('2d').putImageData(out, 0, 0);
    ctx.save(); ctx.beginPath(); ctx.arc(cx, cy, R, 0, Math.PI * 2); ctx.clip();
    ctx.drawImage(tmp, x0, y0); ctx.restore();
    brush(ctx, cx, cy, R, [plus, minus, arrow][phase]);
  };
}

/** Suppression tracée : le pinceau suit le câble, qui disparaît derrière lui. */
function stroke(ctx) {
  const base = photo(W, H, { seed: 6, hue: 10 });
  const wire = [];
  for (let i = 0; i <= 64; i++) {
    const u = i / 64;
    wire.push([u * W, H * (0.16 + 0.1 * u) + Math.sin(u * Math.PI) * H * 0.12]);
  }
  const line = (from, to, width, color) => {
    if (to <= from) return;
    ctx.beginPath();
    ctx.moveTo(...wire[from]);
    for (let i = from + 1; i <= to; i++) ctx.lineTo(...wire[i]);
    ctx.lineWidth = width; ctx.strokeStyle = color; ctx.stroke();
  };
  return (t) => {
    const u = clamp(((t % 5) / 5) * 1.35 - 0.12);
    const at = Math.round(u * 64);
    ctx.drawImage(base, 0, 0);
    // Le poteau à gauche, puis le câble qui reste à effacer.
    ctx.fillStyle = 'rgba(8,8,12,.92)'; ctx.fillRect(10, wire[0][1] - 6, 7, H);
    ctx.lineCap = 'round';
    line(at, 64, 3, 'rgba(10,10,14,.9)');
    // Le trait tracé, en pointillés, qui s'efface doucement.
    ctx.setLineDash([6, 8]);
    line(Math.max(0, at - 26), at, 1.5, 'rgba(255,255,255,.55)');
    ctx.setLineDash([]);
    if (u > 0 && u < 1) brush(ctx, wire[at][0], wire[at][1], 18);
  };
}

/** Un histogramme de luminance en 48 cases, normalisé. */
function histogramOf(c) {
  const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
  const bins = new Float32Array(48);
  for (let i = 0; i < d.length; i += 16) {
    const l = (d[i] * 0.3 + d[i + 1] * 0.59 + d[i + 2] * 0.11) / 256;
    bins[Math.min(47, Math.floor(l * 48))]++;
  }
  const max = Math.max(...bins);
  return bins.map((v) => Math.sqrt(v / max));
}

/** Tons auto : la photo terne prend ses tons d'un coup, l'histogramme s'étale. */
function autotone(ctx) {
  const after = photo(W, H, { seed: 8, hue: -20 });
  const before = rawOf(after);
  const hb = histogramOf(before), ha = histogramOf(after);
  return (t) => {
    const u = t % 6;
    const k = u < 1.4 ? 0 : u < 3 ? smooth((u - 1.4) / 1.2) : u < 5 ? 1 : 1 - smooth((u - 5) / 1);
    ctx.drawImage(before, 0, 0);
    ctx.globalAlpha = k; ctx.drawImage(after, 0, 0); ctx.globalAlpha = 1;
    // L'histogramme, en bas à droite.
    const bx = W - 196, by = H - 96, bw = 176, bh = 76;
    ctx.fillStyle = 'rgba(8,8,12,.62)'; round(ctx, bx - 10, by - 10, bw + 20, bh + 20, 12); ctx.fill();
    const g = ctx.createLinearGradient(bx, 0, bx + bw, 0);
    g.addColorStop(0, '#8b7bff'); g.addColorStop(0.5, '#4af2c8'); g.addColorStop(1, '#ffcf4a');
    ctx.fillStyle = g;
    ctx.beginPath(); ctx.moveTo(bx, by + bh);
    for (let i = 0; i < 48; i++) ctx.lineTo(bx + (i + 0.5) * bw / 48, by + bh - lerp(hb[i], ha[i], k) * bh);
    ctx.lineTo(bx + bw, by + bh); ctx.closePath(); ctx.fill();
    // Le bouton « tons auto » qu'on presse : une baguette lumineuse.
    const press = u > 1 && u < 1.6 ? 1 - Math.abs(u - 1.3) / 0.3 : 0;
    ctx.save();
    ctx.translate(40, 40);
    ctx.fillStyle = `rgba(255,255,255,${0.16 + press * 0.5})`;
    ctx.beginPath(); ctx.arc(0, 0, 22 + press * 4, 0, Math.PI * 2); ctx.fill();
    ctx.strokeStyle = '#fff'; ctx.lineWidth = 2.4; ctx.lineCap = 'round';
    ctx.beginPath(); ctx.moveTo(-8, 8); ctx.lineTo(6, -6); ctx.stroke();
    ctx.fillStyle = '#ffcf4a';
    [[8, -9], [-1, -11], [10, 1]].forEach(([x, y]) => { ctx.beginPath(); ctx.arc(x, y, 2, 0, Math.PI * 2); ctx.fill(); });
    ctx.restore();
  };
}

/** Traces GPX : la trace se dessine sur la carte, les photos s'y posent une à une. */
function gpx(ctx) {
  const pts = [];
  for (let i = 0; i <= 120; i++) {
    const u = i / 120;
    pts.push([60 + u * (W - 120) + Math.sin(u * 9) * 18, H * 0.56 + Math.sin(u * 5.2 + 0.6) * H * 0.2 + Math.cos(u * 13) * 10]);
  }
  const stops = [14, 38, 62, 86, 110];
  const thumbs = stops.map((_, i) => photo(84, 60, { seed: 3 + i * 2, hue: i * 36 - 50 }));
  // Le fond de carte : des courbes de niveau, peintes une fois.
  const map = canvasOf(W, H), m = map.getContext('2d');
  m.fillStyle = '#0d1016'; m.fillRect(0, 0, W, H);
  m.strokeStyle = 'rgba(120,140,170,.12)'; m.lineWidth = 1;
  for (let k = 0; k < 14; k++) {
    m.beginPath();
    for (let x = 0; x <= W; x += 8) {
      const y = k * 34 - 40 + Math.sin(x * 0.012 + k) * 22 + Math.sin(x * 0.031 + k * 2) * 8;
      x ? m.lineTo(x, y) : m.moveTo(x, y);
    }
    m.stroke();
  }
  m.strokeStyle = 'rgba(74,168,255,.28)'; m.lineWidth = 6; m.lineCap = 'round';
  m.beginPath(); m.moveTo(W * 0.7, 0); m.bezierCurveTo(W * 0.62, H * 0.3, W * 0.9, H * 0.55, W * 0.8, H);
  m.stroke();
  return (t) => {
    const u = (t % 7) / 5.5;
    const n = Math.min(120, Math.floor(u * 120));
    ctx.drawImage(map, 0, 0);
    // La trace, aux couleurs du spectre.
    const g = ctx.createLinearGradient(60, 0, W - 60, 0);
    g.addColorStop(0, '#ff6b3d'); g.addColorStop(0.35, '#ffcf4a'); g.addColorStop(0.7, '#4af2c8'); g.addColorStop(1, '#8b7bff');
    ctx.strokeStyle = g; ctx.lineWidth = 3.5; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    ctx.beginPath(); ctx.moveTo(...pts[0]);
    for (let i = 1; i <= n; i++) ctx.lineTo(...pts[i]);
    ctx.stroke();
    stops.forEach((s, i) => {
      if (n < s) return;
      const k = smooth((n - s) / 10);
      const [x, y] = pts[s];
      ctx.fillStyle = '#fff'; ctx.beginPath(); ctx.arc(x, y, 5, 0, Math.PI * 2); ctx.fill();
      // La vignette de la photo, au‑dessus de son point.
      const w = 84 * k, h = 60 * k, up = i % 2 ? 1 : -1;
      const tx = clamp(x - w / 2, 8, W - w - 8), ty = up < 0 ? y - h - 18 : y + 18;
      ctx.save();
      ctx.shadowColor = 'rgba(0,0,0,.5)'; ctx.shadowBlur = 12;
      ctx.fillStyle = '#fff'; round(ctx, tx - 3, ty - 3, w + 6, h + 6, 7); ctx.fill();
      ctx.restore();
      ctx.save(); round(ctx, tx, ty, w, h, 5); ctx.clip();
      ctx.drawImage(thumbs[i], tx, ty, w, h); ctx.restore();
    });
    // L'horloge du boîtier : l'aiguille cherche l'écart, puis s'arrête.
    const c = clamp(u * 1.6);
    ctx.save(); ctx.translate(46, 46);
    ctx.fillStyle = 'rgba(255,255,255,.1)'; ctx.beginPath(); ctx.arc(0, 0, 24, 0, Math.PI * 2); ctx.fill();
    ctx.strokeStyle = 'rgba(255,255,255,.85)'; ctx.lineWidth = 2; ctx.stroke();
    ctx.lineCap = 'round';
    const a1 = -Math.PI / 2 + (1 - Math.pow(1 - c, 3)) * Math.PI * 4.6;
    ctx.beginPath(); ctx.moveTo(0, 0); ctx.lineTo(Math.cos(a1) * 16, Math.sin(a1) * 16); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(0, 0); ctx.lineTo(Math.cos(a1 / 12 - 1) * 10, Math.sin(a1 / 12 - 1) * 10); ctx.stroke();
    if (c >= 1) { ctx.strokeStyle = '#4af2c8'; ctx.lineWidth = 3; ctx.beginPath(); ctx.arc(0, 0, 29, 0, Math.PI * 2); ctx.stroke(); }
    ctx.restore();
  };
}

/** Agrandir à l'export : le cadre passe de la taille de la photo à quatre fois plus. */
function enlarge(ctx) {
  const big = photo(W, H, { seed: 12, hue: 26 });
  return (t) => {
    const u = t % 6;
    const k = u < 1 ? 0 : u < 3.4 ? smooth((u - 1) / 2.4) : u < 5.2 ? 1 : 1 - smooth((u - 5.2) / 0.8);
    const s = lerp(0.25, 1, k), w = W * s, h = H * s;
    ctx.fillStyle = '#0b0b10'; ctx.fillRect(0, 0, W, H);
    // La grille du plan de travail.
    ctx.strokeStyle = 'rgba(255,255,255,.05)'; ctx.lineWidth = 1;
    for (let x = 0; x < W; x += 32) { ctx.beginPath(); ctx.moveTo(x + 0.5, 0); ctx.lineTo(x + 0.5, H); ctx.stroke(); }
    for (let y = 0; y < H; y += 32) { ctx.beginPath(); ctx.moveTo(0, y + 0.5); ctx.lineTo(W, y + 0.5); ctx.stroke(); }
    ctx.drawImage(big, 0, H - h, w, h);
    // La taille d'origine, en pointillés, et le facteur.
    ctx.setLineDash([5, 6]); ctx.strokeStyle = 'rgba(255,255,255,.8)'; ctx.lineWidth = 1.5;
    ctx.strokeRect(0.75, H - H * 0.25 + 0.75, W * 0.25 - 1.5, H * 0.25 - 1.5);
    ctx.setLineDash([]);
    ctx.strokeStyle = 'rgba(255,207,74,.9)'; ctx.lineWidth = 2;
    ctx.strokeRect(1, H - h + 1, w - 2, h - 2);
    const f = `×${lerp(1, 4, k).toFixed(1).replace('.0', '')}`;
    ctx.font = '600 22px "JetBrains Mono", ui-monospace, monospace';
    const tw = ctx.measureText(f).width;
    ctx.fillStyle = 'rgba(8,8,12,.7)'; round(ctx, W - tw - 44, 18, tw + 26, 38, 19); ctx.fill();
    ctx.fillStyle = '#ffcf4a'; ctx.fillText(f, W - tw - 31, 45);
  };
}

/** Le ciel plus clair, plus net : la même photo, sous un voile laiteux. */
function hazeOver(src, amount) {
  const c = canvasOf(src.width, src.height), x = c.getContext('2d');
  x.drawImage(src, 0, 0);
  const g = x.createLinearGradient(0, 0, 0, src.height * 0.66);
  g.addColorStop(0, `rgba(205,200,215,${amount})`);
  g.addColorStop(1, `rgba(205,200,215,${amount * 0.35})`);
  x.fillStyle = g; x.fillRect(0, 0, src.width, src.height * 0.66);
  return c;
}

/** Un dégradé linéaire de masque : ses trois lignes de réglage. */
function gradientLines(ctx, y0, y1, color) {
  ctx.save();
  ctx.strokeStyle = color; ctx.lineWidth = 1.6;
  [y0, (y0 + y1) / 2, y1].forEach((y, i) => {
    ctx.setLineDash(i === 1 ? [] : [7, 7]);
    ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(W, y); ctx.stroke();
  });
  ctx.setLineDash([]);
  ctx.fillStyle = '#fff';
  ctx.beginPath(); ctx.arc(W / 2, (y0 + y1) / 2, 7, 0, Math.PI * 2); ctx.fill();
  ctx.restore();
}

/** La zone d'un masque en couleur, d'un dégradé vertical. */
function zone(ctx, y0, y1, rgb, a, invert) {
  const g = ctx.createLinearGradient(0, y0, 0, y1);
  g.addColorStop(0, `rgba(${rgb},${invert ? 0 : a})`);
  g.addColorStop(1, `rgba(${rgb},${invert ? a : 0})`);
  ctx.fillStyle = g;
  ctx.fillRect(0, y0, W, y1 - y0);
  ctx.fillStyle = `rgba(${rgb},${a})`;
  if (invert) ctx.fillRect(0, y1, W, H - y1);
  else ctx.fillRect(0, 0, W, y0);
}

/** Correction du voile dans un masque : le dégradé se pose, le ciel voilé se débouche. */
function maskfx(ctx) {
  const clear = photo(W, H, { seed: 10, hue: -30 });
  const hazy = hazeOver(clear, 0.55);
  const y0 = H * 0.18, y1 = H * 0.6;
  return (t) => {
    const u = t % 7;
    const show = u < 0.8 ? 0 : u < 1.6 ? smooth((u - 0.8) / 0.8) : u < 2.6 ? 1 : 1 - smooth((u - 2.6) / 0.6);
    const k = u < 2.8 ? 0 : u < 4.4 ? smooth((u - 2.8) / 1.6) : u < 6.2 ? 1 : 1 - smooth((u - 6.2) / 0.8);
    ctx.drawImage(hazy, 0, 0);
    ctx.globalAlpha = k; ctx.drawImage(clear, 0, 0); ctx.globalAlpha = 1;
    if (show > 0) zone(ctx, y0, y1, '255,59,48', 0.42 * show, false);
    gradientLines(ctx, y0, y1, `rgba(255,255,255,${0.35 + 0.5 * Math.max(show, 1 - k)})`);
  };
}

/** Dupliquer un masque : le ciel, puis sa copie inversée sur le sol. */
function duplicate(ctx) {
  const base = photo(W, H, { seed: 14, hue: 8 });
  const y0 = H * 0.42, y1 = H * 0.66;
  const chip = (x, y, on, inverted, color) => {
    ctx.fillStyle = on ? 'rgba(255,255,255,.18)' : 'rgba(8,8,12,.55)';
    round(ctx, x, y, 132, 34, 10); ctx.fill();
    ctx.save(); round(ctx, x + 8, y + 7, 30, 20, 5); ctx.clip();
    const g = ctx.createLinearGradient(0, y + 7, 0, y + 27);
    g.addColorStop(0, inverted ? '#222' : color); g.addColorStop(1, inverted ? color : '#222');
    ctx.fillStyle = g; ctx.fillRect(x + 8, y + 7, 30, 20); ctx.restore();
    ctx.fillStyle = 'rgba(255,255,255,.75)';
    ctx.fillRect(x + 48, y + 12, 58, 4); ctx.fillRect(x + 48, y + 20, 38, 4);
  };
  return (t) => {
    const u = t % 6;
    const sky = u < 0.6 ? smooth(u / 0.6) : u < 2.6 ? 1 : 1 - smooth((u - 2.6) / 0.5) * 0.7;
    const copy = u < 2.4 ? 0 : u < 3.2 ? smooth((u - 2.4) / 0.8) : u < 5.4 ? 1 : 1 - smooth((u - 5.4) / 0.6);
    ctx.drawImage(base, 0, 0);
    zone(ctx, y0, y1, '255,59,48', 0.45 * sky * (u > 5.4 ? 1 - copy : 1), false);
    if (copy > 0) zone(ctx, y0, y1, '74,242,200', 0.45 * copy, true);
    gradientLines(ctx, y0, y1, 'rgba(255,255,255,.7)');
    chip(W - 152, 18, copy < 0.5, false, '#ff3b30');
    if (copy > 0) {
      ctx.globalAlpha = copy;
      chip(W - 152, 18 + 42 * copy, copy >= 0.5, true, '#4af2c8');
      ctx.globalAlpha = 1;
    }
  };
}

// Le motif X‑Trans de Fujifilm : 6 × 6 photosites, 20 verts.
const XT = ['GBGGRG', 'RGRBGB', 'GBGGRG', 'GRGGBG', 'BGBRGR', 'GRGGBG'];

/** X‑Trans directionnel : la mosaïque du capteur, puis le contour reconstruit net. */
function xtrans(ctx) {
  const cell = 16, cols = W / cell, rows = H / cell;
  const scene = (x, y, soft = 1.4) => {
    // Un contour en diagonale, légèrement ondulé : clair d'un côté, sombre de l'autre.
    const e = smooth(0.5 + (x * 0.62 + y - 26 + Math.sin(y * 0.35) * 1.6) / soft);
    return [lerp(1, 0.1, e), lerp(0.78, 0.15, e), lerp(0.42, 0.32, e)];
  };
  const mosaic = canvasOf(W, H), clean = canvasOf(W, H);
  const mc = mosaic.getContext('2d'), cc = clean.getContext('2d');
  mc.fillStyle = '#050507'; mc.fillRect(0, 0, W, H);
  for (let j = 0; j < rows; j++) {
    for (let i = 0; i < cols; i++) {
      const [r, g, b] = scene(i + 0.5, j + 0.5);
      const ch = XT[j % 6][i % 6];
      // Chaque photosite ne voit qu'une couleur : la mosaïque, assourdie.
      const v = (ch === 'R' ? [r, 0.06, 0.05] : ch === 'G' ? [0.05, g, 0.06] : [0.05, 0.06, b]).map((c) => c * 0.72);
      mc.fillStyle = `rgb(${v.map((c) => Math.round(c * 255)).join(',')})`;
      mc.fillRect(i * cell, j * cell, cell - 1, cell - 1);
    }
  }
  // Le rendu : chaque pixel à sa vraie couleur, le contour lisse.
  for (let y = 0; y < H; y += 2) {
    for (let x = 0; x < W; x += 2) {
      const [r, g, b] = scene(x / cell, y / cell, 0.3);
      cc.fillStyle = `rgb(${Math.round(r * 255)},${Math.round(g * 255)},${Math.round(b * 255)})`;
      cc.fillRect(x, y, 2, 2);
    }
  }
  return (t) => {
    const u = t % 6;
    const k = u < 1 ? 0 : u < 3 ? smooth((u - 1) / 2) : u < 5 ? 1 : 1 - smooth((u - 5) / 1);
    const sx = k * W;
    ctx.drawImage(mosaic, 0, 0);
    ctx.save(); ctx.beginPath(); ctx.rect(0, 0, sx, H); ctx.clip(); ctx.drawImage(clean, 0, 0); ctx.restore();
    if (k > 0 && k < 1) {
      ctx.fillStyle = '#fff'; ctx.shadowColor = 'rgba(255,255,255,.8)'; ctx.shadowBlur = 14;
      ctx.fillRect(sx - 1, 0, 2, H); ctx.shadowBlur = 0;
    }
    // Le motif 6 × 6, en légende, en haut à gauche.
    const s = 9, ox = 18, oy = 18;
    ctx.fillStyle = 'rgba(8,8,12,.7)'; round(ctx, ox - 8, oy - 8, s * 6 + 16, s * 6 + 16, 9); ctx.fill();
    XT.forEach((row, j) => [...row].forEach((ch, i) => {
      ctx.fillStyle = ch === 'R' ? '#ff4d3d' : ch === 'G' ? '#3ddc84' : '#4a8bff';
      ctx.fillRect(ox + i * s, oy + j * s, s - 1.5, s - 1.5);
    }));
  };
}

/** LÜMA pour Windows : l'interface de l'app, dans sa fenêtre Windows. */
function windows(ctx, canvas) {
  // L'écran est peint en grand, puis réduit : plus net qu'à 640 px.
  const big = canvasOf(1280, 800);
  const draw = macScreen(big, { chrome: 'windows' });
  return (t) => {
    draw({ t, edit: 1 });
    ctx.fillStyle = '#0b0b10'; ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(big, 0, 0, W, H);
  };
}

const DEMOS = { liquify, stroke, autotone, gpx, enlarge, maskfx, duplicate, xtrans, windows };
// L'instant montré, fixe, quand les animations sont réduites.
const STILL = { liquify: 2, stroke: 3, autotone: 4, gpx: 6.5, enlarge: 4, maskfx: 2, duplicate: 4, xtrans: 2.2, windows: 3 };

export function initRecent(section, { reduced } = {}) {
  const live = new Set();
  const scenes = [...section.querySelectorAll('canvas[data-demo]')].map((canvas) => {
    const name = canvas.dataset.demo, make = DEMOS[name];
    if (!make) return null;
    canvas.width = W; canvas.height = H;
    const ctx = canvas.getContext('2d');
    let draw = null;
    const scene = { canvas, name, start: performance.now() / 1000 - (STILL[name] || 0), frame(now) {
      draw = draw || make(ctx, canvas);
      draw(reduced ? STILL[name] || 0 : now - scene.start);
    } };
    return scene;
  }).filter(Boolean);
  // Une première image fixe pour chaque scène, l'une après l'autre : aucune carte vide,
  // même en défilant vite.
  scenes.forEach((s, i) => setTimeout(() => { if (!live.has(s)) s.frame(s.start + (STILL[s.name] || 0)); }, 40 * i));

  let raf = 0;
  const tick = () => {
    raf = 0;
    const now = performance.now() / 1000;
    live.forEach((s) => s.frame(now));
    if (live.size) raf = requestAnimationFrame(tick);
  };
  const io = new IntersectionObserver((entries) => {
    entries.forEach((e) => {
      const s = scenes.find((x) => x.canvas === e.target);
      if (!s) return;
      if (reduced) { if (e.isIntersecting) { s.frame(0); io.unobserve(e.target); } return; }
      if (e.isIntersecting) live.add(s); else live.delete(s);
    });
    if (!reduced && live.size && !raf) raf = requestAnimationFrame(tick);
  }, { rootMargin: '80px 0px' });
  scenes.forEach((s) => io.observe(s.canvas));
}

// ----- La galerie ---------------------------------------------------------------------

export function initGallery(section, { reduced } = {}) {
  section.querySelectorAll('.ba').forEach((fig, k) => {
    const before = fig.querySelector('.ba-before'), after = fig.querySelector('.ba-after');
    const frame = fig.querySelector('.ba-frame'), range = fig.querySelector('.ba-range');
    if (!before || !after || !frame) return;
    const seed = Number(fig.dataset.seed) || 3, hue = Number(fig.dataset.hue) || 0;
    const w = 800, h = 600;
    const developed = photo(w, h, { seed, hue });
    after.width = before.width = w; after.height = before.height = h;
    after.getContext('2d').drawImage(developed, 0, 0);
    before.getContext('2d').drawImage(rawOf(developed), 0, 0);
    frame.classList.add('ba-ready');
    const set = (v) => frame.style.setProperty('--split', `${v}%`);
    if (range) {
      range.addEventListener('input', () => set(range.value));
      set(range.value);
      // Un petit aller‑retour de la barre, la première fois qu'elle paraît.
      if (!reduced) {
        const io = new IntersectionObserver(([e]) => {
          if (!e.isIntersecting) return;
          io.disconnect();
          const t0 = performance.now() + k * 140;
          const step = (now) => {
            if (document.activeElement === range) return;
            const u = clamp((now - t0) / 1400);
            const v = 50 + Math.sin(u * Math.PI * 2) * 22 * (1 - u);
            range.value = String(Math.round(v)); set(v);
            if (u < 1) requestAnimationFrame(step);
          };
          requestAnimationFrame(step);
        }, { threshold: 0.6 });
        io.observe(frame);
      }
    }
  });
}
