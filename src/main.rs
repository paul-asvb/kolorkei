//! kolorkei: press any key, get a random shape, a random color and a random tone.
//! Shapes stay on screen. Space or Enter clears a random half of them.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::audio::AudioSource;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "kolorkei".into(),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::hsl(0.0, 0.0, 0.07)))
        .insert_resource(TargetBackground(Color::hsl(0.0, 0.0, 0.07)))
        .init_resource::<Rng>()
        .init_resource::<Layer>()
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_keys, animate_shapes, fade_background))
        .run();
}

// ---------------------------------------------------------------------------
// Randomness: a tiny xorshift64* so we need no `getrandom` on wasm.
// It is reseeded with the frame time on every key press, which is effectively
// random because it depends on when the human hits the key.
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct Rng(u64);

impl Default for Rng {
    fn default() -> Self {
        Self(0x9E37_79B9_7F4A_7C15)
    }
}

impl Rng {
    fn mix(&mut self, extra: u64) {
        self.0 ^= extra.wrapping_mul(0xD6E8_FEB8_6659_FD93).rotate_left(23);
        self.next_u64();
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform float in `[0, 1)`.
    fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i + 1));
        }
    }
}

// ---------------------------------------------------------------------------
// Components & resources
// ---------------------------------------------------------------------------

/// A top-level shape that stays on screen until cleared.
#[derive(Component)]
struct Shape {
    spin: f32,
}

/// Pop-in animation state; removed once the shape has settled.
#[derive(Component)]
struct PopIn {
    t: f32,
    overshoot: f32,
}

/// The background eases toward this color.
#[derive(Resource)]
struct TargetBackground(Color);

/// Monotonic z-layer so newer shapes draw on top of older ones.
#[derive(Resource, Default)]
struct Layer(u32);

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

// ---------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------

/// A vivid, saturated color. Lightness is kept in the band where hues read as
/// strongly colored rather than pastel or murky.
fn vivid(rng: &mut Rng, hue: f32) -> Color {
    Color::hsl(hue.rem_euclid(360.0), rng.range(0.85, 1.0), rng.range(0.48, 0.66))
}

/// A hue that contrasts with `hue`: complementary, triadic or split-complementary.
fn contrasting_hue(rng: &mut Rng, hue: f32) -> f32 {
    let offset = match rng.below(4) {
        0 => 180.0,
        1 => 120.0,
        2 => 240.0,
        _ => 150.0 + rng.range(0.0, 60.0),
    };
    (hue + offset).rem_euclid(360.0)
}

// ---------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------

/// A five-to-nine pointed star as a triangle fan.
fn star_mesh(points: u32, outer: f32, inner: f32) -> Mesh {
    let n = points * 2;
    let mut positions = vec![[0.0, 0.0, 0.0]];
    for i in 0..n {
        let angle = PI / 2.0 + i as f32 / n as f32 * TAU;
        let r = if i % 2 == 0 { outer } else { inner };
        positions.push([angle.cos() * r, angle.sin() * r, 0.0]);
    }
    let mut indices = Vec::with_capacity(n as usize * 3);
    for i in 0..n {
        indices.extend_from_slice(&[0, 1 + i, 1 + (i + 1) % n]);
    }
    let count = positions.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.5, 0.5]; count])
        .with_inserted_indices(Indices::U32(indices))
}

/// A plus / cross made of two crossing bars.
fn cross_mesh(size: f32, thickness: f32) -> Mesh {
    let h = size;
    let t = thickness / 2.0;
    let positions: Vec<[f32; 3]> = vec![
        [-t, -h, 0.0], [t, -h, 0.0], [t, h, 0.0], [-t, h, 0.0],
        [-h, -t, 0.0], [h, -t, 0.0], [h, t, 0.0], [-h, t, 0.0],
    ];
    let count = positions.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.5, 0.5]; count])
        .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]))
}

