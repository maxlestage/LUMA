// Labo : une photo générée dans le shader en lumière linéaire (le soleil vaut bien plus
// que 1.0), puis développée avec les réglages du panneau. À gauche de la barre,
// l'original (réglages neutres) ; à droite, le rendu. Aucun pixel source n'est modifié.

const VERT = `attribute vec2 p; varying vec2 vUv; void main(){ vUv = p * .5 + .5; gl_Position = vec4(p, 0., 1.); }`;

const FRAG = `
precision highp float;
varying vec2 vUv;
uniform vec2 uRes; uniform float uTime, uSplit, uAspect;
uniform float uExposure, uContrast, uHighlights, uShadows, uTemp, uTint, uSat, uVignette;

float hash(vec2 p){ p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float noise(vec2 p){ vec2 i = floor(p), f = fract(p); f = f*f*(3.-2.*f);
  return mix(mix(hash(i), hash(i+vec2(1,0)), f.x), mix(hash(i+vec2(0,1)), hash(i+vec2(1,1)), f.x), f.y); }
float fbm(vec2 p){ float v = 0., a = .5; for (int i = 0; i < 6; i++){ v += a * noise(p); p *= 2.03; a *= .5; } return v; }

// Scène en lumière linéaire, valeurs HDR.
vec3 sky(vec2 uv, vec2 sun){
  float h = uv.y;
  vec3 top = vec3(.035, .045, .16), mid = vec3(.32, .12, .34), low = vec3(1.6, .55, .18);
  vec3 c = mix(low, mid, smoothstep(.5, .75, h));
  c = mix(c, top, smoothstep(.72, 1.05, h));
  float d = length((uv - sun) * vec2(uAspect, 1.));
  c += vec3(4., 2.2, .9) * exp(-d * 9.) + vec3(1.4, .6, .25) * exp(-d * 2.6);
  c += vec3(60., 45., 28.) * smoothstep(.034, .03, d);           // disque solaire, très au‑delà du blanc
  float cl = fbm(vec2(uv.x * 3. * uAspect + uTime * .01, uv.y * 9.));
  float band = smoothstep(.55, .78, uv.y) * smoothstep(.98, .78, uv.y);
  vec3 cloudCol = mix(vec3(1.8, .7, .35), vec3(.25, .12, .3), smoothstep(.6, .9, uv.y));
  c = mix(c, cloudCol * (1. + 2. * exp(-d * 4.)), smoothstep(.5, .75, cl) * band * .85);
  return c;
}
float ridge(float x, float s, float f){ return fbm(vec2(x * f + s, s)) ; }

vec3 scene(vec2 uv){
  vec2 sun = vec2(.62, .565);
  float hz = .5;
  if (uv.y > hz) {
    vec3 c = sky(uv, sun);
    float x = uv.x * uAspect;
    float m1 = hz + .2 * ridge(x, 1.3, 1.1) - .02;
    float m2 = hz + .13 * ridge(x, 7.1, 1.8) - .025;
    float m3 = hz + .06 * ridge(x, 3.7, 3.5) - .01;
    vec3 haze = vec3(1.2, .45, .22);
    if (uv.y < m1) c = mix(vec3(.16, .07, .16), haze, .35 + .4 * smoothstep(m1 - .15, m1, uv.y));
    if (uv.y < m2) c = mix(vec3(.06, .03, .08), haze * .6, .2 + .3 * smoothstep(m2 - .1, m2, uv.y));
    if (uv.y < m3) c = vec3(.015, .012, .025) + haze * .04;
    return c;
  }
  // Lac : reflet ondulé du ciel et des montagnes.
  float depth = (hz - uv.y) / hz;
  vec2 r = vec2(uv.x + (noise(vec2(uv.x * 40., uv.y * 300. + uTime * .6)) - .5) * .012 * depth, hz + (hz - uv.y) * .9);
  vec3 refl = scene_sky_only(r);
  vec3 c = refl * mix(.75, .25, depth);
  float glint = smoothstep(.85, 1., noise(vec2(uv.x * 90., uv.y * 900. - uTime))) * exp(-abs(uv.x - sun.x) * uAspect * 9.) * (1. - depth);
  c += vec3(6., 3.5, 1.4) * glint;
  float shore = .1 + .05 * fbm(vec2(uv.x * uAspect * 2.5, 2.));
  if (uv.y < shore) c = vec3(.008, .007, .012);
  return c;
}
`;

// Le reflet a besoin du ciel et des montagnes sans le lac : on duplique la partie haute.
const SKY_ONLY = `
vec3 scene_sky_only(vec2 uv){
  vec2 sun = vec2(.62, .565);
  float hz = .5;
  vec3 c = sky(uv, sun);
  float x = uv.x * uAspect;
  float m1 = hz + .2 * ridge(x, 1.3, 1.1) - .02;
  float m2 = hz + .13 * ridge(x, 7.1, 1.8) - .025;
  float m3 = hz + .06 * ridge(x, 3.7, 3.5) - .01;
  vec3 haze = vec3(1.2, .45, .22);
  if (uv.y < m1) c = mix(vec3(.16, .07, .16), haze, .35);
  if (uv.y < m2) c = mix(vec3(.06, .03, .08), haze * .6, .2);
  if (uv.y < m3) c = vec3(.015, .012, .025);
  return c;
}
`;

