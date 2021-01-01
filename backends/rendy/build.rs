use rfw::prelude::BytesConversion;
use spirv_compiler::*;
use std::{collections::HashMap, error::Error, fs::File, io::Write, path::PathBuf};

fn add_to_watch_list(file: &str) {
    println!("cargo:rerun-if-changed={}", file);
}

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

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=shaders");
    let mut extensions: HashMap<&str, ShaderKind> = HashMap::new();
    EXTENSIONS.iter().enumerate().for_each(|(i, ext)| {
        extensions.insert(ext, KINDS[i]);
    });

    // Create compiler
    let mut compiler = CompilerBuilder::new().build().unwrap();

    // Read directory
    let dir = PathBuf::from("./shaders").as_path().read_dir()?;

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
        add_to_watch_list(entry.path().to_str().unwrap());
        let shader = match compiler.compile_from_file(
            entry.path(),
            *extensions
                .get(entry.path().extension().unwrap().to_str().unwrap())
                .unwrap(),
            true,
        ) {
            Ok(shader) => shader,
            Err(e) => {
                panic!(format!("compile error: {}", e));
            }
        };

        let mut save_path = entry.path().to_str().unwrap().to_string();
        save_path.push_str(".spv");
        let mut file = File::create(save_path)?;
        file.write_all(shader.as_bytes())?;
    }

    Ok(())
}
