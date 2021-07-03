use rfw::{prelude::*, GameTimer};
use rfw_font::*;

#[derive(Debug, Default)]
struct FpsSystem {
    timer: Timer,
    average: Averager<f32>,
}

static SPHERE_X: i32 = 20_i32;
static SPHERE_Z: i32 = 15_i32;

fn fps_system(mut font_renderer: ResMut<FontRenderer>, mut fps_component: Query<&mut FpsSystem>) {
    for mut c in fps_component.iter_mut() {
        let elapsed = c.timer.elapsed_in_millis();
        c.timer.reset();
        c.average.add_sample(elapsed);
        let average = c.average.get_average();

        font_renderer.draw(
            Section::default()
                .with_screen_position((0.0, 0.0))
                .add_text(
                    Text::new(
                        format!("FPS: {:.2}\nFRAMETIME: {:.2} MS", 1000.0 / average, average)
                            .as_str(),
                    )
                    .with_scale(32.0)
                    .with_color([1.0; 4]),
                ),
        );
    }
}

fn startup(mut commands: Commands<'_>, mut scene: ResMut<Scene>) {
    scene.add_spot_light(
        vec3(2.5, 15.0, 0.0),
        vec3(0.0, -1.0, 0.3),
        vec3(105.0, 10.0, 10.0),
        45.0,
        60.0,
    );

    scene.add_spot_light(
        vec3(0.0, 15.0, 0.0),
        vec3(0.0, -1.0, 0.3),
        vec3(10.0, 105.0, 10.0),
        45.0,
        60.0,
    );

    scene.add_spot_light(
        vec3(-2.5, 15.0, 0.0),
        vec3(0.0, -1.0, -0.3),
        vec3(10.0, 10.0, 105.0),
        45.0,
        60.0,
    );

    scene.add_directional_light(vec3(0.0, -1.0, 0.5), vec3(0.6, 0.4, 0.4));

    let material = scene
        .get_materials_mut()
        .add(vec3(1.0, 0.2, 0.03), 1.0, Vec3::ONE, 0.0);
    let sphere = Sphere::new(Vec3::ZERO, 0.2, material as u32).with_quality(Quality::Medium);
    let sphere = scene.add_3d_object(sphere);

    for x in -SPHERE_X..=SPHERE_X {
        for z in -SPHERE_Z..=SPHERE_Z {
            let mut instance = scene.add_3d(&sphere);
            instance
                .get_transform()
                .set_matrix(Mat4::from_translation(Vec3::new(x as f32, 0.3, z as f32)));
            // Spawn an entity for the sphere
            commands.spawn().insert(SphereHandle).insert(instance);
        }
    }

    let cesium_man = scene
        .load("assets/models/CesiumMan/CesiumMan.gltf")
        .unwrap()
        .scene()
        .unwrap();

    let mut cesium_man1 = scene.add_3d(&cesium_man);
    cesium_man1
        .get_transform()
        .set_scale(Vec3::splat(3.0))
        .rotate_y(180.0_f32.to_radians());
    commands
        .spawn()
        .insert(CesiumMan)
        .insert(0)
        .insert(cesium_man1);

    let mut cesium_man2 = scene.add_3d(&cesium_man);
    cesium_man2
        .get_transform()
        .translate(Vec3::new(-3.0, 0.0, 0.0))
        .rotate_y(180.0_f32.to_radians());
    commands
        .spawn()
        .insert(CesiumMan)
        .insert(1)
        .insert(cesium_man2);

    let pica_desc = scene
        .load("assets/models/pica/scene.gltf")
        .unwrap()
        .scene()
        .unwrap();
    scene.add_3d(&pica_desc);

    scene.set_animations_time(0.0);

    // Add FPS timer
    commands.spawn().insert(FpsSystem::default());
}

fn render_mode_system(keys: Res<Input<VirtualKeyCode>>, mut system: ResMut<RenderSystem>) {
    if keys.just_pressed(VirtualKeyCode::Key0) {
        system.mode = RenderMode::Default;
    }
    if keys.just_pressed(VirtualKeyCode::Key1) {
        system.mode = RenderMode::Albedo;
    }
    if keys.just_pressed(VirtualKeyCode::Key2) {
        system.mode = RenderMode::Normal;
    }
    if keys.just_pressed(VirtualKeyCode::Key3) {
        system.mode = RenderMode::GBuffer;
    }
    if keys.just_pressed(VirtualKeyCode::Key5) {
        system.mode = RenderMode::ScreenSpace;
    }
    if keys.just_pressed(VirtualKeyCode::Key6) {
        system.mode = RenderMode::Ssao;
    }
    if keys.just_pressed(VirtualKeyCode::Key7) {
        system.mode = RenderMode::FilteredSsao;
    }
}

struct CesiumMan;
struct SphereHandle;

