use bevy::{
    math::{ivec2, vec2},
    prelude::*,
};
use std::{
    hash::{Hash, Hasher},
    ops::{Add, Mul, Sub},
};

use crate::segment::PRECISION_MUL;

/// Point in 2D space. Points that are close together are considered equal and hash to the same value.
#[derive(Copy, Clone, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[inline(always)]
    pub fn new(x: f32, y: f32) -> Self {
        Point { x, y }
    }

    #[inline]
    pub fn as_vec2(self) -> Vec2 {
        vec2(self.x, self.y)
    }

    #[inline]
    pub fn as_ivec2(self) -> IVec2 {
        ivec2(self.x as i32, self.y as i32)
    }

    /// Clamps x and y values between `-max` and `+max`
    #[inline]
    pub fn clamp_axes(self, max: f32) -> Self {
        Point {
            x: self.x.clamp(-max, max),
            y: self.y.clamp(-max, max),
        }
    }

    /// Points that are close together should hash to the same value.
    #[inline]
    pub fn get_hashable(self) -> IVec2 {
        (self * PRECISION_MUL).as_ivec2()
    }
}

impl From<Vec2> for Point {
    fn from(value: Vec2) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

impl PartialEq for Point {
    /// Points that are considered equal should hash to the same value.
    fn eq(&self, other: &Self) -> bool {
        self.get_hashable() == other.get_hashable()
    }
}

/// Signals that `==` for [`Point`] is an equivalence relation (reflexive, transititive, symmetric)
impl Eq for Point {}

impl Hash for Point {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.get_hashable().hash(state);
    }
}

impl Mul<f32> for Point {
    type Output = Point;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x.mul(rhs),
            y: self.y.mul(rhs),
        }
    }
}

impl Add for Point {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Point {
            x: self.x.add(rhs.x),
            y: self.y.add(rhs.y),
        }
    }
}

impl Add<Vec2> for Point {
    type Output = Self;

    fn add(self, rhs: Vec2) -> Self::Output {
        Point {
            x: self.x.add(rhs.x),
            y: self.y.add(rhs.y),
        }
    }
}

impl Sub for Point {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Point {
            x: self.x.sub(rhs.x),
            y: self.y.sub(rhs.y),
        }
    }
}

impl Sub<Vec2> for Point {
    type Output = Self;

    fn sub(self, rhs: Vec2) -> Self::Output {
        Point {
            x: self.x.sub(rhs.x),
            y: self.y.sub(rhs.y),
        }
    }
}
