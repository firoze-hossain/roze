# 🌹 Roze-UI

Modern Reactive Cross-Platform Desktop GUI & Declarative Component Framework for Roze.

## Features
- **Declarative Hierarchy**: `row`, `column`, `container`, `button`, `label`, `text_input`, `checkbox`, `progress_bar`.
- **Flexbox-Style Layout Engine**: Computes geometry, auto-sizes bounds, insets, and handles alignment.
- **Reactive State & Signals**: Atomic state updates and reactive UI graph propagation.
- **Event Hit-Testing**: Hit-testing coordinate click dispatch and button callbacks.

## Installation
```bash
bsr add roze-ui
```

## Quick Start
```roze
import "roze-ui";

func main() {
    let btn = button_new("Submit");
    let lbl = label_new("Welcome to Roze-UI");
    let children = list_new();
    list_push(children, lbl);
    list_push(children, btn);

    let panel = column_new(children);
    ui_compute_layout(panel, 0, 0, 800, 600);
    println("Layout computed! Button bounds: " + btn.bounds.width + "x" + btn.bounds.height);
}
```