fn camera_handler(
    delta: Res<GameTimer>,
    keys: Res<Input<VirtualKeyCode>>,
    mut camera: ResMut<Camera3D>,
) {
    let mut view_change = Vec3::new(0.0, 0.0, 0.0);
    let mut pos_change = Vec3::new(0.0, 0.0, 0.0);

    if keys.pressed(VirtualKeyCode::Up) {
        view_change += (0.0, 1.0, 0.0).into();
    }
    if keys.pressed(VirtualKeyCode::Down) {
        view_change -= (0.0, 1.0, 0.0).into();
    }
    if keys.pressed(VirtualKeyCode::Left) {
        view_change -= (1.0, 0.0, 0.0).into();
    }
    if keys.pressed(VirtualKeyCode::Right) {
        view_change += (1.0, 0.0, 0.0).into();
    }

    if keys.pressed(VirtualKeyCode::W) {
        pos_change += (0.0, 0.0, 1.0).into();
    }
    if keys.pressed(VirtualKeyCode::S) {
        pos_change -= (0.0, 0.0, 1.0).into();
    }
    if keys.pressed(VirtualKeyCode::A) {
        pos_change -= (1.0, 0.0, 0.0).into();
    }
    if keys.pressed(VirtualKeyCode::D) {
        pos_change += (1.0, 0.0, 0.0).into();
    }
    if keys.pressed(VirtualKeyCode::E) {
        pos_change += (0.0, 1.0, 0.0).into();
    }
    if keys.pressed(VirtualKeyCode::Q) {
        pos_change -= (0.0, 1.0, 0.0).into();
    }

    let m = if keys.pressed(VirtualKeyCode::LShift) {
        2.0
    } else {
        1.0
    };

    camera.translate_relative(pos_change * delta.elapsed_ms() / 200.0 * m);
    camera.translate_target(view_change * delta.elapsed_ms() / 1000.0 * m)
}

fn bounce_spheres(
    time: Res<GameTimer>,
    pool: Res<ComputeTaskPool>,
    mut spheres: Query<(&SphereHandle, &mut GraphHandle)>,
) {
    let t = time.elapsed_ms_since_start() / 1000.0;
    spheres.par_for_each_mut(&pool, 512, |(_, mut handle)| {
        let i = handle.get_id() as i32;
        let x = (i as i32 % (SPHERE_X * 2)) - SPHERE_X;
        let z = (i as i32 / (SPHERE_X * 2)) - SPHERE_Z;
        let _x = (((x + SPHERE_X) as f32) + t).sin();
        let _z = (((z + SPHERE_Z) as f32) + t).sin();
        let height = (_z + _x) * 0.5 + 1.0;

        handle
            .get_transform()
            .set_matrix(Mat4::from_translation(Vec3::new(
                x as f32,
                0.3 + height,
                z as f32,
            )));
    });
}

fn set_animation_timers(time: Res<GameTimer>, mut scene: ResMut<Scene>) {
    scene.set_animations_time(time.elapsed_ms_since_start() / 1000.0);
}

fn rotate_spot_lights(time: Res<GameTimer>, mut scene: ResMut<Scene>) {
    let elapsed = time.elapsed_ms_since_start() / 1000.0;
    scene
        .get_lights_mut()
        .spot_lights
        .iter_mut()
        .for_each(|(_, sl)| {
            sl.direction =
                Quat::from_rotation_y((elapsed / 10.0).to_radians()).mul_vec3(sl.direction);
        });
}

fn main() {
    use clap::*;
    let mut app = App::new("rfw animated example")
        .author("Mèir Noordermeer")
        .about("Renders an animated scene using rfw.");

    app.arg(
        Arg::with_name("width")
            .short("w")
            .takes_value(true)
            .multiple(false)
            .default_value("1280"),
    )
    .arg(
        Arg::with_name("height")
            .short("h")
            .takes_value(true)
            .multiple(false)
            .default_value("720"),
    )
    .arg(Arg::with_name("hipdi"));

    #[cfg(target_vendor = "apple")]
    {
        app.arg(
            Arg::with_name("renderer")
                .short("r")
                .takes_value(true)
                .multiple(false)
                .default_value("wgpu")
                .possible_values(["wgpu", "metal"]),
        );
    }

    let matches = app.get_matches();
    let width: u32 = matches.value_of("width").unwrap_or("1280").parse();
    let height: u32 = matches.value_of("height").unwrap_or("720").parse();
    let scale_mode = if matches.is_present("hidpi") {
        rfw::ScaleMode::HiDpi
    } else {
        rfw::ScaleMode::Regular
    };

    let mut instance: rfw::Instance;
    #[cfg(target_vendor = "apple")]
    {
        instance = if matches.value_of("renderer").unwrap_or("wgpu") == "wgpu" {
            rfw::Instance::new::<rfw_backend_wgpu::WgpuBackend>(width, height);
        } else {
            rfw::Instance::new::<rfw_backend_metal::MetalBackend>(width, height);
        };
    }

    #[cfg(target_vendor != "apple")]
    {
        instance = rfw::Instance::new::<rfw_backend_wgpu::WgpuBackend>(width, height);
    }

    instance
        .with_plugin(FontRenderer::from_bytes(include_bytes!(
            "../../../assets/good-times-rg.ttf"
        )))
        .with_startup_system(startup.system())
        .with_system(fps_system.system())
        .with_system(render_mode_system.system())
        .with_system(camera_handler.system())
        .with_system(bounce_spheres.system())
        .with_system(set_animation_timers.system())
        .with_system(rotate_spot_lights.system())
        .run(rfw::Settings { scale_mode })
}
