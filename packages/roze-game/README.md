# 🌹 Roze-Game

High-performance 2D/3D Game Engine for Roze powered by Raylib.

## Features
- **Native Raylib Interop**: Windowing, OpenGL context, audio synthesis, and input polling via native C-FFI.
- **2D & 3D Math Primitives**: `Vector2`, `Vector3`, `Rectangle`, `Camera2D`, `Camera3D`.
- **Physics & Collision**: Axis-Aligned Bounding Box (AABB) and circle-circle collision solvers.
- **Scene & Entity System**: Clean entity-component update and render loops.

## Installation
```bash
bsr add roze-game
```

## Quick Start
```roze
import "roze-game";

func main() {
    let cfg = game_init(800, 600, "My Roze Game", 60);
    let player = game_object_new("Hero", 400.0, 300.0, 32.0, 32.0, color_blue());

    game_object_update(player, 0.016);
    println("Player at: " + player.pos.x + ", " + player.pos.y);
    game_close(cfg);
}
```
