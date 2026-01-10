use rtx_prim::Color;
use rtx_prim::F;
use rtx_prim::Point3;

use crate::TextureInfo;

pub trait Texture {
    fn value(&self, info: TextureInfo, u: F, v: F, point: Point3) -> Color;
}
