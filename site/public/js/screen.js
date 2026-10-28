// Images dessinées au canvas 2D : une photo de paysage procédurale (aucun fichier à
// charger) et les écrans de l'application LÜMA affichés sur le Mac, l'iPhone et l'iPad.
//
// Chaque écran est vivant : `macScreen`, `phoneScreen` et `padScreen` peignent une fois
// ce qui ne bouge pas, puis rendent une fonction qui redessine l'image d'un instant
// (curseur glissé, molette tournée, masque peint à l'Apple Pencil).

// Les libellés des écrans, dans la langue de la page (`<html lang>`) ; le français sinon.
const LABELS = {
  en: {
    'Bibliothèque': 'Library', 'Développement': 'Develop', 'Masques': 'Masks', 'Export': 'Export',
    'Lumière': 'Light', 'Exposition': 'Exposure', 'Contraste': 'Contrast', 'Hautes lumières': 'Highlights',
    'Ombres': 'Shadows', 'Blancs': 'Whites', 'Noirs': 'Blacks', 'Couleur': 'Color', 'Température': 'Temperature',
    'Teinte': 'Tint', 'Vibrance': 'Vibrance', 'Saturation': 'Saturation', 'Ciel': 'Sky', 'Films': 'Films', 'Recadrer': 'Crop',
    'Islande_0427.ARW': 'Iceland_0427.ARW', 'Avant': 'Before', 'Après': 'After', 'Pinceau': 'Brush',
    'Gomme': 'Eraser', 'Taille': 'Size', 'Clarté': 'Clarity', 'Sujet': 'Subject', 'Intensité': 'Amount',
  },
  es: {
    'Bibliothèque': 'Biblioteca', 'Développement': 'Revelado', 'Masques': 'Máscaras', 'Export': 'Exportar',
    'Lumière': 'Luz', 'Exposition': 'Exposición', 'Contraste': 'Contraste', 'Hautes lumières': 'Altas luces',
    'Ombres': 'Sombras', 'Blancs': 'Blancos', 'Noirs': 'Negros', 'Couleur': 'Color',
    'Température': 'Temperatura', 'Teinte': 'Matiz', 'Vibrance': 'Intensidad', 'Saturation': 'Saturación',
    'Ciel': 'Cielo', 'Films': 'Películas', 'Recadrer': 'Recortar', 'Islande_0427.ARW': 'Islandia_0427.ARW',
    'Avant': 'Antes', 'Après': 'Después', 'Pinceau': 'Pincel', 'Gomme': 'Borrador', 'Taille': 'Tamaño',
    'Clarté': 'Claridad', 'Sujet': 'Sujeto', 'Intensité': 'Intensidad',
  },
};
const tr = (text) => (LABELS[document.documentElement.lang] || {})[text] || text;

const rand = (seed) => () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646;

// Ligne de crête : somme de sinusoïdes, stable pour une graine donnée.
function ridge(x, seed, amp, freq) {
  let y = 0, a = amp, f = freq;
  for (let o = 0; o < 5; o++) {
    y += Math.sin(x * f + seed * (o + 1) * 1.7) * a + Math.sin(x * f * 2.1 + seed * 3.1) * a * 0.35;
    a *= 0.5; f *= 2.05;
  }
  return y;
}

