use bytemuck::Pod;
use bytemuck::Zeroable;
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

/// Discriminants for `MaterialInfo::kind`.
///
/// Plain integers rather than an enum: this type is uploaded to the GPU, and only
/// types where every bit pattern is valid can be reinterpreted as bytes.
pub mod material_kind {
    pub const LAMBERTIAN: u32 = 0;
    pub const METAL: u32 = 1;
    pub const DIELECTRIC: u32 = 2;
    pub const DIFFUSE_LIGHT: u32 = 3;
}

#[derive(Clone, Copy, Default, Pod, Zeroable)]
#[repr(C)]
pub struct MaterialInfo {
    pub kind: u32,
    pub index: u32,
}

impl MaterialInfo {
    pub const fn lambertian(index: u32) -> Self {
        Self {
            kind: material_kind::LAMBERTIAN,
            index,
        }
    }

    pub const fn metal(index: u32) -> Self {
        Self {
            kind: material_kind::METAL,
            index,
        }
    }

    pub const fn dielectric(index: u32) -> Self {
        Self {
            kind: material_kind::DIELECTRIC,
            index,
        }
    }

    pub const fn diffuse_light(index: u32) -> Self {
        Self {
            kind: material_kind::DIFFUSE_LIGHT,
            index,
        }
    }
}

/// Every material in a scene, grouped by kind. The host builds these and uploads
/// them; the shader only reads them.
#[derive(Default)]
pub struct MaterialTable<'a> {
    pub lambertians: &'a [Lambertian],
    pub metals: &'a [Metal],
    pub dielectrics: &'a [Dielectric],
    pub diffuse_lights: &'a [DiffuseLight],
}

impl Material for MaterialTable<'_> {
    fn scatter(
        &self,
        state: &mut RandState,
        tex_table: &TextureTable<'_>,
        incoming: &Ray,
        rec: &HitRecord,
        scattered: &mut Ray,
        attenuation: &mut Color,
    ) -> bool {
        let index = rec.mat.index as usize;

        match rec.mat.kind {
            material_kind::LAMBERTIAN => self.lambertians[index].scatter(
                state,
                tex_table,
                incoming,
                rec,
                scattered,
                attenuation,
            ),
            material_kind::METAL => {
                self.metals[index].scatter(state, tex_table, incoming, rec, scattered, attenuation)
            }
            material_kind::DIELECTRIC => self.dielectrics[index].scatter(
                state,
                tex_table,
                incoming,
                rec,
                scattered,
                attenuation,
            ),
            // Diffuse lights don't scatter, they only emit. The kind is a plain
            // integer read from a buffer, so an unknown one lands here too
            _ => false,
        }
    }

    fn emitted(
        &self,
        state: &mut RandState,
        tex_table: &TextureTable<'_>,
        rec: &HitRecord,
        u: F,
        v: F,
        point: Point3,
    ) -> Color {
        match rec.mat.kind {
            material_kind::DIFFUSE_LIGHT => self.diffuse_lights[rec.mat.index as usize]
                .emitted(state, tex_table, rec, u, v, point),
            _ => Color::new(0., 0., 0.),
        }
    }
}
