// Les étapes par pixel de la fin du développement, sur le GPU : rendu (courbe filmique,
// courbes des tonalités), couleur (OKLab : vibrance, saturation, mélangeur,
// étalonnage), film mesuré (LUT 3D), effets (vignetage, grain), puis encodage vers
// l'espace de sortie. Avec l'accentuation, en plusieurs passes : `pre` (jusqu'au film
// mesuré, et la luminance perçue), six passes de flou boîte (`box_h`, `box_v`), puis
// `post` (accentuation, effets, encodage).
//
// C'est le double, ligne à ligne, du code Rust de luma-image (lib.rs, color.rs,
// effects.rs) : les tests de luma-gpu comparent les deux sorties.
//
// Les constantes P_* (positions dans le tableau de paramètres) et T_* (positions des
// tables) sont ajoutées au début de ce fichier par lib.rs.

@group(0) @binding(0) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> P: array<f32>;
@group(0) @binding(2) var<storage, read> T: array<f32>;
@group(0) @binding(3) var<storage, read_write> dst: array<u32>;
// L'accentuation : l'image avant accentuation (RGB, et la luminance perçue en w), et
// deux plans de travail pour le flou (lu dans `pin`, écrit dans `pout`).
@group(0) @binding(4) var<storage, read_write> mid: array<vec4<f32>>;
@group(0) @binding(5) var<storage, read_write> pin: array<f32>;
@group(0) @binding(6) var<storage, read_write> pout: array<f32>;

const TAU: f32 = 6.283185307179586;
const PI: f32 = 3.141592653589793;

fn ss(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn lum(c: vec3<f32>) -> f32 {
    return 0.2627 * c.x + 0.6780 * c.y + 0.0593 * c.z;
}

// Une matrice 3 × 3 rangée par lignes à la position o du tableau de paramètres.
fn mat(o: u32, v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        P[o] * v.x + P[o + 1u] * v.y + P[o + 2u] * v.z,
        P[o + 3u] * v.x + P[o + 4u] * v.y + P[o + 5u] * v.z,
        P[o + 6u] * v.x + P[o + 7u] * v.y + P[o + 8u] * v.z,
    );
}

// Une table de 4097 valeurs sur [0, 1], interpolée (Lut et EncodeLut).
fn lut(o: u32, v: f32) -> f32 {
    let x = clamp(v, 0.0, 1.0) * 4096.0;
    let i = min(u32(x), 4095u);
    let t = x - f32(i);
    return T[o + i] + (T[o + i + 1u] - T[o + i]) * t;
}

// La courbe filmique, indexée par la racine quatrième (FilmicTable).
fn filmic(x: f32) -> f32 {
    let u = sqrt(sqrt(max(x, 0.0) / 4.0)) * 8192.0;
    if u >= 8192.0 {
        return 1.0;
    }
    let i = u32(u);
    let t = u - f32(i);
    return T[T_FILMIC + i] + (T[T_FILMIC + i + 1u] - T[T_FILMIC + i]) * t;
}

fn render(p: vec3<f32>) -> vec3<f32> {
    let mx = max(p.x, max(p.y, p.z));
    let n = max(0.5 * mx + 0.5 * lum(p), 1e-7);
    let f = filmic(n);
    var c = p * (f / n);
    let t = ss(0.72, 1.0, f) * P[P_ROLLOFF];
    c = c + (vec3<f32>(f) - c) * t;
    let over = max(c.x, max(c.y, c.z));
    if over > 1.0 && over > f {
        let k = (1.0 - f) / (over - f);
        c = vec3<f32>(f) + (c - vec3<f32>(f)) * k;
    }
    if P[P_CURVES] != 0.0 {
        let offs = array<u32, 3>(T_LUT_R, T_LUT_G, T_LUT_B);
        for (var k = 0u; k < 3u; k++) {
            if c[k] >= 0.0 {
                c[k] = lut(T_DECODE, lut(offs[k], lut(T_ENCODE, c[k])));
            }
        }
    }
    return c;
}

// La racine cubique rapide de luma-color (même calcul, bit à bit).
fn cbrt(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    var y = bitcast<f32>(bitcast<u32>(x) / 3u + 709921077u);
    y = (2.0 * y + x / (y * y)) * (1.0 / 3.0);
    y = (2.0 * y + x / (y * y)) * (1.0 / 3.0);
    return y;
}

