// ==============================================================================
// 🌹 Roze AI / ML Inference Engine Implementation (ADV-05)
// ==============================================================================

#include "../include/roze_ai.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <time.h>
#include <sys/stat.h>
#include <sys/types.h>

// ------------------------------------------------------------------------------
// 1. Core Tensor Structure & Memory Management
// ------------------------------------------------------------------------------

static float* rz_alloc_f32(size_t count) {
    void* ptr = NULL;
    if (posix_memalign(&ptr, 64, count * sizeof(float)) != 0) {
        ptr = malloc(count * sizeof(float));
    }
    if (ptr) {
        memset(ptr, 0, count * sizeof(float));
    }
    return (float*)ptr;
}

RzTensor* rz_tensor_create_1d(int d0) {
    RzTensor* t = (RzTensor*)calloc(1, sizeof(RzTensor));
    t->ndims = 1;
    t->shape[0] = d0;
    t->strides[0] = 1;
    t->size = d0;
    t->data = rz_alloc_f32(d0);
    return t;
}

RzTensor* rz_tensor_create_2d(int rows, int cols) {
    RzTensor* t = (RzTensor*)calloc(1, sizeof(RzTensor));
    t->ndims = 2;
    t->shape[0] = rows;
    t->shape[1] = cols;
    t->strides[0] = cols;
    t->strides[1] = 1;
    t->size = rows * cols;
    t->data = rz_alloc_f32(t->size);
    return t;
}

RzTensor* rz_tensor_create_3d(int d0, int d1, int d2) {
    RzTensor* t = (RzTensor*)calloc(1, sizeof(RzTensor));
    t->ndims = 3;
    t->shape[0] = d0;
    t->shape[1] = d1;
    t->shape[2] = d2;
    t->strides[0] = d1 * d2;
    t->strides[1] = d2;
    t->strides[2] = 1;
    t->size = d0 * d1 * d2;
    t->data = rz_alloc_f32(t->size);
    return t;
}

void rz_tensor_free(RzTensor* t) {
    if (!t) return;
    if (!t->is_view && t->data) {
        free(t->data);
    }
    free(t);
}

void rz_tensor_zero(RzTensor* t) {
    if (!t || !t->data) return;
    memset(t->data, 0, t->size * sizeof(float));
}

void rz_tensor_fill(RzTensor* t, double val) {
    if (!t || !t->data) return;
    float fval = (float)val;
    for (int i = 0; i < t->size; i++) {
        t->data[i] = fval;
    }
}

void rz_tensor_set_1d(RzTensor* t, int idx, double val) {
    if (!t || idx < 0 || idx >= t->size) return;
    t->data[idx] = (float)val;
}

double rz_tensor_get_1d(const RzTensor* t, int idx) {
    if (!t || idx < 0 || idx >= t->size) return 0.0;
    return (double)t->data[idx];
}

void rz_tensor_set_2d(RzTensor* t, int r, int c, double val) {
    if (!t || t->ndims < 2 || r < 0 || r >= t->shape[0] || c < 0 || c >= t->shape[1]) return;
    t->data[r * t->strides[0] + c * t->strides[1]] = (float)val;
}

double rz_tensor_get_2d(const RzTensor* t, int r, int c) {
    if (!t || t->ndims < 2 || r < 0 || r >= t->shape[0] || c < 0 || c >= t->shape[1]) return 0.0;
    return (double)t->data[r * t->strides[0] + c * t->strides[1]];
}

int rz_tensor_rows(const RzTensor* t) {
    return t ? t->shape[0] : 0;
}

int rz_tensor_cols(const RzTensor* t) {
    return (t && t->ndims > 1) ? t->shape[1] : (t ? t->shape[0] : 0);
}

int rz_tensor_size(const RzTensor* t) {
    return t ? t->size : 0;
}

// ------------------------------------------------------------------------------
// 2. Mathematical Kernels & Activations
// ------------------------------------------------------------------------------

