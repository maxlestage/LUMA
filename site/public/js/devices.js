// Section Apple : un MacBook Pro dont l'écran s'ouvre au défilement, puis un iPhone qui
// vient se poser devant, puis un iPad où l'Apple Pencil peint un masque, tous trois sur
// l'application LÜMA.
//
// Les appareils sont dessinés en HTML et CSS (proportions des vrais : écran 16:10 à
// bords fins et encoche, iPhone à Dynamic Island, iPad Pro 13 pouces et son crayon) ;
// leurs écrans sont des canvas peints par `screen.js`, et vivants : la souris glisse le
// curseur d'exposition du Mac, la barre avant / après balaie la photo, le pouce tourne
// la molette de l'iPhone, le crayon peint le ciel sur l'iPad. Une perspective CSS
// suffit au mouvement : pas de 3D à charger.

const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const lerp = (a, b, t) => a + (b - a) * t;
const ease = (t) => 1 - Math.pow(1 - clamp(t), 3);
const range = (p, a, b) => clamp((p - a) / (b - a));
const f2 = (v) => v.toFixed(2);

function el(tag, cls, parent) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (parent) parent.appendChild(e);
  return e;
}

function canvas(parent, cls, w, h) {
  const c = el('canvas', cls, parent);
  c.width = w; c.height = h;
  return c;
}

/** Le MacBook Pro : l'écran (bord noir fin, encoche) sur sa charnière, et la base. */
function macOf(stage) {
  const mac = el('div', 'mbp', stage);
  const lid = el('div', 'mbp-lid', mac);
  const back = el('div', 'mbp-back', lid);
  el('div', 'mbp-logo', back);
  const bezel = el('div', 'mbp-bezel', lid);
  const display = canvas(bezel, 'mbp-screen', 1280, 820);
  el('div', 'mbp-notch', bezel);
  el('div', 'mbp-glare', bezel);
  const base = el('div', 'mbp-base', mac);
  el('div', 'mbp-lip', base);
  el('div', 'mbp-shadow', mac);
  return { mac, lid, display };
}

/** L'iPhone, de face : cadre en titane, Dynamic Island, boutons. */
function phoneOf(stage) {
  const phone = el('div', 'iph', stage);
  el('i', 'iph-btn iph-action', phone);
  el('i', 'iph-btn iph-up', phone);
  el('i', 'iph-btn iph-down', phone);
  el('i', 'iph-btn iph-power', phone);
  const glass = el('div', 'iph-glass', phone);
  const display = canvas(glass, 'iph-screen', 780, 1688);
  el('div', 'iph-island', glass);
  el('div', 'iph-glare', glass);
  return { phone, display };
}

/** L'iPad Pro, à l'horizontale, et l'Apple Pencil posé sur son écran. */
function padOf(stage) {
  const pad = el('div', 'ipad', stage);
  el('i', 'ipad-btn ipad-power', pad);
  el('i', 'ipad-btn ipad-up', pad);
  el('i', 'ipad-btn ipad-down', pad);
  el('div', 'ipad-glass', pad);
  el('div', 'ipad-cam', pad);
  const box = el('div', 'ipad-box', pad);
  const display = canvas(box, 'ipad-screen', 1600, 1200);
  el('div', 'ipad-glare', box);
  const shadow = el('div', 'pencil-shadow', box);
  const pencil = el('div', 'pencil', box);
  el('div', 'pencil-body', pencil);
  el('div', 'ipad-shadow', pad);
  return { pad, display, pencil, shadow };
}

