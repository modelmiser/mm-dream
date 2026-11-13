# mm-dream

A GPU kaleidoscope for the desktop: one WGSL fragment shader, drawn fullscreen, cycling through seventeen scenes.
Built on libcosmic (winit + wgpu). Runs on Wayland.

## What it shows

Each cycle runs about four and a quarter minutes and then starts again:

1. **Opening.** A 9-second zoom in on ten C64-coloured raster bars, then a 2-second still.
2. **The field** (27 s). One rotating, breathing lattice over a slow plasma, in the bars' own ten colours.
3. **Sixteen bar scenes** (9 s each). Four bar motions (wave, plasma, bounce, cascade) by four folds
   (none, 2, 5 and 10 sectors, each with a drifting radial mirror). Each step changes only one of the two.
4. **Closing.** A 2-second still and a 9-second zoom out: the opening, run backwards frame for frame.

Over everything, a CRT post-process: barrel distortion, scanlines, a phosphor grille and a vignette.

## Running it

```
cargo run --release
```

It starts fullscreen. The first build compiles libcosmic and takes a few minutes.

| Key | |
|---|---|
| `n` or space | on to the next scene |
| `c` | CRT post-process on / off |
| `f` | fullscreen / window |
| `q` or `Esc` | quit |

| Option | |
|---|---|
| `--scene N` | start the cycle at scene N (1 to 17; 1 is the field) |
| `--no-crt` | start with the CRT post-process off |
| `--windowed` | do not start fullscreen |

It is an application, not a session screensaver: it does not start on idle and does not lock the screen.

## What is exact, and what is not

- **Fold symmetry.** A folded scene is the same picture in every sector, mirrored, to rounding error, with the CRT
  barrel on or off. The barrel bends the coordinates before the fold sees them, so it measures radius the way
  each scene is built: in square pixels for folded scenes, per axis for unfolded ones.
- **The zoom.** While the picture turns, each bar pass is drawn turned both ways, so every frame has left-right
  and top-bottom mirrors as well as the half turn.
- **Time.** There is no floating-point clock. Time is a 64-bit tick count, and each motion reads its phase as
  ticks × rate modulo one turn in fixed point, so a phase is as precise after a year as in the first second and
  nothing ever wraps visibly. Scene lengths are whole ticks. `cargo test` covers the clock.
- **Not exact by design.** Scanlines and the grille are horizontal and vertical stripes, and the vignette
  follows the screen's shape, so with the CRT on a folded picture is symmetric in its structure but not pixel
  for pixel. The one-second blends into and out of the stills are still floating-point ramps.

## Changes in 0.2.0

- New scene: the field, first in the cycle, coloured from the bar palette.
- The fold is symmetric with the CRT on. In 0.1 the barrel distortion was elliptical on a non-square screen and
  put mirrored sectors up to 33 pixels apart at 4K.
- The zoom in and out are mirror-symmetric. In 0.1 they had only a half turn and a corner-to-corner mirror.
- An integer clock replaces the `f32` seconds that were wrapped every 10,000 s (a visible jump every 2.8 hours).
  The closing now retraces the opening exactly; in 0.1 the two were offset by about 0.3 degrees.
- Starts fullscreen, without a header bar; keys and command-line options as above.

## Building

Requires a Rust toolchain and a Wayland compositor (COSMIC, niri, sway, ...).

```
cargo build --release
cargo test --release
```

## Dependencies

- [libcosmic](https://github.com/pop-os/libcosmic) — COSMIC widget toolkit (winit + wgpu)
- [bytemuck](https://crates.io/crates/bytemuck) — safe transmute for GPU uniform buffers

## License

MIT