// High-performance cache-ordered matrix multiplication: C = A x B
void rz_tensor_matmul(const RzTensor* A, const RzTensor* B, RzTensor* C) {
    int M = A->shape[0];
    int K = (A->ndims == 1) ? A->shape[0] : A->shape[1];
    int N = (B->ndims == 1) ? 1 : B->shape[1];

    if (A->ndims == 1 && B->ndims == 2) {
        // Vector-Matrix multiplication: 1xK times KxN -> 1xN
        // C_j = sum_k A_k * B_{kj}
        float* c_data = C->data;
        const float* a_data = A->data;
        const float* b_data = B->data;
        memset(c_data, 0, N * sizeof(float));

        for (int k = 0; k < K; k++) {
            float a_k = a_data[k];
            const float* b_row = b_data + k * N;
            for (int j = 0; j < N; j++) {
                c_data[j] += a_k * b_row[j];
            }
        }
        return;
    }

    if (A->ndims == 2 && B->ndims == 1) {
        // Matrix-Vector multiplication: MxK times K -> M
        // C_i = sum_k A_{ik} * B_k
        float* c_data = C->data;
        const float* a_data = A->data;
        const float* b_data = B->data;

        for (int i = 0; i < M; i++) {
            float sum = 0.0f;
            const float* a_row = a_data + i * K;
            for (int k = 0; k < K; k++) {
                sum += a_row[k] * b_data[k];
            }
            c_data[i] = sum;
        }
        return;
    }

    // Standard 2D Matrix-Matrix: (M x K) x (K x N) -> (M x N)
    memset(C->data, 0, M * N * sizeof(float));
    for (int i = 0; i < M; i++) {
        const float* a_row = A->data + i * K;
        float* c_row = C->data + i * N;
        for (int k = 0; k < K; k++) {
            float a_val = a_row[k];
            const float* b_row = B->data + k * N;
            for (int j = 0; j < N; j++) {
                c_row[j] += a_val * b_row[j];
            }
        }
    }
}

void rz_tensor_add(const RzTensor* A, const RzTensor* B, RzTensor* C) {
    int n = (A->size < B->size) ? A->size : B->size;
    if (C->size < n) n = C->size;
    for (int i = 0; i < n; i++) {
        C->data[i] = A->data[i] + B->data[i];
    }
}

void rz_tensor_mul(const RzTensor* A, const RzTensor* B, RzTensor* C) {
    int n = (A->size < B->size) ? A->size : B->size;
    if (C->size < n) n = C->size;
    for (int i = 0; i < n; i++) {
        C->data[i] = A->data[i] * B->data[i];
    }
}

void rz_tensor_scale(RzTensor* t, double scale) {
    if (!t) return;
    float s = (float)scale;
    for (int i = 0; i < t->size; i++) {
        t->data[i] *= s;
    }
}

void rz_tensor_add_bias(RzTensor* t, const RzTensor* bias) {
    if (!t || !bias) return;
    int bias_len = bias->size;
    for (int i = 0; i < t->size; i++) {
        t->data[i] += bias->data[i % bias_len];
    }
}

void rz_tensor_relu(RzTensor* t) {
    if (!t) return;
    for (int i = 0; i < t->size; i++) {
        if (t->data[i] < 0.0f) {
            t->data[i] = 0.0f;
        }
    }
}

void rz_tensor_gelu(RzTensor* t) {
    if (!t) return;
    const float kSqrt2OverPi = 0.7978845608f;
    for (int i = 0; i < t->size; i++) {
        float x = t->data[i];
        float cube = x * x * x;
        float inner = kSqrt2OverPi * (x + 0.044715f * cube);
        t->data[i] = 0.5f * x * (1.0f + tanhf(inner));
    }
}

void rz_tensor_silu(RzTensor* t) {
    if (!t) return;
    for (int i = 0; i < t->size; i++) {
        float x = t->data[i];
        t->data[i] = x / (1.0f + expf(-x));
    }
}

void rz_tensor_softmax(RzTensor* t) {
    if (!t || t->size == 0) return;
    int cols = (t->ndims > 1) ? t->shape[t->ndims - 1] : t->size;
    int rows = t->size / cols;

    for (int r = 0; r < rows; r++) {
        float* row = t->data + r * cols;
        float max_val = row[0];
        for (int c = 1; c < cols; c++) {
            if (row[c] > max_val) max_val = row[c];
        }
        float sum = 0.0f;
        for (int c = 0; c < cols; c++) {
            row[c] = expf(row[c] - max_val);
            sum += row[c];
        }
        float inv_sum = 1.0f / (sum > 0.0f ? sum : 1e-6f);
        for (int c = 0; c < cols; c++) {
            row[c] *= inv_sum;
        }
    }
}

void rz_tensor_rms_norm(const RzTensor* x, const RzTensor* weight, RzTensor* out, double eps) {
    int d = x->size;
    float sum_sq = 0.0f;
    for (int i = 0; i < d; i++) {
        sum_sq += x->data[i] * x->data[i];
    }
    float scale = 1.0f / sqrtf((sum_sq / (float)d) + (float)eps);
    for (int i = 0; i < d; i++) {
        out->data[i] = x->data[i] * scale * weight->data[i];
    }
}

