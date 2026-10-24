// Hero 3D : un objectif photo (fût métallique, bague moletée, diaphragme à 9 lamelles,
// lentille frontale traitée) dans lequel s'engouffre un flux de lumière spectrale.
// Le défilement ouvre le diaphragme, accélère la lumière et fait entrer la caméra.

import * as THREE from '../vendor/three.module.min.js';
import { RoomEnvironment } from '../vendor/RoomEnvironment.js';

const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const lerp = (a, b, t) => a + (b - a) * t;
const smooth = (t) => t * t * (3 - 2 * t);

const SPECTRUM = [0xff6b3d, 0xffcf4a, 0x4af2c8, 0x4aa8ff, 0x8b7bff].map((c) => new THREE.Color(c));
function spectrum(t, out = new THREE.Color()) {
  t = ((t % 1) + 1) % 1;
  const f = t * SPECTRUM.length, i = Math.floor(f);
  return out.copy(SPECTRUM[i % SPECTRUM.length]).lerp(SPECTRUM[(i + 1) % SPECTRUM.length], f - i);
}

function glowTexture() {
  const c = document.createElement('canvas');
  c.width = c.height = 256;
  const g = c.getContext('2d');
  const grad = g.createRadialGradient(128, 128, 0, 128, 128, 128);
  grad.addColorStop(0, 'rgba(255,250,235,1)');
  grad.addColorStop(0.18, 'rgba(255,207,74,.85)');
  grad.addColorStop(0.45, 'rgba(255,107,61,.35)');
  grad.addColorStop(0.7, 'rgba(139,123,255,.15)');
  grad.addColorStop(1, 'rgba(0,0,0,0)');
  g.fillStyle = grad; g.fillRect(0, 0, 256, 256);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

function buildLens() {
  const lens = new THREE.Group();
  const metal = new THREE.MeshStandardMaterial({ color: 0x15151a, metalness: 0.9, roughness: 0.32 });
  const satin = new THREE.MeshStandardMaterial({ color: 0x2a2a31, metalness: 1, roughness: 0.22 });
  const rubber = new THREE.MeshStandardMaterial({ color: 0x0b0b0e, metalness: 0.2, roughness: 0.75 });

  // Fût : profil tourné autour de l'axe (y), puis basculé vers la caméra (z).
  const body = new THREE.Group();
  const profile = [
    [0.86, -2.6], [1.0, -2.6], [1.06, -2.42], [1.16, -2.3], [1.16, -1.5], [1.24, -1.44],
    [1.24, -0.42], [1.2, -0.34], [1.34, -0.26], [1.36, -0.04], [1.32, 0.02], [1.1, 0.03], [1.04, -0.06],
  ].map(([r, y]) => new THREE.Vector2(r, y));
  body.add(new THREE.Mesh(new THREE.LatheGeometry(profile, 160), metal));

  // Bague de mise au point moletée
  const grip = new THREE.CylinderGeometry(1.29, 1.29, 0.82, 360, 1, true);
  const p = grip.attributes.position;
  for (let i = 0; i < p.count; i++) {
    const x = p.getX(i), z = p.getZ(i), a = Math.atan2(z, x);
    const k = 1 + (Math.sin(a * 120) > 0 ? 0.012 : -0.004);
    p.setX(i, x * k); p.setZ(i, z * k);
  }
  grip.computeVertexNormals();
  const gripMesh = new THREE.Mesh(grip, rubber);
  gripMesh.position.y = -0.92;
  body.add(gripMesh);

  // Bague satinée et liseré spectral, la signature de LÜMA
  const ring = new THREE.Mesh(new THREE.CylinderGeometry(1.25, 1.25, 0.08, 160, 1, true), satin);
  ring.position.y = -1.42; body.add(ring);
  const accentGeo = new THREE.TorusGeometry(1.365, 0.014, 10, 260);
  const colors = [];
  const ap = accentGeo.attributes.position, col = new THREE.Color();
  for (let i = 0; i < ap.count; i++) {
    spectrum(Math.atan2(ap.getY(i), ap.getX(i)) / (Math.PI * 2) + 0.5, col);
    colors.push(col.r, col.g, col.b);
  }
  accentGeo.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
  const accent = new THREE.Mesh(accentGeo, new THREE.MeshBasicMaterial({ vertexColors: true, toneMapped: false }));
  accent.rotation.x = Math.PI / 2; accent.position.y = -0.16;
  body.add(accent);
  body.rotation.x = Math.PI / 2;
  lens.add(body);

  // Lueur intérieure, visible à travers le diaphragme
  const glow = new THREE.Mesh(
    new THREE.PlaneGeometry(2.6, 2.6),
    new THREE.MeshBasicMaterial({ map: glowTexture(), transparent: true, blending: THREE.AdditiveBlending, depthWrite: false, toneMapped: false }),
  );
  glow.position.z = -0.75;
  lens.add(glow);

  // Diaphragme : 9 lamelles qui pivotent autour du bord.
  const N = 9;
  const shape = new THREE.Shape();
  shape.moveTo(0, 0);
  shape.quadraticCurveTo(0.85, 0.06, 1.75, 0);
  shape.lineTo(1.75, -0.72);
  shape.quadraticCurveTo(0.9, -0.66, 0, -0.62);
  shape.closePath();
  const bladeGeo = new THREE.ShapeGeometry(shape, 12);
  const bladeMat = new THREE.MeshStandardMaterial({ color: 0x0e0e12, metalness: 0.7, roughness: 0.42, side: THREE.DoubleSide });
  // Les lamelles dépassent du cercle quand elles s'ouvrent : on les découpe à r = 1.05,
  // dans le repère du diaphragme, pour qu'elles restent dans le fût.
  const uInv = { value: new THREE.Matrix4() };
  bladeMat.onBeforeCompile = (sh) => {
    sh.uniforms.uInv = uInv;
    sh.vertexShader = 'varying vec3 vWP;\n' + sh.vertexShader.replace(
      '#include <project_vertex>', '#include <project_vertex>\n vWP = (modelMatrix * vec4(transformed, 1.0)).xyz;');
    sh.fragmentShader = 'uniform mat4 uInv;\nvarying vec3 vWP;\n' + sh.fragmentShader.replace(
      'void main() {', 'void main() {\n if (length((uInv * vec4(vWP, 1.0)).xy) > 1.05) discard;');
  };
  const iris = new THREE.Group();
  const blades = [];
  for (let i = 0; i < N; i++) {
    const theta = (i / N) * Math.PI * 2;
    const pivot = new THREE.Group();
    pivot.position.set(Math.cos(theta) * 1.02, Math.sin(theta) * 1.02, -0.32 - i * 0.004);
    const blade = new THREE.Mesh(bladeGeo, bladeMat);
    pivot.add(blade);
    pivot.userData.theta = theta;
    iris.add(pivot);
    blades.push(pivot);
  }
  lens.add(iris);
  const tube = new THREE.Mesh(new THREE.CylinderGeometry(1.04, 1.04, 0.9, 120, 1, true), rubber.clone());
  tube.material.side = THREE.BackSide;
  tube.rotation.x = Math.PI / 2; tube.position.z = -0.5;
  lens.add(tube);
  // Cache‑lamelles : un anneau devant le diaphragme pour masquer le bord extérieur.
  const lip = new THREE.Mesh(new THREE.RingGeometry(1.0, 1.36, 160), metal);
  lip.position.z = -0.08;
  lens.add(lip);

  // Lentille frontale, traitement multicouche irisé
  const thetaLen = Math.asin(1.06 / 2.2);
  const glassGeo = new THREE.SphereGeometry(2.2, 96, 24, 0, Math.PI * 2, 0, thetaLen);
  const glass = new THREE.Mesh(glassGeo, new THREE.MeshPhysicalMaterial({
    color: 0x1a1a2e, metalness: 0, roughness: 0.04, transparent: true, opacity: 0.32,
    clearcoat: 1, clearcoatRoughness: 0.03, iridescence: 1, iridescenceIOR: 1.7,
    iridescenceThicknessRange: [180, 640], envMapIntensity: 2.6, depthWrite: false,
  }));
  glass.rotation.x = Math.PI / 2;
  glass.position.z = -2.2 * Math.cos(thetaLen) - 0.06;
  lens.add(glass);

  function setAperture(open) {
    // open ∈ [0, 1] : rayon d'ouverture ≈ cos(φ) (voir la géométrie des lamelles).
    const r = lerp(0.1, 0.94, open);
    const phi = Math.acos(r);
    for (const b of blades) b.rotation.z = b.userData.theta + Math.PI / 2 + phi;
  }
  function sync() {
    iris.updateWorldMatrix(true, false);
    uInv.value.copy(iris.matrixWorld).invert();
  }
  setAperture(0.2);
  return { lens, setAperture, sync };
}

function buildLight(count) {
  const geo = new THREE.BufferGeometry();
  const seed = new Float32Array(count * 4);
  const extra = new Float32Array(count * 2);
  for (let i = 0; i < count; i++) {
    seed[i * 4] = Math.pow(Math.random(), 0.6);       // rayon de départ
    seed[i * 4 + 1] = Math.random() * Math.PI * 2;    // angle
    seed[i * 4 + 2] = Math.random();                  // phase
    seed[i * 4 + 3] = 0.6 + Math.random() * 0.8;      // vitesse
    extra[i * 2] = Math.random();                     // teinte
    extra[i * 2 + 1] = 0.4 + Math.pow(Math.random(), 3) * 2.2; // taille
  }
  geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(count * 3), 3));
  geo.setAttribute('aSeed', new THREE.BufferAttribute(seed, 4));
  geo.setAttribute('aExtra', new THREE.BufferAttribute(extra, 2));
  geo.boundingSphere = new THREE.Sphere(new THREE.Vector3(), 100);

  const mat = new THREE.ShaderMaterial({
    transparent: true, depthWrite: false, blending: THREE.AdditiveBlending,
    uniforms: {
      uTime: { value: 0 }, uFlow: { value: 0 }, uPR: { value: 1 },
      uCenter: { value: new THREE.Vector3() }, uScale: { value: 1 }, uFade: { value: 1 },
    },
    vertexShader: /* glsl */`
      attribute vec4 aSeed; attribute vec2 aExtra;
      uniform float uTime, uFlow, uPR, uScale, uFade; uniform vec3 uCenter;
      varying float vHue; varying float vA;
      void main() {
        float t = fract(aSeed.z + uFlow * aSeed.w);
        float funnel = mix(6.5, 0.12, pow(t, 1.35));
        float a = aSeed.y + t * 4.0 + uTime * 0.08;
        float r = aSeed.x * funnel * uScale + 0.05;
        vec3 p = uCenter + vec3(cos(a) * r, sin(a) * r * 0.9, mix(7.5, -0.4, t) * uScale);
        vec4 mv = modelViewMatrix * vec4(p, 1.0);
        gl_Position = projectionMatrix * mv;
        gl_PointSize = aExtra.y * uPR * (26.0 / -mv.z) * (0.7 + t * 0.9);
        vHue = aExtra.x + t * 0.25;
        vA = smoothstep(0.0, 0.18, t) * (1.0 - smoothstep(0.86, 1.0, t)) * uFade;
      }`,
    fragmentShader: /* glsl */`
      varying float vHue; varying float vA;
      vec3 spec(float t) {
        t = fract(t);
        vec3 c0 = vec3(1.0, .42, .24), c1 = vec3(1.0, .81, .29), c2 = vec3(.29, .95, .78), c3 = vec3(.29, .66, 1.0), c4 = vec3(.55, .48, 1.0);
        float f = t * 5.0;
        if (f < 1.0) return mix(c0, c1, f);
        if (f < 2.0) return mix(c1, c2, f - 1.0);
        if (f < 3.0) return mix(c2, c3, f - 2.0);
        if (f < 4.0) return mix(c3, c4, f - 3.0);
        return mix(c4, c0, f - 4.0);
      }
      void main() {
        float d = length(gl_PointCoord - 0.5);
        float m = smoothstep(0.5, 0.0, d);
        m *= m;
        gl_FragColor = vec4(spec(vHue) * m * vA * 1.4, 1.0);
      }`,
  });
  return new THREE.Points(geo, mat);
}

