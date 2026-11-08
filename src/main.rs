//! kolorkei: press any key, get a random shape, a random color and a random tone.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use bevy::audio::AudioSource;
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
        .init_resource::<Rng>()
        .add_systems(Startup, setup)
        .add_systems(Update, (spawn_on_key, animate_shapes, fade_background))
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
}

// ---------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------

#[derive(Component)]
struct Shape {
    age: f32,
    lifetime: f32,
    size: f32,
    spin: f32,
    drift: Vec2,
}

/// The background eases toward this color.
#[derive(Resource)]
struct TargetBackground(Color);

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.insert_resource(TargetBackground(Color::hsl(0.0, 0.0, 0.07)));
}

fn random_mesh(rng: &mut Rng, size: f32) -> Mesh {
    let s = size;
    match rng.below(9) {
        0 => Circle::new(s).into(),
        1 => Rectangle::new(s * rng.range(0.6, 2.0), s * rng.range(0.6, 2.0)).into(),
        2 => RegularPolygon::new(s, 3 + rng.below(6) as u32).into(),
        3 => Annulus::new(s * rng.range(0.4, 0.8), s).into(),
        4 => Ellipse::new(s * rng.range(0.5, 1.5), s).into(),
        5 => Rhombus::new(s * rng.range(0.6, 2.0), s * rng.range(0.6, 2.0)).into(),
        6 => Capsule2d::new(s * rng.range(0.3, 0.7), s).into(),
        7 => CircularSector::new(s, rng.range(0.4, PI)).into(),
        _ => Triangle2d::new(
            Vec2::from_angle(rng.range(0.0, TAU)) * s,
            Vec2::from_angle(rng.range(0.0, TAU)) * s,
            Vec2::from_angle(rng.range(0.0, TAU)) * s,
        )
        .into(),
    }
}

fn spawn_on_key(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    window: Single<&Window>,
    mut rng: ResMut<Rng>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut target_bg: ResMut<TargetBackground>,
) {
    for key in keys.get_just_pressed() {
        // Fold the key and the exact press time into the RNG state.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(key, &mut hasher);
        rng.mix(std::hash::Hasher::finish(&hasher) ^ time.elapsed().as_nanos() as u64);

        let half_w = window.width() / 2.0;
        let half_h = window.height() / 2.0;
        let min_dim = window.width().min(window.height());

        let hue = rng.range(0.0, 360.0);
        let color = Color::hsla(hue, rng.range(0.7, 1.0), rng.range(0.45, 0.75), 1.0);
        let size = min_dim * rng.range(0.04, 0.18);
        let pos = Vec2::new(
            rng.range(-half_w * 0.9, half_w * 0.9),
            rng.range(-half_h * 0.9, half_h * 0.9),
        );
        let lifetime = rng.range(1.2, 3.0);

        commands.spawn((
            Mesh2d(meshes.add(random_mesh(&mut rng, size))),
            MeshMaterial2d(materials.add(color)),
            Transform::from_xyz(pos.x, pos.y, time.elapsed_secs() * 0.001)
                .with_rotation(Quat::from_rotation_z(rng.range(0.0, TAU)))
                .with_scale(Vec3::splat(0.01)),
            Shape {
                age: 0.0,
                lifetime,
                size: 1.0,
                spin: rng.range(-2.0, 2.0),
                drift: Vec2::from_angle(rng.range(0.0, TAU)) * rng.range(0.0, min_dim * 0.08),
            },
        ));

        // The background drifts toward a dark version of the same hue.
        target_bg.0 = Color::hsl(hue, 0.35, 0.08);

        // Sound: a short tone on a pentatonic scale, random waveform.
        let bytes = synthesize_tone(&mut rng, lifetime.min(1.4));
        commands.spawn((
            AudioPlayer(audio.add(AudioSource { bytes: Arc::from(bytes) })),
            PlaybackSettings::DESPAWN,
        ));
    }
}

fn animate_shapes(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut shapes: Query<(Entity, &mut Shape, &mut Transform, &MeshMaterial2d<ColorMaterial>)>,
) {
    let dt = time.delta_secs();
    for (entity, mut shape, mut tf, mat) in &mut shapes {
        shape.age += dt;
        let t = (shape.age / shape.lifetime).clamp(0.0, 1.0);
        if t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }

        // Pop in (overshoot), then settle; fade out at the end.
        let pop = if t < 0.15 {
            let u = t / 0.15;
            1.0 + 0.35 * (u * PI).sin() * (1.0 - u)
        } else {
            1.0
        };
        let ease_in = (t / 0.15).min(1.0);
        let scale = shape.size * pop * ease_in.powf(0.5);
        tf.scale = Vec3::splat(scale.max(0.01));
        tf.rotation *= Quat::from_rotation_z(shape.spin * dt);
        tf.translation += (shape.drift * dt).extend(0.0);

        let alpha = if t > 0.6 { 1.0 - (t - 0.6) / 0.4 } else { 1.0 };
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.color = m.color.with_alpha(alpha * alpha);
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
// Sound: build a little 16-bit mono WAV in memory.
// ---------------------------------------------------------------------------

const SAMPLE_RATE: u32 = 44_100;

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

    let mut data = Vec::with_capacity(44 + n * 2);
    // RIFF header
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

    for i in 0..n {
        let t = i as f32 / SAMPLE_RATE as f32;
        let phase = (t * freq) % 1.0;
        let phase2 = (t * freq * (1.0 + detune)) % 1.0;
        let osc = |p: f32| -> f32 {
            match waveform {
                0 => (p * TAU).sin(),
                1 => (if p < 0.5 { 1.0 } else { -1.0 }) * 0.4,
                2 => 4.0 * (p - 0.5).abs() - 1.0,
                _ => (2.0 * p - 1.0) * 0.6,
            }
        };
        let sample = 0.5 * (osc(phase) + osc(phase2));

        // Envelope: quick attack, exponential decay.
        let env_attack = if i < attack { i as f32 / attack as f32 } else { 1.0 };
        let env_decay = (-t * (3.0 / seconds)).exp() * (1.0 - t / seconds);
        let v = sample * env_attack * env_decay * 0.6;
        data.extend_from_slice(&((v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_consistent() {
        let mut rng = Rng::default();
        rng.mix(42);
        let wav = synthesize_tone(&mut rng, 0.5);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        let riff_size = u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff_size + 8, wav.len());
        let data_size = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
        assert_eq!(data_size + 44, wav.len());
        assert_eq!(data_size, (0.5 * SAMPLE_RATE as f32) as usize * 2);
        // Something audible was produced and it stays within range.
        let peak = wav[44..]
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]).unsigned_abs())
            .max()
            .unwrap();
        assert!(peak > 3000, "peak too quiet: {peak}");
    }

    #[test]
    fn rng_is_not_stuck() {
        let mut rng = Rng::default();
        let samples: Vec<f32> = (0..1000).map(|_| rng.f32()).collect();
        assert!(samples.iter().all(|v| (0.0..1.0).contains(v)));
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!((0.4..0.6).contains(&mean), "mean {mean}");
        assert!((0..100).any(|_| rng.below(9) == 8));
    }
}