void rz_tensor_layer_norm(const RzTensor* x, const RzTensor* weight, const RzTensor* bias, RzTensor* out, double eps) {
    int d = x->size;
    float mean = 0.0f;
    for (int i = 0; i < d; i++) mean += x->data[i];
    mean /= (float)d;

    float var = 0.0f;
    for (int i = 0; i < d; i++) {
        float diff = x->data[i] - mean;
        var += diff * diff;
    }
    var /= (float)d;
    float inv_std = 1.0f / sqrtf(var + (float)eps);

    for (int i = 0; i < d; i++) {
        float norm = (x->data[i] - mean) * inv_std;
        out->data[i] = norm * (weight ? weight->data[i] : 1.0f) + (bias ? bias->data[i] : 0.0f);
    }
}

void rz_tensor_rope(RzTensor* q_or_k, int pos, int head_dim) {
    if (!q_or_k || head_dim <= 0) return;
    int n_heads = q_or_k->size / head_dim;

    for (int h = 0; h < n_heads; h++) {
        float* head_vec = q_or_k->data + h * head_dim;
        for (int i = 0; i < head_dim; i += 2) {
            float freq = 1.0f / powf(10000.0f, (float)i / (float)head_dim);
            float angle = (float)pos * freq;
            float cos_th = cosf(angle);
            float sin_th = sinf(angle);

            float v0 = head_vec[i];
            float v1 = head_vec[i + 1];
            head_vec[i]     = v0 * cos_th - v1 * sin_th;
            head_vec[i + 1] = v0 * sin_th + v1 * cos_th;
        }
    }
}

int rz_tensor_argmax(const RzTensor* t) {
    if (!t || t->size == 0) return 0;
    int max_idx = 0;
    float max_val = t->data[0];
    for (int i = 1; i < t->size; i++) {
        if (t->data[i] > max_val) {
            max_val = t->data[i];
            max_idx = i;
        }
    }
    return max_idx;
}

int rz_tensor_sample(const RzTensor* logits, double temperature) {
    if (temperature <= 0.01) {
        return rz_tensor_argmax(logits);
    }
    RzTensor* probs = rz_tensor_create_1d(logits->size);
    for (int i = 0; i < logits->size; i++) {
        probs->data[i] = logits->data[i] / (float)temperature;
    }
    rz_tensor_softmax(probs);

    float r = (float)rand() / (float)RAND_MAX;
    float cdf = 0.0f;
    int selected = probs->size - 1;
    for (int i = 0; i < probs->size; i++) {
        cdf += probs->data[i];
        if (r <= cdf) {
            selected = i;
            break;
        }
    }
    rz_tensor_free(probs);
    return selected;
}

void rz_tensor_print(const RzTensor* t, const char* name) {
    if (!t) return;
    printf("📊 Tensor '%s' [shape=(", name ? name : "tensor");
    for (int i = 0; i < t->ndims; i++) {
        printf("%d%s", t->shape[i], (i + 1 < t->ndims) ? ", " : "");
    }
    printf("), size=%d]:\n  [ ", t->size);
    int preview = (t->size < 8) ? t->size : 8;
    for (int i = 0; i < preview; i++) {
        printf("%.4f%s", t->data[i], (i + 1 < t->size) ? ", " : "");
    }
    if (preview < t->size) printf("... (%d more)", t->size - preview);
    printf(" ]\n");
}

// ------------------------------------------------------------------------------
// 3. Generative AI: LLaMA-Style Decoder Transformer
// ------------------------------------------------------------------------------

