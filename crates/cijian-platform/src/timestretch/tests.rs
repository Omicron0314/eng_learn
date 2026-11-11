use super::*;

const RATE: u32 = 16_000;

/// A source over a buffer that remembers the last requested seek.
struct Tone {
    samples: Vec<f32>,
    position: usize,
    seeked: Option<f64>,
}
impl Tone {
    fn sine(frequency: f64, seconds: f64) -> Self {
        let count = (RATE as f64 * seconds) as usize;
        Self {
            samples: (0..count)
                .map(|index| {
                    (frequency * std::f64::consts::TAU * index as f64 / RATE as f64).sin() as f32
                })
                .collect(),
            position: 0,
            seeked: None,
        }
    }
}
impl Iterator for Tone {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let value = self.samples.get(self.position).copied();
        self.position += 1;
        value
    }
}
impl Source for Tone {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.samples.len() - self.position.min(self.samples.len()))
    }
    fn channels(&self) -> u16 {
        1
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.samples.len() as f64 / f64::from(RATE),
        ))
    }
    fn try_seek(&mut self, position: Duration) -> Result<(), rodio::source::SeekError> {
        self.seeked = Some(position.as_secs_f64());
        self.position = (position.as_secs_f64() * f64::from(RATE)) as usize;
        Ok(())
    }
}

/// Mean frequency of the steady middle of the signal, from zero crossings.
fn frequency(samples: &[f32]) -> f64 {
    let skip = samples.len() / 5;
    let body = &samples[skip..samples.len() - skip];
    let crossings = body
        .windows(2)
        .filter(|pair| pair[0] <= 0. && pair[1] > 0.)
        .count();
    crossings as f64 * f64::from(RATE) / body.len() as f64
}

fn render(speed: f64) -> Vec<f32> {
    let tone = Tone::sine(220., 2.);
    stretch(tone, speed).collect()
}

#[test]
fn unity_speed_passes_the_original_samples_through() {
    let tone = Tone::sine(220., 0.5);
    let expected = tone.samples.clone();
    let played = stretch(Tone::sine(220., 0.5), 1.).collect::<Vec<_>>();
    assert_eq!(played.len(), expected.len());
    assert!(
        played
            .iter()
            .zip(&expected)
            .all(|(a, b)| (a - b).abs() < 1e-6)
    );
}

#[test]
fn speeding_up_keeps_the_pitch_and_shortens_the_audio() {
    let original = 2.;
    for speed in [1.1, 1.25, 1.5, 2.] {
        let played = render(speed);
        let expected = original / speed;
        let actual = played.len() as f64 / f64::from(RATE);
        assert!(
            (actual - expected).abs() < 0.06,
            "{speed}× produced {actual}s, expected about {expected}s"
        );
        let pitch = frequency(&played);
        assert!(
            (pitch - 220.).abs() < 220. * 0.03,
            "{speed}× moved the pitch to {pitch}Hz instead of 220Hz"
        );
    }
}

#[test]
fn slowing_down_keeps_the_pitch_and_lengthens_the_audio() {
    for speed in [0.9, 0.75] {
        let played = render(speed);
        let expected = 2. / speed;
        let actual = played.len() as f64 / f64::from(RATE);
        assert!(
            (actual - expected).abs() < 0.12,
            "{speed}× produced {actual}s, expected about {expected}s"
        );
        let pitch = frequency(&played);
        assert!(
            (pitch - 220.).abs() < 220. * 0.03,
            "{speed}× moved the pitch to {pitch}Hz instead of 220Hz"
        );
    }
}

#[test]
fn overlap_add_has_no_amplitude_ripple() {
    let played = render(1.25);
    let skip = played.len() / 4;
    let peak = played[skip..]
        .iter()
        .fold(0_f32, |peak, sample| peak.max(sample.abs()));
    assert!(
        (peak - 1.).abs() < 0.05,
        "windowed overlap-add should stay near unit gain, peak was {peak}"
    );
}

#[test]
fn stereo_channels_stay_interleaved_and_aligned() {
    let count = RATE as usize;
    let samples = (0..count)
        .flat_map(|index| {
            let value = (220. * std::f64::consts::TAU * index as f64 / f64::from(RATE)).sin();
            [value as f32, (value * 0.5) as f32]
        })
        .collect::<Vec<_>>();
    let inner = Tone {
        samples,
        position: 0,
        seeked: None,
    };
    let mut stretched = TimeStretch::with_format(inner, 1.25, 2, RATE);
    assert_eq!(stretched.channels(), 2);
    let played = (&mut stretched).collect::<Vec<_>>();
    assert_eq!(played.len() % 2, 0);
    let left = played.iter().step_by(2).copied().collect::<Vec<_>>();
    for pair in played.as_chunks::<2>().0 {
        assert!(
            (pair[1] - pair[0] * 0.5).abs() < 0.02,
            "right channel drifted from the left"
        );
    }
    assert!((frequency(&left) - 220.).abs() < 220. * 0.03);
}

#[test]
fn seeking_maps_the_played_time_back_to_the_file() {
    let mut stretched = stretch(Tone::sine(220., 10.), 1.25);
    let _ = stretched.next();
    stretched.try_seek(Duration::from_secs_f64(2.)).unwrap();
    assert_eq!(stretched.input_start, 2.5 * f64::from(RATE));
    // Seeking a play-through position keeps the following samples in range.
    let played = (&mut stretched).take(1000).collect::<Vec<_>>();
    assert_eq!(played.len(), 1000);
    assert!(played.iter().all(|sample| sample.is_finite()));
}

#[test]
fn total_duration_shrinks_with_speed() {
    let stretched = stretch(Tone::sine(220., 4.), 1.25);
    let total = stretched.total_duration().unwrap().as_secs_f64();
    assert!((total - 3.2).abs() < 0.01, "got {total}");
    let unity = stretch(Tone::sine(220., 4.), 1.);
    assert_eq!(unity.total_duration().unwrap().as_secs_f64(), 4.);
}