fn random_mesh(rng: &mut Rng, s: f32) -> Mesh {
    let ring = s * rng.range(0.12, 0.3);
    match rng.below(20) {
        0 => Circle::new(s).into(),
        1 => Rectangle::new(s * rng.range(0.5, 2.2), s * rng.range(0.5, 2.2)).into(),
        2 => Rectangle::new(s * 1.4, s * 1.4).into(),
        3 => RegularPolygon::new(s, 3 + rng.below(7) as u32).into(),
        4 => Annulus::new(s * rng.range(0.35, 0.8), s).into(),
        5 => Ellipse::new(s * rng.range(0.4, 1.6), s).into(),
        6 => Rhombus::new(s * rng.range(0.5, 2.2), s * rng.range(0.5, 2.2)).into(),
        7 => Capsule2d::new(s * rng.range(0.25, 0.7), s).into(),
        8 => CircularSector::new(s, rng.range(0.4, PI)).into(),
        9 => CircularSegment::new(s, rng.range(0.8, PI)).into(),
        10 => Triangle2d::new(
            Vec2::from_angle(rng.range(0.0, TAU)) * s,
            Vec2::from_angle(rng.range(0.0, TAU)) * s,
            Vec2::from_angle(rng.range(0.0, TAU)) * s,
        )
        .into(),
        11 => star_mesh(5 + rng.below(5) as u32, s, s * rng.range(0.35, 0.6)),
        12 => cross_mesh(s, s * rng.range(0.3, 0.7)),
        13 => RegularPolygon::new(s, 3 + rng.below(7) as u32).to_ring(ring).into(),
        14 => Rectangle::new(s * rng.range(0.6, 2.0), s * rng.range(0.6, 2.0)).to_ring(ring).into(),
        15 => Rhombus::new(s * rng.range(0.6, 2.0), s * rng.range(0.6, 2.0)).to_ring(ring).into(),
        16 => Capsule2d::new(s * rng.range(0.3, 0.7), s).to_ring(ring).into(),
        17 => Triangle2d::new(Vec2::Y * s, Vec2::new(-s, -s * 0.8), Vec2::new(s, -s * 0.8))
            .to_ring(ring)
            .into(),
        18 => CircularSegment::new(s, rng.range(1.0, PI)).to_ring(ring).into(),
        _ => Ring::new(CircularSector::new(s, rng.range(0.6, 2.5)), CircularSector::new(s - ring, rng.range(0.6, 2.5))).into(),
    }
}

fn handle_keys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    window: Single<&Window>,
    mut rng: ResMut<Rng>,
    mut layer: ResMut<Layer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut target_bg: ResMut<TargetBackground>,
    shapes: Query<Entity, With<Shape>>,
) {
    for key in keys.get_just_pressed() {
        // Fold the key and the exact press time into the RNG state.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(key, &mut hasher);
        rng.mix(std::hash::Hasher::finish(&hasher) ^ time.elapsed().as_nanos() as u64);

        if matches!(key, KeyCode::Space | KeyCode::Enter | KeyCode::NumpadEnter) {
            clear_half(&mut commands, &mut rng, &shapes);
            let bytes = synthesize_sweep(&mut rng);
            commands.spawn((
                AudioPlayer(audio.add(AudioSource { bytes: Arc::from(bytes) })),
                PlaybackSettings::DESPAWN,
            ));
            continue;
        }

        let half_w = window.width() / 2.0;
        let half_h = window.height() / 2.0;
        let min_dim = window.width().min(window.height());

        let hue = rng.range(0.0, 360.0);
        let color = vivid(&mut rng, hue);
        let size = min_dim * rng.range(0.03, 0.17);
        let pos = Vec2::new(
            rng.range(-half_w * 0.92, half_w * 0.92),
            rng.range(-half_h * 0.92, half_h * 0.92),
        );
        layer.0 += 1;
        let z = layer.0 as f32 * 0.01;

        let mut shape = commands.spawn((
            Mesh2d(meshes.add(random_mesh(&mut rng, size))),
            MeshMaterial2d(materials.add(color)),
            Transform::from_xyz(pos.x, pos.y, z)
                .with_rotation(Quat::from_rotation_z(rng.range(0.0, TAU)))
                .with_scale(Vec3::splat(0.01)),
            Shape { spin: rng.range(-0.8, 0.8) },
            PopIn { t: 0.0, overshoot: rng.range(0.15, 0.5) },
        ));

        // About half the shapes get a smaller contrasting shape nested inside,
        // and some get a third one: two- and three-tone compositions.
        let mut inner_hue = hue;
        let mut inner_size = size;
        let layers = match rng.below(10) {
            0..=4 => 0,
            5..=8 => 1,
            _ => 2,
        };
        for depth in 0..layers {
            inner_hue = contrasting_hue(&mut rng, inner_hue);
            inner_size *= rng.range(0.4, 0.65);
            let inner_color = vivid(&mut rng, inner_hue);
            let inner_mesh = meshes.add(random_mesh(&mut rng, inner_size));
            let inner_mat = materials.add(inner_color);
            let rot = rng.range(0.0, TAU);
            shape.with_children(|parent| {
                parent.spawn((
                    Mesh2d(inner_mesh),
                    MeshMaterial2d(inner_mat),
                    Transform::from_xyz(0.0, 0.0, 0.001 * (depth as f32 + 1.0))
                        .with_rotation(Quat::from_rotation_z(rot)),
                ));
            });
        }

        // The background drifts toward a dark version of the same hue.
        target_bg.0 = Color::hsl(hue, 0.45, 0.09);

        // Sound: a short tone on a pentatonic scale, random waveform.
        let tone_len = rng.range(0.5, 1.4);
        let bytes = synthesize_tone(&mut rng, tone_len);
        commands.spawn((
            AudioPlayer(audio.add(AudioSource { bytes: Arc::from(bytes) })),
            PlaybackSettings::DESPAWN,
        ));
    }
}