RzLlamaModel* rz_llama_create(int vocab_size, int dim, int hidden_dim, int n_layers, int n_heads, int max_seq_len) {
    RzLlamaModel* m = (RzLlamaModel*)calloc(1, sizeof(RzLlamaModel));
    m->config.vocab_size = vocab_size;
    m->config.dim = dim;
    m->config.hidden_dim = hidden_dim;
    m->config.n_layers = n_layers;
    m->config.n_heads = n_heads;
    m->config.max_seq_len = max_seq_len;

    // Weights allocation with proper matrix shapes for vector matmul
    m->weights.token_embedding_table = rz_tensor_create_2d(vocab_size, dim);
    m->weights.rms_att_weight = (RzTensor**)calloc(n_layers, sizeof(RzTensor*));
    m->weights.wq = (RzTensor**)calloc(n_layers, sizeof(RzTensor*));
    m->weights.wk = (RzTensor**)calloc(n_layers, sizeof(RzTensor*));
    m->weights.wv = (RzTensor**)calloc(n_layers, sizeof(RzTensor*));
    m->weights.wo = (RzTensor**)calloc(n_layers, sizeof(RzTensor*));
    m->weights.rms_ffn_weight = (RzTensor**)calloc(n_layers, sizeof(RzTensor*));
    m->weights.w_gate = (RzTensor**)calloc(n_layers, sizeof(RzTensor*));
    m->weights.w_up = (RzTensor**)calloc(n_layers, sizeof(RzTensor*));
    m->weights.w_down = (RzTensor**)calloc(n_layers, sizeof(RzTensor*));

    for (int l = 0; l < n_layers; l++) {
        m->weights.rms_att_weight[l] = rz_tensor_create_1d(dim);
        m->weights.wq[l] = rz_tensor_create_2d(dim, dim);
        m->weights.wk[l] = rz_tensor_create_2d(dim, dim);
        m->weights.wv[l] = rz_tensor_create_2d(dim, dim);
        m->weights.wo[l] = rz_tensor_create_2d(dim, dim);
        m->weights.rms_ffn_weight[l] = rz_tensor_create_1d(dim);
        // SwiGLU: (dim -> hidden_dim) and (hidden_dim -> dim)
        m->weights.w_gate[l] = rz_tensor_create_2d(dim, hidden_dim);
        m->weights.w_up[l] = rz_tensor_create_2d(dim, hidden_dim);
        m->weights.w_down[l] = rz_tensor_create_2d(hidden_dim, dim);
    }
    m->weights.rms_final_weight = rz_tensor_create_1d(dim);
    m->weights.w_cls = rz_tensor_create_2d(dim, vocab_size);

    // KV Cache
    m->key_cache = (RzTensor**)calloc(n_layers, sizeof(RzTensor*));
    m->value_cache = (RzTensor**)calloc(n_layers, sizeof(RzTensor*));
    for (int l = 0; l < n_layers; l++) {
        m->key_cache[l] = rz_tensor_create_2d(max_seq_len, dim);
        m->value_cache[l] = rz_tensor_create_2d(max_seq_len, dim);
    }

    // Working state buffers
    m->x = rz_tensor_create_1d(dim);
    m->xb = rz_tensor_create_1d(dim);
    m->q = rz_tensor_create_1d(dim);
    m->k = rz_tensor_create_1d(dim);
    m->v = rz_tensor_create_1d(dim);
    m->att = rz_tensor_create_1d(max_seq_len);
    m->hb = rz_tensor_create_1d(hidden_dim);
    m->hb2 = rz_tensor_create_1d(hidden_dim);
    m->logits = rz_tensor_create_1d(vocab_size);

    return m;
}

static float rand_weight(float scale) {
    return (((float)rand() / (float)RAND_MAX) * 2.0f - 1.0f) * scale;
}

void rz_llama_init_weights(RzLlamaModel* model, unsigned int seed) {
    if (!model) return;
    srand(seed);
    float scale = 1.0f / sqrtf((float)model->config.dim);

    for (int i = 0; i < model->weights.token_embedding_table->size; i++) {
        model->weights.token_embedding_table->data[i] = rand_weight(scale);
    }

    for (int l = 0; l < model->config.n_layers; l++) {
        rz_tensor_fill(model->weights.rms_att_weight[l], 1.0);
        rz_tensor_fill(model->weights.rms_ffn_weight[l], 1.0);

        for (int i = 0; i < model->weights.wq[l]->size; i++) model->weights.wq[l]->data[i] = rand_weight(scale);
        for (int i = 0; i < model->weights.wk[l]->size; i++) model->weights.wk[l]->data[i] = rand_weight(scale);
        for (int i = 0; i < model->weights.wv[l]->size; i++) model->weights.wv[l]->data[i] = rand_weight(scale);
        for (int i = 0; i < model->weights.wo[l]->size; i++) model->weights.wo[l]->data[i] = rand_weight(scale);

        for (int i = 0; i < model->weights.w_gate[l]->size; i++) model->weights.w_gate[l]->data[i] = rand_weight(scale);
        for (int i = 0; i < model->weights.w_up[l]->size; i++) model->weights.w_up[l]->data[i] = rand_weight(scale);
        for (int i = 0; i < model->weights.w_down[l]->size; i++) model->weights.w_down[l]->data[i] = rand_weight(scale);
    }

    rz_tensor_fill(model->weights.rms_final_weight, 1.0);
    for (int i = 0; i < model->weights.w_cls->size; i++) {
        model->weights.w_cls->data[i] = rand_weight(scale);
    }
}

