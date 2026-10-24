// Section Apple : un MacBook dont l'écran s'ouvre au défilement, puis un iPhone qui
// arrive en montrant ses objectifs avant de se retourner sur l'application LÜMA.

import * as THREE from 'three';
import { RoomEnvironment } from '../vendor/RoomEnvironment.js';
import { RoundedBoxGeometry } from '../vendor/RoundedBoxGeometry.js';
import { drawDesktopUI, drawPhoneUI } from './screen.js';

const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const lerp = (a, b, t) => a + (b - a) * t;
const ease = (t) => 1 - Math.pow(1 - clamp(t), 3);
const range = (p, a, b) => clamp((p - a) / (b - a));

function screenTexture(draw, w, h, renderer) {
  const c = document.createElement('canvas');
  c.width = w; c.height = h;
  draw(c);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  t.anisotropy = renderer.capabilities.getMaxAnisotropy();
  return t;
}

function shadowTexture() {
  const c = document.createElement('canvas');
  c.width = c.height = 128;
  const g = c.getContext('2d');
  const grad = g.createRadialGradient(64, 64, 0, 64, 64, 64);
  grad.addColorStop(0, 'rgba(0,0,0,.75)');
  grad.addColorStop(1, 'rgba(0,0,0,0)');
  g.fillStyle = grad; g.fillRect(0, 0, 128, 128);
  return new THREE.CanvasTexture(c);
}

function buildMac(renderer) {
  const mac = new THREE.Group();
  const alu = new THREE.MeshStandardMaterial({ color: 0x2e3036, metalness: 1, roughness: 0.34 });
  const dark = new THREE.MeshStandardMaterial({ color: 0x0a0a0c, metalness: 0.4, roughness: 0.6 });
  const W = 3.2, D = 2.2;

  const base = new THREE.Mesh(new RoundedBoxGeometry(W, 0.11, D, 4, 0.05), alu);
  base.position.y = 0.055;
  mac.add(base);
  const deck = new THREE.Mesh(new THREE.PlaneGeometry(W * 0.86, D * 0.42), dark);
  deck.rotation.x = -Math.PI / 2; deck.position.set(0, 0.112, -0.32);
  mac.add(deck);
  // Touches : une grille d'instances.
  const keyGeo = new RoundedBoxGeometry(0.17, 0.02, 0.15, 2, 0.02);
  const keys = new THREE.InstancedMesh(keyGeo, new THREE.MeshStandardMaterial({ color: 0x111114, roughness: 0.55 }), 14 * 5);
  const m = new THREE.Matrix4();
  let k = 0;
  for (let r = 0; r < 5; r++) for (let c = 0; c < 14; c++) {
    m.makeTranslation(-1.24 + c * 0.19, 0.12, -0.7 + r * 0.17);
    keys.setMatrixAt(k++, m);
  }
  mac.add(keys);
  const pad = new THREE.Mesh(new RoundedBoxGeometry(1.15, 0.01, 0.7, 2, 0.005), new THREE.MeshStandardMaterial({ color: 0x34363c, metalness: 0.9, roughness: 0.2 }));
  pad.position.set(0, 0.112, 0.6);
  mac.add(pad);

  // Écran : pivote autour de l'arête arrière.
  const hinge = new THREE.Group();
  hinge.position.set(0, 0.11, -D / 2 + 0.03);
  const lidH = 2.08;
  const lid = new THREE.Mesh(new RoundedBoxGeometry(W, lidH, 0.05, 4, 0.024), alu);
  lid.position.set(0, lidH / 2, -0.012);
  hinge.add(lid);
  const bezel = new THREE.Mesh(new THREE.PlaneGeometry(W - 0.06, lidH - 0.06), new THREE.MeshBasicMaterial({ color: 0x000000 }));
  bezel.position.set(0, lidH / 2, 0.0145);
  hinge.add(bezel);
  const tex = screenTexture(drawDesktopUI, 1280, 820, renderer);
  const screenMat = new THREE.MeshBasicMaterial({ map: tex, toneMapped: false, transparent: true, opacity: 0 });
  const screen = new THREE.Mesh(new THREE.PlaneGeometry(W - 0.16, (W - 0.16) * 820 / 1280), screenMat);
  screen.position.set(0, lidH / 2 - 0.02, 0.016);
  hinge.add(screen);
  // Logo à l'arrière, discret
  const logo = new THREE.Mesh(new THREE.CircleGeometry(0.16, 48), new THREE.MeshStandardMaterial({ color: 0x55575e, metalness: 1, roughness: 0.15 }));
  logo.position.set(0, lidH / 2, -0.038); logo.rotation.y = Math.PI;
  hinge.add(logo);
  mac.add(hinge);

  const shadow = new THREE.Mesh(new THREE.PlaneGeometry(W * 1.6, D * 1.8), new THREE.MeshBasicMaterial({ map: shadowTexture(), transparent: true, depthWrite: false }));
  shadow.rotation.x = -Math.PI / 2; shadow.position.y = -0.002;
  mac.add(shadow);

  return {
    group: mac,
    set(open) {
      // Fermé : π/2 (rabattu sur le clavier). Ouvert : légèrement incliné vers l'arrière.
      hinge.rotation.x = lerp(Math.PI / 2 - 0.02, -0.26, open);
      screenMat.opacity = clamp((open - 0.35) / 0.4);
    },
  };
}

