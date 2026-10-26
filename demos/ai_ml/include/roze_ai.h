// ==============================================================================
// 🌹 Roze AI / ML Inference Engine (ADV-05)
// ==============================================================================
// High-performance, zero-dependency C tensor library and neural model runtime.
// Supports:
// - N-Dimensional Tensors (F32) with matrix multiplication, broadcasting, activations
// - Activations: ReLU, GELU, SiLU (SwiGLU), Softmax, RMSNorm, LayerNorm, RoPE
// - Generative AI: LLaMA-style decoder Transformer with KV cache & sampling
// - Predictive Classification: Multi-Layer Perceptrons / Vision classifiers
// - Model serialization & GGUF / binary weight checkpoints
// ==============================================================================

#ifndef ROZE_AI_H
#define ROZE_AI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ------------------------------------------------------------------------------
// 1. Core Tensor Structure & Memory Management
// ------------------------------------------------------------------------------

typedef struct RzTensor {
    int ndims;
    int shape[4];
    int strides[4];
    int size;
    float* data;
    int is_view;
} RzTensor;

RzTensor* rz_tensor_create_1d(int d0);
RzTensor* rz_tensor_create_2d(int rows, int cols);
RzTensor* rz_tensor_create_3d(int d0, int d1, int d2);
void rz_tensor_free(RzTensor* t);

void rz_tensor_zero(RzTensor* t);
void rz_tensor_fill(RzTensor* t, double val);
void rz_tensor_set_1d(RzTensor* t, int idx, double val);
double rz_tensor_get_1d(const RzTensor* t, int idx);
void rz_tensor_set_2d(RzTensor* t, int r, int c, double val);
double rz_tensor_get_2d(const RzTensor* t, int r, int c);
int rz_tensor_rows(const RzTensor* t);
int rz_tensor_cols(const RzTensor* t);
int rz_tensor_size(const RzTensor* t);

// ------------------------------------------------------------------------------
// 2. Mathematical Kernels & Activations
// ------------------------------------------------------------------------------

void rz_tensor_matmul(const RzTensor* A, const RzTensor* B, RzTensor* C);
void rz_tensor_add(const RzTensor* A, const RzTensor* B, RzTensor* C);
void rz_tensor_mul(const RzTensor* A, const RzTensor* B, RzTensor* C);
void rz_tensor_scale(RzTensor* t, double scale);
void rz_tensor_add_bias(RzTensor* t, const RzTensor* bias);

void rz_tensor_relu(RzTensor* t);
void rz_tensor_gelu(RzTensor* t);
void rz_tensor_silu(RzTensor* t);
void rz_tensor_softmax(RzTensor* t);
void rz_tensor_rms_norm(const RzTensor* x, const RzTensor* weight, RzTensor* out, double eps);
void rz_tensor_layer_norm(const RzTensor* x, const RzTensor* weight, const RzTensor* bias, RzTensor* out, double eps);
void rz_tensor_rope(RzTensor* q_or_k, int pos, int head_dim);

int rz_tensor_argmax(const RzTensor* t);
int rz_tensor_sample(const RzTensor* logits, double temperature);
void rz_tensor_print(const RzTensor* t, const char* name);

// ------------------------------------------------------------------------------
// 3. Generative AI: LLaMA-Style Decoder Transformer
// ------------------------------------------------------------------------------

typedef struct RzLlamaConfig {
    int vocab_size;
    int dim;
    int hidden_dim;
    int n_layers;
    int n_heads;
    int max_seq_len;
} RzLlamaConfig;

typedef struct RzLlamaWeights {
    RzTensor* token_embedding_table; // (vocab_size, dim)
    RzTensor** rms_att_weight;       // (dim) per layer
    RzTensor** wq;                   // (dim, dim) per layer
    RzTensor** wk;                   // (dim, dim) per layer
    RzTensor** wv;                   // (dim, dim) per layer
    RzTensor** wo;                   // (dim, dim) per layer
    RzTensor** rms_ffn_weight;       // (dim) per layer
    RzTensor** w_gate;               // (dim, hidden_dim) per layer
    RzTensor** w_up;                 // (dim, hidden_dim) per layer
    RzTensor** w_down;               // (hidden_dim, dim) per layer
    RzTensor* rms_final_weight;      // (dim)
    RzTensor* w_cls;                 // (dim, vocab_size)
} RzLlamaWeights;

typedef struct RzLlamaModel {
    RzLlamaConfig config;
    RzLlamaWeights weights;
    // KV Cache
    RzTensor** key_cache;   // (max_seq_len, dim) per layer
    RzTensor** value_cache; // (max_seq_len, dim) per layer
    // Working buffers
    RzTensor* x;
    RzTensor* xb;
    RzTensor* q;
    RzTensor* k;
    RzTensor* v;
    RzTensor* att;
    RzTensor* hb;
    RzTensor* hb2;
    RzTensor* logits;
} RzLlamaModel;

RzLlamaModel* rz_llama_create(int vocab_size, int dim, int hidden_dim, int n_layers, int n_heads, int max_seq_len);
void rz_llama_init_weights(RzLlamaModel* model, unsigned int seed);
int rz_llama_save(const RzLlamaModel* model, const char* filepath);
RzLlamaModel* rz_llama_load(const char* filepath);
void rz_llama_free(RzLlamaModel* model);

int rz_llama_forward(RzLlamaModel* model, int token, int pos);
double rz_llama_get_logit(const RzLlamaModel* model, int token_id);
int rz_llama_sample_next(RzLlamaModel* model, double temperature);

// Tokenizer utilities
void rz_tokenizer_init(void);
void rz_tokenizer_add_token(const char* word, int id);
int rz_tokenizer_encode(const char* text, int* out_tokens, int max_tokens);
const char* rz_tokenizer_decode(int token_id);
void rz_tokenizer_print_token(int token_id);

// ------------------------------------------------------------------------------
// 4. Predictive Classification: Deep Neural Network (MLP / Vision / Tabular)
// ------------------------------------------------------------------------------

typedef struct RzClassifier {
    int in_dim;
    int hidden1;
    int hidden2;
    int out_dim;
    RzTensor* w1; // (in_dim, hidden1)
    RzTensor* b1; // (hidden1)
    RzTensor* w2; // (hidden1, hidden2)
    RzTensor* b2; // (hidden2)
    RzTensor* w3; // (hidden2, out_dim)
    RzTensor* b3; // (out_dim)
    // Buffers
    RzTensor* h1;
    RzTensor* h2;
    RzTensor* out;
} RzClassifier;

RzClassifier* rz_classifier_create(int in_dim, int hidden1, int hidden2, int out_dim);
void rz_classifier_init_weights(RzClassifier* model, unsigned int seed);
int rz_classifier_predict_tensor(RzClassifier* model, const RzTensor* input_tensor);
double rz_classifier_get_prob(const RzClassifier* model, int class_idx);
void rz_classifier_free(RzClassifier* model);

// ------------------------------------------------------------------------------
// 5. System Inspection & GFLOPS Benchmarks
// ------------------------------------------------------------------------------

void rz_ai_print_system_info(void);
void rz_print_stat_int(const char* label, int val, const char* suffix);
void rz_print_stat_float(const char* label, double val, const char* suffix);
void rz_print_str_no_newline(const char* s);
double rz_benchmark_matmul(int size, int iterations);

#ifdef __cplusplus
}
#endif

#endif // ROZE_AI_H
