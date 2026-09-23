use crate::game::{VIEW_H, VIEW_W};

/// Pixels the view leads the hero by in the facing direction.
const LOOK_AHEAD: f32 = 10.0;
const LOOK_EASE: f32 = 0.04;
/// The hero can roam this far from the view center before the camera moves.
const DEAD_X: f32 = 8.0;
const DEAD_Y: f32 = 10.0;
const EASE_X: f32 = 0.2;
const EASE_Y: f32 = 0.25;
/// The eased camera may lag, but never so far that the hero gets closer than this to the view edge.
const MARGIN_X: f32 = 24.0;
const MARGIN_Y: f32 = 12.0;

/// Two-axis follow camera, clamped to the level.
pub struct Camera {
    x: f32,
    y: f32,
    look: f32,
    max_x: f32,
    max_y: f32,
}

impl Camera {
    pub fn new(level_w: i32, level_h: i32) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            look: 0.0,
            max_x: (level_w - VIEW_W as i32).max(0) as f32,
            max_y: (level_h - VIEW_H as i32).max(0) as f32,
        }
    }

    /// Centers on `focus` immediately (level start, respawn).
    pub fn snap(&mut self, focus: (f32, f32), face: f32) {
        self.look = face * LOOK_AHEAD;
        self.x = focus.0 + self.look - VIEW_W as f32 / 2.0;
        self.y = focus.1 - VIEW_H as f32 / 2.0;
        self.clamp();
    }

    pub fn follow(&mut self, focus: (f32, f32), face: f32) {
        self.look += (face * LOOK_AHEAD - self.look) * LOOK_EASE;
        self.x = ease(self.x, focus.0 + self.look, VIEW_W as f32, DEAD_X, EASE_X, MARGIN_X);
        self.y = ease(self.y, focus.1, VIEW_H as f32, DEAD_Y, EASE_Y, MARGIN_Y);
        self.clamp();
    }

    /// Whole-pixel top-left of the view in level pixels, always inside the level.
    pub fn origin(&self) -> (i32, i32) {
        (self.x.round() as i32, self.y.round() as i32)
    }

    fn clamp(&mut self) {
        self.x = self.x.clamp(0.0, self.max_x);
        self.y = self.y.clamp(0.0, self.max_y);
    }
}

/// One axis: keep `focus` inside the dead zone around the view center, easing toward that.
fn ease(cam: f32, focus: f32, view: f32, dead: f32, ease: f32, margin: f32) -> f32 {
    let center = cam + view / 2.0;
    let target = center.clamp(focus - dead, focus + dead);
    let center = center + (target - center) * ease;
    let reach = view / 2.0 - margin;
    center.clamp(focus - reach, focus + reach) - view / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: i32 = 512;
    const H: i32 = 360;

    #[test]
    fn never_leaves_the_level() {
        let mut c = Camera::new(W, H);
        for (fx, fy) in [(0.0, 0.0), (W as f32, H as f32), (0.0, H as f32), (W as f32, 0.0), (250.0, 180.0)] {
            c.snap((fx, fy), 1.0);
            for _ in 0..200 {
                c.follow((fx, fy), -1.0);
                let (x, y) = c.origin();
                assert!((0..=W - VIEW_W as i32).contains(&x) && (0..=H - VIEW_H as i32).contains(&y), "{x},{y}");
            }
        }
    }

    #[test]
    fn small_levels_stay_at_the_origin() {
        let mut c = Camera::new(VIEW_W as i32 - 10, VIEW_H as i32);
        c.snap((50.0, 30.0), 1.0);
        c.follow((50.0, 30.0), 1.0);
        assert_eq!(c.origin(), (0, 0));
    }

    #[test]
    fn dead_zone_holds_still_then_follows() {
        let mut c = Camera::new(W, H);
        c.snap((200.0, 150.0), 1.0);
        for _ in 0..300 {
            c.follow((200.0, 150.0), 1.0);
        }
        let still = c.origin();
        // Small moves inside the dead zone do not move the camera.
        for dx in [-3.0, 3.0, 0.0] {
            c.follow((200.0 + dx, 150.0), 1.0);
            assert_eq!(c.origin(), still);
        }
        // A big move drags it along, and the hero stays on screen the whole way.
        let mut fx = 200.0;
        for _ in 0..100 {
            fx += 2.0;
            c.follow((fx, 150.0), 1.0);
            let on_screen = fx - c.origin().0 as f32;
            assert!((MARGIN_X - LOOK_AHEAD..=VIEW_W as f32).contains(&on_screen), "{on_screen}");
        }
        assert!(c.origin().0 > still.0 + 100);
    }

    #[test]
    fn look_ahead_shifts_the_view_toward_the_facing_direction() {
        let mut c = Camera::new(W, H);
        c.snap((250.0, 180.0), 1.0);
        for _ in 0..300 {
            c.follow((250.0, 180.0), 1.0);
        }
        let right = c.origin().0;
        for _ in 0..300 {
            c.follow((250.0, 180.0), -1.0);
        }
        // The view leads the hero: facing left it sits further left than facing right.
        assert!(c.origin().0 < right);
    }

    #[test]
    fn falling_fast_keeps_the_hero_in_view_and_motion_is_smooth() {
        let mut c = Camera::new(W, H);
        c.snap((250.0, 40.0), 1.0);
        let (mut fy, mut prev) = (40.0, c.origin().1);
        for _ in 0..80 {
            fy += 3.5; // terminal fall speed
            c.follow((250.0, fy), 1.0);
            let y = c.origin().1;
            assert!((y - prev).abs() <= 4, "camera jumped {prev} -> {y}");
            assert!((0.0..VIEW_H as f32).contains(&(fy - y as f32)), "hero off screen");
            prev = y;
        }
    }
}