int rz_llama_save(const RzLlamaModel* model, const char* filepath) {
    if (!model || !filepath) return 0;
    FILE* f = fopen(filepath, "wb");
    if (!f) {
        char dir[256];
        const char* slash = strrchr(filepath, '/');
        if (slash) {
            size_t len = slash - filepath;
            if (len < sizeof(dir)) {
                strncpy(dir, filepath, len);
                dir[len] = '\0';
                mkdir(dir, 0755);
                f = fopen(filepath, "wb");
            }
        }
    }
    if (!f) return 0;

    uint32_t magic = 0x4D4C5A52; // "RZLM"
    fwrite(&magic, sizeof(uint32_t), 1, f);
    fwrite(&model->config, sizeof(RzLlamaConfig), 1, f);

    fwrite(model->weights.token_embedding_table->data, sizeof(float), model->weights.token_embedding_table->size, f);
    for (int l = 0; l < model->config.n_layers; l++) {
        fwrite(model->weights.rms_att_weight[l]->data, sizeof(float), model->weights.rms_att_weight[l]->size, f);
        fwrite(model->weights.wq[l]->data, sizeof(float), model->weights.wq[l]->size, f);
        fwrite(model->weights.wk[l]->data, sizeof(float), model->weights.wk[l]->size, f);
        fwrite(model->weights.wv[l]->data, sizeof(float), model->weights.wv[l]->size, f);
        fwrite(model->weights.wo[l]->data, sizeof(float), model->weights.wo[l]->size, f);
        fwrite(model->weights.rms_ffn_weight[l]->data, sizeof(float), model->weights.rms_ffn_weight[l]->size, f);
        fwrite(model->weights.w_gate[l]->data, sizeof(float), model->weights.w_gate[l]->size, f);
        fwrite(model->weights.w_up[l]->data, sizeof(float), model->weights.w_up[l]->size, f);
        fwrite(model->weights.w_down[l]->data, sizeof(float), model->weights.w_down[l]->size, f);
    }
    fwrite(model->weights.rms_final_weight->data, sizeof(float), model->weights.rms_final_weight->size, f);
    fwrite(model->weights.w_cls->data, sizeof(float), model->weights.w_cls->size, f);

    fclose(f);
    return 1;
}

RzLlamaModel* rz_llama_load(const char* filepath) {
    if (!filepath) return NULL;
    FILE* f = fopen(filepath, "rb");
    if (!f) return NULL;

    uint32_t magic = 0;
    if (fread(&magic, sizeof(uint32_t), 1, f) != 1 || magic != 0x4D4C5A52) {
        fclose(f);
        return NULL;
    }

    RzLlamaConfig cfg;
    if (fread(&cfg, sizeof(RzLlamaConfig), 1, f) != 1) {
        fclose(f);
        return NULL;
    }

    RzLlamaModel* m = rz_llama_create(cfg.vocab_size, cfg.dim, cfg.hidden_dim, cfg.n_layers, cfg.n_heads, cfg.max_seq_len);
    size_t r = 0;
    r += fread(m->weights.token_embedding_table->data, sizeof(float), m->weights.token_embedding_table->size, f);
    for (int l = 0; l < m->config.n_layers; l++) {
        r += fread(m->weights.rms_att_weight[l]->data, sizeof(float), m->weights.rms_att_weight[l]->size, f);
        r += fread(m->weights.wq[l]->data, sizeof(float), m->weights.wq[l]->size, f);
        r += fread(m->weights.wk[l]->data, sizeof(float), m->weights.wk[l]->size, f);
        r += fread(m->weights.wv[l]->data, sizeof(float), m->weights.wv[l]->size, f);
        r += fread(m->weights.wo[l]->data, sizeof(float), m->weights.wo[l]->size, f);
        r += fread(m->weights.rms_ffn_weight[l]->data, sizeof(float), m->weights.rms_ffn_weight[l]->size, f);
        r += fread(m->weights.w_gate[l]->data, sizeof(float), m->weights.w_gate[l]->size, f);
        r += fread(m->weights.w_up[l]->data, sizeof(float), m->weights.w_up[l]->size, f);
        r += fread(m->weights.w_down[l]->data, sizeof(float), m->weights.w_down[l]->size, f);
    }
    r += fread(m->weights.rms_final_weight->data, sizeof(float), m->weights.rms_final_weight->size, f);
    r += fread(m->weights.w_cls->data, sizeof(float), m->weights.w_cls->size, f);
    (void)r;

    fclose(f);
    return m;
}

