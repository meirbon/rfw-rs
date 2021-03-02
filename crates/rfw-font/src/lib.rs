use glyph_brush::{
    ab_glyph::{point, FontArc},
    BrushAction, BrushError, GlyphBrush, GlyphBrushBuilder,
};
use rfw::prelude::{RenderSystem, *};

pub use glyph_brush::{Section, Text};

pub struct FontRenderer {
    brush: GlyphBrush<BrushVertex>,
    tex_width: u32,
    tex_height: u32,
    tex_id: usize,
    mesh_id: MeshId2D,
    prev_dims: (u32, u32),
    instance: Option<InstanceHandle2D>,
}

pub struct FontSystem {
    tex_data: Vec<u32>,
}

impl FontRenderer {
    pub fn draw(&mut self, section: Section) {
        self.brush.queue(section);
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let font = FontArc::try_from_vec(bytes.to_vec()).unwrap();
        let brush = GlyphBrushBuilder::using_font(font).build();
        Self {
            brush,
            tex_width: 0,
            tex_height: 0,
            tex_id: 0,
            mesh_id: MeshId2D::INVALID,
            instance: None,
            prev_dims: (0, 0),
        }
    }
}

impl Plugin for FontRenderer {
    fn init(&mut self, resources: &mut ResourceList, scheduler: &mut Scheduler) {
        let mut scene = resources.get_resource_mut::<Scene>().unwrap();
        let system = resources.get_resource_mut::<RenderSystem>().unwrap();

        let (tex_width, tex_height) = self.brush.texture_dimensions();
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

        let tex_id = scene.add_texture(texture);
        let mesh_id = scene.add_2d(Mesh2D::new(
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
        ));

        let mut instance = scene.add_2d_instance(mesh_id).unwrap();
        let width = system.render_width() as f32;
        let height = system.render_height() as f32;
        instance.set_matrix(
            Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0))
                * Mat4::from_translation(Vec3::new(
                    -(width as f32 / 2.0),
                    -(height as f32 / 2.0),
                    0.0,
                )),
        );

        self.tex_width = tex_width;
        self.tex_height = tex_height;
        self.tex_id = tex_id;
        self.mesh_id = mesh_id;
        self.instance = Some(instance);

        scheduler.add_system(FontSystem { tex_data });
    }
}

impl System for FontSystem {
    fn run(&mut self, resources: &ResourceList) {
        let mut scene = resources.get_resource_mut::<Scene>().unwrap();
        let system = resources.get_resource_mut::<RenderSystem>().unwrap();
        let mut font = resources.get_resource_mut::<FontRenderer>().unwrap();

        let width = system.render_width();
        let height = system.render_height();
        if font.prev_dims.0 != width || font.prev_dims.1 != height {
            if let Some(instance) = &mut font.instance {
                instance.set_matrix(
                    Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0))
                        * Mat4::from_translation(Vec3::new(
                            -(width as f32 / 2.0),
                            -(height as f32 / 2.0),
                            0.0,
                        )),
                );
            }
        }

        let mut tex_changed = false;
        let tex_width = font.tex_width;
        let brush = &mut font.brush;
        let tex_data = self.tex_data.as_mut_slice();
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
                let has_tex = font.tex_id as u32;
                let mut verts = Vec::with_capacity(vertices.len() * 6);
                let vertices: Vec<_> = vertices
                    .iter()
                    .map(|v| {
                        let v0 = Vertex2D {
                            vertex: [v.min_x, v.min_y, 0.5],
                            uv: [v.uv_min_x, v.uv_min_y],
                            has_tex,
                            color: v.color,
                        };
                        let v1 = Vertex2D {
                            vertex: [v.max_x, v.min_y, 0.5],
                            uv: [v.uv_max_x, v.uv_min_y],
                            has_tex,
                            color: v.color,
                        };
                        let v2 = Vertex2D {
                            vertex: [v.max_x, v.max_y, 0.5],
                            uv: [v.uv_max_x, v.uv_max_y],
                            has_tex,
                            color: v.color,
                        };
                        let v3 = Vertex2D {
                            vertex: [v.min_x, v.max_y, 0.5],
                            uv: [v.uv_min_x, v.uv_max_y],
                            has_tex,
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
                mesh.tex_id = Some(font.tex_id);
                scene.set_2d_object(font.mesh_id, mesh).unwrap();
            }
            Ok(BrushAction::ReDraw) => {}
            Err(BrushError::TextureTooSmall { suggested }) => {
                self.tex_data
                    .resize((suggested.0 * suggested.1) as usize, 0);
                font.tex_width = suggested.0;
                font.tex_height = suggested.1;
            }
        }

        if tex_changed {
            if let Some(tex) = scene.materials.get_texture_mut(font.tex_id) {
                tex.data = self.tex_data.clone();
                tex.width = font.tex_width;
                tex.height = font.tex_height;
                tex.mip_levels = 1;
            }
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
