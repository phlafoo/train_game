use crate::point::Point;
use bevy::{math::vec2, prelude::*};
use std::hash::{Hash, Hasher};

/// If points are closer together than this, they will be considered equal and hash to the same value.
pub const PRECISION: f32 = 0.01;
pub const PRECISION_MUL: f32 = 1.0 / PRECISION;

/// A 2D line segment defined by its start and end points.
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    /// Start point
    pub a: Point,
    /// End point
    pub b: Point,
}

impl Segment {
    #[inline(always)]
    pub fn new(a: Point, b: Point) -> Self {
        Segment { a, b }
    }

    #[inline]
    pub fn as_vec2(&self) -> Vec2 {
        vec2(self.b.x - self.a.x, self.b.y - self.a.y)
        // vec2(self.b.x - self.a.x, self.b.y - self.b.x)
    }

    /// Returns the angle (in radians) between `self` and `rhs` (anchored at the origin) in the
    /// range `[-π, +π]`.
    ///
    /// Segments must have non-zero length.
    #[inline]
    pub fn angle_between(&self, other: &Self) -> f32 {
        let v1 = self.as_vec2();
        let v2 = other.as_vec2();
        v1.angle_between(v2)
    }

    /// Returns a segment with swapped start and end points.
    #[inline]
    pub fn reverse(&self) -> Self {
        Segment {
            a: self.b,
            b: self.a,
        }
    }

    /// Rise over run.
    #[inline(always)]
    pub fn get_slope(&self) -> f32 {
        (self.b.y - self.a.y) / (self.b.x - self.a.x)
    }

    /// Limits precision so similar slopes are considered equal.
    #[inline]
    pub fn get_slope_correlate(&self) -> i32 {
        (self.get_slope() * PRECISION_MUL) as i32
    }
}

impl PartialEq for Segment {
    /// Two segments are considered equal if they overlap completely (even if they are pointing in
    /// opposite directions).
    fn eq(&self, other: &Self) -> bool {
        if self.a == other.a && self.b == other.b {
            return true;
        }
        self.a == other.b && self.b == other.a
    }
}

/// Signals that `==` for [`Segment`] is an equivalence relation (reflexive, transititive, symmetric)
impl Eq for Segment {}

impl Hash for Segment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // e.g. these segments must hash to same value
        // s1: {(1,2),(1,4)}
        // s2: {(1,4),(1,2)}

        let mut p1 = self.a.get_hashable();
        let mut p2 = self.b.get_hashable();
        // p1.x must be smaller. If equal, p1.y must be smaller
        if p1.x > p2.x || (p1.x == p2.x && p1.y > p2.y) {
            std::mem::swap(&mut p1, &mut p2);
        }
        (p1, p2).hash(state)
    }
}
