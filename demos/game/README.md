# 🌹 Cosmic Roze: Galactic Defender (ADV-04 Game Engine Demo)

A full-featured 2D/3D arcade space combat game and real-time engine built entirely in **Roze**, demonstrating Roze's C Foreign Function Interface (FFI) capabilities interfacing directly with **Raylib 5.0** and **miniaudio**.

---

## 🎮 Features

1. **60 FPS Real-Time Game Loop**:
   - Double-buffered 60 FPS rendering pipeline with delta-time synchronization.
   - Low-latency input polling and FPS counter rendering.

2. **2D Space Combat Engine**:
   - **Player Spaceship**: Inertial flight model with acceleration, velocity damping, screen boundary wrapping, and turning physics.
   - **Asteroid Hazards**: Orbiting celestial obstacles with randomized trajectories, multi-hit durability, and velocity bounce.
   - **Plasma Cannon**: High-velocity projectile firing with lifetime expiration and continuous hit tracking.
   - **Collision Detection**: Circle-circle collision detection for laser impacts and hull breaches.

3. **Dynamic Particle Engine**:
   - High-performance, zero-allocation particle pool running at 60 FPS.
   - Dynamic thruster exhaust trails adapting to engine power.
   - Radiant multi-directional explosion bursts upon asteroid detonation.

4. **3D Holographic Mode (Raylib 3D)**:
   - Instant toggle with `[TAB]` between 2D tactical combat and 3D orbital perspective.
   - Real-time 3D perspective camera with orbiting camera controls.
   - 3D rendering pipeline: volumetric cubes, wireframe meshes, spheres, and infinite ground grid floor.

5. **Procedural miniaudio Sound Synthesis**:
   - Pure procedural sound generation embedded in the Raylib bridge without needing external `.wav` or `.mp3` assets:
     - **Laser Sound**: Downward frequency sweep (880 Hz $\to$ 220 Hz).
     - **Explosion Sound**: Low-frequency resonant noise burst with exponential decay.
     - **Powerup / Mode Chime**: Harmonic arpeggio (D5 $\to$ A5).
     - **Hull Impact Tone**: Low warning tone (220 Hz).

6. **Dual Execution Modes**:
   - **Interactive GUI Mode**: Windowed 800x600 real-time arcade game.
   - **Automated Headless Benchmark Mode**: Deterministic 180-frame simulation mode for headless CI pipelines and performance benchmarks.

---

## 🕹️ Controls

| Key / Input | Action |
|---|---|
| **W** / **Up Arrow** | Engage forward thrusters (spawns fiery plasma exhaust) |
| **S** / **Down Arrow** | Engage retro-thrusters (aerodynamic brake damping) |
| **A** / **Left Arrow** | Rotate spaceship counter-clockwise |
| **D** / **Right Arrow** | Rotate spaceship clockwise |
| **SPACE** / **Left Mouse** | Fire high-energy plasma laser bolts |
| **TAB** | Toggle 3D Holographic Visualizer Mode |

---

## 🏗️ Architecture

```
demos/game/
├── Makefile                 # Build automation for compilation, execution & benchmarking
├── README.md                # Documentation and architecture specification
├── raylib/                  # Raylib 5.0 static library & C headers
│   ├── include/             # raylib.h, raymath.h, rlgl.h
│   └── lib/libraylib.a      # Precompiled x86_64 Raylib static archive
└── src/
    ├── main.roze            # Roze game loop, physics, input, HUD & 3D orchestration
    └── raylib_bridge.c      # Clean C ABI bridge for windowing, drawing, audio & pools
```

- **`stdlib/src/raylib.roze`**: Roze standard library module providing `extern "C"` declarations and math/color primitives (`import "raylib";`).
- **`demos/game/src/raylib_bridge.c`**: Pure C ABI interface bridging Raylib's data types (`Color`, `Vector2`, `Camera3D`) to standard 64-bit integer and floating point registers, alongside procedural audio synthesis and 60 FPS entity buffers.
- **`demos/game/src/main.roze`**: The Roze game code orchestrating the game loop, flight physics, weapon systems, HUD rendering, and 3D visualizer mode.

---

## 🚀 Building & Running

### Requirements
- Roze Compiler (`roze`)
- GCC / Clang (for native linking)
- Linux x86_64 system libraries: `libm`, `libpthread`, `libdl`

### Quick Commands

```bash
# Build the native game executable
make build

# Launch the interactive game (GUI window, 60 FPS)
make run

# Run headless 180-frame verification benchmark (for headless servers / CI)
make benchmark

# Clean build artifacts
make clean
```

---

## 🧪 Benchmark Output

Running `make benchmark` executes 180 simulated frames in headless mode:

```
🎮 Running Cosmic Roze in Headless Engine Verification Mode...
✅ 180 Frames Simulated Successfully at 60 FPS target!
🎮 Final Simulation Stats:
   - Frames Elapsed:  180
   - Player Score:    0
   - Hull Health:     100%
   - Active Asteroids:6
   - Total Particles: 6
   - Active Bullets:  6
🚀 Game Engine 2D/3D Platform Verified!
```