fn to_lab(c: vec3<f32>) -> vec3<f32> {
    let lms = mat(P_TO_LMS, c);
    let lab = mat(P_LMS_TO_LAB, vec3<f32>(cbrt(lms.x), cbrt(lms.y), cbrt(lms.z)));
    return vec3<f32>(lab.x, lab.y - lab.x * P[P_WHITE_A], lab.z - lab.x * P[P_WHITE_B]);
}

fn from_lab(lab0: vec3<f32>) -> vec3<f32> {
    let lab = vec3<f32>(lab0.x, lab0.y + lab0.x * P[P_WHITE_A], lab0.z + lab0.x * P[P_WHITE_B]);
    let l = mat(P_LAB_TO_LMS, lab);
    return mat(P_FROM_LMS, l * l * l);
}

fn center(i: u32) -> f32 {
    return P[P_CENTERS + i];
}

fn gap(i: u32, forward: bool) -> f32 {
    if forward {
        var next = 0.0;
        if i == 7u { next = center(0u) + TAU; } else { next = center(i + 1u); }
        return next - center(i);
    }
    var prev = 0.0;
    if i == 0u { prev = center(7u) - TAU; } else { prev = center(i - 1u); }
    return center(i) - prev;
}

fn hsl(band: u32, k: u32) -> f32 {
    return P[P_HSL + band * 3u + k];
}

fn color(rgb: vec3<f32>) -> vec3<f32> {
    let lab0 = to_lab(rgb);
    if P[P_SIMPLE] != 0.0 {
        let k = (1.0 + P[P_SATURATION]) * P[P_PROFILE_CHROMA];
        return from_lab(vec3<f32>(lab0.x, lab0.y * k, lab0.z * k));
    }
    var l = lab0.x;
    let a0 = lab0.y;
    let b0 = lab0.z;
    let c0 = sqrt(a0 * a0 + b0 * b0);
    var c = c0;
    let h = atan2(b0, a0);
    var dh = 0.0;
    let chroma0 = c;

    let vibrance = P[P_VIBRANCE];
    if vibrance != 0.0 {
        var d = h - P[P_SKIN_HUE];
        if d > PI {
            d -= TAU;
        } else if d < -PI {
            d += TAU;
        }
        let q = d / 0.8;
        let s0 = max(1.0 - q * q, 0.0);
        let skin = s0 * s0;
        let w = (1.0 - ss(0.0, 0.2, c)) * (1.0 - 0.6 * skin);
        c *= 1.0 + vibrance * w;
    }
    c *= (1.0 + P[P_SATURATION]) * P[P_PROFILE_CHROMA];

    // Les deux bandes du mélangeur qui encadrent la teinte.
    var hh = h;
    while hh < center(0u) {
        hh += TAU;
    }
    while hh >= center(0u) + TAU {
        hh -= TAU;
    }
    var i = 7u;
    var j = 0u;
    var wi = 1.0;
    for (var k = 0u; k < 8u; k++) {
        var c1 = 0.0;
        if k == 7u { c1 = center(0u) + TAU; } else { c1 = center(k + 1u); }
        if hh < c1 {
            wi = 1.0 - ss(0.0, 1.0, (hh - center(k)) / (c1 - center(k)));
            i = k;
            j = (k + 1u) % 8u;
            break;
        }
    }
    let wj = 1.0 - wi;
    let bi = vec3<f32>(hsl(i, 0u), hsl(i, 1u), hsl(i, 2u));
    let bj = vec3<f32>(hsl(j, 0u), hsl(j, 1u), hsl(j, 2u));
    if any(bi != vec3<f32>(0.0)) || any(bj != vec3<f32>(0.0)) {
        let si = bi.x * 0.5 * gap(i, bi.x > 0.0);
        let sj = bj.x * 0.5 * gap(j, bj.x > 0.0);
        dh = wi * si + wj * sj;
        c *= max(1.0 + wi * bi.y + wj * bj.y, 0.0);
        let colored = ss(0.01, 0.06, chroma0);
        // En noir et blanc : le mélange noir et blanc, plus ample (±3 IL).
        var gain = 0.6;
        if P[P_MONO] != 0.0 {
            gain = 1.0;
        }
        l *= exp2(gain * (wi * bi.z + wj * bj.z) * colored);
    }
    if P[P_MONO] != 0.0 {
        c = 0.0;
    }
    var k = 0.0;
    if c0 > 1e-9 {
        k = c / c0;
    }
    var a = a0 * k;
    var b = b0 * k;
    if dh != 0.0 {
        let s = sin(dh);
        let co = cos(dh);
        let na = a * co - b * s;
        let nb = a * s + b * co;
        a = na;
        b = nb;
    }

    if P[P_GRADING] != 0.0 {
        let mid = P[P_GRADING_MID];
        let blend = P[P_GRADING_BLEND];
        let width = 0.2 + 0.5 * blend;
        let sh = 1.0 - ss(0.0, mid + width * 0.5, l);
        let hi = ss(mid - width * 0.5, 1.0, l);
        let z = (l - mid) / (0.12 + 0.25 * blend);
        let md = exp(-(z * z));
        let weights = array<f32, 4>(sh, md, hi, 1.0);
        let floor_ = ss(0.0, 0.08, l);
        for (var w = 0u; w < 4u; w++) {
            let o = P_WHEELS + w * 3u;
            a += weights[w] * P[o] * floor_;
            b += weights[w] * P[o + 1u] * floor_;
            l += weights[w] * P[o + 2u] * 0.12;
        }
        l = max(l, 0.0);
    }
    return from_lab(vec3<f32>(l, a, b));
}

