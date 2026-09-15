# Aero — 2D Aerodynamics Simulator

A real-time 2D wind tunnel powered by the **Lattice Boltzmann Method (D2Q9)**
running entirely on the GPU via compute shaders. Drop in any obstacle silhouette —
a circle, a tree, a cat — and watch air flow around it, shed vortices, and form wakes.

> <!-- HERO IMAGE: tree PNG at Re=100, vorticity + tracers (~15k steps) -->
> <!-- ![Wind tunnel with tree obstacle](docs/images/hero-tree.png) -->

## What it does

- Solves LBM D2Q9 on the GPU (BGK collision + push-scheme streaming, link bounce-back)
- Renders velocity magnitude, pressure, and vorticity fields in real time
- Advects 2,000 particle tracers through the flow for visible streamlines
- Loads arbitrary PNG silhouettes as obstacles (alpha or luminance, auto-inverting)
- Interactive GUI: wind speed, Reynolds number, pause/reset, steps-per-frame, viz modes

## Validation

Circle at 10% blockage, U = 0.05:

| Re  | Measured St | Literature |
|-----|-------------|------------|
| 100 | 0.163       | ~0.164     |
| 200 | 0.192       | ~0.197     |

Both within ~2%. The bounce-back and solver are quantitatively correct.

> <!-- SHOT: circle at Re=100, velocity magnitude + tracers -->
> <!-- ![Circle, Re=100, velocity](docs/images/circle-re100-velocity.png) -->

> <!-- SHOT: circle at Re=200, vorticity + tracers, developed street -->
> <!-- ![Circle, Re=200, vorticity](docs/images/circle-re200-vorticity.png) -->

> <!-- SHOT: circle at Re=200, vorticity, tracers OFF (clean reference) -->
> <!-- ![Circle, Re=200, vorticity, no tracers](docs/images/circle-re200-vorticity-clean.png) -->

## Quick start

```sh
# Build (native macOS binary; Docker flow below is for Linux CI)
cargo run --release
```

Headless diagnostics:

```sh
./target/release/aero --tests              # compute + LBM physics validation
./target/release/aero --stability=500      # Re stability sweep
./target/release/aero --tracer-sample      # particle buffer readback
./target/release/aero --tracer-hist        # particle x-distribution histogram
./target/release/aero --mask-test=<png>    # PNG → mask conversion check
```

### Docker (Linux builds)

```sh
docker compose up -d
docker compose exec rust-dev cargo build --release
```

> The container produces Linux binaries. For a native macOS window with Metal
> access, build on the host with `cargo build --release`.

## Controls

| Control           | Range / action                              |
|-------------------|---------------------------------------------|
| Wind speed        | 0.01 – 0.10 (lattice units, keep < 0.1)     |
| Reynolds number   | 10 – 500 (log slider)                       |
| Steps per frame   | 1 – 10                                      |
| Pause / Reset     | freeze field / rebuild equilibrium          |
| Visualization     | velocity magnitude · pressure · vorticity   |
| Particle tracers  | toggle 2k advected tracer particles         |
| Load PNG…         | file picker for obstacle silhouette         |
| Circle            | restore default circular obstacle           |

> <!-- SHOT: GUI side panel over the flow field -->
> <!-- ![GUI controls](docs/images/gui-panel.png) -->

## Stability limits (512×256 grid, circle d = H/10)

- At U = 0.05: clean shedding through Re = 500. Note τ clamps at 0.51 above
  Re ≈ 400, so higher nominal Re values don't reduce viscosity further.
- At U ≥ 0.08 with high Re expect saturated colors or blow-up: Mach number
  nears 0.1 while the τ floor removes physical viscosity scaling. The GUI
  shows a warning in this regime — reduce U or refine the grid.
- Vorticity mode needs ~10–30k developed steps before the street appears;
  fresh runs correctly show flat green until shedding starts.

## How it works

- `shaders/lbm.wgsl` — D2Q9 collision + streaming, inlet/outlet/wall BCs,
  link bounce-back against a `mask` SSBO, writes ρ/ux/uy per cell
- `shaders/render.wgsl` — fullscreen-triangle viz (jet colormap, U²-relative
  pressure scale, central-difference vorticity, solid-cell sentinel)
- `shaders/tracers.wgsl` — bilinear velocity sampling, advection, inlet respawn
- `src/gpu/lbm.rs` — ping-pong buffers, params uniform, equilibrium init with
  symmetry-breaking seed perturbation
- `src/sim/obstacle.rs` — procedural circle + PNG → mask (alpha/luminance,
  auto-invert, sanity bounds)
- `src/render/` — flow pipeline + instanced-quad tracer renderer
- `src/gui/` — egui side panel state

## Performance

~60 FPS at 512×256 on Apple M2 (1 LBM step + render + tracers per frame).
~3.9k LBM steps/s headless. Tracers: 2,000 particles ≈ 14% pixel coverage.

## License

MIT
