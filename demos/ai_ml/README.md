# 🌹 Roze AI & Machine Learning Inference Engine (ADV-05)

A high-performance, zero-external-dependency tensor computation engine and deep learning inference runtime built in **Roze**, demonstrating Roze's native C Foreign Function Interface (FFI) capabilities for artificial intelligence, generative large language models, and predictive neural network workflows.

---

## 🧠 Features

1. **High-Performance Tensor Computational Core**:
   - N-dimensional tensor structures with 64-byte alignment for cache-line optimization and SIMD vectorization.
   - High-throughput matrix multiplication ($C = A \times B$).
   - Elementwise broadcasting, tensor addition, multiplication, and scalar scaling.
   - Deterministic sampling and argmax decision primitives.

2. **Modern Activation & Normalization Functions**:
   - **SwiGLU / SiLU (Swish)**: $f(x) = x \cdot \sigma(x)$ with gated linear activations.
   - **GELU (Gaussian Error Linear Unit)**: Standard BERT/GPT activation function.
   - **ReLU**: Rectified linear unit for deep MLP networks.
   - **Softmax**: Numerically stable normalized probability distribution calculation.
   - **RMSNorm (Root Mean Square Normalization)**: Fast pre-normalization standard in modern LLMs (LLaMA, Mistral).
   - **LayerNorm**: Mean-centered layer normalization.
   - **RoPE (Rotary Position Embeddings)**: Complex rotation of query/key vectors for state-of-the-art relative positional encoding.

3. **Generative AI: LLaMA-Style Decoder Transformer**:
   - Complete autoregressive Transformer decoder architecture matching modern open-weight LLMs (LLaMA 2/3):
     - Multi-Head Attention (MHA) with Query, Key, and Value linear projections.
     - Rotary Position Embeddings (RoPE) applied to query and key projections.
     - **KV-Cache Acceleration**: Fast $O(1)$ token generation step latency by preserving past key and value states.
     - SwiGLU Feed-Forward Network ($W_{\text{gate}} \cdot \text{SiLU} \times W_{\text{up}} \to W_{\text{down}}$).
     - RMSNorm pre-attention and pre-feedforward normalization.
   - **Binary Checkpoint Persistence**:
     - Fast serialization and deserialization (`models/mini_llama.bin`) with `RZLM` magic header and binary floating-point weights.
   - **Subword / Byte-Pair Tokenizer**:
     - Token lookup table with bidirectional encoding and decoding.

4. **Predictive Neural Network Classifier**:
   - Multi-Layer Perceptron (MLP) for feature classification and pattern recognition.
   - Multi-layer dense feedforward pipeline with ReLU hidden activations and Softmax categorical probability outputs.
   - Direct integration with high-level Roze classes (`rz_classifier_predict_tensor`).

5. **First-Class Roze Standard Library AI Module**:
   - Integrated as `import "ai";` in the Roze compiler (`stdlib/src/ai.roze`).
   - Clean, idiomatic Roze wrappers for `Tensor` and `LlamaModel`.
   - Direct C-ABI bridge with Cranelift native compilation.

6. **Tensor Engine GFLOPS Benchmark**:
   - Integrated single-core FP32 matrix multiplication benchmark measuring real compute throughput.

---

## 🏗️ Architecture

```
+-------------------------------------------------------------------------+
|                              Roze Application                           |
|                            (demos/ai_ml/main.roze)                      |
+-------------------------------------------------------------------------+
                                    │
                                    ▼
+-------------------------------------------------------------------------+
|                         Roze AI Standard Library                        |
|                            (import "ai";)                               |
+-------------------------------------------------------------------------+
                                    │
                         Native C-ABI Foreign Calls
                                    │
                                    ▼
+-------------------------------------------------------------------------+
|                  Roze AI Runtime (roze_ai.h / roze_ai.c)                |
+-------------------------------------------------------------------------+
|  Tensors & SIMD Alignment  │  Activations & Math  │   Tokenizer & Dict  |
|  - 64-byte aligned floats  │  - SiLU / SwiGLU     │   - Token encode    |
|  - Matmul & BLAS-style ops │  - GELU / Softmax    │   - Token decode    |
|  - Add / Mul / Scale       │  - RMSNorm / LayerNorm│  - Vocabulary map  |
+----------------------------+----------------------+---------------------+
|                      LLaMA Transformer Decoder                          |
|  - Multi-Head Self-Attention + RoPE                                     |
|  - Key-Value (KV) Caching for fast autoregressive decode                |
|  - SwiGLU Multi-Layer Perceptron (Gate, Up, Down projections)          |
|  - Checkpoint Serialization (`.bin` format with RZLM magic header)      |
+-------------------------------------------------------------------------+
|                      Predictive Neural Classifier                       |
|  - Multi-layer dense feature classification                             |
|  - Softmax probability distributions                                    |
+-------------------------------------------------------------------------+
```

