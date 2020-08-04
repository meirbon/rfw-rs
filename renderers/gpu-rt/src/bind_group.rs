use std::collections::HashMap;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum Binding {
    ReadStorageBuffer(wgpu::Buffer, std::ops::Range<wgpu::BufferAddress>),
    WriteStorageBuffer(wgpu::Buffer, std::ops::Range<wgpu::BufferAddress>),
    DynamicStorageBuffer(wgpu::Buffer, std::ops::Range<wgpu::BufferAddress>),
    UniformBuffer(wgpu::Buffer, std::ops::Range<wgpu::BufferAddress>),
    DynamicUniformBuffer(wgpu::Buffer, std::ops::Range<wgpu::BufferAddress>),
    SampledTexture(
        wgpu::TextureView,
        wgpu::TextureFormat,
        wgpu::TextureComponentType,
        wgpu::TextureViewDimension,
    ),
    MultiSampledTexture(
        wgpu::TextureView,
        wgpu::TextureFormat,
        wgpu::TextureComponentType,
        wgpu::TextureViewDimension,
    ),
    ReadStorageTexture(
        wgpu::TextureView,
        wgpu::TextureFormat,
        wgpu::TextureComponentType,
        wgpu::TextureViewDimension,
    ),
    WriteStorageTexture(
        wgpu::TextureView,
        wgpu::TextureFormat,
        wgpu::TextureComponentType,
        wgpu::TextureViewDimension,
    ),
    Sampler(wgpu::Sampler),
    ComparisonSampler(wgpu::Sampler),
}

impl std::cmp::PartialEq for Binding {
    fn eq(&self, other: &Self) -> bool {
        let a = match &self {
            Binding::ReadStorageBuffer(_, _) => 0,
            Binding::WriteStorageBuffer(_, _) => 1,
            Binding::DynamicStorageBuffer(_, _) => 2,
            Binding::UniformBuffer(_, _) => 3,
            Binding::DynamicUniformBuffer(_, _) => 4,
            Binding::SampledTexture(_, _, _, _) => 5,
            Binding::MultiSampledTexture(_, _, _, _) => 6,
            Binding::ReadStorageTexture(_, _, _, _) => 7,
            Binding::WriteStorageTexture(_, _, _, _) => 8,
            Binding::Sampler(_) => 9,
            Binding::ComparisonSampler(_) => 10,
        };
        let b = match other {
            Binding::ReadStorageBuffer(_, _) => 0,
            Binding::WriteStorageBuffer(_, _) => 1,
            Binding::DynamicStorageBuffer(_, _) => 2,
            Binding::UniformBuffer(_, _) => 3,
            Binding::DynamicUniformBuffer(_, _) => 4,
            Binding::SampledTexture(_, _, _, _) => 5,
            Binding::MultiSampledTexture(_, _, _, _) => 6,
            Binding::ReadStorageTexture(_, _, _, _) => 7,
            Binding::WriteStorageTexture(_, _, _, _) => 8,
            Binding::Sampler(_) => 9,
            Binding::ComparisonSampler(_) => 10,
        };

        a == b
    }
}

impl Display for Binding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Binding({})",
            match self {
                Binding::ReadStorageBuffer(_, _) => "ReadStorageBuffer",
                Binding::WriteStorageBuffer(_, _) => "WriteStorageBuffer",
                Binding::DynamicStorageBuffer(_, _) => "DynamicStorageBuffer",
                Binding::UniformBuffer(_, _) => "UniformBuffer",
                Binding::DynamicUniformBuffer(_, _) => "DynamicUniformBuffer",
                Binding::SampledTexture(_, _, _, _) => "SampledTexture",
                Binding::MultiSampledTexture(_, _, _, _) => "MultiSampledTexture",
                Binding::ReadStorageTexture(_, _, _, _) => "ReadStorageTexture",
                Binding::WriteStorageTexture(_, _, _, _) => "WriteStorageTexture",
                Binding::Sampler(_) => "Sampler",
                Binding::ComparisonSampler(_) => "ComparisonSampler",
            }
        )
    }
}

