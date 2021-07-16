use rfw_utils::BytesConversion;
use spirv_compiler::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use std::{fs, path::PathBuf};

const EXTENSIONS: [&'static str; 15] = [
    "vert", "vs", "frag", "fs", "comp", "geom", "tese", "tesc", "rgen", "chit", "ahit", "miss",
    "call", "mesh", "rint",
];
const KINDS: [ShaderKind; 15] = [
    ShaderKind::Vertex,
    ShaderKind::Vertex,
    ShaderKind::Fragment,
    ShaderKind::Fragment,
    ShaderKind::Compute,
    ShaderKind::Geometry,
    ShaderKind::TessEvaluation,
    ShaderKind::TessControl,
    ShaderKind::RayGeneration,
    ShaderKind::ClosestHit,
    ShaderKind::AnyHit,
    ShaderKind::Miss,
    ShaderKind::Callable,
    ShaderKind::Mesh,
    ShaderKind::Intersection,
];

fn main() {
    let mut definitions = vec![];

    if cfg!(target_os = "windows") {
        definitions.push(("WINDOWS", "1"));
    }

    if cfg!(target_os = "linux") {
        definitions.push(("LINUX", "1"));
    }

    if cfg!(feature = "validation_layers") {
        definitions.push(("ENABLE_VALIDATION_LAYERS", "1"));
    }

    let vulkan_sdk = std::env!("VULKAN_SDK");
    if vulkan_sdk.is_empty() {
        panic!("Could not find Vulkan SDK, is it installed? Please provide the location of a Vulkan SDK as VULKAN_SDK");
    }

    let vulkan_sdk = PathBuf::from(vulkan_sdk);
    let vulkan_libs = if cfg!(windows) {
        vec!["vulkan-1", "VkLayer_utils"]
    } else {
        vec!["vulkan"]
    };
    let vulkan_include_dir = vulkan_sdk.join("include");
    if cfg!(target_pointer_width = "64") {
        println!(
            "cargo:rustc-link-search={}",
            vulkan_sdk.join("Lib32").display()
        );
    } else {
        println!(
            "cargo:rustc-link-search={}",
            vulkan_sdk.join("Lib").display()
        );
    }

    let files_to_ignore = vec![];
    let mut build = cc::Build::new();
    build
        .files(vec![
            "cpp/src/renderer.cpp",
            "cpp/src/library.cpp",
            "cpp/src/device.cpp",
            "cpp/src/vulkan_loader.cpp",
        ])
        .includes(vec![
            "cpp/deps",
            vulkan_include_dir.to_string_lossy().as_ref(),
        ])
        .cpp(true)
        .flag_if_supported("-std=c++17")
        .flag_if_supported("/std:c++17")
        .flag_if_supported("/EHsc")
        .warnings_into_errors(true);

    for (def, val) in definitions.iter() {
        build.define(*def, *val);
    }

    build.compile("VulkanRT");

    for lib in vulkan_libs {
        println!("cargo:rustc-link-lib={}", lib);
    }

    for path in fs::read_dir("cpp/src")
        .unwrap()
        .filter_map(|p| match p {
            Ok(e) => Some(e),
            _ => None,
        })
        .filter(|p| {
            let path = p.path();
            !files_to_ignore.contains(&path)
        })
    {
        println!("cargo:rerun-if-changed={}", path.path().display());
    }

    let mut bg = bindgen::Builder::default()
        .header("cpp/src/library.h")
        .detect_include_paths(true)
        .default_enum_style(bindgen::EnumVariation::Rust {
            non_exhaustive: false,
        })
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .derive_copy(true)
        .derive_debug(true)
        .derive_default(true)
        .rustfmt_bindings(true)
        .layout_tests(false);

    for (def, val) in definitions {
        bg = bg.clang_arg(format!("-D{}={}", def, val));
    }

    match bg.generate() {
        Ok(bindings) => {
            let out_path = std::env::current_dir().unwrap().join("src/ffi.rs");
            let bindings_output = bindings.to_string();

            if let Ok(file) = std::fs::read(&out_path) {
                let signature = md5::compute(file);
                if signature != md5::compute(bindings_output.as_bytes()) {
                    std::fs::write(out_path, bindings_output)
                        .expect("Could not write out bg_ffi bindings");
                }
            } else {
                std::fs::write(out_path, bindings_output)
                    .expect("Could not write out bg_ffi bindings");
            }
        }
        Err(_) => panic!("Could not generate bindings for \"src/ffi.rs\""),
    };

    // Shader compilation
    let mut extensions: HashMap<&str, ShaderKind> = HashMap::new();
    EXTENSIONS.iter().enumerate().for_each(|(i, ext)| {
        extensions.insert(ext, KINDS[i]);
    });

    // Create compiler
    let mut compiler = CompilerBuilder::new().build().unwrap();

    // Read directory
    let dir = PathBuf::from("./shaders").as_path().read_dir().unwrap();

    // Filter entries
    let entries = dir
        .map(|d| d)
        .filter(|d| d.is_ok())
        .map(|e| e.unwrap())
        .filter(|e| {
            if !e.path().is_file() {
                return false;
            }

            if let Some(extension) = e.path().extension() {
                extensions.contains_key(extension.to_str().unwrap())
            } else {
                false
            }
        })
        .collect::<Vec<_>>();

    // Compile shaders
    for entry in entries.into_iter() {
        println!("cargo:rerun-if-changed={}", entry.path().display());
        let shader = match compiler.compile_from_file(
            entry.path(),
            *extensions
                .get(entry.path().extension().unwrap().to_str().unwrap())
                .unwrap(),
            true,
        ) {
            Ok(shader) => shader,
            Err(e) => {
                panic!("compile error: {}", e);
            }
        };

        let mut save_path = entry.path().to_str().unwrap().to_string();
        save_path.push_str(".spv");
        let mut file = File::create(&save_path).unwrap();
        file.write_all(shader.as_bytes()).unwrap();

        let mut out_path = save_path.clone();
        out_path.push_str(".h");

        Command::new("xxd")
            .arg("-i")
            .arg(format!("{}", save_path))
            .arg(format!("{}", out_path))
            .spawn()
            .unwrap()
            .wait_with_output()
            .unwrap();
    }
}
