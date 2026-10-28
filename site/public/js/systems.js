// Scène 3D de la section Linux et Windows (#bureau) : un écran (boîtier arrondi en verre
// physique) qui tourne lentement, éclairé par l'environnement, la face avant allumée aux
// couleurs de LÜMA. Elle évoque LÜMA sur Linux et sur Windows : le même moteur, la même
// image, hors d'Apple. Chargée à l'approche de la section (scenes.js), et ne tourne que
// lorsqu'elle est visible.

import * as THREE from '../vendor/three.module.min.js';
import { RoomEnvironment } from '../vendor/RoomEnvironment.js';
import { RoundedBoxGeometry } from '../vendor/RoundedBoxGeometry.js';

const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const lerp = (a, b, t) => a + (b - a) * t;

// La face « écran » : un dégradé aux couleurs du logo, une vignette et trois pistes de
// réglage, pour rappeler l'éditeur sans rien promettre de précis.
function screenTexture() {
  const c = document.createElement('canvas');
  c.width = 1024;
  c.height = 672;
  const g = c.getContext('2d');
  g.fillStyle = '#070709';
  g.fillRect(0, 0, c.width, c.height);
  const grad = g.createLinearGradient(0, 0, c.width, c.height);
  grad.addColorStop(0, '#ff6b3d');
  grad.addColorStop(0.3, '#ffcf4a');
  grad.addColorStop(0.6, '#4af2c8');
  grad.addColorStop(0.8, '#4aa8ff');
  grad.addColorStop(1, '#8b7bff');
  g.globalAlpha = 0.9;
  g.fillStyle = grad;
  g.fillRect(40, 40, c.width - 80, c.height - 230);
  // Vignette, pour un rendu « photo ».
  const vig = g.createRadialGradient(c.width / 2, 300, 120, c.width / 2, 300, 620);
  vig.addColorStop(0, 'rgba(0,0,0,0)');
  vig.addColorStop(1, 'rgba(0,0,0,.45)');
  g.fillStyle = vig;
  g.fillRect(40, 40, c.width - 80, c.height - 230);
  g.globalAlpha = 1;
  // Le panneau de réglages, en bas : trois pistes.
  for (let i = 0; i < 3; i++) {
    const y = c.height - 150 + i * 42;
    g.fillStyle = 'rgba(255,255,255,.14)';
    g.fillRect(60, y, c.width - 320, 8);
    g.fillStyle = '#ffcf4a';
    const knob = 120 + i * 180;
    g.fillRect(60, y, knob, 8);
    g.beginPath();
    g.arc(60 + knob, y + 4, 11, 0, Math.PI * 2);
    g.fillStyle = '#fff';
    g.fill();
  }
  // Le mot LÜMA, discret, en haut à gauche.
  g.fillStyle = 'rgba(255,255,255,.9)';
  g.font = '600 46px system-ui, -apple-system, Segoe UI, sans-serif';
  g.fillText('LÜMA', 60, 110);
  const tex = new THREE.CanvasTexture(c);
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.anisotropy = 4;
  return tex;
}

