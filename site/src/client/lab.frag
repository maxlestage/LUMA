// Le labo : une scene generee en lumiere lineaire (le soleil vaut bien plus que 1.0),
// puis developpee avec les reglages du panneau, comme le ferait le moteur de LUMA.

precision highp float;
varying vec2 vUv;
uniform vec2 uRes; uniform float uTime, uSplit, uAspect;
uniform float uExposure, uContrast, uHighlights, uShadows, uTemp, uTint, uSat, uVignette;

float hash(vec2 p){ p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float noise(vec2 p){ vec2 i = floor(p), f = fract(p); f = f*f*(3.-2.*f);
  return mix(mix(hash(i), hash(i+vec2(1,0)), f.x), mix(hash(i+vec2(0,1)), hash(i+vec2(1,1)), f.x), f.y); }
float fbm(vec2 p){ float v = 0., a = .5; for (int i = 0; i < 6; i++){ v += a * noise(p); p *= 2.03; a *= .5; } return v; }

// Scene en lumiere lineaire, valeurs HDR.
vec3 sky(vec2 uv, vec2 sun){
  float h = uv.y;
  vec3 top = vec3(.035, .045, .16), mid = vec3(.32, .12, .34), low = vec3(1.6, .55, .18);
  vec3 c = mix(low, mid, smoothstep(.5, .75, h));
  c = mix(c, top, smoothstep(.72, 1.05, h));
  float d = length((uv - sun) * vec2(uAspect, 1.));
  c += vec3(4., 2.2, .9) * exp(-d * 9.) + vec3(1.4, .6, .25) * exp(-d * 2.6);
  c += vec3(60., 45., 28.) * smoothstep(.034, .03, d);           // disque solaire, tres audela du blanc
  float cl = fbm(vec2(uv.x * 3. * uAspect + uTime * .01, uv.y * 9.));
  float band = smoothstep(.55, .78, uv.y) * smoothstep(.98, .78, uv.y);
  vec3 cloudCol = mix(vec3(1.8, .7, .35), vec3(.25, .12, .3), smoothstep(.6, .9, uv.y));
  c = mix(c, cloudCol * (1. + 2. * exp(-d * 4.)), smoothstep(.5, .75, cl) * band * .85);
  return c;
}
float ridge(float x, float s, float f){ return fbm(vec2(x * f + s, s)) ; }

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
  // Lac : reflet ondule du ciel et des montagnes.
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

float luma(vec3 c){ return dot(c, vec3(.2126, .7152, .0722)); }
vec3 aces(vec3 x){ return clamp((x * (2.51 * x + .03)) / (x * (2.43 * x + .59) + .14), 0., 1.); }
vec3 toSRGB(vec3 c){ return mix(c * 12.92, 1.055 * pow(c, vec3(1. / 2.4)) - .055, step(.0031308, c)); }

vec3 develop(vec3 c, vec2 uv, bool edited){
  float ex = 0., co = 0., hi = 0., sh = 0., te = 0., ti = 0., sa = 0., vi = 0.;
  if (edited) { ex = uExposure; co = uContrast; hi = uHighlights; sh = uShadows; te = uTemp; ti = uTint; sa = uSat; vi = uVignette; }
  // Balance des blancs (multiplicateurs, en lineaire)
  c *= vec3(1. + te * .35, 1. - ti * .18, 1. - te * .35) * vec3(1. + ti * .12, 1., 1. + ti * .12);
  // Exposition en IL
  c *= exp2(ex);
  // Hautes lumieres et ombres : masques doux sur la luminance logarithmique
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
  c += (hash(uv * uRes + fract(uTime)) - .5) / 255.;   // tramage, evite les bandes
  gl_FragColor = vec4(c, 1.);
}
