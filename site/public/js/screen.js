// Images dessinées au canvas 2D : une photo de paysage procédurale (aucun fichier à
// charger) et les écrans de l'application LÜMA affichés sur le Mac et l'iPhone 3D.

// Les libellés des écrans, dans la langue de la page (`<html lang>`) ; le français sinon.
const LABELS = {
  en: {
    'Bibliothèque': 'Library', 'Développement': 'Develop', 'Masques': 'Masks', 'Export': 'Export',
    'Lumière': 'Light', 'Exposition': 'Exposure', 'Contraste': 'Contrast', 'Hautes lumières': 'Highlights',
    'Ombres': 'Shadows', 'Blancs': 'Whites', 'Noirs': 'Blacks', 'Couleur': 'Color', 'Température': 'Temperature',
    'Teinte': 'Tint', 'Vibrance': 'Vibrance', 'Saturation': 'Saturation', 'Ciel · IA': 'Sky · AI',
    'Islande_0427.ARW': 'Iceland_0427.ARW',
  },
  es: {
    'Bibliothèque': 'Biblioteca', 'Développement': 'Revelado', 'Masques': 'Máscaras', 'Export': 'Exportar',
    'Lumière': 'Luz', 'Exposition': 'Exposición', 'Contraste': 'Contraste', 'Hautes lumières': 'Altas luces',
    'Ombres': 'Sombras', 'Blancs': 'Blancos', 'Noirs': 'Negros', 'Couleur': 'Color',
    'Température': 'Temperatura', 'Teinte': 'Matiz', 'Vibrance': 'Intensidad', 'Saturation': 'Saturación',
    'Ciel · IA': 'Cielo · IA', 'Islande_0427.ARW': 'Islandia_0427.ARW',
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

function histogram(ctx, x, y, w, h) {
  ctx.fillStyle = 'rgba(255,255,255,.04)';
  roundRect(ctx, x, y, w, h, 8); ctx.fill();
  ctx.globalCompositeOperation = 'lighter';
  [['rgba(255,70,70,.55)', 0.3, 0.2], ['rgba(70,255,140,.5)', 0.45, 0.4], ['rgba(70,140,255,.55)', 0.6, 0.65]].forEach(([c, m, k]) => {
    ctx.fillStyle = c;
    ctx.beginPath(); ctx.moveTo(x, y + h);
    for (let i = 0; i <= 80; i++) {
      const t = i / 80;
      const v = Math.exp(-Math.pow((t - m) / 0.16, 2)) * 0.8 + Math.exp(-Math.pow((t - k - 0.2) / 0.07, 2)) * 0.45 + 0.05 * Math.sin(t * 40 + m * 9) ** 2;
      ctx.lineTo(x + t * w, y + h - v * h * 0.9);
    }
    ctx.lineTo(x + w, y + h); ctx.fill();
  });
  ctx.globalCompositeOperation = 'source-over';
}

/** L'interface de LÜMA sur Mac : film, photo, panneau de réglages. */
export function drawDesktopUI(canvas) {
  const ctx = canvas.getContext('2d');
  const W = canvas.width, H = canvas.height, s = W / 1280;
  ctx.fillStyle = '#0d0d12'; ctx.fillRect(0, 0, W, H);

  // Barre de titre
  ctx.fillStyle = '#16161d'; ctx.fillRect(0, 0, W, 38 * s);
  ['#ff5f57', '#febc2e', '#28c840'].forEach((c, i) => { ctx.fillStyle = c; ctx.beginPath(); ctx.arc((20 + i * 20) * s, 19 * s, 6 * s, 0, 7); ctx.fill(); });
  ctx.fillStyle = '#fff'; ctx.font = `600 ${13 * s}px -apple-system, Inter, sans-serif`;
  ctx.textAlign = 'center'; ctx.fillText(`LÜMA — ${tr('Islande_0427.ARW')}`, W / 2, 24 * s); ctx.textAlign = 'left';
  ctx.font = `${12 * s}px -apple-system, Inter, sans-serif`;
  ['Bibliothèque', 'Développement', 'Masques', 'Export'].forEach((t, i) => {
    ctx.fillStyle = i === 1 ? '#fff' : 'rgba(255,255,255,.4)';
    ctx.fillText(tr(t), W - (420 - i * 100) * s, 24 * s);
  });

  // Film
  const fx = 14 * s, fw = 150 * s;
  for (let i = 0; i < 6; i++) {
    const ty = (52 + i * 104) * s;
    ctx.save(); roundRect(ctx, fx, ty, fw, 92 * s, 8 * s); ctx.clip();
    drawPhoto(ctx, fx, ty, fw, 92 * s, { hue: i * 47 - 40, seed: 2 + i, mono: i === 4 });
    ctx.restore();
    if (i === 0) { ctx.strokeStyle = '#fff'; ctx.lineWidth = 2 * s; roundRect(ctx, fx, ty, fw, 92 * s, 8 * s); ctx.stroke(); }
  }

  // Photo principale
  const px = 180 * s, py = 52 * s, pw = 790 * s, ph = H - 66 * s;
  ctx.save(); roundRect(ctx, px, py, pw, ph, 10 * s); ctx.clip();
  drawPhoto(ctx, px, py, pw, ph, { seed: 3 });
  ctx.restore();

  // Panneau
  const rx = 986 * s, rw = 280 * s;
  histogram(ctx, rx, 52 * s, rw, 90 * s);
  ctx.fillStyle = '#fff'; ctx.font = `600 ${13 * s}px -apple-system, Inter, sans-serif`;
  ctx.fillText(tr('Lumière'), rx, 172 * s);
  [['Exposition', 0.18], ['Contraste', 0.22], ['Hautes lumières', -0.56], ['Ombres', 0.34], ['Blancs', 0.08], ['Noirs', -0.12]]
    .forEach(([l, v], i) => slider(ctx, rx, (196 + i * 40) * s, rw, tr(l), v, s));
  ctx.fillStyle = '#fff'; ctx.font = `600 ${13 * s}px -apple-system, Inter, sans-serif`;
  ctx.fillText(tr('Couleur'), rx, 452 * s);
  slider(ctx, rx, 476 * s, rw, tr('Température'), 0.24, s, ['#4aa8ff', '#ddd', '#ffb03b']);
  slider(ctx, rx, 516 * s, rw, tr('Teinte'), 0.06, s, ['#34c759', '#ddd', '#e040fb']);
  slider(ctx, rx, 556 * s, rw, tr('Vibrance'), 0.3, s);
  slider(ctx, rx, 596 * s, rw, tr('Saturation'), -0.08, s);
  ctx.fillStyle = 'rgba(255,255,255,.35)'; ctx.font = `${10 * s}px ui-monospace, monospace`;
  ctx.fillText('f32 · Display P3 · Metal', rx, H - 22 * s);
}

/** L'interface de LÜMA sur iPhone. */
export function drawPhoneUI(canvas) {
  const ctx = canvas.getContext('2d');
  const W = canvas.width, H = canvas.height, s = W / 390;
  ctx.fillStyle = '#000'; ctx.fillRect(0, 0, W, H);
  ctx.fillStyle = '#fff'; ctx.font = `600 ${15 * s}px -apple-system, Inter, sans-serif`;
  ctx.fillText('9:41', 34 * s, 36 * s);
  ctx.fillRect(W - 62 * s, 25 * s, 24 * s, 11 * s);
  ctx.fillStyle = '#000'; roundRect(ctx, W / 2 - 62 * s, 14 * s, 124 * s, 34 * s, 17 * s); ctx.fill();

  ctx.fillStyle = '#fff'; ctx.font = `700 ${22 * s}px -apple-system, Inter, sans-serif`;
  ctx.fillText('LÜMA', 22 * s, 92 * s);
  ctx.font = `${13 * s}px -apple-system, Inter, sans-serif`; ctx.fillStyle = 'rgba(255,255,255,.5)';
  ctx.fillText('ProRAW · 48 Mpx', 22 * s, 112 * s);

  ctx.save(); roundRect(ctx, 0, 130 * s, W, 470 * s, 0); ctx.clip();
  drawPhoto(ctx, -60 * s, 130 * s, W + 120 * s, 470 * s, { seed: 5, hue: -12 });
  ctx.restore();

  // Masque de ciel (sélection IA) dessiné comme au Pencil
  ctx.strokeStyle = 'rgba(139,123,255,.9)'; ctx.setLineDash([6 * s, 5 * s]); ctx.lineWidth = 2 * s;
  ctx.beginPath(); ctx.moveTo(0, 360 * s);
  for (let x = 0; x <= W; x += 6) ctx.lineTo(x, (380 - Math.abs(ridge(x / W, 6, 0.09, 4)) * 470) * s);
  ctx.stroke(); ctx.setLineDash([]);
  ctx.fillStyle = 'rgba(139,123,255,.9)'; roundRect(ctx, 16 * s, 146 * s, 92 * s, 26 * s, 13 * s); ctx.fill();
  ctx.fillStyle = '#fff'; ctx.font = `600 ${12 * s}px -apple-system, Inter, sans-serif`; ctx.fillText(tr('Ciel · IA'), 30 * s, 163 * s);

  slider(ctx, 24 * s, 636 * s, W - 48 * s, tr('Exposition'), 0.22, s);
  slider(ctx, 24 * s, 682 * s, W - 48 * s, tr('Température'), 0.3, s, ['#4aa8ff', '#ddd', '#ffb03b']);
  const tools = ['☀', '◐', '◍', '✎', '⬚'];
  tools.forEach((t, i) => {
    const cx = (W / 6) * (i + 1), cy = H - 70 * s;
    ctx.fillStyle = i === 0 ? '#fff' : 'rgba(255,255,255,.1)';
    ctx.beginPath(); ctx.arc(cx, cy, 22 * s, 0, 7); ctx.fill();
    ctx.fillStyle = i === 0 ? '#000' : '#fff'; ctx.font = `${17 * s}px -apple-system, sans-serif`; ctx.textAlign = 'center';
    ctx.fillText(t, cx, cy + 6 * s); ctx.textAlign = 'left';
  });
  ctx.fillStyle = 'rgba(255,255,255,.6)'; roundRect(ctx, W / 2 - 68 * s, H - 16 * s, 136 * s, 5 * s, 3 * s); ctx.fill();
}
