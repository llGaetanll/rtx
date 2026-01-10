use rtx_prim::Array;
use rtx_prim::Color;
use rtx_prim::F;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_tex::TextureTable;

use crate::Dielectric;
use crate::DiffuseLight;
use crate::HitRecord;
use crate::Lambertian;
use crate::Material;
use crate::Metal;

pub const MAT_TBL_LEN: usize = 32;

#[repr(C)]
pub struct MaterialTable {
    pub lambertians: Array<Lambertian, MAT_TBL_LEN>,
    pub metals: Array<Metal, MAT_TBL_LEN>,
    pub dielectrics: Array<Dielectric, MAT_TBL_LEN>,
    pub diffuse_lights: Array<DiffuseLight, MAT_TBL_LEN>,
}

impl MaterialTable {
    pub fn new() -> Self {
        Self {
            lambertians: Array::new(),
            metals: Array::new(),
            dielectrics: Array::new(),
            diffuse_lights: Array::new(),
        }
    }
}

impl Default for MaterialTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Material for MaterialTable {
    fn scatter(
        &self,
        state: &mut RandState,
        tex_table: &TextureTable,
        incoming: &Ray,
        rec: &HitRecord,
        scattered: &mut Ray,
        attenuation: &mut Color,
    ) -> bool {
        let mat = &rec.mat;
        match mat.kind {
            MaterialKind::Lambertian => self.lambertians[mat.index].scatter(
                state,
                tex_table,
                incoming,
                rec,
                scattered,
                attenuation,
            ),
            MaterialKind::Metal => self.metals[mat.index].scatter(
                state,
                tex_table,
                incoming,
                rec,
                scattered,
                attenuation,
            ),
            MaterialKind::Dielectric => self.dielectrics[mat.index].scatter(
                state,
                tex_table,
                incoming,
                rec,
                scattered,
                attenuation,
            ),
            // Diffuse lights don't scatter, they only emit
            MaterialKind::DiffuseLight => false,
        }
    }

    fn emitted(
        &self,
        state: &mut RandState,
        tex_table: &TextureTable,
        rec: &HitRecord,
        u: F,
        v: F,
        point: Point3,
    ) -> Color {
        match rec.mat.kind {
            MaterialKind::DiffuseLight => {
                self.diffuse_lights[rec.mat.index].emitted(state, tex_table, rec, u, v, point)
            }
            _ => Color::new(0., 0., 0.),
        }
    }
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct MaterialInfo {
    pub kind: MaterialKind,
    pub index: usize,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub enum MaterialKind {
    #[default]
    Lambertian,
    Metal,
    Dielectric,
    DiffuseLight,
}
