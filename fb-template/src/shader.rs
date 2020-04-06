use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use shaderc;

static mut INCLUDE_DIRS: Vec<PathBuf> = Vec::new();

pub use shaderc::GlslProfile;
pub use shaderc::Limit;
pub use shaderc::OptimizationLevel;
pub use shaderc::ResourceKind;
pub use shaderc::ShaderKind;
pub use shaderc::SourceLanguage;
pub use shaderc::TargetEnv;

pub struct CompilerBuilder<'a> {
    options: shaderc::CompileOptions<'a>,
}

impl<'a> CompilerBuilder<'a> {
    pub fn new() -> CompilerBuilder<'a> {
        CompilerBuilder {
            options: shaderc::CompileOptions::new().unwrap(),
        }
    }

    pub fn with_macro(mut self, name: &str, value: Option<&str>) -> Self {
        self.options.add_macro_definition(name, value);
        self
    }

    pub fn with_auto_bind_uniforms(mut self, auto_bind: bool) -> Self {
        self.options.set_auto_bind_uniforms(auto_bind);
        self
    }

    pub fn with_binding_base(mut self, kind: shaderc::ResourceKind, base: u32) -> Self {
        self.options.set_binding_base(kind, base);
        self
    }

    pub fn generate_debug_info(mut self) -> Self {
        self.options.set_generate_debug_info();
        self
    }

    pub fn force_version_profile(mut self, version: u32, profile: shaderc::GlslProfile) -> Self {
        self.options.set_forced_version_profile(version, profile);
        self
    }

    pub fn with_target_env(mut self, env: shaderc::TargetEnv, version: u32) -> Self {
        self.options.set_target_env(env, version);
        self
    }

    pub fn with_hlsl_io_mapping(mut self, iomap: bool) -> Self {
        self.options.set_hlsl_io_mapping(iomap);
        self
    }

    pub fn with_hlsl_register_set_and_binding(
        mut self,
        register: &str,
        set: &str,
        binding: &str,
    ) -> Self {
        self.options
            .set_hlsl_register_set_and_binding(register, set, binding);
        self
    }

    pub fn with_hlsl_offsets(mut self, offsets: bool) -> Self {
        self.options.set_hlsl_offsets(offsets);
        self
    }

    pub fn with_source_language(mut self, lang: shaderc::SourceLanguage) -> Self {
        self.options.set_source_language(lang);
        self
    }

    pub fn with_binding_base_for_stage(
        mut self,
        kind: shaderc::ShaderKind,
        resource_kind: shaderc::ResourceKind,
        base: u32,
    ) -> Self {
        self.options
            .set_binding_base_for_stage(kind, resource_kind, base);
        self
    }

    pub fn with_opt_level(mut self, level: shaderc::OptimizationLevel) -> Self {
        self.options.set_optimization_level(level);
        self
    }

    pub fn supress_warnings(mut self) -> Self {
        self.options.set_suppress_warnings();
        self
    }

    pub fn with_warnings_as_errors(mut self) -> Self {
        self.options.set_warnings_as_errors();
        self
    }

    pub fn with_limit(mut self, limit: shaderc::Limit, value: i32) -> Self {
        self.options.set_limit(limit, value);
        self
    }

    pub fn with_include_dir<T: AsRef<Path>>(self, path: T) -> Self {
        assert!(path.as_ref().exists());
        unsafe {
            INCLUDE_DIRS.push(path.as_ref().to_path_buf());
        }
        self
    }

    pub fn build(self) -> Compiler<'a> {
        Compiler {
            compiler: shaderc::Compiler::new().unwrap(),
            options: self.options,
        }
    }
}

pub struct Compiler<'a> {
    compiler: shaderc::Compiler,
    options: shaderc::CompileOptions<'a>,
}

