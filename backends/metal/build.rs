use std::fs;
use std::process::Command;

fn main() {
    let mut files_to_ignore = Vec::new();
    for path in fs::read_dir("cpp/src")
        .unwrap()
        .filter_map(|p| match p {
            Ok(p) => Some(p),
            _ => None,
        })
        .filter(|p| match p.path().extension() {
            Some(e) => e.to_str() == Some("metal"),
            None => false,
        })
    {
        let path = path.path();
        let out = path.with_extension("air");

        match Command::new("xcrun")
            .arg("-sdk")
            .arg("macosx")
            .arg("metal")
            .arg("-c")
            .arg(format!("{}", path.display()))
            .arg("-o")
            .arg(format!("{}", out.display()))
            .spawn()
        {
            Ok(c) => {
                let output = c
                    .wait_with_output()
                    .expect(format!("Failed to compile: {}", path.display()).as_str());

                if !output.stdout.is_empty() {
                    println!("\tstdout: {}", unsafe {
                        String::from_utf8_unchecked(output.stdout)
                    });
                }
                if !output.stderr.is_empty() {
                    eprintln!("\tstderr: {}", unsafe {
                        String::from_utf8_unchecked(output.stderr)
                    });
                }

                if !output.status.success() {
                    panic!("Could not compile: {}", out.display());
                }

                let lib_out = out.with_extension("metallib");
                Command::new("xcrun")
                    .arg("-sdk")
                    .arg("macosx")
                    .arg("metallib")
                    .arg(format!("{}", out.display()))
                    .arg("-o")
                    .arg(format!("{}", lib_out.display()))
                    .spawn()
                    .unwrap()
                    .wait()
                    .expect("Could not convert to metallib");

                let out_path = out.with_extension("h");
                Command::new("xxd")
                    .arg("-i")
                    .arg(format!("{}", lib_out.display()))
                    .arg(format!("{}", out_path.display()))
                    .spawn()
                    .unwrap()
                    .wait_with_output()
                    .unwrap();

                files_to_ignore.push(out);
                files_to_ignore.push(lib_out);
                files_to_ignore.push(out_path);
            }
            Err(e) => {
                panic!("Could not compile {}: {}", out.display(), e);
            }
        }
    }

    cc::Build::new()
        .files(vec!["cpp/src/renderer.mm", "cpp/src/library.mm"])
        .cpp(true)
        .flag_if_supported("-std=c++17")
        .flag_if_supported("/std:c++17")
        .flag("-fobjc-arc")
        .warnings_into_errors(false)
        .compile("MetalCpp");

    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=QuartzCore");

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

    match bindgen::Builder::default()
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
        .layout_tests(false)
        .generate()
    {
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
