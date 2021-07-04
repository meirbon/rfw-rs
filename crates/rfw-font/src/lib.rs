use glyph_brush::{
    ab_glyph::{point, FontArc},
    BrushAction, BrushError, GlyphBrush, GlyphBrushBuilder,
};
use rfw::prelude::*;

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

#[derive(Debug, Clone)]
pub struct FontSource(Vec<u8>);

impl From<Vec<u8>> for FontSource {
    fn from(b: Vec<u8>) -> Self {
        Self(b)
    }
}

impl From<&[u8]> for FontSource {
    fn from(b: &[u8]) -> Self {
        Self(b.to_vec())
    }
}

impl<const COUNT: usize> From<&[u8; COUNT]> for FontSource {
    fn from(b: &[u8; COUNT]) -> Self {
        Self(b.to_vec())
    }
}

impl<const COUNT: usize> From<[u8; COUNT]> for FontSource {
    fn from(b: [u8; COUNT]) -> Self {
        Self(b.to_vec())
    }
}

impl FontRenderer {
    pub fn draw(&mut self, section: Section) {
        self.brush.queue(section);
    }

    pub fn from_bytes<B: Into<FontSource>>(bytes: B) -> Self {
        let font = FontArc::try_from_vec(bytes.into().0).unwrap();
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
    fn init(&mut self, instance: &mut rfw::Instance) {
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

        let tex_id = instance
            .get_resource_mut::<Scene>()
            .unwrap()
            .add_texture(texture);
        let mesh_id = instance
            .get_resource_mut::<Scene>()
            .unwrap()
            .add_2d(Mesh2D::new(
                vec![
                    vec3(-0.5, -0.5, 0.5),
                    vec3(0.5, -0.5, 0.5),
                    vec3(0.5, 0.5, 0.5),
                    vec3(-0.5, 0.5, 0.5),
                    vec3(-0.5, -0.5, 0.5),
                    vec3(0.5, 0.5, 0.5),
                ],
                vec![
                    vec2(0.01, 0.01),
                    vec2(0.99, 0.01),
                    vec2(0.99, 0.99),
                    vec2(0.01, 0.99),
                    vec2(0.01, 0.01),
                    vec2(0.99, 0.99),
                ],
                Some(tex_id),
                Vec4::ONE,
            ));

        let mut mesh_instance = instance
            .get_resource_mut::<Scene>()
            .unwrap()
            .add_2d_instance(mesh_id)
            .unwrap();

        let width = instance
            .get_resource_mut::<RenderSystem>()
            .unwrap()
            .render_width() as f32;
        let height = instance
            .get_resource_mut::<RenderSystem>()
            .unwrap()
            .render_height() as f32;
        mesh_instance.set_matrix(
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
        self.instance = Some(mesh_instance);

        instance
            .add_resource(FontSystem { tex_data })
            .add_system(update_fonts.system());
    }
}

fn update_fonts(
    mut this: ResMut<FontSystem>,
    mut font: ResMut<FontRenderer>,
    mut scene: ResMut<Scene>,
    system: Res<RenderSystem>,
) {
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

                    let index =
                        (x + offset[0] as usize) + (offset[1] as usize + y) * tex_width as usize;

                    this.tex_data[index] = (alpha << 24) | 0xFFFFFF;
                }
            }

            tex_changed = true;
        },
        to_vertex,
    ) {
        Ok(BrushAction::Draw(vertices)) => {
            let tex = font.tex_id as u32;
            let font_object = scene.get_2d_object_mut(font.mesh_id).unwrap();
            font_object
                .vertices
                .resize(vertices.len() * 6, Default::default());

            for (i, v) in vertices.into_iter().enumerate() {
                let v0 = Vertex2D {
                    vertex: vec3(v.min_x, v.min_y, 0.5),
                    uv: vec2(v.uv_min_x, v.uv_min_y),
                    tex,
                    color: v.color,
                };
                let v1 = Vertex2D {
                    vertex: vec3(v.max_x, v.min_y, 0.5),
                    uv: vec2(v.uv_max_x, v.uv_min_y),
                    tex,
                    color: v.color,
                };
                let v2 = Vertex2D {
                    vertex: vec3(v.max_x, v.max_y, 0.5),
                    uv: vec2(v.uv_max_x, v.uv_max_y),
                    tex,
                    color: v.color,
                };
                let v3 = Vertex2D {
                    vertex: vec3(v.min_x, v.max_y, 0.5),
                    uv: vec2(v.uv_min_x, v.uv_max_y),
                    tex,
                    color: v.color,
                };

                font_object.vertices[i * 6] = v0;
                font_object.vertices[i * 6 + 1] = v1;
                font_object.vertices[i * 6 + 2] = v2;
                font_object.vertices[i * 6 + 3] = v3;
                font_object.vertices[i * 6 + 4] = v0;
                font_object.vertices[i * 6 + 5] = v2;
            }

            font_object.update_triangles();
        }
        Ok(BrushAction::ReDraw) => {}
        Err(BrushError::TextureTooSmall { suggested }) => {
            this.tex_data
                .resize((suggested.0 * suggested.1) as usize, 0);
            font.tex_width = suggested.0;
            font.tex_height = suggested.1;
            tex_changed = true;
        }
    }

    if tex_changed {
        if let Some(tex) = scene.get_materials_mut().get_texture_mut(font.tex_id) {
            tex.data.resize(this.tex_data.len(), 0);
            tex.data.copy_from_slice(&this.tex_data);
            tex.width = font.tex_width;
            tex.height = font.tex_height;
            tex.mip_levels = 1;
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
    pub color: Vec4,
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
    use glyph_brush::ab_glyph::Rect;

    let mut rect = Rect {
        min: point(pixel_coords.min.x as f32, pixel_coords.min.y as f32),
        max: point(pixel_coords.max.x as f32, pixel_coords.max.y as f32),
    };

    // handle overlapping bounds, modify uv_rect to preserve texture aspect
    if rect.max.x > bounds.max.x {
        let old_width = rect.width();
        rect.max.x = bounds.max.x;
        tex_coords.max.x = tex_coords.min.x + tex_coords.width() * rect.width() / old_width;
    }

    if rect.min.x < bounds.min.x {
        let old_width = rect.width();
        rect.min.x = bounds.min.x;
        tex_coords.min.x = tex_coords.max.x - tex_coords.width() * rect.width() / old_width;
    }

    if rect.max.y > bounds.max.y {
        let old_height = rect.height();
        rect.max.y = bounds.max.y;
        tex_coords.max.y = tex_coords.min.y + tex_coords.height() * rect.height() / old_height;
    }

    if rect.min.y < bounds.min.y {
        let old_height = rect.height();
        rect.min.y = bounds.min.y;
        tex_coords.min.y = tex_coords.max.y - tex_coords.height() * rect.height() / old_height;
    }

    BrushVertex {
        min_x: rect.min.x,
        min_y: rect.min.y,
        max_x: rect.max.x,
        max_y: rect.max.y,
        uv_min_x: tex_coords.min.x,
        uv_min_y: tex_coords.min.y,
        uv_max_x: tex_coords.max.x,
        uv_max_y: tex_coords.max.y,
        color: Vec4::from(extra.color),
    }
}