impl<'a> Compiler<'a> {
    pub fn new() -> Option<Compiler<'a>> {
        if let Some(compiler) = shaderc::Compiler::new() {
            return Some(Compiler {
                compiler,
                options: shaderc::CompileOptions::new().unwrap(),
            });
        }
        None
    }

    pub fn add_macro_definition(&mut self, name: &str, value: Option<&str>) {
        self.options.add_macro_definition(name, value);
    }

    pub fn set_options(&mut self, options: shaderc::CompileOptions<'a>) {
        self.options = options;
        self.options.set_include_callback(
            |requested_source, include_type, requesting_source, include_depth| {
                Self::include_callback(
                    requested_source,
                    include_type,
                    requesting_source,
                    include_depth,
                )
            },
        );
    }

    fn include_callback(
        requested_source: &str,
        include_type: shaderc::IncludeType,
        requesting_source: &str,
        include_depth: usize,
    ) -> Result<shaderc::ResolvedInclude, String> {
        use shaderc::{IncludeType, ResolvedInclude};
        if include_depth >= 32 {
            return Err(String::from(format!(
                "Include depth {} too high!",
                include_depth
            )));
        }

        let requested_path = PathBuf::from(String::from(requested_source));
        let requesting_path = PathBuf::from(String::from(requesting_source));

        if include_type == IncludeType::Standard {
            // #include <>
            unsafe {
                for path in &INCLUDE_DIRS {
                    let final_path = path.join(requested_path.as_path());
                    if final_path.exists() {
                        if let Ok(mut file) = File::open(final_path.clone()) {
                            let mut source = String::new();
                            file.read_to_string(&mut source).unwrap();
                            return Ok(ResolvedInclude {
                                resolved_name: String::from(final_path.to_str().unwrap()),
                                content: source,
                            });
                        }
                    }
                }
            }

            return Err(String::from(format!(
                "Could not find file: {}",
                requested_source
            )));
        } else if include_type == IncludeType::Relative {
            // #include ""
            let base_folder = requesting_path.as_path().parent().unwrap();
            let final_path = base_folder.join(requested_path.clone());
            if final_path.exists() {
                if let Ok(mut file) = File::open(final_path.clone()) {
                    let mut source = String::new();
                    file.read_to_string(&mut source).unwrap();
                    return Ok(ResolvedInclude {
                        resolved_name: String::from(final_path.to_str().unwrap()),
                        content: source,
                    });
                }
            }

            unsafe {
                for path in &INCLUDE_DIRS {
                    let final_path = path.join(requested_path.as_path());
                    if final_path.exists() {
                        if let Ok(mut file) = File::open(final_path.clone()) {
                            let mut source = String::new();
                            file.read_to_string(&mut source).unwrap();
                            return Ok(ResolvedInclude {
                                resolved_name: String::from(final_path.to_str().unwrap()),
                                content: source,
                            });
                        }
                    }
                }
            }

            return Err(String::from(format!(
                "Could not find file: {}",
                requested_source
            )));
        }

        Err(String::from(format!(
            "Unkown error resolving file: {}",
            requested_source
        )))
    }

    pub fn compile_from_string(
        &mut self,
        source: &str,
        kind: shaderc::ShaderKind,
    ) -> Result<Vec<u32>, Box<dyn Error>> {
        let binary_result =
            self.compiler
                .compile_into_spirv(source, kind, "memory", "main", Some(&self.options));

        if let Err(e) = binary_result {
            return Err(Box::new(e));
        }

        let binary_result = binary_result.unwrap();

        Ok(Vec::from(binary_result.as_binary()))
    }

    pub fn compile_from_file<T: AsRef<Path>>(
        &mut self,
        path: T,
        kind: shaderc::ShaderKind,
    ) -> Result<Vec<u32>, Box<dyn Error>> {
        let file = File::open(&path);
        if let Err(e) = file {
            eprintln!("Could not open file: {}", path.as_ref().to_str().unwrap());
            return Err(Box::new(e));
        }

        let mut file = file.unwrap();
        let mut source = String::new();
        file.read_to_string(&mut source).unwrap();

        let binary_result = self.compiler.compile_into_spirv(
            source.as_str(),
            kind,
            path.as_ref().to_str().unwrap(),
            "main",
            Some(&self.options),
        );

        if let Err(e) = binary_result {
            return Err(Box::new(e));
        }

        let binary_result = binary_result.unwrap();

        Ok(Vec::from(binary_result.as_binary()))
    }
}