/** Peint un paysage au coucher du soleil dans le rectangle donné. */
export function drawPhoto(ctx, x, y, w, h, opts = {}) {
  const { hue = 0, mono = false, seed = 3 } = opts;
  ctx.save();
  ctx.beginPath(); ctx.rect(x, y, w, h); ctx.clip();
  const horizon = y + h * 0.62;

  const sky = ctx.createLinearGradient(0, y, 0, horizon);
  sky.addColorStop(0, `hsl(${235 + hue} 45% 14%)`);
  sky.addColorStop(0.45, `hsl(${280 + hue} 40% 32%)`);
  sky.addColorStop(0.78, `hsl(${18 + hue} 85% 58%)`);
  sky.addColorStop(1, `hsl(${38 + hue} 100% 72%)`);
  ctx.fillStyle = sky; ctx.fillRect(x, y, w, h);

  const sx = x + w * 0.62, sy = horizon - h * 0.07;
  const glow = ctx.createRadialGradient(sx, sy, 0, sx, sy, w * 0.45);
  glow.addColorStop(0, `hsla(${45 + hue} 100% 90% / .95)`);
  glow.addColorStop(0.08, `hsla(${40 + hue} 100% 70% / .6)`);
  glow.addColorStop(1, 'hsla(20 100% 50% / 0)');
  ctx.fillStyle = glow; ctx.fillRect(x, y, w, h);
  ctx.fillStyle = `hsl(${50 + hue} 100% 94%)`;
  ctx.beginPath(); ctx.arc(sx, sy, w * 0.028, 0, Math.PI * 2); ctx.fill();

  const layers = [
    { base: 0.5, amp: 0.07, col: `hsl(${290 + hue} 30% 34%)`, f: 4 },
    { base: 0.56, amp: 0.06, col: `hsl(${270 + hue} 32% 22%)`, f: 6 },
    { base: 0.61, amp: 0.035, col: `hsl(${250 + hue} 30% 12%)`, f: 9 },
  ];
  layers.forEach((l, i) => {
    ctx.fillStyle = l.col;
    ctx.beginPath(); ctx.moveTo(x, horizon + 2);
    for (let px = 0; px <= w; px += 3) {
      const t = px / w;
      ctx.lineTo(x + px, y + h * (l.base - Math.abs(ridge(t, seed + i, l.amp, l.f))));
    }
    ctx.lineTo(x + w, horizon + 2); ctx.closePath(); ctx.fill();
  });

  // Lac : reflet du ciel, assombri, avec des stries horizontales.
  const lake = ctx.createLinearGradient(0, horizon, 0, y + h);
  lake.addColorStop(0, `hsl(${30 + hue} 80% 55%)`);
  lake.addColorStop(0.25, `hsl(${290 + hue} 30% 24%)`);
  lake.addColorStop(1, `hsl(${235 + hue} 40% 8%)`);
  ctx.fillStyle = lake; ctx.fillRect(x, horizon, w, h);
  const r = rand(seed * 977);
  for (let i = 0; i < 70; i++) {
    const ly = horizon + Math.pow(r(), 1.6) * (h * 0.38);
    const lw = w * (0.03 + r() * 0.18) * (1 - (ly - horizon) / h);
    ctx.fillStyle = `hsla(${45 + hue} 100% 80% / ${0.5 * (1 - (ly - horizon) / (h * 0.38))})`;
    ctx.fillRect(sx - lw / 2 + (r() - 0.5) * w * 0.1, ly, lw, Math.max(1, h * 0.004));
  }
  ctx.fillStyle = `hsl(${250 + hue} 30% 5%)`;
  ctx.beginPath(); ctx.moveTo(x, y + h);
  for (let px = 0; px <= w; px += 4) ctx.lineTo(x + px, y + h * (0.93 - Math.abs(ridge(px / w, seed + 9, 0.05, 3))));
  ctx.lineTo(x + w, y + h); ctx.fill();

  if (mono) {
    const img = ctx.getImageData(x, y, w, h), d = img.data;
    for (let i = 0; i < d.length; i += 4) {
      const l = d[i] * 0.3 + d[i + 1] * 0.59 + d[i + 2] * 0.11;
      d[i] = d[i + 1] = d[i + 2] = l;
    }
    ctx.putImageData(img, x, y);
  }
  ctx.restore();
}

/** Une image de la photo, utile comme fond CSS (couches du pipeline). */
export function photoURL(w, h, opts) {
  const c = document.createElement('canvas');
  c.width = w; c.height = h;
  drawPhoto(c.getContext('2d'), 0, 0, w, h, opts);
  return c;
}

function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath();
  ctx.moveTo(x + r, y); ctx.arcTo(x + w, y, x + w, y + h, r); ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r); ctx.arcTo(x, y, x + w, y, r); ctx.closePath();
}

function slider(ctx, x, y, w, label, v, s, accent) {
  ctx.fillStyle = 'rgba(255,255,255,.55)';
  ctx.font = `${11 * s}px -apple-system, Inter, sans-serif`;
  ctx.fillText(label, x, y);
  ctx.textAlign = 'right';
  ctx.fillStyle = '#fff';
  ctx.fillText((v > 0 ? '+' : '') + (v * 100).toFixed(0), x + w, y);
  ctx.textAlign = 'left';
  ctx.fillStyle = 'rgba(255,255,255,.14)';
  roundRect(ctx, x, y + 8 * s, w, 3 * s, 1.5 * s); ctx.fill();
  if (accent) {
    const g = ctx.createLinearGradient(x, 0, x + w, 0);
    accent.forEach((c, i) => g.addColorStop(i / (accent.length - 1), c));
    ctx.fillStyle = g; roundRect(ctx, x, y + 8 * s, w, 3 * s, 1.5 * s); ctx.fill();
  }
  ctx.fillStyle = '#fff';
  ctx.beginPath(); ctx.arc(x + w * (0.5 + v / 2), y + 9.5 * s, 6 * s, 0, Math.PI * 2); ctx.fill();
}

/** L'histogramme ; `shift` le pousse vers les hautes lumières (l'exposition). */
function histogram(ctx, x, y, w, h, shift = 0) {
  ctx.fillStyle = 'rgba(255,255,255,.04)';
  roundRect(ctx, x, y, w, h, 8); ctx.fill();
  ctx.globalCompositeOperation = 'lighter';
  [['rgba(255,70,70,.55)', 0.3, 0.2], ['rgba(70,255,140,.5)', 0.45, 0.4], ['rgba(70,140,255,.55)', 0.6, 0.65]].forEach(([c, m, k]) => {
    ctx.fillStyle = c;
    ctx.beginPath(); ctx.moveTo(x, y + h);
    for (let i = 0; i <= 80; i++) {
      const t = i / 80;
      const u = t - shift;
      const v = Math.exp(-Math.pow((u - m) / 0.16, 2)) * 0.8 + Math.exp(-Math.pow((u - k - 0.2) / 0.07, 2)) * 0.45 + 0.05 * Math.sin(t * 40 + m * 9) ** 2;
      ctx.lineTo(x + t * w, y + h - v * h * 0.9);
    }
    ctx.lineTo(x + w, y + h); ctx.fill();
  });
  ctx.globalCompositeOperation = 'source-over';
}

