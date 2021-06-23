use rfw::prelude::*;

#[allow(dead_code, non_snake_case, improper_ctypes, non_camel_case_types)]
mod ffi;

#[derive(Default)]
#[repr(C)]
pub struct CameraUniform {
    pub projection: Mat4,
    pub view_matrix: Mat4,
    pub combined: Mat4,
    pub matrix_2d: Mat4,
    pub view: CameraView3D,
}

pub struct MetalBackend {
    instance: *mut std::ffi::c_void,
}

impl FromWindowHandle for MetalBackend {
    fn init<W: HasRawWindowHandle>(
        window: &W,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Result<Box<Self>, Box<dyn std::error::Error>> {
        let instance;

        #[cfg(target_os = "macos")]
        {
            match window.raw_window_handle() {
                RawWindowHandle::MacOS(handle) => unsafe {
                    instance = ffi::create_instance(
                        handle.ns_window,
                        handle.ns_view,
                        width,
                        height,
                        scale_factor,
                    );
                },
                _ => panic!("Unsupported type of window handle."),
            }
        }

        #[cfg(target_os = "ios")]
        {
            todo!()
        }

        if !instance.is_null() {
            Ok(Box::new(Self { instance }))
        } else {
            panic!("Could not initialize Metal renderer.");
        }
    }
}

impl Backend for MetalBackend {
    fn set_2d_mesh(&mut self, id: usize, data: MeshData2D<'_>) {
        unsafe {
            ffi::set_2d_mesh(
                self.instance,
                id as u32,
                ffi::MeshData2D {
                    vertices: data.vertices.as_ptr() as *const ffi::Vertex2D,
                    num_vertices: data.vertices.len() as _,
                    tex_id: data.tex_id.map(|id| id as i32).unwrap_or(-1),
                },
            );
        }
    }

    fn set_2d_instances(&mut self, mesh: usize, instances: InstancesData2D<'_>) {
        unsafe {
            ffi::set_2d_instances(
                self.instance,
                mesh as u32,
                ffi::InstancesData2D {
                    matrices: instances.matrices.as_ptr() as *const ffi::simd_float4x4,
                    num_matrices: instances.matrices.len() as _,
                },
            )
        }
    }

    fn set_3d_mesh(&mut self, id: usize, data: MeshData3D<'_>) {
        unsafe {
            let mut bounds = ffi::Aabb::default();
            std::ptr::write(
                &mut bounds as *mut ffi::Aabb as *mut ffi::simd_float4,
                *(&data.bounds.min as *const Vec3 as *const ffi::simd_float4),
            );
            std::ptr::write(
                (&mut bounds as *mut ffi::Aabb as *mut ffi::simd_float4).add(1),
                *(&data.bounds.max as *const Vec3 as *const ffi::simd_float4),
            );

            ffi::set_3d_mesh(
                self.instance,
                id as u32,
                ffi::MeshData3D {
                    vertices: data.vertices.as_ptr() as *const ffi::Vertex3D,
                    num_vertices: data.vertices.len() as _,
                    triangles: data.triangles.as_ptr() as *const ffi::RTTriangle,
                    num_triangles: data.triangles.len() as _,
                    ranges: data.ranges.as_ptr() as *const ffi::VertexRange,
                    num_ranges: data.ranges.len() as _,
                    skin_data: data.skin_data.as_ptr() as *const ffi::JointData,
                    flags: std::ptr::read((&data.flags) as *const Mesh3dFlags as *const u32),
                    __bindgen_padding_0: [],
                    bounds,
                },
            );
        }
    }

    fn unload_3d_meshes(&mut self, ids: &[usize]) {
        unsafe {
            let ids = ids.iter().copied().map(|i| i as u32).collect::<Vec<_>>();
            ffi::unload_3d_meshes(self.instance, ids.as_ptr(), ids.len() as _);
        }
    }

    fn set_3d_instances(&mut self, mesh: usize, instances: InstancesData3D<'_>) {
        unsafe {
            let mut bounds = ffi::Aabb::default();
            std::ptr::write(
                &mut bounds as *mut ffi::Aabb as *mut ffi::simd_float4,
                *(&instances.local_aabb.min as *const Vec3 as *const ffi::simd_float4),
            );
            std::ptr::write(
                (&mut bounds as *mut ffi::Aabb as *mut ffi::simd_float4).add(1),
                *(&instances.local_aabb.max as *const Vec3 as *const ffi::simd_float4),
            );

            ffi::set_3d_instances(
                self.instance,
                mesh as _,
                ffi::InstancesData3D {
                    local_aabb: bounds,
                    matrices: instances.matrices.as_ptr() as *const ffi::matrix_float4x4,
                    num_matrices: instances.matrices.len() as _,
                    skin_ids: instances.skin_ids.as_ptr() as *const i32,
                    num_skin_ids: instances.skin_ids.len() as _,
                    flags: instances.flags.as_ptr() as *const u32,
                    num_flags: instances.flags.len() as _,
                },
            );
        }
    }

    fn set_materials(&mut self, materials: &[DeviceMaterial], _changed: &BitSlice) {
        unsafe {
            ffi::set_materials(
                self.instance,
                materials.as_ptr() as *const ffi::DeviceMaterial,
                materials.len() as _,
            );
        }
    }

