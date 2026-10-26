//! Les étapes par pixel de la fin du développement (rendu, couleur, effets, encodage)
//! sur le GPU, avec wgpu : Metal sur Mac et iPhone, Vulkan ailleurs.
//!
//! Le moteur sur le processeur reste la référence : le shader (`finish.wgsl`) en est
//! le double ligne à ligne, et les tests comparent les deux sorties. Les étapes de
//! voisinage (bruit, tonalité locale) restent pour l'instant sur le processeur ; leur
//! résultat, la « base », est gardé en cache par `luma-render` et envoyé une seule fois
//! au GPU. Quand on règle le contraste, les courbes, les couleurs ou les effets, seul le
//! GPU travaille.
//!
//! L'accentuation, qui n'apparaît qu'à partir de 50 % (en dessous, elle ne se voit
//! pas), n'est pas encore portée : [`Gpu::can_finish`] dit quand le processeur doit
//! prendre la main.

use std::sync::Mutex;

use luma_color::{EncodeLut, OutputSpace};
use luma_core::RgbImage;
use luma_image::{color::ColorStage, curve::Lut, effects};
use luma_project::Settings;

/// Les paramètres du shader, rangés à plat dans un tableau de `f32`. Le nom de chaque
/// paramètre devient une constante `P_…` du shader : les positions ne peuvent pas se
/// désaccorder entre Rust et WGSL.
#[derive(Default)]
struct Params {
    values: Vec<f32>,
    names: Vec<(&'static str, usize)>,
}

impl Params {
    fn push(&mut self, name: &'static str, values: &[f32]) {
        self.names.push((name, self.values.len()));
        self.values.extend_from_slice(values);
    }

