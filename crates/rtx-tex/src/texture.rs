use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::F;

pub trait Texture {
    fn value(&self, u: F, v: F, point: Point3) -> Color;
}
