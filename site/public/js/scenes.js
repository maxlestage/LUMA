// Les trois scènes du site : l'objectif 3D du haut de page (three.js), le Mac et
// l'iPhone de la section Apple (HTML et CSS), et l'écran et le portable de la section
// Linux et Windows (three.js). Tout le reste de la page (animations, labo, îlots) est
// en Rust, compilé en WebAssembly ; ce module ne fait que démarrer les scènes.
//
// - l'objectif démarre tout de suite et s'entrouvre quand le préchargement se termine
//   (`luma:intro`, envoyé par le bundle Rust) ;
// - les appareils des sections Apple et Linux et Windows ne sont chargés qu'à
//   l'approche de leur section.

const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
// La version de ce fichier (`?v=…`, posée par la page) suit les modules qu'il charge :
// à chaque mise à jour, le navigateur et le service worker prennent les nouveaux.
const V = new URL(import.meta.url).search;
const opts = { reduced };
const root = document.documentElement;

let hero = null;
let introduced = false;
function intro() {
  introduced = true;
  if (hero) hero.intro();
}
document.addEventListener('luma:intro', intro);
// Sans WebAssembly, pas de préchargement : l'objectif s'ouvre quand même.
setTimeout(() => { if (!introduced) { root.classList.add('loaded'); intro(); } }, 6000);

function start() {
  const heroCanvas = document.getElementById('hero-gl');
  if (heroCanvas) {
    import(`./hero.js${V}`)
      .then(({ initHero }) => {
        hero = initHero(heroCanvas, opts);
        if (introduced || root.classList.contains('loaded')) hero.intro();
      })
      .catch((e) => { console.warn('LÜMA : objectif 3D indisponible', e); root.classList.add('no-webgl'); });
  }

  // Une scène chargée à l'approche de sa section.
  const nearby = (section, load) => {
    const io = new IntersectionObserver(([e]) => {
      if (!e.isIntersecting) return;
      io.disconnect();
      load();
    }, { rootMargin: '600px 0px' });
    io.observe(section);
  };

  const apple = document.getElementById('apple');
  const devices = document.getElementById('devices');
  if (apple && devices) {
    nearby(apple, () => import(`./devices.js${V}`)
      .then(({ initDevices }) => initDevices(devices, apple, opts))
      .catch((e) => console.warn('LÜMA : appareils indisponibles', e)));
  }

  const desk = document.getElementById('bureau');
  const deskStage = document.getElementById('desktop-stage');
  if (desk && deskStage) {
    nearby(desk, () => import(`./desktop.js${V}`)
      .then(({ initDesktop }) => initDesktop(deskStage, desk, opts))
      .catch((e) => { console.warn('LÜMA : écran et portable 3D indisponibles', e); deskStage.classList.add('desktop-off'); }));
  }
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start);
else start();
