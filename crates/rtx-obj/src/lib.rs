#![cfg_attr(target_arch = "spirv", no_std)]
#![no_std]

pub mod quad;
pub mod sphere;

pub use quad::*;
pub use sphere::*;
