use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_prim::F;
use rtx_tex::TextureTable;

use crate::Dielectric;
use crate::HitRecord;
use crate::Lambertian;
use crate::Material;
use crate::Metal;

#[repr(C)]
pub struct MaterialTable<const NL: usize, const NM: usize, const ND: usize> {
    pub lambertians: [Lambertian; NL],
    pub metals: [Metal; NM],
    pub dielectrics: [Dielectric; ND],
}

impl<const NL: usize, const NM: usize, const ND: usize> Material for MaterialTable<NL, NM, ND> {
    fn scatter<const NS: usize>(
        &self,
        state: &mut RandState,
        tex_table: &TextureTable<NS>,
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
        }
    }

    fn emitted<const NS: usize>(
        &self,
        _state: &mut RandState,
        _tex_table: &TextureTable<NS>,
        _u: F,
        _v: F,
        _point: Point3,
    ) -> Color {
        // Color::new(0., 0., 0.)
        todo!("Emission not yet supported")
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct MaterialInfo {
    pub kind: MaterialKind,
    pub index: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub enum MaterialKind {
    Lambertian,
    Metal,
    Dielectric,
}
