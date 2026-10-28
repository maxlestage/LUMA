// Section Linux et Windows : un écran de bureau (Windows) et un portable (Linux) en 3D,
// tous deux sur l'application LÜMA, posés sur une flaque de lumière aux couleurs du logo.
//
// Au défilement, l'écran monte, le capot du portable s'ouvre et les deux écrans
// s'allument ; les curseurs se mettent en place, puis la barre avant / après balaie la
// photo de l'écran. La souris fait pivoter la scène. Les écrans sont les mêmes canvas que
// ceux du Mac (`screen.js`), habillés à la manière de chaque système.
//
// Sans WebGL, les deux écrans restent à plat, dessinés une fois.

import * as THREE from '../vendor/three.module.min.js';
import { RoundedBoxGeometry } from '../vendor/RoundedBoxGeometry.js';
import { RoomEnvironment } from '../vendor/RoomEnvironment.js';

const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const lerp = (a, b, t) => a + (b - a) * t;
const ease = (t) => 1 - Math.pow(1 - clamp(t), 3);
const range = (p, a, b) => clamp((p - a) / (b - a));

function canvasOf(w, h) {
  const c = document.createElement('canvas');
  c.width = w; c.height = h;
  return c;
}

/** Un écran : le canvas peint par `screen.js`, en texture, et sa fonction de dessin. */
function screenOf(macScreen, w, h, os, maxAniso) {
  const canvas = canvasOf(w, h);
  const draw = macScreen(canvas, { os });
  const map = new THREE.CanvasTexture(canvas);
  map.colorSpace = THREE.SRGBColorSpace;
  map.anisotropy = maxAniso;
  // L'écran émet sa propre lumière : pas de tons ni d'éclairage, seulement sa luminosité.
  const material = new THREE.MeshBasicMaterial({ map, toneMapped: false, color: 0x000000 });
  return { draw, map, material };
}

