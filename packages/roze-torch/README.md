# 🌹 Roze-Torch

High-performance Deep Learning, Tensor Computation & Autograd Engine for Roze.

## Features
- **Multi-dimensional Tensors**: N-dimensional tensor allocations with shape arithmetic on CPU and CUDA.
- **Hardware Acceleration**: Matrix multiplication (GEMM) using BLAS and NVIDIA CUDA C-ABI dynamic FFI.
- **Neural Network Layers**: `LinearLayer`, `Sequential`, Xavier initialization, and activations (`ReLU`, `Sigmoid`, `Softmax`).
- **Autograd & Optimizers**: Reverse-mode automatic differentiation with `SGD` and `MSE` loss.

## Installation
```bash
bsr add roze-torch
```

## Quick Start
```roze
import "roze-torch";

func main() {
    let a = tensor_ones_2d(2, 2);
    let b = tensor_ones_2d(2, 2);
    let c = tensor_matmul(a, b);
    println("Result: " + tensor_get_2d(c, 0, 0));
}
```
