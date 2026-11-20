use std::f64::consts::PI;
use std::time::Duration;

const MIN_FRAME_RATE: u32 = 60;
const POP_RISE: f64 = 0.12;
const POP_FALL: f64 = 0.24;
const BURST: f64 = 0.9;
const FIRST_BURST_GAP: f64 = 3.0;
const LATER_BURST_GAP: f64 = 10.0;
const FREQUENT_BURSTS_FOR: f64 = 120.0;
const REST_THICKNESS: f64 = 6.0;
const STILL_THICKNESS: f64 = 8.0;
const POP_FROM: f64 = 4.0;
const POP_PEAK: f64 = 12.0;
const BURST_SWELL: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    Settle,
    Pop,
}

pub fn cubic_bezier([x1, y1, x2, y2]: [f64; 4], progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    let coordinate = |a: f64, b: f64, s: f64| {
        3.0 * a * s * (1.0 - s).powi(2) + 3.0 * b * s * s * (1.0 - s) + s.powi(3)
    };
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..32 {
        let middle = (low + high) / 2.0;
        if coordinate(x1, x2, middle) < progress {
            low = middle;
        } else {
            high = middle;
        }
    }
    coordinate(y1, y2, (low + high) / 2.0)
}

impl Curve {
    pub fn at(self, progress: f64) -> f64 {
        cubic_bezier(
            match self {
                Curve::Settle => [0.2, 0.8, 0.2, 1.0],
                Curve::Pop => [0.34, 1.56, 0.64, 1.0],
            },
            progress,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween {
    from: f64,
    to: f64,
    start: f64,
    duration: f64,
    curve: Curve,
}

impl Tween {
    pub fn rest(value: f64) -> Tween {
        Tween {
            from: value,
            to: value,
            start: 0.0,
            duration: 0.0,
            curve: Curve::Settle,
        }
    }

    pub fn between(from: f64, to: f64, now: f64, duration: f64, curve: Curve) -> Tween {
        Tween {
            from,
            to,
            start: now,
            duration,
            curve,
        }
    }

    pub fn value(&self, now: f64) -> f64 {
        if self.moving(now) {
            self.from + (self.to - self.from) * self.curve.at((now - self.start) / self.duration)
        } else {
            self.to
        }
    }

    pub fn target(&self) -> f64 {
        self.to
    }

    pub fn moving(&self, now: f64) -> bool {
        now < self.start + self.duration
    }

    pub fn head_to(&mut self, to: f64, now: f64, duration: f64, curve: Curve) {
        if to != self.to {
            *self = Tween::between(self.value(now), to, now, duration, curve);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strip {
    pub thickness: f64,
    pub glow: f64,
}

fn last_burst_start(unseen_for: f64) -> Option<f64> {
    if unseen_for < FIRST_BURST_GAP {
        None
    } else if unseen_for <= FREQUENT_BURSTS_FOR {
        Some((unseen_for / FIRST_BURST_GAP).floor() * FIRST_BURST_GAP)
    } else {
        Some(
            FREQUENT_BURSTS_FOR
                + ((unseen_for - FREQUENT_BURSTS_FOR) / LATER_BURST_GAP).floor() * LATER_BURST_GAP,
        )
    }
}

pub fn next_burst(unseen_for: f64) -> f64 {
    if unseen_for < FREQUENT_BURSTS_FOR {
        ((unseen_for / FIRST_BURST_GAP).floor() + 1.0) * FIRST_BURST_GAP
    } else {
        FREQUENT_BURSTS_FOR
            + (((unseen_for - FREQUENT_BURSTS_FOR) / LATER_BURST_GAP).floor() + 1.0)
                * LATER_BURST_GAP
    }
}

fn burst_progress(unseen_for: f64) -> Option<f64> {
    last_burst_start(unseen_for)
        .map(|start| (unseen_for - start) / BURST)
        .filter(|progress| *progress < 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attention {
    pub since_first: f64,
    pub since_newest: f64,
}

pub fn unseen_moving(attention: Attention) -> bool {
    attention.since_first < POP_RISE + POP_FALL || burst_progress(attention.since_newest).is_some()
}

pub fn unseen_strip(attention: Attention, reduced_motion: bool) -> Strip {
    let popped = |thickness: f64| Strip {
        thickness,
        glow: ((thickness - POP_FROM) / (POP_PEAK - POP_FROM)).clamp(0.0, 1.0),
    };
    let Attention {
        since_first,
        since_newest,
    } = attention;
    if reduced_motion {
        Strip {
            thickness: STILL_THICKNESS,
            glow: 0.0,
        }
    } else if since_first < POP_RISE {
        popped(POP_FROM + (POP_PEAK - POP_FROM) * Curve::Pop.at(since_first / POP_RISE))
    } else if since_first < POP_RISE + POP_FALL {
        popped(
            POP_PEAK
                - (POP_PEAK - REST_THICKNESS)
                    * Curve::Settle.at((since_first - POP_RISE) / POP_FALL),
        )
    } else {
        let swell =
            burst_progress(since_newest).map_or(0.0, |progress| (PI * progress).sin().powi(2));
        Strip {
            thickness: REST_THICKNESS + BURST_SWELL * swell,
            glow: swell,
        }
    }
}

pub fn frame_interval(refresh_rate: u32) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(refresh_rate.max(MIN_FRAME_RATE)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steady(unseen_for: f64) -> Attention {
        Attention {
            since_first: unseen_for,
            since_newest: unseen_for,
        }
    }

    fn near(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() < 1e-3
    }

    #[test]
    fn curves_start_at_zero_and_end_at_one() {
        for curve in [Curve::Settle, Curve::Pop] {
            assert!(near(curve.at(0.0), 0.0), "{curve:?}");
            assert!(near(curve.at(1.0), 1.0), "{curve:?}");
            assert!(near(curve.at(-1.0), 0.0) && near(curve.at(2.0), 1.0));
        }
    }

    #[test]
    fn settle_is_fast_early_and_never_overshoots() {
        assert!(Curve::Settle.at(0.25) > 0.6);
        let samples: Vec<f64> = (0..=100)
            .map(|step| Curve::Settle.at(f64::from(step) / 100.0))
            .collect();
        assert!(samples.windows(2).all(|pair| pair[1] >= pair[0] - 1e-9));
        assert!(samples.iter().all(|value| *value <= 1.0 + 1e-9));
    }

    #[test]
    fn pop_overshoots_then_returns() {
        let peak = (0..=100)
            .map(|step| Curve::Pop.at(f64::from(step) / 100.0))
            .fold(0.0, f64::max);
        assert!(peak > 1.05 && peak < 1.15, "{peak}");
    }

    #[test]
    fn a_linear_bezier_is_the_identity() {
        for progress in [0.1, 0.5, 0.9] {
            assert!(near(
                cubic_bezier([0.25, 0.25, 0.75, 0.75], progress),
                progress
            ));
        }
    }

    #[test]
    fn tweens_ease_from_their_current_value_and_stop_at_the_target() {
        let mut tween = Tween::rest(4.0);
        assert!(!tween.moving(0.0));
        tween.head_to(6.0, 1.0, 0.12, Curve::Settle);
        assert!(tween.moving(1.06));
        let midway = tween.value(1.06);
        assert!(midway > 5.0 && midway < 6.0, "{midway}");
        assert_eq!(tween.value(1.12), 6.0);
        assert!(!tween.moving(1.12));
        tween.head_to(4.0, 1.12, 0.12, Curve::Settle);
        assert!(near(tween.value(1.12), 6.0));
        assert!(tween.value(1.18) < 6.0 && tween.value(1.18) > 4.0);
        let mut instant = Tween::rest(0.0);
        instant.head_to(1.0, 3.0, 0.0, Curve::Settle);
        assert_eq!(instant.value(3.0), 1.0);
        assert!(!instant.moving(3.0));
    }

    #[test]
    fn a_burst_swells_from_rest_to_ten_and_back() {
        let at = |phase: f64| unseen_strip(steady(3.0 + phase * 0.9), false);
        assert_eq!(
            at(0.0),
            Strip {
                thickness: 6.0,
                glow: 0.0
            }
        );
        assert!(near(at(0.5).thickness, 10.0) && near(at(0.5).glow, 1.0));
        assert!(near(at(1.0).thickness, 6.0) && near(at(1.0).glow, 0.0));
        assert_eq!(
            unseen_strip(steady(2.0), false),
            Strip {
                thickness: 6.0,
                glow: 0.0
            }
        );
    }

    #[test]
    fn the_first_unseen_alert_pops_to_twelve_then_settles_at_six() {
        assert!(near(unseen_strip(steady(0.0), false).thickness, 4.0));
        assert!(near(unseen_strip(steady(0.12), false).thickness, 12.0));
        assert!(near(unseen_strip(steady(0.36), false).thickness, 6.0));
        assert!(unseen_moving(steady(0.3)));
        assert!(!unseen_moving(steady(1.0)));
        let later_alert = Attention {
            since_first: 10.0,
            since_newest: 0.05,
        };
        assert_eq!(unseen_strip(later_alert, false).thickness, 6.0);
        assert!(!unseen_moving(later_alert));
        assert!(unseen_moving(Attention {
            since_first: 10.0,
            since_newest: 3.2,
        }));
    }

    #[test]
    fn reduced_motion_holds_a_still_eight_point_strip() {
        for unseen_for in [0.0, 0.1, 3.45, 200.0] {
            assert_eq!(
                unseen_strip(steady(unseen_for), true),
                Strip {
                    thickness: 8.0,
                    glow: 0.0
                }
            );
        }
    }

    #[test]
    fn bursts_repeat_every_three_seconds_for_two_minutes_then_every_ten() {
        assert_eq!(next_burst(0.0), 3.0);
        assert_eq!(next_burst(3.5), 6.0);
        assert_eq!(next_burst(119.0), 120.0);
        assert_eq!(next_burst(120.0), 130.0);
        assert_eq!(next_burst(131.0), 140.0);
        assert!(unseen_moving(steady(117.5)) && !unseen_moving(steady(118.0)));
        assert!(unseen_moving(steady(130.5)) && !unseen_moving(steady(133.0)));
    }

    #[test]
    fn frames_follow_the_refresh_rate_but_never_drop_below_sixty() {
        assert_eq!(frame_interval(144), Duration::from_secs_f64(1.0 / 144.0));
        assert_eq!(frame_interval(60), Duration::from_secs_f64(1.0 / 60.0));
        assert_eq!(frame_interval(30), Duration::from_secs_f64(1.0 / 60.0));
        assert_eq!(frame_interval(0), Duration::from_secs_f64(1.0 / 60.0));
    }
}
