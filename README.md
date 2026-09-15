# Aero — 2D Aerodynamics Simulator

GPU-accelerated Lattice Boltzmann (D2Q9) wind tunnel in Rust + wgpu.

## Stability limits (512×256 grid, circle d = H/10)

- At U = 0.05: clean shedding through Re = 500 (nominal; τ clamps at
  0.51 above Re ≈ 400, so higher "Re" values don't reduce viscosity further).
- Higher wind speeds (U ≥ 0.08) at high Re may show saturated colors or
  blow up: Mach number approaches 0.1 and the τ floor removes physical
  viscosity scaling. Reduce U or increase grid resolution.
- Vorticity mode needs ~10–30k steps of developed flow before the vortex
  street appears; fresh runs show flat green until shedding starts.

## Run

```sh
cargo run --release
./target/release/aero --tests        # headless physics validation
./target/release/aero --stability=500
./target/release/aero --tracer-sample
./target/release/aero --mask-test=<png>
```
