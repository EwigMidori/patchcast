/// Animation timing, easing, and interpolation utilities.
///
/// Maps frame numbers to animation progress values for smooth
/// rendering of code walkthroughs.
use crate::style::{ease_in_out_cubic, ease_out_quad};

/// Progress value in [0.0, 1.0] for a given frame within a scene.
pub fn scene_progress(frame: u32, total_frames: u32) -> f32 {
    if total_frames == 0 {
        return 1.0;
    }
    (frame as f32 / total_frames as f32).clamp(0.0, 1.0)
}

/// Number of lines to reveal at a given frame in a code reveal animation.
pub fn lines_revealed(frame: u32, total_frames: u32, total_lines: usize) -> usize {
    if total_frames == 0 || total_lines == 0 {
        return total_lines;
    }
    let progress = scene_progress(frame, total_frames);
    let eased = ease_out_quad(progress);
    let count = (eased * total_lines as f32).ceil() as usize;
    count.min(total_lines)
}

/// Alpha value for a fade-in effect.
pub fn fade_in_alpha(frame: u32, total_frames: u32) -> f32 {
    let progress = scene_progress(frame, total_frames);
    ease_in_out_cubic(progress)
}

/// Alpha value for a fade-out effect.
pub fn fade_out_alpha(frame: u32, total_frames: u32) -> f32 {
    let progress = scene_progress(frame, total_frames);
    1.0 - ease_in_out_cubic(progress)
}

/// Crossfade progress: returns (old_alpha, new_alpha).
pub fn crossfade(frame: u32, total_frames: u32) -> (f32, f32) {
    let progress = scene_progress(frame, total_frames);
    let eased = ease_in_out_cubic(progress);
    (1.0 - eased, eased)
}

/// Pulsing highlight effect: returns an alpha multiplier that
/// pulses between `min_alpha` and `max_alpha`.
pub fn pulse_highlight(frame: u32, total_frames: u32, min_alpha: f32, max_alpha: f32) -> f32 {
    if total_frames == 0 {
        return max_alpha;
    }
    let progress = scene_progress(frame, total_frames);
    // One full pulse cycle: 0 -> max -> min -> max.
    let cycle = (progress * 2.0 * std::f32::consts::PI).sin();
    let normalized = (cycle + 1.0) / 2.0; // Map [-1, 1] to [0, 1].
    min_alpha + normalized * (max_alpha - min_alpha)
}

/// Slide-in offset (from right to final position).
/// Returns an x-offset in pixels that decreases to 0.
pub fn slide_in_offset(frame: u32, total_frames: u32, max_offset: f32) -> f32 {
    let progress = scene_progress(frame, total_frames);
    let eased = ease_out_quad(progress);
    max_offset * (1.0 - eased)
}

/// A keyframe-based timeline for complex multi-stage animations.
#[derive(Debug, Clone)]
pub struct Timeline {
    /// Each segment is (start_fraction, end_fraction, label).
    segments: Vec<(f32, f32, String)>,
}

impl Timeline {
    /// Create a timeline from a list of (duration_weight, label) pairs.
    /// Weights are normalized so they sum to 1.0.
    pub fn new(segments: &[(f32, &str)]) -> Self {
        let total_weight: f32 = segments.iter().map(|(w, _)| w).sum();
        let mut built = Vec::new();
        let mut cursor = 0.0f32;

        for (weight, label) in segments {
            let fraction = weight / total_weight;
            built.push((cursor, cursor + fraction, label.to_string()));
            cursor += fraction;
        }

        Self { segments: built }
    }

    /// Given overall progress [0, 1], return which segment is active
    /// and the local progress within that segment.
    pub fn resolve(&self, progress: f32) -> (&str, f32) {
        let progress = progress.clamp(0.0, 1.0);
        for (start, end, label) in &self.segments {
            if progress >= *start && progress <= *end {
                let local = if (end - start).abs() < f32::EPSILON {
                    1.0
                } else {
                    (progress - start) / (end - start)
                };
                return (label, local.clamp(0.0, 1.0));
            }
        }
        // Past the end — return last segment at 100%.
        if let Some((_, _, label)) = self.segments.last() {
            (label, 1.0)
        } else {
            ("", 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_progress() {
        assert!((scene_progress(0, 30) - 0.0).abs() < 0.001);
        assert!((scene_progress(15, 30) - 0.5).abs() < 0.001);
        assert!((scene_progress(30, 30) - 1.0).abs() < 0.001);
        assert!((scene_progress(0, 0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_lines_revealed() {
        assert_eq!(lines_revealed(0, 30, 10), 0);
        assert_eq!(lines_revealed(30, 30, 10), 10);
        // Midpoint should reveal some but not all.
        let mid = lines_revealed(15, 30, 10);
        assert!(mid > 0 && mid <= 10);
    }

    #[test]
    fn test_lines_revealed_edge_cases() {
        assert_eq!(lines_revealed(0, 0, 10), 10);
        assert_eq!(lines_revealed(5, 10, 0), 0);
    }

    #[test]
    fn test_fade_in_out() {
        assert!(fade_in_alpha(0, 30) < 0.01);
        assert!(fade_in_alpha(30, 30) > 0.99);
        assert!(fade_out_alpha(0, 30) > 0.99);
        assert!(fade_out_alpha(30, 30) < 0.01);
    }

    #[test]
    fn test_crossfade() {
        let (old, new) = crossfade(0, 30);
        assert!(old > 0.99);
        assert!(new < 0.01);

        let (old, new) = crossfade(30, 30);
        assert!(old < 0.01);
        assert!(new > 0.99);

        let (old, new) = crossfade(15, 30);
        assert!((old + new - 1.0).abs() < 0.01, "Crossfade should sum to ~1.0");
    }

    #[test]
    fn test_pulse_highlight() {
        let v = pulse_highlight(0, 60, 0.3, 1.0);
        assert!(v >= 0.3 && v <= 1.0);
    }

    #[test]
    fn test_slide_in_offset() {
        assert!((slide_in_offset(0, 30, 100.0) - 100.0).abs() < 1.0);
        assert!(slide_in_offset(30, 30, 100.0).abs() < 1.0);
    }

    #[test]
    fn test_timeline_resolve() {
        let tl = Timeline::new(&[
            (1.0, "intro"),
            (2.0, "main"),
            (1.0, "outro"),
        ]);

        let (seg, _) = tl.resolve(0.0);
        assert_eq!(seg, "intro");

        let (seg, _) = tl.resolve(0.5);
        assert_eq!(seg, "main");

        let (seg, _) = tl.resolve(0.99);
        assert_eq!(seg, "outro");
    }

    #[test]
    fn test_timeline_local_progress() {
        let tl = Timeline::new(&[
            (1.0, "a"),
            (1.0, "b"),
        ]);

        let (_, local) = tl.resolve(0.25);
        assert!((local - 0.5).abs() < 0.01);

        let (_, local) = tl.resolve(0.75);
        assert!((local - 0.5).abs() < 0.01);
    }
}