/** Une petite icône d'outil, dessinée au trait (soleil, gouttes, film, masque, recadrage). */
function toolIcon(ctx, kind, cx, cy, r, color) {
  ctx.save();
  ctx.strokeStyle = color; ctx.fillStyle = color; ctx.lineWidth = r * 0.16; ctx.lineCap = 'round';
  if (kind === 'light') {
    ctx.beginPath(); ctx.arc(cx, cy, r * 0.42, 0, 7); ctx.stroke();
    for (let k = 0; k < 8; k++) {
      const a = (k * Math.PI) / 4;
      ctx.beginPath();
      ctx.moveTo(cx + Math.cos(a) * r * 0.66, cy + Math.sin(a) * r * 0.66);
      ctx.lineTo(cx + Math.cos(a) * r * 0.92, cy + Math.sin(a) * r * 0.92);
      ctx.stroke();
    }
  } else if (kind === 'color') {
    [[-0.32, -0.18, '#ff7a59'], [0.32, -0.18, '#4cc6ff'], [0, 0.34, '#8be28b']].forEach(([dx, dy, c]) => {
      ctx.globalAlpha = 0.85; ctx.fillStyle = c;
      ctx.beginPath(); ctx.arc(cx + dx * r, cy + dy * r, r * 0.5, 0, 7); ctx.fill();
    });
  } else if (kind === 'film') {
    ctx.strokeRect(cx - r * 0.62, cy - r * 0.8, r * 1.24, r * 1.6);
    for (let k = 0; k < 4; k++) {
      ctx.fillRect(cx - r * 0.5, cy - r * 0.66 + k * r * 0.42, r * 0.14, r * 0.2);
      ctx.fillRect(cx + r * 0.36, cy - r * 0.66 + k * r * 0.42, r * 0.14, r * 0.2);
    }
  } else if (kind === 'mask') {
    ctx.beginPath(); ctx.arc(cx, cy, r * 0.8, 0, 7); ctx.stroke();
    ctx.beginPath(); ctx.arc(cx, cy, r * 0.8, -Math.PI / 2, Math.PI / 2); ctx.fill();
  } else if (kind === 'crop') {
    ctx.beginPath();
    ctx.moveTo(cx - r * 0.5, cy - r * 0.9); ctx.lineTo(cx - r * 0.5, cy + r * 0.5); ctx.lineTo(cx + r * 0.9, cy + r * 0.5);
    ctx.moveTo(cx - r * 0.9, cy - r * 0.5); ctx.lineTo(cx + r * 0.5, cy - r * 0.5); ctx.lineTo(cx + r * 0.5, cy + r * 0.9);
    ctx.stroke();
  }
  ctx.restore();
}

const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const smooth = (t) => { t = clamp(t); return t * t * (3 - 2 * t); };

function canvasOf(w, h) {
  const c = document.createElement('canvas');
  c.width = Math.max(1, Math.round(w)); c.height = Math.max(1, Math.round(h));
  return c;
}

/** Retouche les pixels d'une image : `f(r, g, b)` rend la couleur nouvelle (0 à 1). */
function grade(src, f) {
  const c = canvasOf(src.width, src.height);
  const ctx = c.getContext('2d');
  ctx.drawImage(src, 0, 0);
  const img = ctx.getImageData(0, 0, c.width, c.height), d = img.data;
  for (let i = 0; i < d.length; i += 4) {
    const o = f(d[i] / 255, d[i + 1] / 255, d[i + 2] / 255);
    d[i] = o[0] * 255; d[i + 1] = o[1] * 255; d[i + 2] = o[2] * 255;
  }
  ctx.putImageData(img, 0, 0);
  return c;
}

// Les trois états de la photo : le RAW tel qu'il sort du boîtier (terne, plat), la
// photo développée, et la même plus exposée (le curseur ou la molette poussés).
const asRaw = (r, g, b) => {
  const l = r * 0.3 + g * 0.59 + b * 0.11;
  const k = (v) => 0.2 + (l + (v - l) * 0.45 - 0.2) * 0.62;
  return [k(r), k(g), k(b)];
};
const brighter = (r, g, b) => [r, g, b].map((v) => 1 - Math.pow(1 - v, 1.7));
// Le ciel plus dense, comme sous un masque : moins d'exposition, plus de couleur.
const denser = (r, g, b) => {
  const l = r * 0.3 + g * 0.59 + b * 0.11;
  return [r, g, b].map((v) => clamp(Math.pow(clamp(l + (v - l) * 1.5), 1.45) * 0.95));
};

function photoStates(w, h, opts) {
  const edited = photoURL(w, h, opts);
  return { edited, raw: grade(edited, asRaw), bright: grade(edited, brighter) };
}