void rz_llama_free(RzLlamaModel* m) {
    if (!m) return;
    rz_tensor_free(m->weights.token_embedding_table);
    for (int l = 0; l < m->config.n_layers; l++) {
        rz_tensor_free(m->weights.rms_att_weight[l]);
        rz_tensor_free(m->weights.wq[l]);
        rz_tensor_free(m->weights.wk[l]);
        rz_tensor_free(m->weights.wv[l]);
        rz_tensor_free(m->weights.wo[l]);
        rz_tensor_free(m->weights.rms_ffn_weight[l]);
        rz_tensor_free(m->weights.w_gate[l]);
        rz_tensor_free(m->weights.w_up[l]);
        rz_tensor_free(m->weights.w_down[l]);
        rz_tensor_free(m->key_cache[l]);
        rz_tensor_free(m->value_cache[l]);
    }
    free(m->weights.rms_att_weight);
    free(m->weights.wq);
    free(m->weights.wk);
    free(m->weights.wv);
    free(m->weights.wo);
    free(m->weights.rms_ffn_weight);
    free(m->weights.w_gate);
    free(m->weights.w_up);
    free(m->weights.w_down);
    free(m->key_cache);
    free(m->value_cache);

    rz_tensor_free(m->weights.rms_final_weight);
    rz_tensor_free(m->weights.w_cls);

    rz_tensor_free(m->x);
    rz_tensor_free(m->xb);
    rz_tensor_free(m->q);
    rz_tensor_free(m->k);
    rz_tensor_free(m->v);
    rz_tensor_free(m->att);
    rz_tensor_free(m->hb);
    rz_tensor_free(m->hb2);
    rz_tensor_free(m->logits);
    free(m);
}

int rz_llama_forward(RzLlamaModel* model, int token, int pos) {
    if (!model || token < 0 || token >= model->config.vocab_size) return 0;
    int dim = model->config.dim;
    int head_dim = dim / model->config.n_heads;
    int n_heads = model->config.n_heads;

    // 1. Token Embedding lookup: copy embedding row into x
    const float* emb_row = model->weights.token_embedding_table->data + token * dim;
    memcpy(model->x->data, emb_row, dim * sizeof(float));

    // 2. Transformer layers
    for (int l = 0; l < model->config.n_layers; l++) {
        // a. Attention RMSNorm
        rz_tensor_rms_norm(model->x, model->weights.rms_att_weight[l], model->xb, 1e-5);

        // b. Compute Q, K, V projections: xb (1, dim) x w (dim, dim) -> q/k/v (1, dim)
        rz_tensor_matmul(model->xb, model->weights.wq[l], model->q);
        rz_tensor_matmul(model->xb, model->weights.wk[l], model->k);
        rz_tensor_matmul(model->xb, model->weights.wv[l], model->v);

        // c. Rotary Positional Embeddings (RoPE)
        rz_tensor_rope(model->q, pos, head_dim);
        rz_tensor_rope(model->k, pos, head_dim);

        // d. Cache Key & Value
        if (pos < model->config.max_seq_len) {
            memcpy(model->key_cache[l]->data + pos * dim, model->k->data, dim * sizeof(float));
            memcpy(model->value_cache[l]->data + pos * dim, model->v->data, dim * sizeof(float));
        }

        // e. Multi-Head Attention
        float head_scale = 1.0f / sqrtf((float)head_dim);
        for (int h = 0; h < n_heads; h++) {
            const float* q_h = model->q->data + h * head_dim;
            float* att_h = model->att->data;

            for (int t = 0; t <= pos; t++) {
                const float* k_h = model->key_cache[l]->data + t * dim + h * head_dim;
                float score = 0.0f;
                for (int i = 0; i < head_dim; i++) {
                    score += q_h[i] * k_h[i];
                }
                att_h[t] = score * head_scale;
            }

            // Softmax over past positions 0..pos
            float max_s = att_h[0];
            for (int t = 1; t <= pos; t++) if (att_h[t] > max_s) max_s = att_h[t];
            float sum_s = 0.0f;
            for (int t = 0; t <= pos; t++) {
                att_h[t] = expf(att_h[t] - max_s);
                sum_s += att_h[t];
            }
            float inv_s = 1.0f / (sum_s > 0.0f ? sum_s : 1e-6f);
            for (int t = 0; t <= pos; t++) att_h[t] *= inv_s;

            // Weighted sum of Value vectors
            float* out_h = model->xb->data + h * head_dim;
            memset(out_h, 0, head_dim * sizeof(float));
            for (int t = 0; t <= pos; t++) {
                const float* v_h = model->value_cache[l]->data + t * dim + h * head_dim;
                float a = att_h[t];
                for (int i = 0; i < head_dim; i++) {
                    out_h[i] += a * v_h[i];
                }
            }
        }

        // f. Output projection: q = wo * xb
        rz_tensor_matmul(model->xb, model->weights.wo[l], model->q);

        // g. Residual connection: x = x + q
        rz_tensor_add(model->x, model->q, model->x);

        // h. Feed-Forward Network: RMSNorm
        rz_tensor_rms_norm(model->x, model->weights.rms_ffn_weight[l], model->xb, 1e-5);

        // i. SwiGLU MLP:
        // xb (1, dim) x w_gate (dim, hidden_dim) -> hb (1, hidden_dim)
        // xb (1, dim) x w_up   (dim, hidden_dim) -> hb2 (1, hidden_dim)
        rz_tensor_matmul(model->xb, model->weights.w_gate[l], model->hb);
        rz_tensor_matmul(model->xb, model->weights.w_up[l], model->hb2);
        rz_tensor_silu(model->hb);
        rz_tensor_mul(model->hb, model->hb2, model->hb);
        // hb (1, hidden_dim) x w_down (hidden_dim, dim) -> xb (1, dim)
        rz_tensor_matmul(model->hb, model->weights.w_down[l], model->xb);

        // j. Residual connection: x = x + xb
        rz_tensor_add(model->x, model->xb, model->x);
    }

    // 3. Final RMSNorm
    rz_tensor_rms_norm(model->x, model->weights.rms_final_weight, model->x, 1e-5);

    // 4. LM Head projection: x (1, dim) x w_cls (dim, vocab_size) -> logits (1, vocab_size)
    rz_tensor_matmul(model->x, model->weights.w_cls, model->logits);

    return rz_tensor_argmax(model->logits);
}

