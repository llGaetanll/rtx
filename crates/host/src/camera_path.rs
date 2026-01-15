use glam::Vec3;

use crate::spline::CatmullRomSpline;

/// A camera path defined by position and look-at splines over time.
///
/// Interpolates camera position and look-at target along Catmull-Rom splines,
/// allowing smooth camera motion through a scene.
pub struct CameraPath {
    position: CatmullRomSpline,
    look_at: CatmullRomSpline,
    duration: f32,
}

/// The result of evaluating a camera path at a point in time.
pub struct CameraFrame {
    /// Camera position in world space.
    pub position: Vec3,
    /// Point the camera is looking at in world space.
    pub look_at: Vec3,
}

impl CameraFrame {
    /// Compute the camera's forward direction (normalized).
    pub fn direction(&self) -> Vec3 {
        (self.look_at - self.position).normalize()
    }

    /// Compute the camera's up vector, given a world up reference.
    ///
    /// Uses the standard camera basis construction: right = forward × world_up,
    /// then up = right × forward. This ensures up is perpendicular to forward.
    pub fn up(&self, world_up: Vec3) -> Vec3 {
        let forward = self.direction();
        let right = forward.cross(world_up).normalize();
        right.cross(forward)
    }
}

impl CameraPath {
    /// Create a new camera path from position and look-at control points.
    ///
    /// Both splines must have at least 4 control points.
    /// Duration is in seconds.
    pub fn new(position_points: Vec<Vec3>, look_at_points: Vec<Vec3>, duration: f32) -> Self {
        Self {
            position: CatmullRomSpline::new(position_points),
            look_at: CatmullRomSpline::new(look_at_points),
            duration,
        }
    }

    /// Duration of the camera path in seconds.
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Evaluate the camera path at time t (in seconds).
    ///
    /// Returns the camera position and look-at target at that time.
    /// Times outside [0, duration] are clamped.
    pub fn evaluate(&self, time: f32) -> CameraFrame {
        let t = (time / self.duration).clamp(0.0, 1.0);
        CameraFrame {
            position: self.position.evaluate(t),
            look_at: self.look_at.evaluate(t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: Vec3, b: Vec3, epsilon: f32) -> bool {
        (a - b).length() < epsilon
    }

    #[test]
    fn evaluate_at_start() {
        let position_points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, 15.0),
        ];
        let look_at_points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        ];
        let path = CameraPath::new(position_points, look_at_points, 10.0);

        let frame = path.evaluate(0.0);
        assert!(
            approx_eq(frame.position, Vec3::new(0.0, 0.0, 5.0), 1e-6),
            "Expected position (0, 0, 5), got {:?}",
            frame.position
        );
        assert!(
            approx_eq(frame.look_at, Vec3::new(0.0, 1.0, 0.0), 1e-6),
            "Expected look_at (0, 1, 0), got {:?}",
            frame.look_at
        );
    }

    #[test]
    fn evaluate_at_end() {
        let position_points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, 15.0),
        ];
        let look_at_points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        ];
        let path = CameraPath::new(position_points, look_at_points, 10.0);

        let frame = path.evaluate(10.0);
        assert!(
            approx_eq(frame.position, Vec3::new(0.0, 0.0, 10.0), 1e-6),
            "Expected position (0, 0, 10), got {:?}",
            frame.position
        );
        assert!(
            approx_eq(frame.look_at, Vec3::new(0.0, 2.0, 0.0), 1e-6),
            "Expected look_at (0, 2, 0), got {:?}",
            frame.look_at
        );
    }

    #[test]
    fn evaluate_midpoint() {
        let position_points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, 15.0),
        ];
        let look_at_points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        ];
        let path = CameraPath::new(position_points, look_at_points, 10.0);

        let frame = path.evaluate(5.0);
        assert!(
            approx_eq(frame.position, Vec3::new(0.0, 0.0, 7.5), 1e-6),
            "Expected position (0, 0, 7.5), got {:?}",
            frame.position
        );
        assert!(
            approx_eq(frame.look_at, Vec3::new(0.0, 1.5, 0.0), 1e-6),
            "Expected look_at (0, 1.5, 0), got {:?}",
            frame.look_at
        );
    }

    #[test]
    fn direction_and_up() {
        let frame = CameraFrame {
            position: Vec3::new(0.0, 0.0, 5.0),
            look_at: Vec3::new(0.0, 0.0, 0.0),
        };

        let dir = frame.direction();
        assert!(
            approx_eq(dir, Vec3::new(0.0, 0.0, -1.0), 1e-6),
            "Expected direction (0, 0, -1), got {:?}",
            dir
        );

        let up = frame.up(Vec3::Y);
        assert!(
            approx_eq(up, Vec3::new(0.0, 1.0, 0.0), 1e-6),
            "Expected up (0, 1, 0), got {:?}",
            up
        );
    }

    #[test]
    fn clamps_time_out_of_range() {
        let position_points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
        ];
        let look_at_points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::new(0.0, 0.0, 3.0),
        ];
        let path = CameraPath::new(position_points, look_at_points, 5.0);

        let at_start = path.evaluate(0.0);
        let before_start = path.evaluate(-1.0);
        assert!(
            approx_eq(at_start.position, before_start.position, 1e-6),
            "Negative time should clamp to start"
        );

        let at_end = path.evaluate(5.0);
        let after_end = path.evaluate(10.0);
        assert!(
            approx_eq(at_end.position, after_end.position, 1e-6),
            "Time past duration should clamp to end"
        );
    }
}
