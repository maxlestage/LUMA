// Les deux scènes 3D du site, sur three.js : l'objectif du haut de page et le Mac et
// l'iPhone de la section Apple. Tout le reste de la page (animations, labo, îlots) est
// en Rust, compilé en WebAssembly ; ce module ne fait que démarrer les scènes.
//
// - l'objectif démarre tout de suite et s'entrouvre quand le préchargement se termine
//   (`luma:intro`, envoyé par le bundle Rust) ;
// - le Mac et l'iPhone ne sont chargés qu'à l'approche de leur section.

const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
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
    import('./hero.js')
      .then(({ initHero }) => {
        hero = initHero(heroCanvas, opts);
        if (introduced || root.classList.contains('loaded')) hero.intro();
      })
      .catch((e) => { console.warn('LÜMA : objectif 3D indisponible', e); root.classList.add('no-webgl'); });
  }

  const apple = document.getElementById('apple');
  const devices = document.getElementById('devices-gl');
  if (apple && devices) {
    const io = new IntersectionObserver(([e]) => {
      if (!e.isIntersecting) return;
      io.disconnect();
      import('./devices.js')
        .then(({ initDevices }) => initDevices(devices, apple, opts))
        .catch((e) => console.warn('LÜMA : appareils 3D indisponibles', e));
    }, { rootMargin: '600px 0px' });
    io.observe(apple);
  }
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start);
else start();
