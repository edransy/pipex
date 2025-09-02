//! GPU computation support for pipex
//! 
//! This module provides GPU acceleration for pipeline operations using wgpu and WGSL.
//! 
//! # Features
//! - WGSL kernel execution
//! - Automatic GPU/CPU memory management
//! - Type-safe vector processing
//! - Cross-platform GPU support
//! 
//! # Requirements
//! - Requires the "gpu" feature to be enabled
//! - Input/output types must implement `bytemuck::Pod + bytemuck::Zeroable`

use std::borrow::Cow;
use wgpu::util::DeviceExt;

/// GPU computation errors
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    /// GPU device or adapter initialization failed
    #[error("GPU device initialization failed: {0}")]
    InitializationFailed(String),
    
    /// WGSL shader compilation or validation failed
    #[error("Shader compilation failed: {0}")]
    ShaderCompilationFailed(String),
    
    /// GPU buffer creation or allocation failed
    #[error("Buffer creation failed: {0}")]
    BufferCreationFailed(String),
    
    /// Compute kernel execution failed
    #[error("Compute execution failed: {0}")]
    ComputeExecutionFailed(String),
    
    /// Data transfer between GPU and CPU failed
    #[error("Data transfer failed: {0}")]
    DataTransferFailed(String),
}

/// GPU pipeline for executing compute shaders
pub struct GpuPipeline {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GpuPipeline {
    /// Initialize a new GPU pipeline
    pub async fn new() -> Result<Self, GpuError> {
        // Request adapter
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| GpuError::InitializationFailed("No suitable GPU adapter found".to_string()))?;
        
        // 🎯 PRINT GPU DEVICE INFORMATION
        let adapter_info = adapter.get_info();
        println!("🎮 GPU DEVICE DETECTED:");
        println!("  📱 Name: {}", adapter_info.name);
        println!("  🏭 Vendor: {:?}", adapter_info.vendor);
        println!("  🔧 Device Type: {:?}", adapter_info.device_type);
        println!("  🖥️  Backend: {:?}", adapter_info.backend);
        
        // Check if it's Apple Silicon
        if adapter_info.name.contains("Apple") || adapter_info.name.contains("M1") || 
           adapter_info.name.contains("M2") || adapter_info.name.contains("M3") {
            println!("  🚀 APPLE SILICON DETECTED! Using Metal backend!");
        }
        
        // Request device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Pipex GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::InitializationFailed(format!("Device request failed: {}", e)))?;
        
        println!("  ✅ GPU initialization successful!");
        
        Ok(Self { device, queue })
    }
    
    /// Execute a WGSL compute kernel on input data
    pub async fn execute_kernel<T>(&self, input: Vec<T>, kernel_source: &str) -> Result<Vec<T>, GpuError>
    where
        T: bytemuck::Pod + bytemuck::Zeroable + Clone,
    {
        let input_size = input.len();
        if input_size == 0 {
            return Ok(Vec::new());
        }
        
        // Create shader module
        let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Pipex Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(kernel_source)),
        });
        
        // Create input buffer
        let input_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Input Buffer"),
            contents: bytemuck::cast_slice(&input),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        
        // Create output buffer
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Buffer"),
            size: (input_size * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        
        // Create staging buffer for reading results
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: (input_size * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Create bind group layout
        let bind_group_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        
        // Create compute pipeline
        let compute_pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let compute_pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        
        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });
        
        // Create command encoder and dispatch compute
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder"),
        });
        
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&compute_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(input_size as u32, 1, 1);
        }
        
        // Copy output to staging buffer
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, (input_size * std::mem::size_of::<T>()) as u64);
        
        // Submit commands
        self.queue.submit(std::iter::once(encoder.finish()));
        
        // Map staging buffer and read results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures_channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
        
        // Wait for mapping to complete
        self.device.poll(wgpu::Maintain::wait()).panic_on_timeout();
        receiver.await.unwrap().map_err(|e| GpuError::DataTransferFailed(format!("Buffer mapping failed: {:?}", e)))?;
        
        // Read data
        let data = buffer_slice.get_mapped_range();
        let result: Vec<T> = bytemuck::cast_slice(&data).to_vec();
        
        // Clean up
        drop(data);
        staging_buffer.unmap();
        
        Ok(result)
    }
}

/// Global GPU pipeline instance
static GPU_PIPELINE: std::sync::OnceLock<std::sync::Arc<GpuPipeline>> = std::sync::OnceLock::new();

/// Initialize the global GPU pipeline
pub async fn init_gpu() -> Result<(), GpuError> {
    let pipeline = GpuPipeline::new().await?;
    GPU_PIPELINE.set(std::sync::Arc::new(pipeline))
        .map_err(|_| GpuError::InitializationFailed("GPU pipeline already initialized".to_string()))?;
    Ok(())
}