    fn mat(&mut self, name: &'static str, m: &luma_color::Mat3) {
        self.push(name, &m.concat());
    }
}

/// La liste des paramètres, avec des valeurs neutres : elle sert à fabriquer les
/// constantes du shader, et à remplir les vraies valeurs dans le même ordre.
fn params(s: &Settings, width: usize, height: usize, scale: f32, space: OutputSpace) -> Params {
    let mut p = Params::default();
    p.push("WIDTH", &[width as f32]);
    p.push("HEIGHT", &[height as f32]);
    p.push("SCALE", &[scale]);
    let (_, curves) = luma_image::render_tables(s);
    p.push("CURVES", &[f32::from(u8::from(curves.is_some()))]);
    p.push("ROLLOFF", &[luma_image::highlight_rolloff(s)]);

    let stage = ColorStage::new(s);
    p.push("COLOR", &[f32::from(u8::from(stage.is_some()))]);
    let parts = stage.map(|c| c.parts());
    let ok = parts.as_ref().map_or_else(luma_color::OkLab::for_working, |c| c.ok);
    let ([to_lms, from_lms, lms_to_lab, lab_to_lms], (wa, wb)) = ok.parts();
    p.mat("TO_LMS", &to_lms);
    p.mat("FROM_LMS", &from_lms);
    p.mat("LMS_TO_LAB", &lms_to_lab);
    p.mat("LAB_TO_LMS", &lab_to_lms);
    p.push("WHITE_A", &[wa]);
    p.push("WHITE_B", &[wb]);
    let flag = |b: bool| f32::from(u8::from(b));
    match &parts {
        Some(c) => {
            p.push("SIMPLE", &[flag(c.simple)]);
            p.push("SATURATION", &[c.saturation]);
            p.push("PROFILE_CHROMA", &[c.profile_chroma]);
            p.push("MONO", &[flag(c.mono)]);
            p.push("VIBRANCE", &[c.vibrance]);
            p.push("SKIN_HUE", &[c.skin_hue]);
            p.push("CENTERS", &c.centers);
            p.push("HSL", &c.hsl.concat());
            match c.grading {
                Some((wheels, mid, blend)) => {
                    p.push("GRADING", &[1.0]);
                    p.push("GRADING_MID", &[mid]);
                    p.push("GRADING_BLEND", &[blend]);
                    let w: Vec<f32> = wheels.iter().flat_map(|&(a, b, l)| [a, b, l]).collect();
                    p.push("WHEELS", &w);
                }
                None => {
                    p.push("GRADING", &[0.0]);
                    p.push("GRADING_MID", &[0.5]);
                    p.push("GRADING_BLEND", &[0.5]);
                    p.push("WHEELS", &[0.0; 12]);
                }
            }
        }
        None => {
            p.push("SIMPLE", &[1.0]);
            p.push("SATURATION", &[0.0]);
            p.push("PROFILE_CHROMA", &[1.0]);
            p.push("MONO", &[0.0]);
            p.push("VIBRANCE", &[0.0]);
            p.push("SKIN_HUE", &[0.0]);
            p.push("CENTERS", &[0.0; 8]);
            p.push("HSL", &[0.0; 24]);
            p.push("GRADING", &[0.0]);
            p.push("GRADING_MID", &[0.5]);
            p.push("GRADING_BLEND", &[0.5]);
            p.push("WHEELS", &[0.0; 12]);
        }
    }

    // Le film mesuré (LUT 3D) : taille (0 sans LUT), intensité, domaine ; ses couleurs
    // sont à la fin des tables.
    let cube = luma_image::cube::Stage::new(s);
    match &cube {
        Some(c) => {
            p.push("CUBE", &[c.cube.size as f32, c.amount]);
            p.push("CUBE_MIN", &c.cube.domain_min);
            p.push("CUBE_MAX", &c.cube.domain_max);
        }
        None => {
            p.push("CUBE", &[0.0, 0.0]);
            p.push("CUBE_MIN", &[0.0; 3]);
            p.push("CUBE_MAX", &[1.0; 3]);
        }
    }
    p.mat(
        "TO_SRGB",
        &luma_color::rgb_to_rgb(&luma_color::WORKING, &luma_color::SRGB),
    );
    p.mat(
        "FROM_SRGB",
        &luma_color::rgb_to_rgb(&luma_color::SRGB, &luma_color::WORKING),
    );

    let e = effects::params(&s.effects, scale);
    p.push("VIGNETTE", &[e.vignette]);
    p.push("CIRCLE", &[e.circle]);
    p.push("POWER", &[e.power]);
    p.push("INNER", &[e.inner]);
    p.push("OUTER", &[e.outer]);
    p.push("PROTECT", &[e.protect]);
    p.push("GRAIN", &[e.grain]);
    p.push("CELL", &[e.cell]);
    p.push("ROUGH", &[e.rough]);
    p.mat(
        "OUTPUT",
        &luma_color::rgb_to_rgb(&luma_color::WORKING, &space.primaries()),
    );
    p
}

const LUT: usize = Lut::SIZE + 1;
const FILMIC: usize = luma_image::curve::FilmicTable::SIZE + 1;
/// Les tables, à la suite : filmique, trois courbes, encodage et décodage sRGB (pour les
/// courbes et la LUT), encodage de sortie ; puis, s'il y en a une, les couleurs de la
/// LUT 3D (T_CUBE).
const TABLES: [(&str, usize); 7] = [
    ("FILMIC", FILMIC),
    ("LUT_R", LUT),
    ("LUT_G", LUT),
    ("LUT_B", LUT),
    ("ENCODE", LUT),
    ("DECODE", LUT),
    ("OUTPUT", LUT),
];

fn tables(s: &Settings, space: OutputSpace) -> Vec<f32> {
    let (filmic, curves) = luma_image::render_tables(s);
    let identity = Lut::identity().values().to_vec();
    let [r, g, b] = curves.unwrap_or_else(|| [identity.clone(), identity.clone(), identity]);
    let lut = |f: fn(f32) -> f32| -> Vec<f32> { (0..LUT).map(|i| f(i as f32 / Lut::SIZE as f32)).collect() };
    let mut t = filmic;
    for part in [r, g, b] {
        t.extend(part);
    }
    // Les mêmes tables que le processeur (EncodeLut et Lut::from_fn).
    let srgb = EncodeLut::new(luma_color::srgb_encode);
    t.extend((0..LUT).map(|i| srgb.get(i as f32 / Lut::SIZE as f32)));
    t.extend(lut(luma_color::srgb_decode));
    let enc = EncodeLut::new(|v| space.encode(v));
    t.extend((0..LUT).map(|i| enc.get(i as f32 / Lut::SIZE as f32)));
    debug_assert_eq!(t.len(), TABLES.iter().map(|t| t.1).sum::<usize>());
    if let Some(c) = luma_image::cube::Stage::new(s) {
        t.extend(c.cube.data.iter().flatten());
    }
    t
}

/// Le texte du shader, précédé des constantes de position.
fn shader_source() -> String {
    let mut out = String::new();
    let p = params(&Settings::default(), 1, 1, 1.0, OutputSpace::Srgb);
    for (name, pos) in &p.names {
        out += &format!("const P_{name}: u32 = {pos}u;\n");
    }
    let mut pos = 0;
    for (name, len) in TABLES {
        out += &format!("const T_{name}: u32 = {pos}u;\n");
        pos += len;
    }
    out += &format!("const T_CUBE: u32 = {pos}u;\n");
    out + include_str!("finish.wgsl")
}

/// La base envoyée au GPU, gardée tant que son identifiant ne change pas.
struct Uploaded {
    id: u64,
    width: usize,
    height: usize,
    input: wgpu::Buffer,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
}

/// Le GPU, prêt à finir des développements.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    name: String,
    max_buffer: u64,
    /// Les bases déjà envoyées, la plus récente en tête : celle de la photo et celles
    /// des films de masques qui ont la leur.
    uploaded: Mutex<Vec<Uploaded>>,
}

