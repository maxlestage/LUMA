// Scène 3D de la section Linux et Windows (#bureau) : un moniteur fin (dalle en aluminium,
// bords noirs étroits, pied incliné) sur lequel tourne LÜMA, la même interface vivante que
// sur le Mac (`screen.js` : la souris glisse l'exposition, la barre avant / après balaie
// la photo), avec la barre de fenêtre de Windows. L'écran reste face au visiteur : il
// pivote de quelques degrés, suit la souris, et entre en scène au défilement. Chargée à
// l'approche de la section (scenes.js) ; ne dessine que lorsqu'elle est visible.

import * as THREE from '../vendor/three.module.min.js';
import { RoomEnvironment } from '../vendor/RoomEnvironment.js';
import { RoundedBoxGeometry } from '../vendor/RoundedBoxGeometry.js';
import { macScreen } from './screen.js';

const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const lerp = (a, b, t) => a + (b - a) * t;
const ease = (t) => 1 - Math.pow(1 - clamp(t), 3);

// Les proportions : une dalle 16:10 de 5 unités, bords de 0,07.
const PANEL_W = 5.0;
const PANEL_H = 3.18;
const DEPTH = 0.1;
const BEZEL = 0.07;

/** Une ombre douce posée sous le pied : un dégradé radial sur un plan. */
function contactShadow() {
  const c = document.createElement('canvas');
  c.width = c.height = 128;
  const g = c.getContext('2d');
  const grad = g.createRadialGradient(64, 64, 4, 64, 64, 64);
  grad.addColorStop(0, 'rgba(0,0,0,.55)');
  grad.addColorStop(1, 'rgba(0,0,0,0)');
  g.fillStyle = grad;
  g.fillRect(0, 0, 128, 128);
  const tex = new THREE.CanvasTexture(c);
  const mesh = new THREE.Mesh(
    new THREE.PlaneGeometry(3.4, 1.6),
    new THREE.MeshBasicMaterial({ map: tex, transparent: true, depthWrite: false }),
  );
  mesh.rotation.x = -Math.PI / 2;
  return mesh;
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
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, mobile ? 2 : 1.75));
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  const envTex = pmrem.fromScene(new RoomEnvironment(), 0.03).texture;
  scene.environment = envTex;
  pmrem.dispose();

  const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 100);
  const key = new THREE.DirectionalLight(0xffffff, 1.4);
  key.position.set(3, 6, 8);
  scene.add(key);
  // Deux lueurs aux couleurs du logo, sur les tranches.
  const warm = new THREE.PointLight(0xff6b3d, 18, 12);
  warm.position.set(-4.5, 1.5, 1.5);
  scene.add(warm);
  const cool = new THREE.PointLight(0x8b7bff, 18, 12);
  cool.position.set(4.5, -1, 1.5);
  scene.add(cool);

  // L'écran : l'interface de LÜMA, redessinée à chaque image (30 par seconde suffisent).
  const display = document.createElement('canvas');
  display.width = 1280;
  display.height = 814;
  const draw = macScreen(display, { chrome: 'windows' });
  draw({ t: 0 });
  const screenTex = new THREE.CanvasTexture(display);
  screenTex.colorSpace = THREE.SRGBColorSpace;
  screenTex.anisotropy = renderer.capabilities.getMaxAnisotropy();

  const monitor = new THREE.Group();
  // La dalle : le dos en aluminium sombre, la face en verre noir aux bords étroits.
  const alu = new THREE.MeshPhysicalMaterial({ color: 0x2a2c33, metalness: 0.85, roughness: 0.32, envMapIntensity: 1.2 });
  const glass = new THREE.MeshPhysicalMaterial({ color: 0x050507, metalness: 0, roughness: 0.08, clearcoat: 1, clearcoatRoughness: 0.05, envMapIntensity: 1.6 });
  const shellGeo = new RoundedBoxGeometry(PANEL_W, PANEL_H, DEPTH, 5, 0.045);
  monitor.add(new THREE.Mesh(shellGeo, alu));
  const frontGeo = new THREE.PlaneGeometry(PANEL_W - 0.02, PANEL_H - 0.02);
  const front = new THREE.Mesh(frontGeo, glass);
  front.position.z = DEPTH / 2 + 0.001;
  monitor.add(front);
  const screenGeo = new THREE.PlaneGeometry(PANEL_W - 2 * BEZEL, PANEL_H - 2 * BEZEL);
  const screenMat = new THREE.MeshBasicMaterial({ map: screenTex, toneMapped: false });
  const screen = new THREE.Mesh(screenGeo, screenMat);
  screen.position.z = DEPTH / 2 + 0.002;
  monitor.add(screen);
  // Un reflet très léger en travers du verre.
  const glareGeo = new THREE.PlaneGeometry(PANEL_W - 2 * BEZEL, PANEL_H - 2 * BEZEL);
  const glareMat = new THREE.MeshPhysicalMaterial({
    color: 0xffffff, transparent: true, opacity: 0.06, roughness: 0.05, metalness: 0, clearcoat: 1, depthWrite: false,
  });
  const glare = new THREE.Mesh(glareGeo, glareMat);
  glare.position.z = DEPTH / 2 + 0.003;
  monitor.add(glare);

  // Le pied : une colonne fine, cachée en haut derrière la dalle, sur un socle bas.
  const standMat = new THREE.MeshPhysicalMaterial({ color: 0x1b1c22, metalness: 0.9, roughness: 0.38, envMapIntensity: 0.9 });
  const standGeo = new RoundedBoxGeometry(0.42, 2.1, 0.12, 4, 0.05);
  const stand = new THREE.Mesh(standGeo, standMat);
  stand.position.set(0, -PANEL_H / 2 - 0.15, -0.22);
  const footGeo = new RoundedBoxGeometry(1.5, 0.06, 0.95, 5, 0.028);
  const foot = new THREE.Mesh(footGeo, standMat);
  foot.position.set(0, -PANEL_H / 2 - 1.2, -0.22);
  const shadow = contactShadow();
  shadow.position.set(0, -PANEL_H / 2 - 1.235, -0.2);

  const holder = new THREE.Group();
  holder.add(monitor, stand, foot, shadow);
  holder.position.y = 0.3;
  scene.add(holder);

  const state = { tx: 0, ty: 0, mx: 0, my: 0, visible: false, fade: 0, enter: reduced ? 1 : 0 };

  function resize() {
    const w = canvas.clientWidth || canvas.parentElement.clientWidth || window.innerWidth;
    const h = canvas.clientHeight || Math.round(w * 0.75);
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    // Tout le moniteur dans le cadre, quelle que soit la largeur.
    const fit = Math.max(1, 1.3 / camera.aspect);
    camera.position.set(0, 0.2, 10 * fit);
    camera.lookAt(0, -0.25, 0);
    camera.updateProjectionMatrix();
  }
  resize();
  window.addEventListener('resize', resize);

  if (!reduced) {
    window.addEventListener('pointermove', (e) => {
      const r = canvas.getBoundingClientRect();
      state.tx = clamp(((e.clientX - r.left) / r.width) * 2 - 1, -1, 1);
      state.ty = clamp(((e.clientY - r.top) / r.height) * 2 - 1, -1, 1);
    }, { passive: true });
  }

  // L'entrée en scène suit le défilement : le moniteur se redresse et se tourne vers vous.
  function progress() {
    const r = canvas.getBoundingClientRect();
    const vh = window.innerHeight || 800;
    return clamp((vh - r.top) / (vh * 0.75));
  }

  const io = new IntersectionObserver(([e]) => { state.visible = e.isIntersecting; }, { threshold: 0.05 });
  io.observe(canvas);
  document.addEventListener('visibilitychange', () => { if (document.hidden) state.visible = false; });

  const clock = new THREE.Clock();
  let t = 0;
  let paint = 0;
  let raf = 0;
  function frame() {
    raf = requestAnimationFrame(frame);
    const show = state.visible && !document.hidden;
    state.fade = lerp(state.fade, show ? 1 : 0, 0.08);
    canvas.style.opacity = state.fade.toFixed(3);
    if (!show && state.fade < 0.01) { clock.getDelta(); return; }
    const dt = Math.min(clock.getDelta(), 0.05);
    t += dt;

    if (!reduced) state.enter = lerp(state.enter, progress(), 0.08);
    const k = ease(state.enter);
    state.mx = lerp(state.mx, state.tx, 0.05);
    state.my = lerp(state.my, state.ty, 0.05);

    // De trois quarts à presque de face ; un balancement de quelques degrés, jamais le dos.
    const sway = reduced ? 0 : Math.sin(t * 0.4) * 0.05;
    holder.rotation.y = lerp(-0.75, -0.2, k) + sway + state.mx * 0.12;
    holder.rotation.x = lerp(0.18, 0.04, k) + state.my * 0.05;
    holder.position.y = 0.3 + lerp(-0.35, 0, k) + (reduced ? 0 : Math.sin(t * 0.8) * 0.03);

    // L'interface vit : l'exposition glisse, puis la barre avant / après balaie la photo.
    paint += dt;
    if (paint > 1 / 30) {
      paint = 0;
      const cycle = t % 14;
      const split = cycle > 8 && cycle < 13 ? 0.5 - 0.42 * Math.cos(((cycle - 8) / 5) * Math.PI * 2) : -1;
      draw({ t: reduced ? 0 : t, edit: clamp(k * 1.2), split });
      screenTex.needsUpdate = true;
    }
    renderer.render(scene, camera);
  }
  frame();

  return {
    dispose() {
      cancelAnimationFrame(raf);
      io.disconnect();
      window.removeEventListener('resize', resize);
      [shellGeo, frontGeo, screenGeo, glareGeo, standGeo, footGeo].forEach((g) => g.dispose());
      [alu, glass, standMat, screenMat, glareMat].forEach((m) => m.dispose());
      screenTex.dispose();
      envTex.dispose();
      renderer.dispose();
    },
  };
}