double rz_llama_get_logit(const RzLlamaModel* model, int token_id) {
    if (!model || token_id < 0 || token_id >= model->config.vocab_size) return 0.0;
    return (double)model->logits->data[token_id];
}

int rz_llama_sample_next(RzLlamaModel* model, double temperature) {
    if (!model) return 0;
    return rz_tensor_sample(model->logits, temperature);
}

// ------------------------------------------------------------------------------
// Tokenizer Dictionary
// ------------------------------------------------------------------------------

#define MAX_VOCAB 512
static char g_vocab[MAX_VOCAB][64];
static int g_vocab_size = 0;

void rz_tokenizer_init(void) {
    g_vocab_size = 0;
    const char* default_words[] = {
        "<pad>", "<bos>", "<eos>", "<unk>",
        "Roze", "is", "a", "modern", "systems", "language", "that", "combines",
        "memory", "safety", "with", "bare-metal", "native", "speed", ".",
        "It", "compiles", "to", "JVM", "bytecode", "and", "freestanding", "kernels",
        "supporting", "AI", "and", "real-time", "game", "development", "efficiently",
        "high-performance", "tensor", "computations", "accelerated", "inference",
        "neural", "networks", "transform", "intelligent", "applications", "seamlessly"
    };
    int count = sizeof(default_words) / sizeof(default_words[0]);
    for (int i = 0; i < count; i++) {
        rz_tokenizer_add_token(default_words[i], i);
    }
}

void rz_tokenizer_add_token(const char* word, int id) {
    if (id >= 0 && id < MAX_VOCAB && word) {
        strncpy(g_vocab[id], word, 63);
        g_vocab[id][63] = '\0';
        if (id >= g_vocab_size) g_vocab_size = id + 1;
    }
}

int rz_tokenizer_encode(const char* text, int* out_tokens, int max_tokens) {
    if (!text || !out_tokens || max_tokens <= 0) return 0;
    if (g_vocab_size == 0) rz_tokenizer_init();

    char buf[1024];
    strncpy(buf, text, sizeof(buf) - 1);
    buf[sizeof(buf) - 1] = '\0';

    int token_count = 0;
    char* word = strtok(buf, " \t\r\n");
    while (word && token_count < max_tokens) {
        int found = 3; // <unk>
        for (int i = 0; i < g_vocab_size; i++) {
            if (strcmp(g_vocab[i], word) == 0) {
                found = i;
                break;
            }
        }
        out_tokens[token_count++] = found;
        word = strtok(NULL, " \t\r\n");
    }
    return token_count;
}

const char* rz_tokenizer_decode(int token_id) {
    if (token_id >= 0 && token_id < g_vocab_size) {
        return g_vocab[token_id];
    }
    return "<unk>";
}

void rz_tokenizer_print_token(int token_id) {
    printf("%s ", rz_tokenizer_decode(token_id));
    fflush(stdout);
}

// ------------------------------------------------------------------------------
// 4. Predictive Classification: Deep Neural Network (MLP / Vision / Tabular)
// ------------------------------------------------------------------------------

RzClassifier* rz_classifier_create(int in_dim, int hidden1, int hidden2, int out_dim) {
    RzClassifier* m = (RzClassifier*)calloc(1, sizeof(RzClassifier));
    m->in_dim = in_dim;
    m->hidden1 = hidden1;
    m->hidden2 = hidden2;
    m->out_dim = out_dim;

    // Weight matrices for vector-matrix multiplication: (in_dim, hidden1)
    m->w1 = rz_tensor_create_2d(in_dim, hidden1);
    m->b1 = rz_tensor_create_1d(hidden1);
    m->w2 = rz_tensor_create_2d(hidden1, hidden2);
    m->b2 = rz_tensor_create_1d(hidden2);
    m->w3 = rz_tensor_create_2d(hidden2, out_dim);
    m->b3 = rz_tensor_create_1d(out_dim);

    m->h1 = rz_tensor_create_1d(hidden1);
    m->h2 = rz_tensor_create_1d(hidden2);
    m->out = rz_tensor_create_1d(out_dim);
    return m;
}

