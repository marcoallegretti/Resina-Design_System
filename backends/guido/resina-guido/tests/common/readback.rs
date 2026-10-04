use guido::renderer::RenderTarget;
use std::path::Path;

pub fn read_frame(target: &RenderTarget) -> Vec<u8> {
    let RenderTarget::Offscreen(target) = target else {
        panic!("offscreen target required")
    };
    let width = target.texture.width();
    let height = target.texture.height();
    let row_bytes = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = target.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Resina label conformance readback"),
        size: u64::from(row_bytes) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = target.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    target.queue.submit([encoder.finish()]);
    let (sent, received) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            sent.send(result).unwrap();
        });
    target
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU readback must complete");
    received.recv().unwrap().expect("GPU readback must map");
    let mapped = buffer.slice(..).get_mapped_range().unwrap();
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in mapped.chunks_exact(row_bytes as usize) {
        pixels.extend_from_slice(&row[..(width * 4) as usize]);
    }
    pixels
}

pub fn capture(path: &Path, width: u32, height: u32, pixels: &[u8]) {
    let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
    for pixel in pixels.as_chunks::<4>().0 {
        ppm.extend_from_slice(&pixel[..3]);
    }
    std::fs::write(path, ppm).unwrap();
}
