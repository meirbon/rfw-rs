use glyph_brush::{
    ab_glyph::{point, FontArc, InvalidFont},
    BrushAction, BrushError, GlyphBrush, GlyphBrushBuilder,
};
use rfw::prelude::*;

pub use glyph_brush::{Section, Text};
pub struct Font {
    brush: GlyphBrush<BrushVertex>,
    tex_data: Vec<u32>,
    tex_width: u32,
    tex_height: u32,
    tex_id: u32,
    mesh_id: u32,
    inst_id: u32,
}

impl Font {
    pub fn from_vec<T: Backend>(
        system: &mut RenderSystem<T>,
        data: Vec<u8>,
    ) -> Result<Self, InvalidFont> {
        let font = FontArc::try_from_vec(data)?;
        let brush = GlyphBrushBuilder::using_font(font).build();
        let (tex_width, tex_height) = brush.texture_dimensions();
        let tex_data = vec![0_u32; (tex_width * tex_height) as usize];
        let texture = Texture::from_bytes(
            unsafe {
                std::slice::from_raw_parts(
                    tex_data.as_ptr() as *const u8,
                    (tex_width * tex_height * 4) as usize,
                )
            },
            tex_width,
            tex_height,
            TextureFormat::BGRA,
            std::mem::size_of::<u32>(),
        );

        let tex_id = system.add_texture(texture).unwrap();
        let mesh = system
            .add_2d_object(Mesh2D::new(
                vec![
                    [-0.5, -0.5, 0.5],
                    [0.5, -0.5, 0.5],
                    [0.5, 0.5, 0.5],
                    [-0.5, 0.5, 0.5],
                    [-0.5, -0.5, 0.5],
                    [0.5, 0.5, 0.5],
                ],
                vec![
                    [0.01, 0.01],
                    [0.99, 0.01],
                    [0.99, 0.99],
                    [0.01, 0.99],
                    [0.01, 0.01],
                    [0.99, 0.99],
                ],
                Some(tex_id),
                [1.0; 4],
            ))
            .unwrap();

        let inst = system.create_2d_instance(mesh).unwrap();
        let width = system.render_width() as f32;
        let height = system.render_height() as f32;
        if let Some(inst) = system.get_2d_instance_mut(inst) {
            inst.transform =
                Mat4::orthographic_lh(0.0, width, height, 0.0, 1.0, -1.0).to_cols_array();
        }

        Ok(Self {
            brush,
            tex_data,
            tex_width,
            tex_height,
            tex_id,
            mesh_id: mesh,
            inst_id: inst as u32,
        })
    }

    pub fn from_bytes<T: Backend>(
        system: &mut RenderSystem<T>,
        data: &[u8],
    ) -> Result<Self, InvalidFont> {
        Self::from_vec(system, data.to_vec())
    }

    pub fn resize<T: Backend>(&mut self, system: &mut RenderSystem<T>) {
        let width = system.render_width();
        let height = system.render_height();
        if let Some(inst) = system.get_2d_instance_mut(self.inst_id as usize) {
            inst.transform =
                Mat4::orthographic_lh(0.0, width as f32, height as f32, 0.0, 1.0, -1.0)
                    .to_cols_array();
        }
    }

    pub fn draw(&mut self, section: Section) {
        self.brush.queue(section);
    }