/// Despawn a random half (rounded up) of all top-level shapes.
fn clear_half(commands: &mut Commands, rng: &mut Rng, shapes: &Query<Entity, With<Shape>>) {
    let mut all: Vec<Entity> = shapes.iter().collect();
    if all.is_empty() {
        return;
    }
    rng.shuffle(&mut all);
    let remove = all.len().div_ceil(2);
    for entity in all.into_iter().take(remove) {
        commands.entity(entity).despawn();
    }
}

fn animate_shapes(
    mut commands: Commands,
    time: Res<Time>,
    mut shapes: Query<(Entity, &Shape, &mut Transform, Option<&mut PopIn>)>,
) {
    let dt = time.delta_secs();
    for (entity, shape, mut tf, pop) in &mut shapes {
        tf.rotation *= Quat::from_rotation_z(shape.spin * dt);

        if let Some(mut pop) = pop {
            pop.t += dt / 0.35;
            if pop.t >= 1.0 {
                tf.scale = Vec3::ONE;
                commands.entity(entity).remove::<PopIn>();
            } else {
                // Elastic pop: overshoot once, then settle.
                let t = pop.t;
                let scale = 1.0 + pop.overshoot * (t * PI).sin() * (1.0 - t) - (1.0 - t).powi(3);
                tf.scale = Vec3::splat(scale.max(0.01));
            }
        }
    }
}

fn fade_background(
    time: Res<Time>,
    target: Res<TargetBackground>,
    mut clear: ResMut<ClearColor>,
) {
    let k = 1.0 - (-time.delta_secs() * 2.5).exp();
    clear.0 = clear.0.mix(&target.0, k);
}

// ---------------------------------------------------------------------------
// Sound: build little 16-bit mono WAVs in memory.
// ---------------------------------------------------------------------------

const SAMPLE_RATE: u32 = 44_100;