const DEVELOP = `
float luma(vec3 c){ return dot(c, vec3(.2126, .7152, .0722)); }
vec3 aces(vec3 x){ return clamp((x * (2.51 * x + .03)) / (x * (2.43 * x + .59) + .14), 0., 1.); }
vec3 toSRGB(vec3 c){ return mix(c * 12.92, 1.055 * pow(c, vec3(1. / 2.4)) - .055, step(.0031308, c)); }

vec3 develop(vec3 c, vec2 uv, bool edited){
  float ex = 0., co = 0., hi = 0., sh = 0., te = 0., ti = 0., sa = 0., vi = 0.;
  if (edited) { ex = uExposure; co = uContrast; hi = uHighlights; sh = uShadows; te = uTemp; ti = uTint; sa = uSat; vi = uVignette; }
  // Balance des blancs (multiplicateurs, en linéaire)
  c *= vec3(1. + te * .35, 1. - ti * .18, 1. - te * .35) * vec3(1. + ti * .12, 1., 1. + ti * .12);
  // Exposition en IL
  c *= exp2(ex);
  // Hautes lumières et ombres : masques doux sur la luminance logarithmique
  float L = max(luma(c), 1e-5), lg = log2(L);
  float hiMask = smoothstep(-1.5, 2.5, lg), shMask = 1. - smoothstep(-6., -1.5, lg);
  c *= exp2(hi * hiMask * 1.6 + sh * shMask * 1.8);
  // Rendu filmique, puis contraste en espace perceptuel
  vec3 t = aces(c * .6);
  vec3 g = pow(t, vec3(1. / 2.2));
  g = clamp((g - .5) * (1. + co * .9) + .5, 0., 1.);
  float y = luma(g);
  g = mix(vec3(y), g, 1. + sa);
  if (edited) {
    float d = length((uv - .5) * vec2(uAspect, 1.)) / length(vec2(uAspect, 1.) * .5);
    g *= 1. - vi * smoothstep(.35, 1.05, d) * .85;
  }
  return pow(clamp(g, 0., 1.), vec3(2.2));
}

void main(){
  vec2 uv = vUv;
  vec3 raw = scene(uv);
  bool edited = uv.x > uSplit;
  vec3 c = develop(raw, uv, edited);
  c = toSRGB(c);
  c += (hash(uv * uRes + fract(uTime)) - .5) / 255.;   // tramage, évite les bandes
  gl_FragColor = vec4(c, 1.);
}
`;

const PRESETS = {
  natural: { exposure: 0.15, contrast: 0.12, highlights: -0.55, shadows: 0.4, temp: 0.08, tint: 0, sat: 0.12, vignette: 0.2 },
  golden: { exposure: 0.3, contrast: 0.18, highlights: -0.7, shadows: 0.5, temp: 0.6, tint: 0.15, sat: 0.3, vignette: 0.35 },
  cine: { exposure: -0.2, contrast: 0.4, highlights: -0.4, shadows: 0.2, temp: -0.35, tint: 0.3, sat: -0.15, vignette: 0.6 },
  mono: { exposure: 0.1, contrast: 0.45, highlights: -0.5, shadows: 0.3, temp: 0, tint: 0, sat: -1, vignette: 0.45 },
  reset: { exposure: 0, contrast: 0, highlights: 0, shadows: 0, temp: 0, tint: 0, sat: 0, vignette: 0 },
};