    fn set_textures(&mut self, textures: &[TextureData<'_>], changed: &BitSlice) {
        let textures = textures
            .iter()
            .map(|t| ffi::TextureData {
                width: t.width,
                height: t.height,
                mip_levels: t.mip_levels,
                bytes: t.bytes.as_ptr(),
                format: unsafe {
                    std::ptr::read(&t.format as *const DataFormat as *const ffi::DataFormat)
                },
            })
            .collect::<Vec<ffi::TextureData>>();

        let changed = changed
            .iter()
            .take(textures.len())
            .map(|i| if *i { 1 } else { 0 })
            .collect::<Vec<u32>>();
        unsafe {
            ffi::set_textures(
                self.instance,
                textures.as_ptr(),
                textures.len() as _,
                changed.as_ptr(),
            );
        }
    }

    fn synchronize(&mut self) {
        unsafe {
            ffi::synchronize(self.instance);
        }
    }

    fn render(&mut self, camera_2d: CameraView2D, camera: CameraView3D, _mode: RenderMode) {
        unsafe {
            ffi::render(
                self.instance,
                std::ptr::read(&camera_2d.matrix as *const Mat4 as *const ffi::matrix_float4x4),
                ffi::CameraView3D {
                    pos: ffi::Vector3 {
                        x: camera.pos.x,
                        y: camera.pos.y,
                        z: camera.pos.z,
                    },
                    right: ffi::Vector3 {
                        x: camera.right.x,
                        y: camera.right.y,
                        z: camera.right.z,
                    },
                    up: ffi::Vector3 {
                        x: camera.up.x,
                        y: camera.up.y,
                        z: camera.up.z,
                    },
                    p1: ffi::Vector3 {
                        x: camera.p1.x,
                        y: camera.p1.y,
                        z: camera.p1.z,
                    },
                    direction: ffi::Vector3 {
                        x: camera.direction.x,
                        y: camera.direction.y,
                        z: camera.direction.z,
                    },
                    lens_size: camera.lens_size,
                    spread_angle: camera.spread_angle,
                    epsilon: camera.epsilon,
                    inv_width: camera.inv_width,
                    inv_height: camera.inv_height,
                    near_plane: camera.near_plane,
                    far_plane: camera.far_plane,
                    aspect_ratio: camera.aspect_ratio,
                    fov: camera.fov,
                    custom0: Default::default(),
                    custom1: Default::default(),
                },
            );
        }
    }

    fn resize(&mut self, window_size: (u32, u32), scale_factor: f64) {
        unsafe {
            ffi::resize(self.instance, window_size.0, window_size.1, scale_factor);
        }
    }

    fn set_point_lights(&mut self, _lights: &[PointLight], _changed: &BitSlice) {}

    fn set_spot_lights(&mut self, _lights: &[SpotLight], _changed: &BitSlice) {}

    fn set_area_lights(&mut self, _lights: &[AreaLight], _changed: &BitSlice) {}

    fn set_directional_lights(&mut self, _lights: &[DirectionalLight], _changed: &BitSlice) {}

    fn set_skybox(&mut self, _skybox: TextureData<'_>) {}

    fn set_skins(&mut self, _skins: &[SkinData<'_>], _changed: &BitSlice) {}
}

impl Drop for MetalBackend {
    fn drop(&mut self) {
        unsafe { ffi::destroy_instance(self.instance) };
        self.instance = std::ptr::null_mut();
    }
}

#[cfg(test)]
mod tests {
    use rfw::prelude::*;

    #[test]
    fn test_layout() {
        use crate::ffi;
        assert_eq!(
            std::mem::size_of::<Vec4>(),
            std::mem::size_of::<ffi::simd_float4>()
        );
        assert_eq!(
            std::mem::size_of::<Vec3>(),
            std::mem::size_of::<ffi::float3>()
        );
        assert_eq!(
            std::mem::size_of::<Vec2>(),
            std::mem::size_of::<ffi::simd_float2>()
        );
        assert_eq!(
            std::mem::size_of::<Mat4>(),
            std::mem::size_of::<ffi::matrix_float4x4>()
        );

        assert_eq!(
            std::mem::size_of::<Vertex3D>(),
            std::mem::size_of::<ffi::Vertex3D>()
        );
        assert_eq!(
            std::mem::size_of::<Vertex2D>(),
            std::mem::size_of::<ffi::Vertex2D>()
        );
        assert_eq!(
            std::mem::size_of::<CameraView3D>(),
            std::mem::size_of::<ffi::CameraView3D>()
        );
        assert_eq!(
            std::mem::size_of::<DeviceMaterial>(),
            std::mem::size_of::<ffi::DeviceMaterial>()
        );
        assert_eq!(
            std::mem::size_of::<Aabb>(),
            std::mem::size_of::<ffi::Aabb>()
        );
        assert_eq!(
            std::mem::size_of::<VertexMesh>(),
            std::mem::size_of::<ffi::VertexRange>()
        );
        assert_eq!(
            std::mem::size_of::<JointData>(),
            std::mem::size_of::<ffi::JointData>()
        );

        assert_eq!(
            std::mem::size_of::<CameraView3D>(),
            std::mem::size_of::<ffi::CameraView3D>()
        );
        assert_eq!(
            std::mem::size_of::<DeviceMaterial>(),
            std::mem::size_of::<ffi::DeviceMaterial>()
        );
        assert_eq!(
            std::mem::size_of::<Vertex2D>(),
            std::mem::size_of::<ffi::Vertex2D>()
        );
        assert_eq!(
            std::mem::size_of::<Vertex3D>(),
            std::mem::size_of::<ffi::Vertex3D>()
        );
        assert_eq!(
            std::mem::size_of::<Aabb>(),
            std::mem::size_of::<ffi::Aabb>()
        );
        assert_eq!(
            std::mem::size_of::<RTTriangle>(),
            std::mem::size_of::<ffi::RTTriangle>()
        );
    }
}
