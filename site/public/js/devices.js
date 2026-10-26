// Section Apple : un MacBook Pro dont l'écran s'ouvre au défilement, puis un iPhone qui
// vient se poser devant, tous deux sur l'application LÜMA.
//
// Les appareils sont dessinés en HTML et CSS (proportions des vrais : écran 16:10 à
// bords fins et encoche, iPhone à Dynamic Island) ; leurs écrans sont des canvas
// peints par `screen.js`. Une perspective CSS suffit au mouvement : pas de 3D à charger.


const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const lerp = (a, b, t) => a + (b - a) * t;
const ease = (t) => 1 - Math.pow(1 - clamp(t), 3);
const range = (p, a, b) => clamp((p - a) / (b - a));

function el(tag, cls, parent) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (parent) parent.appendChild(e);
  return e;
}

function screen(parent, cls, w, h, draw) {
  const c = el('canvas', cls, parent);
  c.width = w; c.height = h;
  draw(c);
  return c;
}

/** Le MacBook Pro : l'écran (bord noir fin, encoche) sur sa charnière, et la base. */
function macOf(stage, drawDesktopUI) {
  const mac = el('div', 'mbp', stage);
  const lid = el('div', 'mbp-lid', mac);
  const back = el('div', 'mbp-back', lid);
  el('div', 'mbp-logo', back);
  const bezel = el('div', 'mbp-bezel', lid);
  const display = screen(bezel, 'mbp-screen', 1280, 820, drawDesktopUI);
  el('div', 'mbp-notch', bezel);
  el('div', 'mbp-glare', bezel);
  const base = el('div', 'mbp-base', mac);
  el('div', 'mbp-lip', base);
  el('div', 'mbp-shadow', mac);
  return { mac, lid, display };
}

/** L'iPhone, de face : cadre en titane, Dynamic Island, boutons. */
function phoneOf(stage, drawPhoneUI) {
  const phone = el('div', 'iph', stage);
  el('i', 'iph-btn iph-action', phone);
  el('i', 'iph-btn iph-up', phone);
  el('i', 'iph-btn iph-down', phone);
  el('i', 'iph-btn iph-power', phone);
  const glass = el('div', 'iph-glass', phone);
  screen(glass, 'iph-screen', 780, 1688, drawPhoneUI);
  el('div', 'iph-island', glass);
  el('div', 'iph-glare', glass);
  return phone;
}

export async function initDevices(stage, section, { reduced = false } = {}) {
  // Les écrans, dans la même version que ce fichier.
  const { drawDesktopUI, drawPhoneUI } = await import(`./screen.js${new URL(import.meta.url).search}`);
  stage.classList.add('devices-ready');
  const buildMac = (s) => macOf(s, drawDesktopUI);
  const buildPhone = (s) => phoneOf(s, drawPhoneUI);
  const { mac, lid, display } = buildMac(stage);
  const phone = buildPhone(stage);
  // Sur téléphone, les appareils finissent bien de face : un plan incliné y serait
  // rastérisé en basse définition.
  const flat = matchMedia('(max-width: 1000px)').matches;

  const panels = [...section.querySelectorAll('.apple-panel')];
  const dots = [...section.querySelectorAll('.apple-dots i')];
  let visible = false, sp = 0, mx = 0, tmx = 0, current = -1, drawn = '';
  new IntersectionObserver(([e]) => { visible = e.isIntersecting; }).observe(section);
  window.addEventListener('pointermove', (e) => { tmx = (e.clientX / window.innerWidth) * 2 - 1; }, { passive: true });

  function progress() {
    const r = section.getBoundingClientRect();
    return clamp(-r.top / (r.height - window.innerHeight));
  }

  function frame() {
    requestAnimationFrame(frame);
    if (!visible) return;
    const p = progress();
    sp = reduced || Math.abs(p - sp) < 1e-4 ? p : lerp(sp, p, 0.1);
    mx = Math.abs(tmx - mx) < 1e-4 ? tmx : lerp(mx, tmx, 0.05);
    const key = `${sp.toFixed(4)}|${mx.toFixed(4)}`;
    if (key === drawn) return;
    drawn = key;

    const step = sp < 0.36 ? 0 : sp < 0.68 ? 1 : 2;
    if (step !== current) {
      current = step;
      panels.forEach((x, i) => x.classList.toggle('on', i === step));
    }
    dots.forEach((d, i) => d.style.setProperty('--f', clamp(sp * 3 - i).toFixed(3)));

    // Le Mac : l'écran se relève (de rabattu à légèrement incliné), l'image s'allume.
    const open = ease(range(sp, 0.02, 0.4));
    lid.style.transform = `rotateX(${lerp(-86, flat ? 0 : 4, open).toFixed(2)}deg)`;
    display.style.opacity = clamp((open - 0.45) / 0.4).toFixed(3);
    // Puis il recule un peu pour laisser la place à l'iPhone.
    const shift = ease(range(sp, 0.6, 0.95));
    mac.style.transform =
      `translate3d(${lerp(0, -4, shift).toFixed(2)}%, 0, 0) ` +
      (flat
        ? `rotateX(${lerp(14, 0, open).toFixed(2)}deg) `
        : `rotateX(${lerp(14, 8, open).toFixed(2)}deg) rotateY(${(lerp(-14, -6, open) + mx * 4 + shift * 6).toFixed(2)}deg) `) +
      `scale(${lerp(1, 0.94, shift).toFixed(3)})`;

    // L'iPhone : arrive par la droite et se pose devant le coin du Mac.
    const arrive = ease(range(sp, 0.55, 0.85));
    phone.style.opacity = clamp(arrive * 1.6).toFixed(3);
    phone.style.transform =
      `translate3d(${lerp(60, 0, arrive).toFixed(2)}%, ${lerp(18, 0, arrive).toFixed(2)}%, 0) ` +
      `rotateY(${(lerp(-30, flat ? 0 : -6, arrive) + (flat ? 0 : mx * 4)).toFixed(2)}deg) rotateZ(${lerp(8, flat ? 0 : 1.5, arrive).toFixed(2)}deg)`;
  }
  requestAnimationFrame(frame);
}