/// Wrap mono `f32` samples in `[-1, 1]` into a 16-bit PCM WAV file.
fn wav_from_samples(samples: impl ExactSizeIterator<Item = f32>) -> Vec<u8> {
    let n = samples.len();
    let mut data = Vec::with_capacity(44 + n * 2);
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + n as u32 * 2).to_le_bytes());
    data.extend_from_slice(b"WAVE");
    data.extend_from_slice(b"fmt ");
    data.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    data.extend_from_slice(&1u16.to_le_bytes()); // PCM
    data.extend_from_slice(&1u16.to_le_bytes()); // mono
    data.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    data.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    data.extend_from_slice(&2u16.to_le_bytes()); // block align
    data.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    data.extend_from_slice(b"data");
    data.extend_from_slice(&(n as u32 * 2).to_le_bytes());
    for v in samples {
        data.extend_from_slice(&((v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    data
}

fn oscillator(waveform: usize, p: f32) -> f32 {
    match waveform {
        0 => (p * TAU).sin(),
        1 => (if p < 0.5 { 1.0 } else { -1.0 }) * 0.4,
        2 => 4.0 * (p - 0.5).abs() - 1.0,
        _ => (2.0 * p - 1.0) * 0.6,
    }
}

fn synthesize_tone(rng: &mut Rng, seconds: f32) -> Vec<u8> {
    // C major pentatonic across three octaves: pleasant no matter what you mash.
    const PENTATONIC: [f32; 5] = [0.0, 2.0, 4.0, 7.0, 9.0];
    let octave = rng.below(3) as f32;
    let semitone = PENTATONIC[rng.below(PENTATONIC.len())] + 12.0 * octave;
    let freq = 130.81 * 2f32.powf(semitone / 12.0); // from C3

    let waveform = rng.below(4);
    let detune = rng.range(0.001, 0.006);
    let n = (seconds * SAMPLE_RATE as f32) as usize;
    let attack = (0.01 * SAMPLE_RATE as f32) as usize;

    wav_from_samples((0..n).map(move |i| {
        let t = i as f32 / SAMPLE_RATE as f32;
        let a = oscillator(waveform, (t * freq) % 1.0);
        let b = oscillator(waveform, (t * freq * (1.0 + detune)) % 1.0);
        let env_attack = if i < attack { i as f32 / attack as f32 } else { 1.0 };
        let env_decay = (-t * (3.0 / seconds)).exp() * (1.0 - t / seconds);
        0.5 * (a + b) * env_attack * env_decay * 0.6
    }))
}

/// A falling "whoosh" for clearing shapes: a sine sweeping down two octaves.
fn synthesize_sweep(rng: &mut Rng) -> Vec<u8> {
    let seconds = 0.45;
    let start = rng.range(600.0, 1100.0);
    let n = (seconds * SAMPLE_RATE as f32) as usize;
    let mut phase = 0.0f32;
    wav_from_samples((0..n).map(move |i| {
        let t = i as f32 / SAMPLE_RATE as f32;
        let u = t / seconds;
        let freq = start * 0.25f32.powf(u);
        phase = (phase + freq / SAMPLE_RATE as f32) % 1.0;
        let env = (u * PI).sin().powf(0.5) * (1.0 - u);
        ((phase * TAU).sin() + 0.3 * (phase * 2.0 * TAU).sin()) * env * 0.5
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_wav(wav: &[u8], expected_samples: usize) -> u16 {
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        let riff_size = u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff_size + 8, wav.len());
        let data_size = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
        assert_eq!(data_size + 44, wav.len());
        assert_eq!(data_size, expected_samples * 2);
        wav[44..]
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]).unsigned_abs())
            .max()
            .unwrap()
    }

    #[test]
    fn tone_wav_is_consistent_and_audible() {
        let mut rng = Rng::default();
        rng.mix(42);
        let wav = synthesize_tone(&mut rng, 0.5);
        let peak = check_wav(&wav, (0.5 * SAMPLE_RATE as f32) as usize);
        assert!(peak > 3000, "peak too quiet: {peak}");
    }

    #[test]
    fn sweep_wav_is_consistent_and_audible() {
        let mut rng = Rng::default();
        let wav = synthesize_sweep(&mut rng);
        let peak = check_wav(&wav, (0.45 * SAMPLE_RATE as f32) as usize);
        assert!(peak > 3000, "peak too quiet: {peak}");
    }

    #[test]
    fn rng_is_not_stuck() {
        let mut rng = Rng::default();
        let samples: Vec<f32> = (0..1000).map(|_| rng.f32()).collect();
        assert!(samples.iter().all(|v| (0.0..1.0).contains(v)));
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!((0.4..0.6).contains(&mean), "mean {mean}");
        assert!((0..200).any(|_| rng.below(20) == 19));
    }

    #[test]
    fn shuffle_keeps_all_items() {
        let mut rng = Rng::default();
        let mut items: Vec<u32> = (0..50).collect();
        rng.shuffle(&mut items);
        let mut sorted = items.clone();
        sorted.sort();
        assert_eq!(sorted, (0..50).collect::<Vec<_>>());
        assert_ne!(items, sorted, "shuffle left the order unchanged");
    }

    #[test]
    fn every_mesh_variant_builds() {
        let mut rng = Rng::default();
        for _ in 0..200 {
            let mesh = random_mesh(&mut rng, 40.0);
            assert!(mesh.count_vertices() >= 3);
        }
    }
}