void rz_classifier_init_weights(RzClassifier* model, unsigned int seed) {
    if (!model) return;
    srand(seed);

    float s1 = 1.0f / sqrtf((float)model->in_dim);
    for (int i = 0; i < model->w1->size; i++) model->w1->data[i] = rand_weight(s1);
    rz_tensor_fill(model->b1, 0.05);

    float s2 = 1.0f / sqrtf((float)model->hidden1);
    for (int i = 0; i < model->w2->size; i++) model->w2->data[i] = rand_weight(s2);
    rz_tensor_fill(model->b2, 0.05);

    float s3 = 1.0f / sqrtf((float)model->hidden2);
    for (int i = 0; i < model->w3->size; i++) model->w3->data[i] = rand_weight(s3);
    rz_tensor_fill(model->b3, 0.0);
}

int rz_classifier_predict_tensor(RzClassifier* model, const RzTensor* input_tensor) {
    if (!model || !input_tensor || !input_tensor->data) return 0;

    // Layer 1: h1 = ReLU(input_tensor x W1 + b1)
    rz_tensor_matmul(input_tensor, model->w1, model->h1);
    rz_tensor_add_bias(model->h1, model->b1);
    rz_tensor_relu(model->h1);

    // Layer 2: h2 = ReLU(h1 x W2 + b2)
    rz_tensor_matmul(model->h1, model->w2, model->h2);
    rz_tensor_add_bias(model->h2, model->b2);
    rz_tensor_relu(model->h2);

    // Layer 3: out = Softmax(h2 x W3 + b3)
    rz_tensor_matmul(model->h2, model->w3, model->out);
    rz_tensor_add_bias(model->out, model->b3);
    rz_tensor_softmax(model->out);

    return rz_tensor_argmax(model->out);
}

double rz_classifier_get_prob(const RzClassifier* model, int class_idx) {
    if (!model || class_idx < 0 || class_idx >= model->out_dim) return 0.0;
    return (double)model->out->data[class_idx];
}

void rz_classifier_free(RzClassifier* model) {
    if (!model) return;
    rz_tensor_free(model->w1);
    rz_tensor_free(model->b1);
    rz_tensor_free(model->w2);
    rz_tensor_free(model->b2);
    rz_tensor_free(model->w3);
    rz_tensor_free(model->b3);
    rz_tensor_free(model->h1);
    rz_tensor_free(model->h2);
    rz_tensor_free(model->out);
    free(model);
}

// ------------------------------------------------------------------------------
// 5. System Inspection & GFLOPS Benchmarks
// ------------------------------------------------------------------------------

void rz_ai_print_system_info(void) {
    printf("🧠 Roze AI / ML Tensor Inference Engine (GGML / C-FFI)\n");
    printf("   - Backend: Native Cranelift C-ABI Bridge\n");
    printf("   - Precision: FP32 Single Precision & KV Cached Tensors\n");
    printf("   - Architectures: LLaMA Transformer Decoder (RoPE, SwiGLU, RMSNorm) & Deep MLP Classifier\n");
}

void rz_print_stat_int(const char* label, int val, const char* suffix) {
    printf("%s%d%s\n", label, val, suffix ? suffix : "");
}

void rz_print_stat_float(const char* label, double val, const char* suffix) {
    printf("%s%.4f%s\n", label, val, suffix ? suffix : "");
}

void rz_print_str_no_newline(const char* s) {
    printf("%s", s);
    fflush(stdout);
}

double rz_benchmark_matmul(int size, int iterations) {
    if (size <= 0 || iterations <= 0) return 0.0;
    RzTensor* A = rz_tensor_create_2d(size, size);
    RzTensor* B = rz_tensor_create_2d(size, size);
    RzTensor* C = rz_tensor_create_2d(size, size);
    rz_tensor_fill(A, 0.5);
    rz_tensor_fill(B, 0.5);

    clock_t start = clock();
    for (int it = 0; it < iterations; it++) {
        rz_tensor_matmul(A, B, C);
    }
    clock_t end = clock();

    double elapsed_sec = (double)(end - start) / (double)CLOCKS_PER_SEC;
    if (elapsed_sec <= 0.0) elapsed_sec = 0.0001;

    double total_ops = 2.0 * (double)size * (double)size * (double)size * (double)iterations;
    double gflops = (total_ops / elapsed_sec) / 1e9;

    rz_tensor_free(A);
    rz_tensor_free(B);
    rz_tensor_free(C);
    return gflops;
}