export function initLab(root, { reduced = false } = {}) {
  const canvas = root.querySelector('#lab-gl');
  const gl = canvas.getContext('webgl', { antialias: false, preserveDrawingBuffer: false });
  if (!gl) { root.classList.add('no-webgl'); return; }

  const sh = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  };
  // GLSL ES 1.0 n'a pas de déclaration anticipée : la fonction du reflet est insérée
  // avant scene() qui l'appelle.
  const frag = FRAG.replace('vec3 scene(vec2 uv){', SKY_ONLY + '\nvec3 scene(vec2 uv){') + DEVELOP;
  const prog = gl.createProgram();
  gl.attachShader(prog, sh(gl.VERTEX_SHADER, VERT));
  gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, frag));
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog));
  gl.useProgram(prog);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  const loc = gl.getAttribLocation(prog, 'p');
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  const U = (n) => gl.getUniformLocation(prog, n);
  const uni = {
    res: U('uRes'), time: U('uTime'), split: U('uSplit'), aspect: U('uAspect'),
    exposure: U('uExposure'), contrast: U('uContrast'), highlights: U('uHighlights'), shadows: U('uShadows'),
    temp: U('uTemp'), tint: U('uTint'), sat: U('uSat'), vignette: U('uVignette'),
  };

  const values = { ...PRESETS.reset };
  const target = { ...PRESETS.natural };
  const inputs = [...root.querySelectorAll('input[data-u]')];
  const fmt = (k, v) => k === 'exposure' ? `${v >= 0 ? '+' : ''}${v.toFixed(2)} IL` : `${v >= 0 ? '+' : ''}${Math.round(v * 100)}`;
  const syncInputs = () => inputs.forEach((i) => {
    i.value = target[i.dataset.u];
    i.previousElementSibling.querySelector('output').textContent = fmt(i.dataset.u, target[i.dataset.u]);
  });
  syncInputs();
  const presetButtons = [...root.querySelectorAll('[data-preset]')];
  inputs.forEach((i) => i.addEventListener('input', () => {
    target[i.dataset.u] = values[i.dataset.u] = parseFloat(i.value);
    i.previousElementSibling.querySelector('output').textContent = fmt(i.dataset.u, target[i.dataset.u]);
    presetButtons.forEach((b) => b.classList.remove('on'));
    dirty = true;
  }));
  presetButtons.forEach((b) => b.addEventListener('click', () => {
    Object.assign(target, PRESETS[b.dataset.preset]);
    presetButtons.forEach((x) => x.classList.toggle('on', x === b));
    syncInputs();
    dirty = true;
  }));

  // Barre avant/après
  const view = root.querySelector('.lab-view');
  const handle = root.querySelector('.lab-split');
  let split = 0.5, splitTarget = 0.5, dragging = false;
  const setSplit = (v) => {
    splitTarget = Math.min(0.98, Math.max(0.02, v));
    handle.setAttribute('aria-valuenow', Math.round(splitTarget * 100));
    dirty = true;
  };
  const fromEvent = (e) => { const r = view.getBoundingClientRect(); setSplit((e.clientX - r.left) / r.width); };
  handle.addEventListener('pointerdown', (e) => { dragging = true; handle.setPointerCapture(e.pointerId); fromEvent(e); });
  handle.addEventListener('pointermove', (e) => dragging && fromEvent(e));
  handle.addEventListener('pointerup', () => { dragging = false; });
  handle.addEventListener('pointercancel', () => { dragging = false; });
  view.addEventListener('click', (e) => { if (e.target === canvas) fromEvent(e); });
  handle.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowLeft') { setSplit(splitTarget - 0.05); e.preventDefault(); }
    if (e.key === 'ArrowRight') { setSplit(splitTarget + 0.05); e.preventDefault(); }
  });

  let w = 0, h = 0;
  function resize() {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    w = Math.round(canvas.clientWidth * dpr); h = Math.round(canvas.clientHeight * dpr);
    canvas.width = w; canvas.height = h;
    gl.viewport(0, 0, w, h);
    dirty = true;
  }
  let dirty = true;
  window.addEventListener('resize', resize);
  resize();

  let visible = false, intro = false;
  new IntersectionObserver(([e]) => {
    visible = e.isIntersecting;
    // Première apparition : la barre balaie l'image pour montrer la différence.
    if (visible && !intro) { intro = true; split = 0.96; setSplit(0.5); }
  }, { threshold: 0.25 }).observe(view);

  const fpsEl = root.querySelector('.lab-fps');
  let frames = 0, lastFps = performance.now();
  const t0 = performance.now();
  function frame(now) {
    requestAnimationFrame(frame);
    if (!visible) return;
    let moving = false;
    for (const k in values) {
      const d = target[k] - values[k];
      if (Math.abs(d) > 1e-4) { values[k] += d * (reduced ? 1 : 0.14); moving = true; } else values[k] = target[k];
    }
    const ds = splitTarget - split;
    if (Math.abs(ds) > 1e-4) { split += ds * (dragging || reduced ? 1 : 0.08); moving = true; } else split = splitTarget;
    handle.parentElement.style.setProperty('--split', `${split * 100}%`);

    // L'eau ondule : on redessine en continu, sauf en mouvement réduit.
    if (!moving && !dirty && reduced) return;
    dirty = false;
    gl.uniform2f(uni.res, w, h);
    gl.uniform1f(uni.time, reduced ? 0 : (now - t0) / 1000);
    gl.uniform1f(uni.split, split);
    gl.uniform1f(uni.aspect, w / h);
    for (const k in values) gl.uniform1f(uni[k], values[k]);
    gl.drawArrays(gl.TRIANGLES, 0, 3);

    frames++;
    if (now - lastFps > 500) {
      fpsEl.textContent = `${Math.round((frames * 1000) / (now - lastFps))} i/s · GPU`;
      frames = 0; lastFps = now;
    }
  }
  requestAnimationFrame(frame);
}