// Le hachage 32 bits du grain (effects::hash).
fn hash(x: i32, y: i32) -> f32 {
    var h = (bitcast<u32>(x) * 0x8DA6B343u) ^ (bitcast<u32>(y) * 0xD8163841u);
    h ^= h >> 15u;
    h *= 0x2C1B3C6Du;
    h ^= h >> 12u;
    h *= 0x297A2D39u;
    h ^= h >> 15u;
    return f32(h & 0xFFFFu) / 65535.0 * 2.0 - 1.0;
}

fn value_noise(x: f32, y: f32) -> f32 {
    let x0 = floor(x);
    let y0 = floor(y);
    let tx = x - x0;
    let ty = y - y0;
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let xi = i32(x0);
    let yi = i32(y0);
    let top = hash(xi, yi) + (hash(xi + 1, yi) - hash(xi, yi)) * sx;
    let bottom = hash(xi, yi + 1) + (hash(xi + 1, yi + 1) - hash(xi, yi + 1)) * sx;
    return top + (bottom - top) * sy;
}

fn effects(p0: vec3<f32>, x: u32, y: u32) -> vec3<f32> {
    var p = p0;
    let w = P[P_WIDTH];
    let h = P[P_HEIGHT];
    let vignette = P[P_VIGNETTE];
    if vignette != 0.0 {
        let circle = P[P_CIRCLE];
        let power = P[P_POWER];
        let nx = (f32(x) + 0.5) / w * 2.0 - 1.0;
        let ny = (f32(y) + 0.5) / h * 2.0 - 1.0;
        var ny2 = ny;
        var nx2 = nx;
        if w >= h {
            ny2 = ny * (1.0 - circle + circle * h / w);
        } else {
            nx2 = nx * (1.0 - circle + circle * w / h);
        }
        var d = 0.0;
        if power == 2.0 {
            d = sqrt(nx2 * nx2 + ny2 * ny2) * 0.7071067811865476;
        } else {
            d = pow(pow(abs(nx2), power) + pow(abs(ny2), power), 1.0 / power) / pow(sqrt(2.0), 2.0 / power);
        }
        let k = ss(P[P_INNER], P[P_OUTER], d);
        if vignette < 0.0 {
            let bright = ss(0.4, 1.0, lum(p));
            let g = 1.0 + vignette * k * (1.0 - P[P_PROTECT] * bright);
            p = p * g;
        } else {
            p = p + max(vec3<f32>(1.0) - p, vec3<f32>(0.0)) * vignette * k;
        }
    }
    let grain = P[P_GRAIN];
    if grain != 0.0 {
        let scale = P[P_SCALE];
        let cell = P[P_CELL];
        let rough = P[P_ROUGH];
        let fx = f32(x) / scale;
        let fy = f32(y) / scale;
        let coarse = value_noise(fx / cell, fy / cell);
        let fine = value_noise(fx / (cell * 0.45) + 17.3, fy / (cell * 0.45) - 9.1);
        let n = coarse * (1.0 - 0.5 * rough) + fine * 0.5 * rough;
        let yv = sqrt(clamp(lum(p), 0.0, 1.0));
        let strength = grain * 0.12 * max(4.0 * yv * (1.0 - yv), 0.15);
        p = max(p * (1.0 + n * strength), vec3<f32>(0.0));
    }
    return p;
}