---

## 📁 Project Structure

```
demos/ai_ml/
├── include/
│   └── roze_ai.h         # C AI/ML engine header (tensors, llama, classifier, tokenizer)
├── src/
│   ├── roze_ai.c         # C implementation of AI engine and mathematical kernels
│   └── main.roze         # Roze demonstration program covering all 4 AI sections
├── models/
│   └── mini_llama.bin    # Generated binary model weights checkpoint
├── Makefile              # Build automation (build, run, benchmark, clean)
└── README.md             # Technical documentation
```

---

## 🚀 Building & Running

### Prerequisites
- Roze Compiler (`cargo build --bin roze`)
- Standard C toolchain (`clang` or `gcc`) with standard math library (`-lm`)

### Quick Start
```bash
# Enter demo directory
cd demos/ai_ml

# Build native executable and run full AI/ML inference suite
make run
```

### Build Only
```bash
make build
# Binary created at: bin/roze_ai_demo
```

### Run Benchmark
```bash
make benchmark
```

### Clean
```bash
make clean
```

---

## 📊 Sample Output

```
==============================================================================
🌹 Roze AI / ML Inference Engine (ONNX & GGML Native Runtime)
==============================================================================
🧠 Roze AI / ML Tensor Inference Engine (GGML / C-FFI)
   - Backend: Native Cranelift C-ABI Bridge
   - Precision: FP32 Single Precision & KV Cached Tensors
   - Architectures: LLaMA Transformer Decoder (RoPE, SwiGLU, RMSNorm) & Deep MLP Classifier

------------------------------------------------------------------------------
📌 [1/4] Tensor Mathematical Engine & Activations
------------------------------------------------------------------------------
📊 Tensor 'A x B (Scaled Result)' [shape=(3, 3), size=9]:
  [ 2.0000, 4.0000, 6.0000, 8.0000, 10.0000, 12.0000, 14.0000, 16.0000, ... (1 more) ]
📊 Tensor 'Raw Vector' [shape=(5), size=5]:
  [ -2.5000, -0.5000, 0.0000, 1.5000, 3.0000 ]
📊 Tensor 'SiLU Activated' [shape=(5), size=5]:
  [ -0.1896, -0.1888, 0.0000, 1.2264, 2.8577 ]
📊 Tensor 'Softmax Probabilities' [shape=(5), size=5]:
  [ 0.0352, 0.0353, 0.0426, 0.1451, 0.7418 ]
   🏆 Argmax Probability Index: 4
✅ Tensor computational operations verified!

------------------------------------------------------------------------------
📌 [2/4] Generative AI: LLaMA-Style Decoder Transformer
------------------------------------------------------------------------------
🔧 Instantiating Transformer Decoder Architecture:
   - Vocab Size:       48 tokens
   - Embedding Dim:    64
   - SwiGLU Hidden:    172
   - Transformer Layers: 2
   - Attention Heads:  4 (Head Dim: 16)
   - Positional Norm:  RoPE (Rotary Position Embeddings) + RMSNorm
💾 Model checkpoint saved to: models/mini_llama.bin
📂 Model checkpoint loaded successfully from binary weights!

📝 Input Prompt: "Roze is a modern systems language that"
🤖 Generated Output: Roze is a modern systems language that kernels kernels that kernels that kernels that AI 
⚡ Autoregressive generation completed with KV-cache acceleration!

------------------------------------------------------------------------------
📌 [3/4] Predictive Classification: Deep Neural Network
------------------------------------------------------------------------------
🧠 Classifier Architecture: [4 Input Features] -> [Dense 16 (ReLU)] -> [Dense 8 (ReLU)] -> [Softmax 3 Classes]
🎯 Test Sample 1 Prediction:
   - Class 0 Probability: 0.3682
   - Class 1 Probability: 0.3422
   - Class 2 Probability: 0.2897
   - Predicted Class:     Class #0
✅ Neural classifier prediction completed!

------------------------------------------------------------------------------
📌 [4/4] Tensor Engine Performance & GFLOPS Benchmark
------------------------------------------------------------------------------
⚡ Running 25 iterations of 128x128 matrix multiplications...
🚀 Benchmark Result: 0.6633 GFLOPS (Single-Core FP32)

==============================================================================
🎉 Roze AI / ML Inference Wrapper Verified Successfully! (ADV-05)
==============================================================================
```