impl Binding {
    pub fn binding_type(&self) -> wgpu::BindingType {
        match self {
            Binding::ReadStorageBuffer(_, _) => wgpu::BindingType::StorageBuffer {
                dynamic: false,
                readonly: true,
            },
            Binding::WriteStorageBuffer(_, _) => wgpu::BindingType::StorageBuffer {
                dynamic: false,
                readonly: false,
            },
            Binding::DynamicStorageBuffer(_, _) => wgpu::BindingType::StorageBuffer {
                dynamic: true,
                readonly: false,
            },
            Binding::UniformBuffer(_, _) => wgpu::BindingType::UniformBuffer { dynamic: false },
            Binding::DynamicUniformBuffer(_, _) => {
                wgpu::BindingType::UniformBuffer { dynamic: true }
            }
            Binding::SampledTexture(_, _, component_type, view_dim) => {
                wgpu::BindingType::SampledTexture {
                    component_type: *component_type,
                    dimension: *view_dim,
                    multisampled: false,
                }
            }
            Binding::MultiSampledTexture(_, _, component_type, view_dim) => {
                wgpu::BindingType::SampledTexture {
                    component_type: *component_type,
                    dimension: *view_dim,
                    multisampled: true,
                }
            }
            Binding::ReadStorageTexture(_, format, component_type, view_dim) => {
                wgpu::BindingType::StorageTexture {
                    dimension: *view_dim,
                    component_type: *component_type,
                    readonly: true,
                    format: *format,
                }
            }
            Binding::WriteStorageTexture(_, format, component_type, view_dim) => {
                wgpu::BindingType::StorageTexture {
                    dimension: *view_dim,
                    component_type: *component_type,
                    readonly: false,
                    format: *format,
                }
            }
            Binding::Sampler(_) => wgpu::BindingType::Sampler { comparison: false },
            Binding::ComparisonSampler(_) => wgpu::BindingType::Sampler { comparison: true },
        }
    }

    pub fn as_resource(&self) -> wgpu::BindingResource {
        match self {
            Binding::ReadStorageBuffer(buffer, range) => wgpu::BindingResource::Buffer {
                buffer,
                range: range.clone(),
            },
            Binding::WriteStorageBuffer(buffer, range) => wgpu::BindingResource::Buffer {
                buffer,
                range: range.clone(),
            },
            Binding::DynamicStorageBuffer(buffer, range) => wgpu::BindingResource::Buffer {
                buffer,
                range: range.clone(),
            },
            Binding::UniformBuffer(buffer, range) => wgpu::BindingResource::Buffer {
                buffer,
                range: range.clone(),
            },
            Binding::DynamicUniformBuffer(buffer, range) => wgpu::BindingResource::Buffer {
                buffer,
                range: range.clone(),
            },
            Binding::SampledTexture(view, _, _, _) => wgpu::BindingResource::TextureView(view),
            Binding::MultiSampledTexture(view, _, _, _) => wgpu::BindingResource::TextureView(view),
            Binding::ReadStorageTexture(view, _, _, _) => wgpu::BindingResource::TextureView(view),
            Binding::WriteStorageTexture(view, _, _, _) => wgpu::BindingResource::TextureView(view),
            Binding::Sampler(sampler) => wgpu::BindingResource::Sampler(sampler),
            Binding::ComparisonSampler(sampler) => wgpu::BindingResource::Sampler(sampler),
        }
    }
}

#[derive(Debug)]
pub struct BindGroupBinding {
    pub index: u32,
    pub visibility: wgpu::ShaderStage,
    pub binding: Binding,
}

impl Display for BindGroupBinding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let has_vertex = (self.visibility & wgpu::ShaderStage::VERTEX) == wgpu::ShaderStage::VERTEX;
        let has_fragment =
            (self.visibility & wgpu::ShaderStage::FRAGMENT) == wgpu::ShaderStage::FRAGMENT;
        let has_compute =
            (self.visibility & wgpu::ShaderStage::COMPUTE) == wgpu::ShaderStage::COMPUTE;

        let mut stage_string = String::new();
        if has_vertex {
            stage_string.push_str("VERTEX");
        }
        if has_fragment {
            if !stage_string.is_empty() {
                stage_string.push_str(", FRAGMENT");
            }
        }
        if has_compute {
            if !stage_string.is_empty() {
                stage_string.push_str(", COMPUTE");
            }
        }

        write!(
            f,
            "BindGroupBinding {{ index: {}, visibility: {}, binding: {} }}",
            self.index, stage_string, self.binding
        )
    }
}