// Vers un espace (matrice à la position m), dans son gamut, par rapprochement du gris
// (to_output).
fn into_gamut(m: u32, p: vec3<f32>) -> vec3<f32> {
    let y = clamp(lum(p), 0.0, 1.0);
    var c = mat(m, p);
    let lo = min(c.x, min(c.y, c.z));
    if lo < 0.0 && y > lo {
        c = vec3<f32>(y) + (c - vec3<f32>(y)) * (y / (y - lo));
    }
    let hi = max(c.x, max(c.y, c.z));
    if hi > 1.0 && hi > y {
        c = vec3<f32>(y) + (c - vec3<f32>(y)) * ((1.0 - y) / (hi - y));
    }
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn to_output(p: vec3<f32>) -> vec3<f32> {
    return into_gamut(P_OUTPUT, p);
}

// Une couleur de la LUT 3D (cube.rs), le rouge variant le plus vite.
fn cube_at(i: vec3<u32>, n: u32) -> vec3<f32> {
    let o = T_CUBE + 3u * (i.x + i.y * n + i.z * n * n);
    return vec3<f32>(T[o], T[o + 1u], T[o + 2u]);
}

// Le film mesuré (cube.rs, Stage::apply) : sRGB encodé, LUT par interpolation
// tétraédrique, retour à l'espace de travail, fondu selon l'intensité.
fn film_lut(p: vec3<f32>) -> vec3<f32> {
    let n = u32(P[P_CUBE]);
    let lin = into_gamut(P_TO_SRGB, p);
    var i = vec3<u32>(0u);
    var f = vec3<f32>(0.0);
    for (var c = 0u; c < 3u; c++) {
        let lo = P[P_CUBE_MIN + c];
        let hi = P[P_CUBE_MAX + c];
        let t = clamp((lut(T_ENCODE, lin[c]) - lo) / (hi - lo), 0.0, 1.0) * f32(n - 1u);
        let k = min(u32(t), n - 2u);
        i[c] = k;
        f[c] = t - f32(k);
    }
    let c000 = cube_at(i, n);
    let c111 = cube_at(i + vec3<u32>(1u, 1u, 1u), n);
    var a: vec3<f32>;
    var b: vec3<f32>;
    var w: vec3<f32>;
    if f.x >= f.y {
        if f.y >= f.z {
            a = cube_at(i + vec3<u32>(1u, 0u, 0u), n);
            b = cube_at(i + vec3<u32>(1u, 1u, 0u), n);
            w = vec3<f32>(f.x - f.y, f.y - f.z, f.z);
        } else if f.x >= f.z {
            a = cube_at(i + vec3<u32>(1u, 0u, 0u), n);
            b = cube_at(i + vec3<u32>(1u, 0u, 1u), n);
            w = vec3<f32>(f.x - f.z, f.z - f.y, f.y);
        } else {
            a = cube_at(i + vec3<u32>(0u, 0u, 1u), n);
            b = cube_at(i + vec3<u32>(1u, 0u, 1u), n);
            w = vec3<f32>(f.z - f.x, f.x - f.y, f.y);
        }
    } else if f.z >= f.y {
        a = cube_at(i + vec3<u32>(0u, 0u, 1u), n);
        b = cube_at(i + vec3<u32>(0u, 1u, 1u), n);
        w = vec3<f32>(f.z - f.y, f.y - f.x, f.x);
    } else if f.z >= f.x {
        a = cube_at(i + vec3<u32>(0u, 1u, 0u), n);
        b = cube_at(i + vec3<u32>(0u, 1u, 1u), n);
        w = vec3<f32>(f.y - f.z, f.z - f.x, f.x);
    } else {
        a = cube_at(i + vec3<u32>(0u, 1u, 0u), n);
        b = cube_at(i + vec3<u32>(1u, 1u, 0u), n);
        w = vec3<f32>(f.y - f.x, f.x - f.z, f.z);
    }
    let w0 = 1.0 - w.x - w.y - w.z;
    let enc = w0 * c000 + w.x * a + w.y * b + w.z * c111;
    let out = vec3<f32>(lut(T_DECODE, enc.x), lut(T_DECODE, enc.y), lut(T_DECODE, enc.z));
    let film = mat(P_FROM_SRGB, out);
    return p + (film - p) * P[P_CUBE + 1u];
}

fn byte(v: f32) -> u32 {
    return min(u32(lut(T_OUTPUT, v) * 255.0 + 0.5), 255u);
}

fn pixel_index(gid: vec3<u32>, groups: vec3<u32>) -> u32 {
    return gid.y * groups.x * 256u + gid.x;
}

fn pixel_count() -> u32 {
    return u32(P[P_WIDTH]) * u32(P[P_HEIGHT]);
}

// Rendu, couleur et film mesuré : tout ce qui précède l'accentuation.
fn develop(index: u32) -> vec3<f32> {
    var p = src[index].xyz;
    p = render(p);
    if P[P_COLOR] != 0.0 {
        p = color(p);
    }
    if P[P_CUBE] != 0.0 {
        p = film_lut(p);
    }
    return p;
}

// Effets et encodage, la fin commune.
fn write(index: u32, p0: vec3<f32>) {
    let width = u32(P[P_WIDTH]);
    let p = effects(p0, index % width, index / width);
    let o = to_output(p);
    dst[index] = byte(o.x) | (byte(o.y) << 8u) | (byte(o.z) << 16u) | (255u << 24u);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let index = pixel_index(gid, groups);
    if index >= pixel_count() {
        return;
    }
    write(index, develop(index));
}

// Accentuation, 1 : l'image développée et sa luminance perçue (gamma 2,2), qui part
// au flou.
@compute @workgroup_size(256)
fn pre(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let index = pixel_index(gid, groups);
    if index >= pixel_count() {
        return;
    }
    let p = develop(index);
    let v = pow(max(lum(p), 0.0), 1.0 / 2.2);
    mid[index] = vec4<f32>(p, v);
    pout[index] = v;
}

// Accentuation, 2 : un flou boîte de rayon P_SHARPEN_R, bords répétés (comme
// Plane::box_rows_into et box_cols_into).
fn box_at(index: u32, horizontal: bool) -> f32 {
    let width = i32(P[P_WIDTH]);
    let height = i32(P[P_HEIGHT]);
    let r = i32(P[P_SHARPEN_R]);
    let x = i32(index) % width;
    let y = i32(index) / width;
    var sum = 0.0;
    for (var k = -r; k <= r; k++) {
        if horizontal {
            sum += pin[u32(y * width + clamp(x + k, 0, width - 1))];
        } else {
            sum += pin[u32(clamp(y + k, 0, height - 1) * width + x)];
        }
    }
    return sum / f32(2 * r + 1);
}

@compute @workgroup_size(256)
fn box_h(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let index = pixel_index(gid, groups);
    if index >= pixel_count() {
        return;
    }
    pout[index] = box_at(index, true);
}

@compute @workgroup_size(256)
fn box_v(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let index = pixel_index(gid, groups);
    if index >= pixel_count() {
        return;
    }
    pout[index] = box_at(index, false);
}

// Accentuation, 3 : l'écart au flou, borné par le Détail, ramené en gain sur les trois
// couches (detail::sharpen), puis effets et encodage.
@compute @workgroup_size(256)
fn post(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let index = pixel_index(gid, groups);
    if index >= pixel_count() {
        return;
    }
    let m = mid[index];
    let v = m.w;
    let limit = P[P_SHARPEN_LIMIT];
    let diff = clamp(v - pin[index], -limit, limit);
    let sharp = max(v + P[P_SHARPEN_AMOUNT] * diff, 0.0);
    var gain = 1.0;
    if v > 1e-4 {
        gain = pow(sharp / v, 2.2);
    }
    write(index, m.xyz * gain);
}