/** La flèche de la souris de macOS, sa pointe en (x, y). */
function pointer(ctx, x, y, k) {
  ctx.save();
  ctx.translate(x, y); ctx.scale(k, k);
  ctx.beginPath();
  ctx.moveTo(0, 0); ctx.lineTo(0, 17); ctx.lineTo(4, 13); ctx.lineTo(7, 20); ctx.lineTo(10, 19);
  ctx.lineTo(7, 12); ctx.lineTo(12, 12); ctx.closePath();
  ctx.fillStyle = '#000'; ctx.strokeStyle = '#fff'; ctx.lineWidth = 1.4; ctx.lineJoin = 'round';
  ctx.shadowColor = 'rgba(0,0,0,.45)'; ctx.shadowBlur = 4; ctx.shadowOffsetY = 1.5;
  ctx.stroke(); ctx.shadowColor = 'transparent'; ctx.fill(); ctx.stroke();
  ctx.restore();
}

/**
 * L'interface de LÜMA sur Mac : film, photo, panneau de réglages.
 * Rend `draw({ t, edit, split })` : `edit` (0 à 1) amène les curseurs du RAW brut à la
 * photo développée, `split` (0 à 1, ou moins pour rien) place la barre avant / après.
 */
export function macScreen(canvas, { chrome = 'mac' } = {}) {
  const ctx = canvas.getContext('2d');
  const W = canvas.width, H = canvas.height, s = W / 1280;
  const font = (w, px) => `${w} ${px * s}px -apple-system, Inter, sans-serif`;
  const px = 180 * s, py = 52 * s, pw = 790 * s, ph = H - 66 * s;
  const rx = 986 * s, rw = 280 * s;

  // Ce qui ne bouge pas : barre de titre, film, cadre.
  const base = canvasOf(W, H);
  const b = base.getContext('2d');
  b.fillStyle = '#0d0d12'; b.fillRect(0, 0, W, H);
  b.fillStyle = '#16161d'; b.fillRect(0, 0, W, 38 * s);
  if (chrome === 'windows') {
    // Réduire, agrandir, fermer : à droite, comme sous Windows.
    b.strokeStyle = 'rgba(255,255,255,.75)'; b.lineWidth = 1.4 * s;
    const cy = 19 * s, cx = W - 22 * s, d = 5 * s;
    b.beginPath(); b.moveTo(cx - d, cy - d); b.lineTo(cx + d, cy + d); b.moveTo(cx + d, cy - d); b.lineTo(cx - d, cy + d); b.stroke();
    b.strokeRect(cx - 46 * s - d, cy - d, 2 * d, 2 * d);
    b.beginPath(); b.moveTo(cx - 92 * s - d, cy); b.lineTo(cx - 92 * s + d, cy); b.stroke();
  } else {
    ['#ff5f57', '#febc2e', '#28c840'].forEach((c, i) => { b.fillStyle = c; b.beginPath(); b.arc((20 + i * 20) * s, 19 * s, 6 * s, 0, 7); b.fill(); });
  }
  b.fillStyle = '#fff'; b.font = font(600, 13);
  b.textAlign = 'center'; b.fillText(`LÜMA — ${tr('Islande_0427.ARW')}`, W / 2, 24 * s); b.textAlign = 'left';
  b.font = font(400, 12);
  const tabs = chrome === 'windows' ? 520 : 420;
  ['Bibliothèque', 'Développement', 'Masques', 'Export'].forEach((t, i) => {
    b.fillStyle = i === 1 ? '#fff' : 'rgba(255,255,255,.4)';
    b.fillText(tr(t), W - (tabs - i * 100) * s, 24 * s);
  });
  const fx = 14 * s, fw = 150 * s;
  for (let i = 0; i < 6; i++) {
    const ty = (52 + i * 104) * s;
    b.save(); roundRect(b, fx, ty, fw, 92 * s, 8 * s); b.clip();
    drawPhoto(b, fx, ty, fw, 92 * s, { hue: i * 47 - 40, seed: 2 + i, mono: i === 4 });
    b.restore();
    if (i === 0) { b.strokeStyle = '#fff'; b.lineWidth = 2 * s; roundRect(b, fx, ty, fw, 92 * s, 8 * s); b.stroke(); }
  }
  b.fillStyle = 'rgba(255,255,255,.35)'; b.font = `${10 * s}px ui-monospace, monospace`;
  b.fillText(chrome === 'windows' ? 'f32 · sRGB · Direct3D 12' : 'f32 · Display P3 · Metal', rx, H - 22 * s);
  const photo = photoStates(pw, ph, { seed: 3 });

  const LIGHT = [['Exposition', 0.18], ['Contraste', 0.22], ['Hautes lumières', -0.56], ['Ombres', 0.34], ['Blancs', 0.08], ['Noirs', -0.12]];

  return function draw({ t = 0, edit = 1, split = -1 } = {}) {
    const e = smooth(edit);
    // L'exposition, glissée à la souris d'un bout à l'autre, sans fin.
    const drag = 0.5 - 0.5 * Math.cos(t * 0.75);
    const expo = (0.08 + 0.3 * drag) * e;
    ctx.drawImage(base, 0, 0);

    ctx.save(); roundRect(ctx, px, py, pw, ph, 10 * s); ctx.clip();
    ctx.drawImage(photo.raw, px, py);
    ctx.globalAlpha = e; ctx.drawImage(photo.edited, px, py);
    ctx.globalAlpha = clamp((expo - 0.08) / 0.3) * 0.75; ctx.drawImage(photo.bright, px, py);
    ctx.globalAlpha = 1;
    if (split >= 0) {
      // Avant / après : le RAW à gauche de la barre.
      const sx = px + pw * split;
      ctx.save(); ctx.beginPath(); ctx.rect(px, py, sx - px, ph); ctx.clip();
      ctx.drawImage(photo.raw, px, py);
      ctx.restore();
      ctx.fillStyle = '#fff'; ctx.fillRect(sx - 1 * s, py, 2 * s, ph);
      ctx.beginPath(); ctx.arc(sx, py + ph / 2, 16 * s, 0, 7); ctx.fill();
      ctx.fillStyle = '#111'; ctx.beginPath();
      ctx.moveTo(sx - 4 * s, py + ph / 2 - 6 * s); ctx.lineTo(sx - 10 * s, py + ph / 2); ctx.lineTo(sx - 4 * s, py + ph / 2 + 6 * s);
      ctx.moveTo(sx + 4 * s, py + ph / 2 - 6 * s); ctx.lineTo(sx + 10 * s, py + ph / 2); ctx.lineTo(sx + 4 * s, py + ph / 2 + 6 * s);
      ctx.fill();
      ctx.font = font(600, 12);
      [[tr('Avant'), px + 14 * s, split > 0.12], [tr('Après'), px + pw - 14 * s, split < 0.88]].forEach(([label, x, on], k) => {
        if (!on) return;
        ctx.textAlign = k ? 'right' : 'left';
        const tw = ctx.measureText(label).width;
        ctx.fillStyle = 'rgba(10,10,14,.7)';
        roundRect(ctx, k ? x - tw - 20 * s : x, py + 14 * s, tw + 20 * s, 24 * s, 12 * s); ctx.fill();
        ctx.fillStyle = '#fff'; ctx.fillText(label, k ? x - 10 * s : x + 10 * s, py + 30 * s);
      });
      ctx.textAlign = 'left';
    }
    ctx.restore();

    // Le panneau : histogramme et curseurs, qui suivent la retouche.
    ctx.fillStyle = '#0d0d12'; ctx.fillRect(rx - 4 * s, 46 * s, rw + 8 * s, 600 * s);
    histogram(ctx, rx, 52 * s, rw, 90 * s, expo * 0.25 - (1 - e) * 0.12);
    ctx.fillStyle = '#fff'; ctx.font = font(600, 13);
    ctx.fillText(tr('Lumière'), rx, 172 * s);
    LIGHT.forEach(([l, v], i) => slider(ctx, rx, (196 + i * 40) * s, rw, tr(l), i === 0 ? expo : v * e, s));
    ctx.fillStyle = '#fff'; ctx.font = font(600, 13);
    ctx.fillText(tr('Couleur'), rx, 452 * s);
    slider(ctx, rx, 476 * s, rw, tr('Température'), 0.24 * e, s, ['#4aa8ff', '#ddd', '#ffb03b']);
    slider(ctx, rx, 516 * s, rw, tr('Teinte'), 0.06 * e, s, ['#34c759', '#ddd', '#e040fb']);
    slider(ctx, rx, 556 * s, rw, tr('Vibrance'), 0.3 * e, s);
    slider(ctx, rx, 596 * s, rw, tr('Saturation'), -0.08 * e, s);
    // La souris tient le curseur d'exposition.
    if (e > 0.98) pointer(ctx, rx + rw * (0.5 + expo / 2) + 2 * s, 200 * s, 1.15 * s);
  };
}