export function initSystems(canvas, { reduced = false } = {}) {
  let renderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: 'high-performance' });
  } catch (e) {
    // Sans WebGL, la scène s'efface et laisse la place aux cartes.
    canvas.parentElement.classList.add('desktop-off');
    return { dispose() {} };
  }
  const mobile = matchMedia('(max-width: 820px)').matches || matchMedia('(pointer: coarse)').matches;
  const dpr = Math.min(window.devicePixelRatio || 1, mobile ? 2 : 1.75);
  renderer.setPixelRatio(dpr);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.1;
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  const envTex = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  scene.environment = envTex;
  pmrem.dispose();

  const camera = new THREE.PerspectiveCamera(34, 1, 0.1, 100);
  camera.position.set(0, 0, 8.2);

  const key = new THREE.DirectionalLight(0xffffff, 1.2);
  key.position.set(4, 5, 6);
  scene.add(key);
  const rimA = new THREE.PointLight(0xff6b3d, 26, 14);
  rimA.position.set(-4, 2.5, 2);
  scene.add(rimA);
  const rimB = new THREE.PointLight(0x8b7bff, 26, 14);
  rimB.position.set(4, -2.5, 2);
  scene.add(rimB);

  // Le boîtier : un écran arrondi, en verre sombre qui prend les reflets de l'environnement.
  const holder = new THREE.Group();
  const bodyGeo = new RoundedBoxGeometry(4.6, 3.0, 0.34, 6, 0.18);
  const bodyMat = new THREE.MeshPhysicalMaterial({
    color: 0x0b0b12,
    metalness: 0.2,
    roughness: 0.22,
    clearcoat: 1,
    clearcoatRoughness: 0.18,
    envMapIntensity: 1.4,
  });
  holder.add(new THREE.Mesh(bodyGeo, bodyMat));

  // La face avant allumée, légèrement devant le boîtier.
  const screenMat = new THREE.MeshBasicMaterial({ map: screenTexture(), toneMapped: false });
  const screen = new THREE.Mesh(new THREE.PlaneGeometry(4.16, 2.56), screenMat);
  screen.position.z = 0.181;
  holder.add(screen);

  // Un pied discret, pour l'allure d'un moniteur.
  const stemMat = new THREE.MeshPhysicalMaterial({ color: 0x15151d, metalness: 0.4, roughness: 0.3, envMapIntensity: 1 });
  const stem = new THREE.Mesh(new RoundedBoxGeometry(0.5, 0.7, 0.3, 4, 0.1), stemMat);
  stem.position.y = -1.78;
  holder.add(stem);
  const foot = new THREE.Mesh(new RoundedBoxGeometry(1.8, 0.16, 0.9, 4, 0.07), stemMat);
  foot.position.y = -2.12;
  holder.add(foot);

  scene.add(holder);

  const state = { tx: 0, ty: 0, mx: 0, my: 0, spin: 0, visible: false, fade: 0 };

  function resize() {
    const w = canvas.clientWidth || canvas.parentElement.clientWidth || window.innerWidth;
    const h = canvas.clientHeight || Math.round(w * 0.9);
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    const narrow = camera.aspect < 1;
    holder.scale.setScalar(narrow ? 0.78 : 1);
    camera.position.z = narrow ? 9.6 : 8.2;
  }
  resize();
  window.addEventListener('resize', resize);

  if (!reduced) {
    window.addEventListener('pointermove', (e) => {
      const r = canvas.getBoundingClientRect();
      state.tx = ((e.clientX - r.left) / r.width) * 2 - 1;
      state.ty = ((e.clientY - r.top) / r.height) * 2 - 1;
    }, { passive: true });
  }

  // Ne tourner que lorsque la scène est à l'écran.
  const io = new IntersectionObserver(([e]) => { state.visible = e.isIntersecting; }, { threshold: 0.05 });
  io.observe(canvas);
  document.addEventListener('visibilitychange', () => { if (document.hidden) state.visible = false; });

  const clock = new THREE.Clock();
  let last = 0;
  let raf = 0;
  function frame() {
    raf = requestAnimationFrame(frame);
    const show = state.visible && !document.hidden;
    state.fade = lerp(state.fade, show ? 1 : 0, 0.08);
    canvas.style.opacity = state.fade.toFixed(3);
    if (!show && state.fade < 0.01) { clock.getDelta(); return; }
    const dt = Math.min(clock.getDelta(), 0.05);
    const t = (last += dt);

    state.mx = lerp(state.mx, state.tx, 0.05);
    state.my = lerp(state.my, state.ty, 0.05);
    state.spin += dt * (reduced ? 0 : 0.2);

    holder.rotation.y = state.spin * 0.5 + state.mx * 0.4 + (reduced ? -0.35 : Math.sin(t * 0.3) * 0.15 - 0.2);
    holder.rotation.x = state.my * 0.2 + (reduced ? 0.05 : Math.sin(t * 0.22) * 0.05);
    holder.position.y = reduced ? 0 : Math.sin(t * 0.6) * 0.08;

    renderer.render(scene, camera);
  }
  frame();

  return {
    dispose() {
      cancelAnimationFrame(raf);
      io.disconnect();
      window.removeEventListener('resize', resize);
      bodyGeo.dispose();
      bodyMat.dispose();
      screenMat.map.dispose();
      screenMat.dispose();
      envTex.dispose();
      renderer.dispose();
    },
  };
}
