import * as motion from './motion.js';

const CRATES = [
  ['luma-core', 'P0', '⌘', 'Commandes, annuler/rétablir, historique, tâches asynchrones, erreurs.'],
  ['luma-project', 'P0', '◫', 'Format de projet versionné et migrable, sauvegarde atomique.'],
  ['luma-raw', 'P0', '▦', 'DNG, Bayer, X‑Trans, ProRAW, dématriçage, EXIF, profils caméra.'],
  ['luma-color', 'P0', '◐', 'ICC, XYZ, sRGB, Display P3, Rec. 2020, EDR/HDR, flottants 32 bits.'],
  ['luma-image', 'P0', '☀', 'Exposition, courbes, HSL, netteté, grain, vignettage, géométrie.'],
  ['luma-render', 'P0', '◇', 'Graphe de traitement, tuiles, cache multi‑résolution, annulation.'],
  ['luma-gpu', 'P0', '▲', 'wgpu sur Metal d\'abord, shaders WGSL, repli sur le CPU.'],
  ['luma-ui', 'P0', '◰', 'Interface adaptative : pointeur, tactile, Apple Pencil, 120 Hz.'],
  ['luma-platform', 'P0', '◈', 'macOS et iOS d\'abord : fichiers, PhotoKit, caméra, EDR, permissions.'],
  ['luma-mask', 'P1', '◎', 'Luminance, couleur, pinceau, dégradés, unions et intersections.'],
  ['luma-library', 'P1', '▤', 'Catalogue local, collections, notes, doublons, recherche.'],
  ['luma-export', 'P1', '⇪', 'JPEG, PNG, TIFF, DNG, lots, préréglages, métadonnées.'],
  ['luma-presets', 'P1', '✦', 'Styles et profils de rendu partageables.'],
  ['luma-ai', 'P2', '◉', 'Sujet, ciel, débruitage, super‑résolution. Local, modèles versionnés.'],
  ['luma-privacy', 'P2', '◌', 'Consentement IA, diagnostics sans contenu photographique.'],
  ['luma-sync', 'P3', '⇄', 'Synchronisation chiffrée et facultative entre appareils.'],
  ['luma-plugins', 'P3', '⬡', 'Extensions isolées, permissions, API stable.'],
  ['luma-video', 'P3', '▶', 'Timeline, étalonnage, rendu. Le module le plus risqué.'],
];

function crates() {
  const root = document.getElementById('crates');
  root.innerHTML = CRATES.map(([name, p, glyph, desc]) =>
    `<article class="crate reveal tilt" data-p="${p}"><div class="glyph">${glyph}</div><span class="prio">${p}</span><h3>${name}</h3><p>${desc}</p></article>`).join('');
  const chips = [...document.querySelectorAll('.chips button')];
  chips.forEach((b) => b.addEventListener('click', () => {
    chips.forEach((c) => c.classList.toggle('on', c === b));
    root.querySelectorAll('.crate').forEach((c) => c.classList.toggle('out', b.dataset.filter !== 'all' && c.dataset.p !== b.dataset.filter));
  }));
}

crates();
motion.splitTitles();
motion.frames();

let hero = null;
motion.loader(() => hero && hero.intro());
motion.reveals();
motion.chrome();
motion.cursor();
motion.magnetic();
motion.tilt();
motion.counters();
motion.manifesto();
motion.pipeline();
motion.roadmap();
motion.terminal();
motion.startScroll();

const opts = { reduced: motion.reduced };

// Les scènes 3D sont chargées à part : si WebGL ou three.js manque, la page reste lisible.
import('./hero.js')
  .then(({ initHero }) => { hero = initHero(document.getElementById('hero-gl'), opts); if (document.documentElement.classList.contains('loaded')) hero.intro(); })
  .catch((e) => { console.warn('hero', e); document.documentElement.classList.add('no-webgl'); });

// Le labo et les appareils ne démarrent qu'à l'approche de leur section.
function whenNear(el, f) {
  const io = new IntersectionObserver(([e]) => { if (e.isIntersecting) { io.disconnect(); f(); } }, { rootMargin: '600px 0px' });
  io.observe(el);
}
whenNear(document.getElementById('labo'), () =>
  import('./lab.js').then(({ initLab }) => initLab(document.getElementById('labo'), opts)).catch((e) => console.warn('lab', e)));
whenNear(document.getElementById('apple'), () =>
  import('./devices.js').then(({ initDevices }) => initDevices(document.getElementById('devices-gl'), document.getElementById('apple'), opts)).catch((e) => console.warn('devices', e)));