/**
 * L'écran de l'iPhone : LÜMA en retouche (la Dynamic Island est dessinée par la page).
 * Rend `draw({ t })` : le pouce fait tourner la molette d'exposition, la photo suit.
 */
export function phoneScreen(canvas) {
  const ctx = canvas.getContext('2d');
  const W = canvas.width, H = canvas.height, s = W / 390;
  const font = (w, px) => `${w} ${px * s}px -apple-system, "SF Pro Text", Inter, sans-serif`;
  const py = 116 * s, ph = 470 * s, dy = 640 * s;

  const base = canvasOf(W, H);
  const b = base.getContext('2d');
  b.fillStyle = '#000'; b.fillRect(0, 0, W, H);
  // Barre d'état : l'heure, le réseau, le Wi‑Fi, la batterie.
  b.fillStyle = '#fff'; b.font = font(600, 17);
  b.textAlign = 'center'; b.fillText('9:41', 66 * s, 40 * s); b.textAlign = 'left';
  [5, 8, 11, 14].forEach((h, k) => { roundRect(b, (296 + k * 6) * s, (40 - h) * s, 4 * s, h * s, 1 * s); b.fill(); });
  b.lineWidth = 2.2 * s; b.strokeStyle = '#fff';
  [5, 9, 13].forEach((r) => { b.beginPath(); b.arc(333 * s, 41 * s, r * s, -Math.PI * 0.78, -Math.PI * 0.22); b.stroke(); });
  b.lineWidth = 1.2 * s; b.globalAlpha = 0.45; roundRect(b, 347 * s, 29 * s, 25 * s, 12 * s, 3.5 * s); b.stroke(); b.globalAlpha = 1;
  roundRect(b, 349 * s, 31 * s, 18 * s, 8 * s, 2 * s); b.fill();
  // En‑tête : retour à la bibliothèque, le nom de la photo, Exporter.
  b.font = font(400, 17); b.fillStyle = '#ffc466';
  b.fillText('‹ ' + tr('Bibliothèque'), 14 * s, 92 * s);
  b.fillStyle = '#fff'; b.font = font(600, 15); b.textAlign = 'center';
  b.fillText(tr('Islande_0427.ARW').replace('.ARW', ''), W / 2 + 40 * s, 92 * s);
  b.fillStyle = 'rgba(255,196,102,.16)'; roundRect(b, 300 * s, 72 * s, 76 * s, 30 * s, 15 * s); b.fill();
  b.fillStyle = '#ffc466'; b.font = font(600, 14);
  b.fillText(tr('Export'), 338 * s, 92 * s);
  b.fillStyle = 'rgba(255,255,255,.6)'; b.font = font(600, 12);
  b.fillText(tr('Exposition').toUpperCase(), W / 2, dy);
  b.textAlign = 'left';
  // Les outils : icône et nom ; Lumière est actif.
  const tools = [['light', 'Lumière'], ['color', 'Couleur'], ['film', 'Films'], ['mask', 'Masques'], ['crop', 'Recadrer']];
  const ty = H - 108 * s;
  tools.forEach(([kind, name], k) => {
    const cx = (W / 5) * (k + 0.5);
    const on = k === 0;
    if (on) { b.fillStyle = 'rgba(255,196,102,.16)'; b.beginPath(); b.arc(cx, ty, 25 * s, 0, 7); b.fill(); }
    toolIcon(b, kind, cx, ty, 13 * s, on ? '#ffc466' : 'rgba(255,255,255,.85)');
    b.fillStyle = on ? '#ffc466' : 'rgba(255,255,255,.6)'; b.font = font(500, 11); b.textAlign = 'center';
    b.fillText(tr(name), cx, ty + 44 * s); b.textAlign = 'left';
  });
  b.fillStyle = 'rgba(255,255,255,.75)'; roundRect(b, W / 2 - 67 * s, H - 18 * s, 134 * s, 5 * s, 2.5 * s); b.fill();

  const photo = photoStates(W + 300 * s, ph, { seed: 5, hue: -12 });
  // Le masque de ciel : un voile violet sur le ciel.
  const sky = new Path2D();
  sky.moveTo(0, py);
  for (let x = 0; x <= W; x += 4) sky.lineTo(x, py + (238 - Math.abs(ridge(x / W, 6, 0.09, 4)) * 300) * s);
  sky.lineTo(W, py); sky.closePath();

  return function draw({ t = 0 } = {}) {
    // Le pouce pousse la molette, la lâche, la reprend : de +0,10 à +0,60.
    const k = smooth(0.5 - 0.5 * Math.cos(t * 0.6));
    const v = 0.1 + 0.5 * k;
    ctx.drawImage(base, 0, 0);
    ctx.save(); ctx.beginPath(); ctx.rect(0, py, W, ph); ctx.clip();
    ctx.drawImage(photo.edited, -150 * s, py);
    ctx.globalAlpha = k * 0.85; ctx.drawImage(photo.bright, -150 * s, py);
    ctx.globalAlpha = 0.16 + 0.08 * Math.sin(t * 2.2);
    ctx.fillStyle = '#8b7bff'; ctx.fill(sky);
    ctx.globalAlpha = 1;
    ctx.restore();
    ctx.fillStyle = 'rgba(20,20,26,.72)'; roundRect(ctx, 14 * s, py + 14 * s, 76 * s, 28 * s, 14 * s); ctx.fill();
    ctx.fillStyle = '#8b7bff'; ctx.beginPath(); ctx.arc(30 * s, py + 28 * s, 5 * s, 0, 7); ctx.fill();
    ctx.fillStyle = '#fff'; ctx.font = font(600, 13); ctx.fillText(tr('Ciel'), 42 * s, py + 33 * s);

    // La valeur et la molette graduée, qui défile sous le repère.
    ctx.textAlign = 'center';
    ctx.fillStyle = '#fff'; ctx.font = font(600, 22);
    const value = (v >= 0 ? '+' : '') + v.toFixed(2);
    ctx.fillText(document.documentElement.lang === 'en' ? value : value.replace('.', ','), W / 2, dy + 30 * s);
    ctx.textAlign = 'left';
    const at = v * 100;
    for (let j = Math.floor(at) - 26; j <= at + 26; j++) {
      const x = W / 2 + (j - at) * 8 * s;
      if (x < 0 || x > W) continue;
      const major = j % 5 === 0;
      const fade = 1 - Math.abs(x - W / 2) / (W / 2);
      ctx.fillStyle = `rgba(255,255,255,${(major ? 0.75 : 0.32) * fade})`;
      ctx.fillRect(x - 0.7 * s, dy + (major ? 46 : 50) * s, 1.4 * s, (major ? 18 : 12) * s);
    }
    ctx.fillStyle = '#ffc466'; roundRect(ctx, W / 2 - 1.5 * s, dy + 40 * s, 3 * s, 30 * s, 1.5 * s); ctx.fill();
  };
}