impl BindGroupBinding {
    pub fn as_layout_entry(&self) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding: self.index,
            visibility: self.visibility,
            ty: self.binding.binding_type(),
        }
    }

    pub fn as_binding(&self) -> wgpu::Binding {
        wgpu::Binding {
            binding: self.index,
            resource: self.binding.as_resource(),
        }
    }
}

#[derive(Debug)]
pub struct BindGroupBuilder {
    label: Option<String>,
    bindings: HashMap<u32, BindGroupBinding>,
}

impl BindGroupBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_label<T: AsRef<str>>(mut self, name: T) -> Self {
        self.label = Some(name.as_ref().to_string());
        self
    }

    pub fn with_binding(mut self, binding: BindGroupBinding) -> Result<Self, String> {
        if let Some(b) = self.bindings.get(&binding.index) {
            Err(format!(
                "Binding ({}) already has a binding of type: {}",
                binding.index, b.binding
            ))
        } else {
            self.bindings.insert(binding.index, binding);
            Ok(self)
        }
    }

    pub fn build(self, device: &wgpu::Device) -> BindGroup {
        let bindings_entries: Vec<wgpu::BindGroupLayoutEntry> = self
            .bindings
            .iter()
            .map(|(_, binding)| binding.as_layout_entry())
            .collect();

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: bindings_entries.as_slice(),
            label: None,
        });

        let bindings: Vec<wgpu::Binding> = self
            .bindings
            .iter()
            .map(|(_, binding)| binding.as_binding())
            .collect();

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            bindings: bindings.as_slice(),
            label: if let Some(label) = self.label.as_ref() {
                Some(label.as_str())
            } else {
                None
            },
        });

        BindGroup {
            layout: bind_group_layout,
            bind_group,
            bindings: self.bindings,
            label: self.label,
            dirty: false,
        }
    }
}

impl Default for BindGroupBuilder {
    fn default() -> Self {
        Self {
            label: None,
            bindings: HashMap::new(),
        }
    }
}

pub struct BindGroup {
    pub layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    bindings: HashMap<u32, BindGroupBinding>,
    label: Option<String>,
    dirty: bool,
}

#[derive(Debug, Copy, Clone)]
pub enum BindGroupError {
    TypeMismatch,
    InvalidBindingIndex,
}

impl Display for BindGroupError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BindGroupError({})",
            match self {
                BindGroupError::TypeMismatch => "binding types do not match",
                BindGroupError::InvalidBindingIndex => "binding index does not exist in layout",
            }
        )
    }
}

impl BindGroup {
    pub fn as_bind_group(&mut self, device: &wgpu::Device) -> &wgpu::BindGroup {
        if self.dirty {
            self.update_bind_group(device);
        }
        &self.bind_group
    }

    pub fn bind(&mut self, index: u32, binding: Binding) -> Result<(), BindGroupError> {
        if let Some(b) = self.bindings.get_mut(&index) {
            if b.binding != binding {
                Err(BindGroupError::TypeMismatch)
            } else {
                b.binding = binding;
                self.dirty = true;
                Ok(())
            }
        } else {
            Err(BindGroupError::InvalidBindingIndex)
        }
    }

    pub fn get(&self, binding: u32) -> Option<&BindGroupBinding> {
        self.bindings.get(&binding)
    }

    pub fn get_mut(&mut self, binding: u32) -> Option<&mut BindGroupBinding> {
        self.bindings.get_mut(&binding)
    }

    fn update_bind_group(&mut self, device: &wgpu::Device) {
        let bindings: Vec<wgpu::Binding> = self
            .bindings
            .iter()
            .map(|(_, binding)| binding.as_binding())
            .collect();
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.layout,
            bindings: bindings.as_slice(),
            label: if let Some(label) = self.label.as_ref() {
                Some(label.as_str())
            } else {
                None
            },
        });
        self.dirty = false;
    }
}
