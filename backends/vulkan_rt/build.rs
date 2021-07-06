use std::{fs, path::PathBuf};

fn main() {
    let mut definitions = vec![];

    if cfg!(windows) {
        definitions.push(("WINDOWS", "1"));
    }

    let vulkan_sdk = std::env!("VULKAN_SDK");
    if vulkan_sdk.is_empty() {
        panic!("Could not find Vulkan SDK, is it installed? Please provide the location of a Vulkan SDK as VULKAN_SDK");
    }

    let vulkan_sdk = PathBuf::from(vulkan_sdk);
    let vulkan_libs = vec!["vulkan-1", "VkLayer_utils"];
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
        .warnings_into_errors(false);

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
}