    pub fn synchronize<T: Backend>(&mut self, system: &mut RenderSystem<T>) {
        let mut tex_changed = false;
        let brush = &mut self.brush;
        let tex_data = &mut self.tex_data;
        let tex_width = self.tex_width;
        match brush.process_queued(
            |rect, t_data| {
                let offset: [u32; 2] = [rect.min[0], rect.min[1]];
                let size: [u32; 2] = [rect.width(), rect.height()];

                let width = size[0] as usize;
                let height = size[1] as usize;

                for y in 0..height {
                    for x in 0..width {
                        let index = x + y * width;
                        let alpha = t_data[index] as u32;

                        let index = (x + offset[0] as usize)
                            + (offset[1] as usize + y) * tex_width as usize;

                        tex_data[index] = (alpha << 24) | 0xFFFFFF;
                    }
                }

                tex_changed = true;
            },
            to_vertex,
        ) {
            Ok(BrushAction::Draw(vertices)) => {
                let mut verts = Vec::with_capacity(vertices.len() * 6);
                let vertices: Vec<_> = vertices
                    .iter()
                    .map(|v| {
                        let v0 = Vertex2D {
                            vertex: [v.min_x, v.min_y, 0.5],
                            uv: [v.uv_min_x, v.uv_min_y],
                            has_tex: self.tex_id,
                            color: v.color,
                        };
                        let v1 = Vertex2D {
                            vertex: [v.max_x, v.min_y, 0.5],
                            uv: [v.uv_max_x, v.uv_min_y],
                            has_tex: self.tex_id,
                            color: v.color,
                        };
                        let v2 = Vertex2D {
                            vertex: [v.max_x, v.max_y, 0.5],
                            uv: [v.uv_max_x, v.uv_max_y],
                            has_tex: self.tex_id,
                            color: v.color,
                        };
                        let v3 = Vertex2D {
                            vertex: [v.min_x, v.max_y, 0.5],
                            uv: [v.uv_min_x, v.uv_max_y],
                            has_tex: self.tex_id,
                            color: v.color,
                        };

                        (v0, v1, v2, v3, v0, v2)
                    })
                    .collect();
                vertices.into_iter().for_each(|vs| {
                    verts.push(vs.0);
                    verts.push(vs.1);
                    verts.push(vs.2);
                    verts.push(vs.3);
                    verts.push(vs.4);
                    verts.push(vs.5);
                });

                let mut mesh = Mesh2D::from(verts);
                mesh.tex_id = Some(self.tex_id);
                system.set_2d_object(self.mesh_id, mesh).unwrap();
            }
            Ok(BrushAction::ReDraw) => {}
            Err(BrushError::TextureTooSmall { suggested }) => {
                tex_data.resize((suggested.0 * suggested.1) as usize, 0);
                self.tex_width = suggested.0;
                self.tex_height = suggested.1;
            }
        }

        if tex_changed {
            system
                .set_texture(
                    self.tex_id,
                    rfw::prelude::Texture {
                        width: self.tex_width,
                        height: self.tex_height,
                        data: self.tex_data.clone(),
                        mip_levels: 1,
                    },
                )
                .unwrap();
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct BrushVertex {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub uv_min_x: f32,
    pub uv_min_y: f32,
    pub uv_max_x: f32,
    pub uv_max_y: f32,
    pub color: [f32; 4],
}

#[inline]
fn to_vertex(
    glyph_brush::GlyphVertex {
        mut tex_coords,
        pixel_coords,
        bounds,
        extra,
    }: glyph_brush::GlyphVertex,
) -> BrushVertex {
    let gl_bounds = bounds;

    use glyph_brush::ab_glyph::Rect;

    let mut gl_rect = Rect {
        min: point(pixel_coords.min.x as f32, pixel_coords.min.y as f32),
        max: point(pixel_coords.max.x as f32, pixel_coords.max.y as f32),
    };

    // handle overlapping bounds, modify uv_rect to preserve texture aspect
    if gl_rect.max.x > gl_bounds.max.x {
        let old_width = gl_rect.width();
        gl_rect.max.x = gl_bounds.max.x;
        tex_coords.max.x = tex_coords.min.x + tex_coords.width() * gl_rect.width() / old_width;
    }

    if gl_rect.min.x < gl_bounds.min.x {
        let old_width = gl_rect.width();
        gl_rect.min.x = gl_bounds.min.x;
        tex_coords.min.x = tex_coords.max.x - tex_coords.width() * gl_rect.width() / old_width;
    }

    if gl_rect.max.y > gl_bounds.max.y {
        let old_height = gl_rect.height();
        gl_rect.max.y = gl_bounds.max.y;
        tex_coords.max.y = tex_coords.min.y + tex_coords.height() * gl_rect.height() / old_height;
    }

    if gl_rect.min.y < gl_bounds.min.y {
        let old_height = gl_rect.height();
        gl_rect.min.y = gl_bounds.min.y;
        tex_coords.min.y = tex_coords.max.y - tex_coords.height() * gl_rect.height() / old_height;
    }

    BrushVertex {
        min_x: gl_rect.min.x,
        min_y: gl_rect.min.y,
        max_x: gl_rect.max.x,
        max_y: gl_rect.max.y,
        uv_min_x: tex_coords.min.x,
        uv_min_y: tex_coords.min.y,
        uv_max_x: tex_coords.max.x,
        uv_max_y: tex_coords.max.y,
        color: extra.color,
    }
}
