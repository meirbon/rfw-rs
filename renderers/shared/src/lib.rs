use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::{cmp::Ordering, ffi::OsString, fmt::Display};

pub mod utils;

static mut INCLUDE_DIRS: Vec<PathBuf> = Vec::new();

pub use shaderc::GlslProfile;
pub use shaderc::Limit;
pub use shaderc::OptimizationLevel;
pub use shaderc::ResourceKind;
pub use shaderc::ShaderKind;
pub use shaderc::SourceLanguage;
pub use shaderc::TargetEnv;
use std::error::Error;

#[derive(Debug, Clone)]
pub enum CompilerError {
    Log(CompilationError),
    LoadError(String),
    WriteError(String),
}

impl Display for CompilerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Error: {}",
            match self {
                CompilerError::Log(e) => format!("{}", e),
                CompilerError::LoadError(e) => format!("could not load file: {}", e),
                CompilerError::WriteError(e) => format!("could not write file: {}", e),
            }
        )
    }
}

impl Error for CompilerError {}

#[derive(Debug, Clone)]
pub struct CompilationError {
    pub file: Option<PathBuf>,
    pub description: String,
}

impl Into<CompilerError> for CompilationError {
    fn into(self) -> CompilerError {
        CompilerError::Log(self)
    }
}

impl Display for CompilationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = if let Some(file) = self.file.as_ref() {
            format!(
                "file: {}, description: {}",
                file.display(),
                self.description.as_str(),
            )
        } else {
            format!("description: {}", self.description.as_str())
        };

        write!(f, "{}", message)
    }
}

pub struct CompilerBuilder<'a> {
    options: shaderc::CompileOptions<'a>,
    has_macros: bool,
}

impl<'a> CompilerBuilder<'a> {
    pub fn new() -> CompilerBuilder<'a> {
        CompilerBuilder {
            options: shaderc::CompileOptions::new().unwrap(),
            has_macros: false,
        }
    }

    pub fn with_macro(mut self, name: &str, value: Option<&str>) -> Self {
        self.options.add_macro_definition(name, value);
        self.has_macros = true;
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
        debug_assert!(path.as_ref().exists());
        unsafe {
            INCLUDE_DIRS.push(path.as_ref().to_path_buf());
        }
        self
    }

    pub fn build(self) -> Option<Compiler<'a>> {
        if let Some(compiler) = shaderc::Compiler::new() {
            let mut compiler = Compiler {
                compiler,
                options: self.options,
                compile_cache: HashMap::new(),
                has_macros: self.has_macros,
            };

            compiler.options.set_include_callback(
                |requested_source, include_type, requesting_source, include_depth| {
                    Compiler::include_callback(
                        requested_source,
                        include_type,
                        requesting_source,
                        include_depth,
                    )
                },
            );

            Some(compiler)
        } else {
            None
        }
    }
}

pub struct Compiler<'a> {
    compiler: shaderc::Compiler,
    options: shaderc::CompileOptions<'a>,
    compile_cache: HashMap<PathBuf, Vec<u32>>,
    has_macros: bool,
}

impl<'a> Compiler<'a> {
    pub fn new() -> Option<Compiler<'a>> {
        if let Some(compiler) = shaderc::Compiler::new() {
            return Some(Compiler {
                compiler,
                options: shaderc::CompileOptions::new().unwrap(),
                compile_cache: HashMap::new(),
                has_macros: false,
            });
        }
        None
    }

    pub fn add_macro_definition(&mut self, name: &str, value: Option<&str>) {
        self.options.add_macro_definition(name, value);
        self.has_macros = true;
    }

    pub fn set_options(&mut self, options: shaderc::CompileOptions<'a>) {
        self.options = options;
        self.has_macros = false;
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

    pub fn include_callback(
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
    ) -> Result<Vec<u32>, CompilerError> {
        let binary_result =
            self.compiler
                .compile_into_spirv(source, kind, "memory", "main", Some(&self.options));

        match binary_result {
            Err(e) => Err(CompilationError {
                file: None,
                description: e.to_string(),
            }
            .into()),
            Ok(result) => Ok(result.as_binary().to_vec()),
        }
    }

    pub fn compile_from_file<T: AsRef<Path>>(
        &mut self,
        path: T,
        kind: shaderc::ShaderKind,
    ) -> Result<Vec<u32>, CompilerError> {
        if let Some(binary) = self.compile_cache.get(&path.as_ref().to_path_buf()) {
            return Ok(binary.clone());
        }

        let mut precompiled = OsString::from(path.as_ref().as_os_str());
        precompiled.push(".spv");
        let precompiled = PathBuf::from(precompiled);
        if precompiled.exists() && !self.has_macros {
            let should_recompile: bool = if let (Ok(meta_data), Ok(pre_meta_data)) =
                (path.as_ref().metadata(), precompiled.metadata())
            {
                let source_last_modified = meta_data.modified();
                let last_modified = pre_meta_data.modified();
                if let (Ok(source_last_modified), Ok(last_modified)) =
                    (source_last_modified, last_modified)
                {
                    source_last_modified.cmp(&last_modified) == Ordering::Less
                } else {
                    true
                }
            } else {
                true
            };

            // Only load pre-compiled files if they are up to date
            if should_recompile {
                if let Ok(mut file) = File::open(&precompiled) {
                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes).unwrap();
                    let bytes: Vec<u32> = Vec::from(unsafe {
                        std::slice::from_raw_parts(bytes.as_ptr() as *const u32, bytes.len() / 4)
                    });

                    self.compile_cache
                        .insert(path.as_ref().to_path_buf(), bytes.clone());
                    return Ok(bytes);
                }
            }
        }

        let file = File::open(&path);
        if let Err(e) = file {
            return Err(CompilerError::LoadError(e.to_string()));
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
            return Err(CompilationError {
                file: Some(path.as_ref().to_path_buf()),
                description: e.to_string(),
            }
            .into());
        }

        let binary_result = binary_result.unwrap();
        let bytes = binary_result.as_binary().to_vec();

        let file = File::create(&precompiled);
        if let Err(e) = file {
            return Err(CompilerError::WriteError(e.to_string()));
        }

        let mut file = file.unwrap();

        if let Err(e) = file.write_all(unsafe {
            std::slice::from_raw_parts(bytes.as_ptr() as *const u8, bytes.len() * 4)
        }) {
            return Err(CompilerError::WriteError(e.to_string()));
        }

        self.compile_cache
            .insert(path.as_ref().to_path_buf(), bytes.clone());
        Ok(bytes)
    }
}