export async function initDevices(stage, section, { reduced = false } = {}) {
  // Les écrans, dans la même version que ce fichier.
  const { macScreen, phoneScreen, padScreen } = await import(`./screen.js${new URL(import.meta.url).search}`);
  stage.classList.add('devices-ready');
  const { mac, lid, display } = macOf(stage);
  const { phone, display: phoneDisplay } = phoneOf(stage);
  const { pad, display: padDisplay, pencil, shadow } = padOf(stage);
  const drawMac = macScreen(display);
  const drawPhone = phoneScreen(phoneDisplay);
  const padUI = padScreen(padDisplay);
  // Sur téléphone, les appareils finissent bien de face : un plan incliné y serait
  // rastérisé en basse définition.
  const flat = matchMedia('(max-width: 1000px)').matches;

  const panels = [...section.querySelectorAll('.apple-panel')];
  const dots = [...section.querySelectorAll('.apple-dots i')];
  const steps = panels.length;
  let visible = false, sp = 0, mx = 0, tmx = 0, current = -1;
  // Les écrans sont repeints 30 fois par seconde au plus ; le crayon a sa propre horloge,
  // remise à zéro quand l'iPad s'en va.
  let last = 0, painted = -1, penClock = 0;
  new IntersectionObserver(([e]) => { visible = e.isIntersecting; }).observe(section);
  window.addEventListener('pointermove', (e) => { tmx = (e.clientX / window.innerWidth) * 2 - 1; }, { passive: true });

  function progress() {
    const r = section.getBoundingClientRect();
    return clamp(-r.top / (r.height - window.innerHeight));
  }

  // Le crayon : il peint le trait, se relève, le voile s'efface, et il recommence.
  const PEN = 7.5;
  function pen(time) {
    if (reduced) return { q: 1, fade: 0, lift: 1 };
    const u = time % PEN;
    const q = range(u, 0.5, 5.2);
    const lift = Math.max(1 - range(u, 0, 0.5), range(u, 5.2, 5.8));
    return { q: u > 6.6 ? 0 : q, fade: range(u, 5.9, 6.6), lift };
  }

  function frame(now) {
    requestAnimationFrame(frame);
    if (!visible) return;
    const dt = Math.min((now - (frame.prev || now)) / 1000, 0.1);
    frame.prev = now;
    const t = reduced ? 0 : now / 1000;
    const p = progress();
    sp = reduced || Math.abs(p - sp) < 1e-4 ? p : lerp(sp, p, 0.1);
    mx = Math.abs(tmx - mx) < 1e-4 ? tmx : lerp(mx, tmx, 0.05);

    const step = Math.min(steps - 1, Math.floor(sp * steps + 0.08));
    if (step !== current) {
      current = step;
      panels.forEach((x, i) => x.classList.toggle('on', i === step));
    }
    dots.forEach((d, i) => d.style.setProperty('--f', clamp(sp * steps - i).toFixed(3)));

    // Le Mac : l'écran se relève (de rabattu à légèrement incliné), l'image s'allume et
    // les curseurs se mettent en place.
    const open = ease(range(sp, 0.02, 0.2));
    const edit = range(sp, 0.1, 0.22);
    // Avant / après : la barre traverse la photo et revient.
    const sweep = range(sp, 0.26, 0.48);
    const split = sweep > 0 && sweep < 1 ? Math.sin(Math.PI * sweep) * 0.8 + Math.sin(t * 1.3) * 0.02 * Math.sin(Math.PI * sweep) : -1;
    const face = ease(range(sp, 0.25, 0.4));
    // Puis il recule pour l'iPhone, et encore pour l'iPad.
    const shift = ease(range(sp, 0.5, 0.68));
    const arrive = ease(range(sp, 0.48, 0.66));
    const land = ease(range(sp, 0.73, 0.9));

    lid.style.transform = `rotateX(${f2(lerp(-86, flat ? 0 : 4, open))}deg)`;
    display.style.opacity = clamp((open - 0.45) / 0.4).toFixed(3);
    mac.style.transform = flat
      ? `translate3d(0, ${f2(lerp(0, -16, land))}%, 0) rotateX(${f2(lerp(14, 0, open))}deg) scale(${(lerp(1, 0.94, shift) * lerp(1, 0.72, land)).toFixed(3)})`
      : `translate3d(${f2(lerp(0, -4, shift) + lerp(0, 4, land))}%, ${f2(lerp(0, -16, land))}%, 0) ` +
        `rotateX(${f2(lerp(14, 8, open) + lerp(0, 4, land))}deg) ` +
        `rotateY(${f2(lerp(-14, -6, open) + face * 4 + mx * 4 + shift * 2)}deg) ` +
        `scale(${(lerp(1, 1.04, face) * lerp(1, 0.92, shift) * lerp(1, 0.84, land)).toFixed(3)})`;
    mac.style.opacity = flat ? (1 - land * 0.55).toFixed(3) : '';

    // L'iPhone : arrive par la droite, se pose devant le coin du Mac, flotte un peu.
    const bob = reduced ? 0 : Math.sin(t * 1.1);
    phone.style.opacity = clamp(arrive * 1.6).toFixed(3);
    phone.style.transform =
      `translate3d(${f2(lerp(60, 0, arrive) + lerp(0, flat ? 30 : 14, land))}%, ${f2(lerp(18, 0, arrive) + bob * 1.2 * arrive + lerp(0, flat ? -6 : 8, land))}%, 0) ` +
      `rotateY(${f2(lerp(-30, flat ? 0 : -6, arrive) + (flat ? 0 : mx * 4))}deg) rotateZ(${f2(lerp(8, flat ? 0 : 1.5, arrive) + bob * 0.6 * arrive + land * (flat ? 6 : 3))}deg) ` +
      `scale(${f2(lerp(1, flat ? 0.8 : 0.9, land))})`;

    // L'iPad : monte du bas, se couche à peine vers nous, et flotte.
    const float = reduced ? 0 : Math.sin(t * 0.8 + 1);
    pad.style.opacity = clamp(land * 1.5).toFixed(3);
    pad.style.transform = flat
      ? `translate3d(0, ${f2(lerp(70, 0, land) + float * 0.8 * land)}%, 0) rotateX(${f2(lerp(40, 0, land))}deg)`
      : `translate3d(${f2(lerp(-10, 0, land))}%, ${f2(lerp(80, 0, land) + float * 0.8 * land)}%, 0) ` +
        `rotateX(${f2(lerp(48, 10, land))}deg) rotateY(${f2(lerp(18, 4, land) + mx * 3)}deg) rotateZ(${f2(lerp(-8, -1.5, land))}deg)`;

    // Le crayon ne dessine qu'une fois l'iPad posé.
    if (land > 0.97) penClock += dt; else penClock = 0;
    const { q, fade, lift } = pen(penClock);
    const [tx, ty] = padUI.tip(q);
    const show = clamp((land - 0.85) / 0.15);
    pencil.style.opacity = show.toFixed(3);
    pencil.style.left = `${(tx * 100).toFixed(2)}%`;
    pencil.style.top = `${(ty * 100).toFixed(2)}%`;
    pencil.style.setProperty('--lift', lift.toFixed(3));
    shadow.style.opacity = (show * (0.55 - lift * 0.3)).toFixed(3);
    shadow.style.left = pencil.style.left;
    shadow.style.top = pencil.style.top;
    shadow.style.setProperty('--lift', lift.toFixed(3));

    // Les écrans, quand ils se voient.
    if (reduced ? painted === sp : now - last < 33) return;
    last = now; painted = sp;
    if (open > 0.4) drawMac({ t, edit, split });
    if (arrive > 0) drawPhone({ t });
    if (land > 0) padUI.draw({ q: land > 0.97 ? q : 0, fade });
  }
  requestAnimationFrame(frame);
}
