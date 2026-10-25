// Hero 3D : l'anneau de lumière du logo de LÜMA (l'éclipse), dans lequel s'engouffre un
// flux de lumière aux couleurs du logo. Le défilement avive le halo, accélère la
// lumière et fait entrer la caméra dans l'anneau.

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

// Les couleurs de l'anneau du logo (luma-brand, STOPS) : angle depuis midi, sens horaire.
const RING_STOPS = [
  [0, 0x7a62ff], [40, 0x2f5cff], [90, 0x4cc6ff], [135, 0x5a86ff], [165, 0x9d58ff],
  [195, 0xe85cb0], [228, 0xff7838], [268, 0xffc466], [305, 0xf6cfae], [335, 0xb08cf0], [360, 0x7a62ff],
];

// Une table de 360 couleurs (lumière linéaire), comme ring_color de luma-brand.
function ringTexture() {
  const data = new Uint8Array(360 * 4);
  const lin = (v) => Math.pow(v / 255, 2.2);
  for (let a = 0; a < 360; a++) {
    let k = RING_STOPS.findIndex((s) => s[0] >= a);
    k = Math.max(1, k < 0 ? RING_STOPS.length - 1 : k);
    const [a0, c0] = RING_STOPS[k - 1], [a1, c1] = RING_STOPS[k];
    const t = (a - a0) / Math.max(a1 - a0, 1e-6);
    for (let i = 0; i < 3; i++) {
      const sh = 16 - 8 * i;
      const v = lin((c0 >> sh) & 255) * (1 - t) + lin((c1 >> sh) & 255) * t;
      data[a * 4 + i] = Math.round(Math.pow(v, 1 / 2.2) * 255);
    }
    data[a * 4 + 3] = 255;
  }
  const tex = new THREE.DataTexture(data, 360, 1, THREE.RGBAFormat);
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.wrapS = THREE.RepeatWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.needsUpdate = true;
  return tex;
}

// L'anneau de lumière du logo, en croissant : le même dessin que luma-brand (disque
// intérieur décalé vers le haut à droite, tube plus clair en dedans, halo), sur un
// plan, en lumière additive.
function buildRing() {
  const lens = new THREE.Group();
  const mat = new THREE.ShaderMaterial({
    transparent: true, depthWrite: false, blending: THREE.AdditiveBlending,
    uniforms: { uRing: { value: ringTexture() }, uTime: { value: 0 }, uOpen: { value: 0 } },
    vertexShader: /* glsl */`
      varying vec2 vUv;
      void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }`,
    fragmentShader: /* glsl */`
      uniform sampler2D uRing; uniform float uTime, uOpen;
      varying vec2 vUv;
      void main() {
        vec2 p = vUv - 0.5;
        float r = 0.25;
        float d = length(p);
        float deg = degrees(atan(p.x, p.y));
        if (deg < 0.0) deg += 360.0;
        vec3 col = texture2D(uRing, vec2(deg / 360.0, 0.5)).rgb;
        // L'éclat : or et bleu plus vifs, et une lente respiration.
        float e = 0.78 + 0.22 * abs(sin(2.0 * (radians(deg) - 0.35)));
        e *= 1.0 + 0.08 * sin(uTime * 0.9 + radians(deg) * 2.0);
        vec2 o = vec2(0.10, 0.09) * r;
        float inner = 0.82 * r;
        float di = length(p - o);
        float aa = fwidth(d) * 1.2;
        float body = (1.0 - smoothstep(r - aa, r + aa, d)) * smoothstep(inner - aa, inner + aa, di);
        float frac = clamp((di - inner) / max((r - d) + (di - inner), 1e-5), 0.0, 1.0);
        float core = 0.5 + 1.05 * exp(-pow((frac - 0.4) / 0.38, 2.0));
        vec3 light = col * body * core * e;
        float thick = smoothstep(0.0, 1.0, max((-(p.x * 0.75) + p.y * 0.66) / r, 0.0));
        light += body * exp(-pow((frac - 0.42) / 0.22, 2.0)) * thick * 0.4;
        float outGlow = exp(-pow(max(d - r, 0.0) / (0.15 * r * (1.0 + uOpen)), 2.0)) * smoothstep(0.86 * r, r, d);
        float inGlow = exp(-pow(max(inner - di, 0.0) / (0.12 * r), 2.0)) * (1.0 - smoothstep(inner - aa, inner + aa, di));
        light += col * (0.5 * outGlow * (1.0 - body) + 0.14 * inGlow) * e * (1.0 + uOpen * 0.6);
        gl_FragColor = vec4(light * 0.72, 1.0);
      }`,
  });
  const plane = new THREE.Mesh(new THREE.PlaneGeometry(6.4, 6.4), mat);
  lens.add(plane);
  function setAperture(open) { mat.uniforms.uOpen.value = open; }
  function sync(t) { mat.uniforms.uTime.value = t; }
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
        vec3 c0 = vec3(1.0, .47, .22), c1 = vec3(1.0, .77, .40), c2 = vec3(.91, .36, .69), c3 = vec3(.30, .78, 1.0), c4 = vec3(.48, .38, 1.0);
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

  const { lens, setAperture, sync } = buildRing();
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
      ? { x: 0, y: 1.2 + (1 / camera.aspect - 1) * 0.6, s: 0.5 }
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
    // L'anneau suit un peu la souris, en restant face à nous.
    lens.rotation.y = lerp(state.mx * 0.22 + (reduced ? 0 : Math.sin(t * 0.35) * 0.05), 0, p);
    lens.rotation.x = lerp(state.my * 0.16, 0, p);

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
    sync(t);
    renderer.render(scene, camera);
  }
  frame();

  return {
    // À la fin du préchargement : le diaphragme s'entrouvre.
    intro() { state.openTarget = 0.38; },
  };
}