// Le trait de l'Apple Pencil sur l'iPad : des allers et retours sur le ciel, en
// coordonnées de la photo (0 à 1).
const STROKE = [
  [0.08, 0.1], [0.3, 0.07], [0.52, 0.12], [0.78, 0.08], [0.94, 0.14],
  [0.9, 0.27], [0.66, 0.22], [0.42, 0.28], [0.18, 0.24], [0.05, 0.32],
  [0.12, 0.42], [0.34, 0.38], [0.56, 0.43], [0.76, 0.37], [0.93, 0.42],
];
const STROKE_LEN = (() => {
  const l = [0];
  for (let i = 1; i < STROKE.length; i++) l.push(l[i - 1] + Math.hypot(STROKE[i][0] - STROKE[i - 1][0], STROKE[i][1] - STROKE[i - 1][1]));
  return l;
})();
function along(q) {
  const d = clamp(q) * STROKE_LEN[STROKE_LEN.length - 1];
  let i = 1;
  while (i < STROKE.length - 1 && STROKE_LEN[i] < d) i++;
  const f = (d - STROKE_LEN[i - 1]) / (STROKE_LEN[i] - STROKE_LEN[i - 1] || 1);
  // Un léger tremblé de la main.
  const w = Math.sin(d * 40) * 0.006;
  return [STROKE[i - 1][0] + (STROKE[i][0] - STROKE[i - 1][0]) * f, STROKE[i - 1][1] + (STROKE[i][1] - STROKE[i - 1][1]) * f + w];
}