export function initHero(canvas, { reduced = false } = {}) {
  let renderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: 'high-performance' });
  } catch (e) {
    document.documentElement.classList.add('no-webgl');
    return { intro() {} };
  }
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

  const camera = new THREE.PerspectiveCamera(32, 1, 0.1, 100);
  camera.position.set(0, 0, 10);

  const key = new THREE.DirectionalLight(0xffffff, 1.4); key.position.set(4, 5, 6); scene.add(key);
  const rimA = new THREE.PointLight(0xff6b3d, 30, 12); rimA.position.set(-3, 2.5, 1); scene.add(rimA);
  const rimB = new THREE.PointLight(0x8b7bff, 30, 12); rimB.position.set(3, -2.5, 1); scene.add(rimB);

  const { lens, setAperture, sync } = buildLens();
  const holder = new THREE.Group();
  holder.add(lens);
  scene.add(holder);

  const light = buildLight(mobile ? 2600 : 6500);
  light.material.uniforms.uPR.value = dpr;
  scene.add(light);

  const state = { mx: 0, my: 0, tx: 0, ty: 0, scroll: 0, flow: 0, open: 0.12, openTarget: 0.12, visible: true, intro: 0 };
  let layout = { x: 0, y: 0, s: 1 };

  function resize() {
    const w = window.innerWidth, h = window.innerHeight;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    const narrow = camera.aspect < 1.05;
    layout = narrow
      ? { x: 0, y: 1.25 + (1 / camera.aspect - 1) * 0.6, s: 0.62 }
      : { x: clamp(1.45 * camera.aspect, 1.9, 3.1), y: -0.1, s: 0.9 };
  }
  resize();
  window.addEventListener('resize', resize);

  if (!reduced) {
    window.addEventListener('pointermove', (e) => {
      state.tx = (e.clientX / window.innerWidth) * 2 - 1;
      state.ty = (e.clientY / window.innerHeight) * 2 - 1;
    }, { passive: true });
  }
  window.addEventListener('scroll', () => {
    state.scroll = clamp(window.scrollY / window.innerHeight, 0, 2);
  }, { passive: true });
  document.addEventListener('visibilitychange', () => { state.visible = !document.hidden; });

  const clock = new THREE.Clock();
  let last = 0;
  function frame() {
    requestAnimationFrame(frame);
    const opacity = 1 - clamp((state.scroll - 0.75) / 0.6);
    canvas.style.opacity = opacity;
    if (!state.visible || opacity <= 0.001) { clock.getDelta(); return; }
    const dt = Math.min(clock.getDelta(), 0.05);
    const t = (last += dt);

    state.mx = lerp(state.mx, state.tx, 0.05);
    state.my = lerp(state.my, state.ty, 0.05);
    const p = smooth(clamp(state.scroll / 1.1));
    state.intro = lerp(state.intro, 1, 0.025);

    // Diaphragme : respire au repos, s'ouvre grand quand on défile.
    const breathe = reduced ? 0 : Math.sin(t * 0.8) * 0.04;
    state.open = lerp(state.open, Math.max(state.openTarget + breathe, p * 1.05), 0.06);
    setAperture(clamp(state.open));

    // Objectif : tourne vers la caméra, se centre et grossit avec le défilement.
    const s = layout.s * lerp(1, 1.9, p) * lerp(0.85, 1, state.intro);
    holder.position.set(lerp(layout.x, 0, p), lerp(layout.y, 0, p), lerp(0, 2.2, p));
    holder.scale.setScalar(s);
    lens.rotation.y = lerp(-0.62 + state.mx * 0.25 + (reduced ? 0 : Math.sin(t * 0.35) * 0.06), 0, p);
    lens.rotation.x = lerp(0.18 + state.my * 0.18, 0, p);
    lens.rotation.z = t * 0.03;

    // Lumière : accélère avec le défilement.
    const u = light.material.uniforms;
    state.flow += dt * (0.05 + p * 0.5) * (reduced ? 0.2 : 1);
    u.uFlow.value = state.flow;
    u.uTime.value = t;
    u.uScale.value = s;
    u.uCenter.value.copy(holder.position);
    u.uFade.value = state.intro;

    camera.position.x = state.mx * 0.25;
    camera.position.y = -state.my * 0.18;
    camera.lookAt(0, 0, 0);
    sync();
    renderer.render(scene, camera);
  }
  frame();

  return {
    // À la fin du préchargement : le diaphragme s'entrouvre.
    intro() { state.openTarget = 0.38; },
  };
}