/// Execute a GPU kernel using the global pipeline
pub async fn execute_gpu_kernel<T>(input: Vec<T>, kernel_source: &str) -> Result<Vec<T>, GpuError>
where
    T: bytemuck::Pod + bytemuck::Zeroable + Clone,
{
    // Auto-initialize GPU pipeline if not already done
    if GPU_PIPELINE.get().is_none() {
        init_gpu().await?;
    }
    
    let pipeline = GPU_PIPELINE.get()
        .ok_or_else(|| GpuError::InitializationFailed("GPU pipeline initialization failed".to_string()))?;
    
    pipeline.execute_kernel(input, kernel_source).await
}

/// Helper function to handle method calls on complex expressions
/// Converts (expression).method() to method(expression)
fn handle_method_calls(expr: &str, method_name: &str) -> String {
    let method_pattern = format!(".{}()", method_name);
    let mut result = expr.to_string();
    
    // Find and replace method calls
    while let Some(method_pos) = result.find(&method_pattern) {
        // Find the matching opening parenthesis
        let mut paren_count = 0;
        let mut start_pos = method_pos;
        
        // Go backwards to find the start of the expression
        while start_pos > 0 {
            start_pos -= 1;
            let ch = result.chars().nth(start_pos).unwrap();
            
            if ch == ')' {
                paren_count += 1;
            } else if ch == '(' {
                if paren_count == 0 {
                    break;
                } else {
                    paren_count -= 1;
                }
            }
        }
        
        // Extract the expression inside parentheses
        if start_pos < method_pos && result.chars().nth(start_pos) == Some('(') {
            let expr_content = &result[start_pos + 1..method_pos];
            let end_pos = method_pos + method_pattern.len();
            
            // Replace (expr).method() with method(expr)
            let replacement = format!("{}({})", method_name, expr_content);
            result.replace_range(start_pos..end_pos, &replacement);
        } else {
            // Simple case: no parentheses, just replace
            break;
        }
    }
    
    result
}

/// Simple runtime Rust-to-WGSL expression transpiler
/// 
/// This is a basic transpiler that handles common mathematical expressions.
/// For more complex expressions, users should provide manual WGSL kernels.
pub fn transpile_rust_expression(expr_str: &str, var_name: &str) -> String {
    // Basic pattern-based transpilation for simple expressions
    let mut wgsl_expr = expr_str.to_string();
    
    // Replace the variable name with the WGSL input reference
    wgsl_expr = wgsl_expr.replace(var_name, "input[index]");
    
    // Handle method calls -> function calls (more comprehensive approach)
    // Process complex chained method calls like (x / 2.0).cos().abs()
    
    // First, handle simple cases
    wgsl_expr = wgsl_expr.replace("input[index].abs()", "abs(input[index])");
    wgsl_expr = wgsl_expr.replace("input[index].sqrt()", "sqrt(input[index])");
    wgsl_expr = wgsl_expr.replace("input[index].sin()", "sin(input[index])");
    wgsl_expr = wgsl_expr.replace("input[index].cos()", "cos(input[index])");
    
    // Handle chained method calls on complex expressions
    // Pattern: (expression).method() -> method(expression)
    wgsl_expr = handle_method_calls(&wgsl_expr, "abs");
    wgsl_expr = handle_method_calls(&wgsl_expr, "sqrt");
    wgsl_expr = handle_method_calls(&wgsl_expr, "sin");
    wgsl_expr = handle_method_calls(&wgsl_expr, "cos");
    wgsl_expr = handle_method_calls(&wgsl_expr, "tan");
    
    // Ensure floating point literals
    wgsl_expr = wgsl_expr.replace(" 1 ", " 1.0 ");
    wgsl_expr = wgsl_expr.replace(" 2 ", " 2.0 ");
    wgsl_expr = wgsl_expr.replace(" 3 ", " 3.0 ");
    wgsl_expr = wgsl_expr.replace(" 4 ", " 4.0 ");
    wgsl_expr = wgsl_expr.replace(" 5 ", " 5.0 ");
    
    // Handle start/end literals
    if wgsl_expr.starts_with("1 ") { wgsl_expr = wgsl_expr.replacen("1 ", "1.0 ", 1); }
    if wgsl_expr.starts_with("2 ") { wgsl_expr = wgsl_expr.replacen("2 ", "2.0 ", 1); }
    if wgsl_expr.ends_with(" 1") { wgsl_expr = wgsl_expr.replace(" 1", " 1.0"); }
    if wgsl_expr.ends_with(" 2") { wgsl_expr = wgsl_expr.replace(" 2", " 2.0"); }
    
    // Generate the complete WGSL shader
    format!(r#"
@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
    let index = global_id.x;
    if (index >= arrayLength(&input)) {{ return; }}
    output[index] = {};
}}
"#, wgsl_expr)
}