/**
 * L'iPad, à l'horizontale : LÜMA dans l'onglet Masques, un pinceau peint à l'Apple
 * Pencil sur le ciel. Rend `{ draw({ q, fade }), tip(q) }` : `q` (0 à 1) est l'avancée
 * du trait, `fade` (0 à 1) efface le voile du masque ; `tip` donne la pointe du crayon
 * en fractions de l'écran.
 */
export function padScreen(canvas) {
  const ctx = canvas.getContext('2d');
  const W = canvas.width, H = canvas.height, s = W / 1366;
  const font = (w, px) => `${w} ${px * s}px -apple-system, "SF Pro Text", Inter, sans-serif`;
  const px = 96 * s, py = 92 * s, pw = 954 * s, ph = 770 * s;
  const rx = 1070 * s, rw = 276 * s;

  const base = canvasOf(W, H);
  const b = base.getContext('2d');
  b.fillStyle = '#0b0b10'; b.fillRect(0, 0, W, H);
  // Barre d'état.
  b.fillStyle = '#fff'; b.font = font(600, 14);
  b.fillText('9:41', 22 * s, 22 * s);
  b.textAlign = 'right'; b.fillText('100 %', W - 56 * s, 22 * s); b.textAlign = 'left';
  b.lineWidth = 1.2 * s; b.strokeStyle = 'rgba(255,255,255,.5)'; roundRect(b, W - 48 * s, 11 * s, 26 * s, 13 * s, 3.5 * s); b.stroke();
  b.fillStyle = '#fff'; roundRect(b, W - 46 * s, 13 * s, 22 * s, 9 * s, 2 * s); b.fill();
  // En‑tête.
  b.font = font(400, 17); b.fillStyle = '#ffc466';
  b.fillText('‹ ' + tr('Bibliothèque'), 22 * s, 66 * s);
  b.fillStyle = '#fff'; b.font = font(600, 16); b.textAlign = 'center';
  b.fillText(tr('Islande_0427.ARW'), px + pw / 2, 66 * s);
  b.fillStyle = 'rgba(255,196,102,.16)'; roundRect(b, W - 112 * s, 46 * s, 90 * s, 30 * s, 15 * s); b.fill();
  b.fillStyle = '#ffc466'; b.font = font(600, 14); b.fillText(tr('Export'), W - 67 * s, 66 * s);
  b.textAlign = 'left';
  // Les outils, en colonne ; Masques est actif.
  [['light', 'Lumière'], ['color', 'Couleur'], ['film', 'Films'], ['mask', 'Masques'], ['crop', 'Recadrer']].forEach(([kind, name], k) => {
    const cx = 48 * s, cy = (130 + k * 84) * s;
    const on = kind === 'mask';
    if (on) { b.fillStyle = 'rgba(139,123,255,.2)'; b.beginPath(); b.arc(cx, cy, 24 * s, 0, 7); b.fill(); }
    toolIcon(b, kind, cx, cy, 12 * s, on ? '#a99cff' : 'rgba(255,255,255,.8)');
    b.fillStyle = on ? '#a99cff' : 'rgba(255,255,255,.55)'; b.font = font(500, 10); b.textAlign = 'center';
    b.fillText(tr(name), cx, cy + 38 * s); b.textAlign = 'left';
  });
  // Le film, sous la photo.
  for (let i = 0; i < 7; i++) {
    const tw = (pw - 6 * 10 * s) / 7, tx = px + i * (tw + 10 * s), ty = py + ph + 18 * s;
    b.save(); roundRect(b, tx, ty, tw, 92 * s, 8 * s); b.clip();
    drawPhoto(b, tx, ty, tw, 92 * s, { hue: i * 41 - 30, seed: 3 + i, mono: i === 5 });
    b.restore();
    if (i === 0) { b.strokeStyle = '#fff'; b.lineWidth = 2 * s; roundRect(b, tx, ty, tw, 92 * s, 8 * s); b.stroke(); }
  }
  // Le panneau des masques.
  b.fillStyle = '#fff'; b.font = font(600, 15);
  b.fillText(tr('Masques'), rx, 118 * s);
  [[tr('Ciel') + ' · ' + tr('Pinceau'), '#8b7bff', true], [tr('Sujet'), '#4cc6ff', false]].forEach(([name, c, on], k) => {
    const y = (136 + k * 52) * s;
    b.fillStyle = on ? 'rgba(139,123,255,.16)' : 'rgba(255,255,255,.05)';
    roundRect(b, rx, y, rw, 42 * s, 12 * s); b.fill();
    if (on) { b.strokeStyle = 'rgba(169,156,255,.6)'; b.lineWidth = 1.2 * s; roundRect(b, rx, y, rw, 42 * s, 12 * s); b.stroke(); }
    b.fillStyle = c; b.beginPath(); b.arc(rx + 20 * s, y + 21 * s, 6 * s, 0, 7); b.fill();
    b.fillStyle = '#fff'; b.font = font(on ? 600 : 400, 14); b.fillText(name, rx + 36 * s, y + 26 * s);
  });
  // Pinceau ou gomme.
  const ty = 262 * s;
  b.fillStyle = 'rgba(255,255,255,.07)'; roundRect(b, rx, ty, rw, 34 * s, 10 * s); b.fill();
  b.fillStyle = 'rgba(255,255,255,.16)'; roundRect(b, rx + 3 * s, ty + 3 * s, rw / 2 - 3 * s, 28 * s, 8 * s); b.fill();
  b.font = font(600, 13); b.textAlign = 'center';
  b.fillStyle = '#fff'; b.fillText(tr('Pinceau'), rx + rw / 4, ty + 22 * s);
  b.fillStyle = 'rgba(255,255,255,.55)'; b.fillText(tr('Gomme'), rx + rw * 0.75, ty + 22 * s);
  b.textAlign = 'left';
  [['Taille', 0.1], ['Intensité', 0.6]].forEach(([l, v], i) => slider(b, rx, (330 + i * 44) * s, rw, tr(l), v, s * 1.05));
  b.fillStyle = '#fff'; b.font = font(600, 15);
  b.fillText(tr('Lumière'), rx, 446 * s);
  [['Exposition', -0.42], ['Contraste', 0.26], ['Clarté', 0.18], ['Saturation', 0.32]].forEach(([l, v], i) =>
    slider(b, rx, (474 + i * 46) * s, rw, tr(l), v, s * 1.05, i === 0 ? ['#4a4a55', '#ddd', '#fff'] : null));
  b.fillStyle = 'rgba(255,255,255,.35)'; b.font = `${10 * s}px ui-monospace, monospace`;
  b.fillText('Apple Pencil · f32 · Display P3', rx, H - 26 * s);

  const photo = photoStates(pw, ph, { seed: 7, hue: 8 });
  const sky = grade(photo.edited, denser);
  // Le masque, peint dab après dab dans sa propre image (fondu au bord).
  const mask = canvasOf(pw, ph), m = mask.getContext('2d');
  const tmp = canvasOf(pw, ph), tc = tmp.getContext('2d');
  const brush = 74 * s;
  let painted = 0;
  function paintTo(q) {
    if (q < painted) { m.clearRect(0, 0, pw, ph); painted = 0; }
    const step = 0.004;
    for (let u = painted; u <= q; u += step) {
      const [x, y] = along(u);
      const g = m.createRadialGradient(x * pw, y * ph, 0, x * pw, y * ph, brush);
      g.addColorStop(0, 'rgba(0,0,0,.5)'); g.addColorStop(0.55, 'rgba(0,0,0,.35)'); g.addColorStop(1, 'rgba(0,0,0,0)');
      m.fillStyle = g; m.fillRect(x * pw - brush, y * ph - brush, brush * 2, brush * 2);
    }
    painted = Math.max(painted, q);
  }

  function draw({ q = 1, fade = 0 } = {}) {
    paintTo(q);
    ctx.drawImage(base, 0, 0);
    ctx.save(); roundRect(ctx, px, py, pw, ph, 12 * s); ctx.clip();
    ctx.drawImage(photo.edited, px, py);
    // Le ciel retouché, là où le crayon est passé.
    tc.globalCompositeOperation = 'copy'; tc.drawImage(sky, 0, 0);
    tc.globalCompositeOperation = 'destination-in'; tc.drawImage(mask, 0, 0);
    ctx.drawImage(tmp, px, py);
    // Le voile violet qui montre la zone du masque.
    if (fade < 1) {
      tc.globalCompositeOperation = 'copy'; tc.fillStyle = '#8b7bff'; tc.fillRect(0, 0, pw, ph);
      tc.globalCompositeOperation = 'destination-in'; tc.drawImage(mask, 0, 0);
      ctx.globalAlpha = 0.62 * (1 - fade); ctx.drawImage(tmp, px, py); ctx.globalAlpha = 1;
    }
    ctx.restore();
  }
  // La pointe du crayon, en fractions de l'écran.
  function tip(q) {
    const [x, y] = along(q);
    return [(px + x * pw) / W, (py + y * ph) / H];
  }
  return { draw, tip };
}