function buildPhone(renderer) {
  const phone = new THREE.Group();
  const W = 0.76, H = 1.58, T = 0.085;
  const titanium = new THREE.MeshStandardMaterial({ color: 0x3b3a38, metalness: 1, roughness: 0.3 });
  const back = new THREE.MeshStandardMaterial({ color: 0x24242a, metalness: 0.3, roughness: 0.55 });
  const body = new THREE.Mesh(new RoundedBoxGeometry(W, H, T, 8, 0.11), titanium);
  phone.add(body);
  const backPlate = new THREE.Mesh(new RoundedBoxGeometry(W - 0.02, H - 0.02, 0.01, 6, 0.005), back);
  backPlate.position.z = -T / 2 + 0.002;
  phone.add(backPlate);

  const tex = screenTexture(drawPhoneUI, 780, 1688, renderer);
  const screen = new THREE.Mesh(
    new RoundedBoxGeometry(W - 0.035, H - 0.035, 0.002, 6, 0.001),
    [null, null, null, null, new THREE.MeshBasicMaterial({ map: tex, toneMapped: false }), new THREE.MeshBasicMaterial({ color: 0 })].map((x) => x || new THREE.MeshBasicMaterial({ color: 0 })),
  );
  screen.position.z = T / 2 + 0.001;
  phone.add(screen);

  // Bloc photo : plateau et trois objectifs.
  const bump = new THREE.Mesh(new RoundedBoxGeometry(0.34, 0.34, 0.03, 4, 0.06), new THREE.MeshStandardMaterial({ color: 0x2c2c32, metalness: 0.5, roughness: 0.25 }));
  bump.position.set(-W / 2 + 0.22, H / 2 - 0.22, -T / 2 - 0.012);
  phone.add(bump);
  const ringMat = new THREE.MeshStandardMaterial({ color: 0x55545a, metalness: 1, roughness: 0.2 });
  const glassMat = new THREE.MeshPhysicalMaterial({ color: 0x05050a, metalness: 0, roughness: 0.05, clearcoat: 1, iridescence: 1, iridescenceIOR: 1.8, envMapIntensity: 2.5 });
  [[-0.075, 0.075], [-0.075, -0.075], [0.075, 0]].forEach(([x, y]) => {
    const g = new THREE.Group();
    const ring = new THREE.Mesh(new THREE.CylinderGeometry(0.062, 0.062, 0.03, 48), ringMat);
    ring.rotation.x = Math.PI / 2;
    const lensMesh = new THREE.Mesh(new THREE.CircleGeometry(0.048, 48), glassMat);
    lensMesh.position.z = -0.0155; lensMesh.rotation.y = Math.PI;
    g.add(ring, lensMesh);
    g.position.set(bump.position.x + x, bump.position.y + y, -T / 2 - 0.03);
    phone.add(g);
  });
  return phone;
}

