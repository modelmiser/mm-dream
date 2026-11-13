# mm-dream

A GPU kaleidoscope for the desktop: one WGSL fragment shader, drawn fullscreen, cycling through seventeen scenes.
Built on libcosmic (winit + wgpu). Runs on Wayland.

## What it shows

Each cycle runs about four and a quarter minutes and then starts again:

1. **Opening.** A 9-second zoom in on ten C64-coloured raster bars, then a 2-second still.
2. **The field** (27 s). One rotating, breathing lattice over a slow plasma, in the bars' own ten colours.
3. **Sixteen bar scenes** (9 s each, after a crossfade of 2 to 4 s). Four bar motions (wave, plasma, bounce,
   cascade) by four folds (none, or 2, 5 or 10 mirrored sector pairs with a drifting radial mirror). From one
   bar scene to the next, only one of the two changes.
4. **Closing.** A 2-second still and a 9-second zoom out: the opening's frames in reverse order.

Over everything, a CRT post-process: barrel distortion, scanlines, a phosphor grille and a vignette.

## Running it

```
cargo run --release
```

It starts fullscreen. The first build compiles libcosmic and takes a few minutes.

| Key | |
|---|---|
| `n` or space | on to the next scene (once the current one has settled) |
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

"Exact" below means an identity of the arithmetic, to floating-point rounding, in the picture before the CRT
stripes and vignette are laid over it.

- **Fold symmetry.** A settled folded scene is the same picture in every sector, mirrored, with the CRT barrel
  on or off. The barrel bends the coordinates before the fold sees them, so it measures radius the way each
  picture is built: in square pixels for folded scenes and for the field, per axis for unfolded bars.
- **The zoom.** The opening and closing show the unfolded bars at rest. While they turn, each bar pass is drawn
  turned both ways, so every frame has left-right and top-bottom mirrors as well as the half turn.
- **The closing retraces the opening.** Held bars do not depend on the clock, so each frame of the zoom is a
  function of its frame number alone, and the closing shows the opening's frames in reverse order.
- **Time.** No floating-point time is accumulated. Time is a count of ticks (64 bits, plus a 32-bit fraction
  for slowed playback). Each motion's phase is time × rate modulo one turn, computed in integers and handed
  to the shader as a 24-bit fraction of a turn. So a phase is as precise after a year as in the first second,
  and no phase ever jumps. Scene lengths are whole ticks. `cargo test` covers the clock and the held bars.

Not exact, by design:

- **The CRT overlay.** Scanlines and the grille are horizontal and vertical stripes, and the vignette follows
  the screen's shape, so with the CRT on a picture keeps its symmetry in structure but not pixel for pixel.
- **Crossfades.** A fade between two scenes mixes two pictures and has the full symmetry of neither. All the
  fades (1 s to and from the stills, 4 s into the field, 2 to 4 s between scenes) are floating-point ramps.
- **Wall-clock time.** Durations are counted in 16 ms ticks; a late timer makes a scene longer, not different.

## Changes in 0.2.0

- New scene: the field, first in the cycle, coloured from the bar palette.
- The fold is symmetric with the CRT on. In 0.1 the barrel distortion was elliptical on a non-square screen and
  put mirrored sectors about 30 pixels apart at 4K.
- The zoom in and out are mirror-symmetric. In 0.1 they had only a half turn and a corner-to-corner mirror.
- An integer clock replaces the `f32` seconds that were wrapped every 10,000 s of animation time (a visible
  jump every 2.8 hours at full speed).
- The closing retraces the opening. In 0.1 the two were offset by up to half a degree, and the bars' width
  pulsed through both.
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