const WORKGROUP: u32 = 256;

/// Bases gardées sur le GPU.
const UPLOADED: usize = 4;

impl Gpu {
    /// Le GPU de la machine, s'il y en a un vrai (sinon tout reste sur le processeur :
    /// un GPU émulé par logiciel, comme llvmpipe, serait plus lent que lui).
    pub fn new() -> Option<Gpu> {
        pollster::block_on(Self::create(false)).ok()
    }

    /// N'importe quel GPU, même émulé : pour les tests, qui vérifient le calcul et non
    /// la vitesse.
    pub fn any() -> Option<Gpu> {
        pollster::block_on(Self::create(true)).ok()
    }

    async fn create(software: bool) -> Result<Gpu, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL | wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let info = adapter.get_info();
        if !software && info.device_type == wgpu::DeviceType::Cpu {
            return Err(format!("{} : GPU émulé par logiciel", info.name));
        }
        let limits = adapter.limits();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("LÜMA"),
                required_limits: limits.clone(),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("finition"),
            source: wgpu::ShaderSource::Wgsl(shader_source().into()),
        });
        let entry = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("finition"),
            entries: &[entry(0, true), entry(1, true), entry(2, true), entry(3, false)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("finition"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("finition"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(Gpu {
            device,
            queue,
            pipeline,
            layout,
            name: format!("{} ({:?})", info.name, info.backend),
            max_buffer: limits.max_storage_buffer_binding_size.min(limits.max_buffer_size),
            uploaded: Mutex::new(Vec::new()),
        })
    }

    /// Le nom du GPU et de son interface (Metal, Vulkan).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Le GPU sait-il finir ce développement ? Non si l'accentuation doit s'appliquer
    /// (pas encore portée) ou si l'image dépasse ce que le GPU accepte d'un bloc.
    pub fn can_finish(&self, s: &Settings, width: usize, height: usize, scale: f32) -> bool {
        let sharpen = s.detail.sharpen_amount > 0.0 && scale >= 0.5;
        let bytes = (width * height * 16) as u64;
        !sharpen && bytes <= self.max_buffer && width * height > 0
    }

    /// Finit `base` (sortie de `develop_base`, identifiée par `id`) et l'encode en RGBA
    /// 8 bits dans `space`. La base n'est envoyée au GPU que si `id` a changé.
    pub fn finish(&self, base: &RgbImage, id: u64, scale: f32, s: &Settings, space: OutputSpace) -> Vec<u8> {
        // Les Blancs et les Noirs, adaptés à la base comme sur le processeur.
        let adapted = luma_image::adaptive_ends(base, s);
        let s = &adapted;
        let (w, h) = (base.width, base.height);
        let n = w * h;
        let mut uploaded = self.uploaded.lock().expect("GPU");
        if let Some(i) = uploaded
            .iter()
            .position(|u| u.id == id && u.width == w && u.height == h)
        {
            let hit = uploaded.remove(i);
            uploaded.insert(0, hit);
        } else {
            let input = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("base"),
                size: (n * 16) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let padded: Vec<f32> = base.data.iter().flat_map(|p| [p[0], p[1], p[2], 0.0]).collect();
            self.queue.write_buffer(&input, 0, bytes(&padded));
            let output = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("sortie"),
                size: (n * 4) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("relecture"),
                size: (n * 4) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            uploaded.retain(|u| u.width == w && u.height == h);
            uploaded.insert(
                0,
                Uploaded {
                    id,
                    width: w,
                    height: h,
                    input,
                    output,
                    readback,
                },
            );
            uploaded.truncate(UPLOADED);
        }
        let u = &uploaded[0];

        let params = params(s, w, h, scale, space).values;
        let tables = tables(s, space);
        let storage = |label, data: &[f32]| {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (data.len() * 4) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue.write_buffer(&buffer, 0, bytes(data));
            buffer
        };
        let params = storage("paramètres", &params);
        let tables = storage("tables", &tables);
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("finition"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: u.input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: tables.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: u.output.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            let groups = (n as u32).div_ceil(WORKGROUP);
            let x = groups.min(65535);
            pass.dispatch_workgroups(x, groups.div_ceil(x), 1);
        }
        encoder.copy_buffer_to_buffer(&u.output, 0, &u.readback, 0, (n * 4) as u64);
        self.queue.submit([encoder.finish()]);

        let slice = u.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let out = slice.get_mapped_range().expect("relecture du GPU").to_vec();
        u.readback.unmap();
        out
    }
}

/// Les octets d'un tableau de `f32` (sans `unsafe`, interdit dans LÜMA : bytemuck
/// le vérifie à la compilation).
fn bytes(data: &[f32]) -> &[u8] {
    bytemuck::cast_slice(data)
}