export function initDevices(canvas, section, { reduced = false } = {}) {
  let renderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
  } catch (e) {
    return;
  }
  const mobile = matchMedia('(max-width: 1000px)').matches;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  renderer.setPixelRatio(dpr);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  pmrem.dispose();
  const key = new THREE.DirectionalLight(0xffffff, 1.2); key.position.set(-3, 6, 4); scene.add(key);
  const rim = new THREE.PointLight(0x8b7bff, 18, 10); rim.position.set(3, 2, -3); scene.add(rim);
  const warm = new THREE.PointLight(0xff6b3d, 10, 10); warm.position.set(-4, 1, 2); scene.add(warm);

  const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 100);
  const mac = buildMac(renderer);
  const phone = buildPhone(renderer);
  scene.add(mac.group, phone);

  const panels = [...section.querySelectorAll('.apple-panel')];
  const dots = [...section.querySelectorAll('.apple-dots i')];
  let visible = false, p = 0, sp = 0, mx = 0, tmx = 0;
  new IntersectionObserver(([e]) => { visible = e.isIntersecting; }).observe(section);
  window.addEventListener('pointermove', (e) => { tmx = (e.clientX / window.innerWidth) * 2 - 1; }, { passive: true });

  function resize() {
    const w = canvas.clientWidth, h = canvas.clientHeight;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  }
  resize();
  window.addEventListener('resize', resize);

  function progress() {
    const r = section.getBoundingClientRect();
    return clamp(-r.top / (r.height - window.innerHeight));
  }

  // Rendu à la demande : la scène ne bouge qu'avec le défilement et le pointeur, on
  // ne la redessine donc que quand l'un des deux change (la batterie de l'iPhone dit merci).
  let current = -1, drawn = null, needs = true;
  window.addEventListener('resize', () => { needs = true; });
  function frame() {
    requestAnimationFrame(frame);
    if (!visible) return;
    p = progress();
    sp = reduced || Math.abs(p - sp) < 1e-4 ? p : lerp(sp, p, 0.08);
    mx = Math.abs(tmx - mx) < 1e-4 ? tmx : lerp(mx, tmx, 0.04);
    const key = `${sp.toFixed(4)}|${mx.toFixed(4)}`;
    if (key === drawn && !needs) return;
    drawn = key; needs = false;

    const step = sp < 0.36 ? 0 : sp < 0.68 ? 1 : 2;
    if (step !== current) {
      current = step;
      panels.forEach((el, i) => el.classList.toggle('on', i === step));
    }
    dots.forEach((d, i) => d.style.setProperty('--f', clamp(sp * 3 - i).toFixed(3)));

    // Mac : ouverture, puis léger recul pour laisser la place à l'iPhone.
    const open = ease(range(sp, 0.02, 0.4));
    mac.set(open);
    const shift = ease(range(sp, 0.62, 0.95));
    const offX = mobile ? 0 : 0.9;
    mac.group.position.set(lerp(offX, offX - 0.5, shift), lerp(-1.1, -1.25, shift), lerp(0, -0.8, shift));
    mac.group.rotation.y = lerp(-0.55, -0.32, open) + mx * 0.08 + lerp(0, 0.22, shift);

    // iPhone : arrive par la droite, dos visible (les objectifs), puis se retourne.
    const arrive = ease(range(sp, 0.55, 0.82));
    const turn = ease(range(sp, 0.7, 1));
    const px = mobile ? 0.75 : 2.55;
    phone.position.set(lerp(px + 2.5, px, arrive), lerp(-0.4, mobile ? -0.15 : -0.05, arrive), lerp(0.4, 1.0, arrive));
    phone.rotation.set(0.08, lerp(Math.PI * 1.05, -0.35, turn) + mx * 0.1, lerp(0.25, 0.04, arrive));
    phone.visible = arrive > 0.001;

    const dist = mobile ? 9.6 / Math.min(1, camera.aspect * 1.3) : 8.4;
    camera.position.set(lerp(0.4, 0.9, shift) + mx * 0.3, lerp(2.4, 1.3, open), dist);
    camera.lookAt(mobile ? 0.3 : 0.95, mobile ? -1.0 : 0.05, 0);
    renderer.render(scene, camera);
  }
  requestAnimationFrame(frame);
}
