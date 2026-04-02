# mm-dream

GPU-accelerated kaleidoscope screensaver built with WGSL and libcosmic.

## What it does

Real-time fragment shader generating kaleidoscopic patterns through rotozoom
transformations with configurable fold symmetry. Runs as a COSMIC desktop
application on Wayland.

- Analytical fold symmetry (2, 5, and 10-fold) with radial mirroring
- Zoom state machine with smoothstep-eased transitions
- CRT post-processing (barrel distortion, scanlines, phosphor grille, vignette)
- Auto-cycling through pattern combinations

## Building

Requires a Rust toolchain and Wayland compositor (COSMIC, sway, etc.).

```
cargo build --release
cargo run --release
```

## Dependencies

- [libcosmic](https://github.com/pop-os/libcosmic) — COSMIC widget toolkit (winit + wgpu)
- [bytemuck](https://crates.io/crates/bytemuck) — safe transmute for GPU uniform buffers

## License

MIT