/** La flaque de lumière sous les appareils : un dégradé radial du spectre. */
function poolTexture() {
  const c = canvasOf(512, 512);
  const g = c.getContext('2d');
  const grad = g.createRadialGradient(256, 256, 0, 256, 256, 256);
  grad.addColorStop(0, 'rgba(255,207,74,.55)');
  grad.addColorStop(0.25, 'rgba(255,107,61,.32)');
  grad.addColorStop(0.5, 'rgba(139,123,255,.18)');
  grad.addColorStop(0.75, 'rgba(74,168,255,.06)');
  grad.addColorStop(1, 'rgba(0,0,0,0)');
  g.fillStyle = grad; g.fillRect(0, 0, 512, 512);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

/** L'écran de bureau : dalle à bords fins, pied et socle, l'écran sur sa face. */
function monitorOf(screen) {
  const group = new THREE.Group();
  const body = new THREE.MeshPhysicalMaterial({ color: 0x1a1a21, metalness: 0.65, roughness: 0.34, clearcoat: 0.7, clearcoatRoughness: 0.25 });
  const alu = new THREE.MeshPhysicalMaterial({ color: 0x3a3a44, metalness: 0.9, roughness: 0.3 });
  const panel = new THREE.Mesh(new RoundedBoxGeometry(5.16, 3.3, 0.14, 4, 0.06), body);
  panel.position.y = 2.55;
  group.add(panel);
  const glass = new THREE.Mesh(new THREE.PlaneGeometry(4.96, 3.1), screen.material);
  glass.position.set(0, 2.57, 0.071);
  group.add(glass);
  const neck = new THREE.Mesh(new RoundedBoxGeometry(0.5, 2.2, 0.12, 3, 0.04), alu);
  neck.position.set(0, 1.15, -0.24);
  group.add(neck);
  const foot = new THREE.Mesh(new RoundedBoxGeometry(1.9, 0.08, 1.15, 3, 0.035), alu);
  foot.position.set(0, 0.04, -0.12);
  group.add(foot);
  return group;
}

/** Le portable : la base, son clavier et son pavé, et le capot qui pivote à l'arrière. */
function laptopOf(screen) {
  const group = new THREE.Group();
  const alu = new THREE.MeshPhysicalMaterial({ color: 0x2b2b33, metalness: 0.85, roughness: 0.3, clearcoat: 0.4 });
  const dark = new THREE.MeshStandardMaterial({ color: 0x0b0b0f, metalness: 0.2, roughness: 0.8 });
  const base = new THREE.Mesh(new RoundedBoxGeometry(3.4, 0.1, 2.3, 3, 0.04), alu);
  base.position.y = 0.05;
  group.add(base);
  const keys = new THREE.Mesh(new THREE.PlaneGeometry(2.9, 1.0), dark);
  keys.rotation.x = -Math.PI / 2;
  keys.position.set(0, 0.101, -0.42);
  group.add(keys);
  const pad = new THREE.Mesh(new THREE.PlaneGeometry(1.1, 0.68), new THREE.MeshStandardMaterial({ color: 0x34343d, metalness: 0.6, roughness: 0.45 }));
  pad.rotation.x = -Math.PI / 2;
  pad.position.set(0, 0.101, 0.6);
  group.add(pad);
  // Le capot : son axe est la charnière, au bord arrière de la base.
  const hinge = new THREE.Group();
  hinge.position.set(0, 0.1, -1.13);
  group.add(hinge);
  const lid = new THREE.Mesh(new RoundedBoxGeometry(3.4, 2.18, 0.07, 3, 0.03), alu);
  lid.position.set(0, 1.09, 0);
  hinge.add(lid);
  const bezel = new THREE.Mesh(new THREE.PlaneGeometry(3.3, 2.08), dark);
  bezel.position.set(0, 1.09, 0.0355);
  hinge.add(bezel);
  const glass = new THREE.Mesh(new THREE.PlaneGeometry(3.2, 2.0), screen.material);
  glass.position.set(0, 1.1, 0.037);
  hinge.add(glass);
  return { group, hinge };
}

/** Sans WebGL : les deux écrans à plat, dessinés une fois, dans des cadres CSS. */
function flat(stage, macScreen) {
  stage.classList.add('desktop-2d');
  for (const os of ['windows', 'linux']) {
    const frame = document.createElement('div');
    frame.className = `desktop-flat desktop-flat-${os}`;
    const c = canvasOf(1280, 800);
    frame.appendChild(c);
    stage.appendChild(frame);
    macScreen(c, { os })({ t: 0, edit: 1 });
  }
}

export async function initDesktop(stage, section, { reduced = false } = {}) {
  const { macScreen } = await import(`./screen.js${new URL(import.meta.url).search}`);
  const canvas = canvasOf(1, 1);
  canvas.className = 'desktop-gl';
  let renderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: 'high-performance' });
  } catch (e) {
    flat(stage, macScreen);
    return;
  }
  stage.appendChild(canvas);
  stage.classList.add('desktop-ready');
  const mobile = matchMedia('(max-width: 820px)').matches || matchMedia('(pointer: coarse)').matches;
  const dpr = Math.min(window.devicePixelRatio || 1, mobile ? 2 : 1.75);
  renderer.setPixelRatio(dpr);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  pmrem.dispose();

  const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 100);
  const key = new THREE.DirectionalLight(0xffffff, 1.2); key.position.set(4, 6, 7); scene.add(key);
  const rimA = new THREE.PointLight(0xff6b3d, 40, 14); rimA.position.set(-5, 3, 1.5); scene.add(rimA);
  const rimB = new THREE.PointLight(0x8b7bff, 40, 14); rimB.position.set(5, 2, 1); scene.add(rimB);

  const aniso = renderer.capabilities.getMaxAnisotropy();
  const winScreen = screenOf(macScreen, 1280, 800, 'windows', aniso);
  const linScreen = screenOf(macScreen, 1024, 640, 'linux', aniso);

  const world = new THREE.Group();
  scene.add(world);
  const monitor = monitorOf(winScreen);
  monitor.position.set(-1.3, 0, -0.7);
  monitor.rotation.y = 0.16;
  world.add(monitor);
  const { group: laptop, hinge } = laptopOf(linScreen);
  laptop.position.set(2.3, 0, 1.2);
  laptop.rotation.y = -0.42;
  world.add(laptop);
  const pool = new THREE.Mesh(
    new THREE.PlaneGeometry(13, 9),
    new THREE.MeshBasicMaterial({ map: poolTexture(), transparent: true, depthWrite: false, blending: THREE.AdditiveBlending }),
  );
  pool.rotation.x = -Math.PI / 2;
  pool.position.set(0.3, -0.01, 0.3);
  world.add(pool);

  // Le cadrage suit la forme de la scène : plus loin quand elle est étroite.
  let w = 1, h = 1;
  function resize() {
    w = Math.max(1, stage.clientWidth); h = Math.max(1, stage.clientHeight);
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  }
  resize();
  new ResizeObserver(resize).observe(stage);

  let visible = false, sp = -1, mx = 0, tmx = 0, last = 0, painted = -1;
  new IntersectionObserver(([e]) => { visible = e.isIntersecting; }).observe(stage);
  if (!reduced) window.addEventListener('pointermove', (e) => { tmx = (e.clientX / window.innerWidth) * 2 - 1; }, { passive: true });

  // L'avancée dans la section : 0 quand la scène entre par le bas, 1 quand elle sort par
  // le haut.
  function progress() {
    const r = stage.getBoundingClientRect();
    const vh = window.innerHeight;
    return clamp((vh - r.top) / (vh + r.height));
  }

  function frame(now) {
    requestAnimationFrame(frame);
    if (!visible) return;
    const t = reduced ? 0 : now / 1000;
    const p = progress();
    sp = reduced || sp < 0 || Math.abs(p - sp) < 1e-4 ? p : lerp(sp, p, 0.1);
    mx = Math.abs(tmx - mx) < 1e-4 ? tmx : lerp(mx, tmx, 0.05);
    if (reduced && painted === sp) return;

    const arrive = ease(range(sp, 0.05, 0.4));
    const open = ease(range(sp, 0.15, 0.48));
    const edit = range(sp, 0.3, 0.52);
    const sweep = range(sp, 0.52, 0.8);
    const split = sweep > 0 && sweep < 1 ? Math.sin(Math.PI * sweep) * 0.8 : -1;
    const bob = reduced ? 0 : Math.sin(t * 0.9);

    // L'écran monte du sol et s'allume ; le capot du portable se relève.
    monitor.position.y = lerp(-1.6, 0, arrive) + bob * 0.03 * arrive;
    hinge.rotation.x = lerp(Math.PI / 2, -0.3, open);
    winScreen.material.color.setScalar(clamp((arrive - 0.35) / 0.5));
    linScreen.material.color.setScalar(clamp((open - 0.5) / 0.4));
    pool.material.opacity = lerp(0.35, 1, arrive);

    world.rotation.y = lerp(0.42, -0.08, ease(sp)) + mx * 0.14;
    world.rotation.x = lerp(0.1, 0, arrive);
    // Plus loin sur un écran étroit, pour que les deux appareils tiennent.
    const fit = Math.max(1, 1.4 / camera.aspect);
    const dist = lerp(15, 12.5, arrive) * fit;
    camera.position.set(Math.sin(mx * 0.15) * 1.2, lerp(3.4, 2.6, arrive), dist);
    camera.lookAt(0.3, 1.45, 0);

    // Les écrans, 30 fois par seconde au plus, quand ils se voient.
    if (reduced || now - last > 33) {
      last = now; painted = sp;
      if (arrive > 0.35) { winScreen.draw({ t, edit, split }); winScreen.map.needsUpdate = true; }
      if (open > 0.5) { linScreen.draw({ t: t + 2, edit }); linScreen.map.needsUpdate = true; }
    }
    renderer.render(scene, camera);
  }
  requestAnimationFrame(frame);
}
