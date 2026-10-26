// ==============================================================================
// 🌹 Roze Game Engine C-FFI Bridge for Raylib (ADV-04)
// ==============================================================================
// Provides a clean C ABI interface between Roze native code and Raylib.
// Handles Color conversion, 2D/3D drawing, input, miniaudio procedural sound,
// and high-performance game entity pools (particles, bullets, asteroids).
// ==============================================================================

#include "../raylib/include/raylib.h"
#include <stdio.h>
#include <stdlib.h>
#include <math.h>

// Helper to convert uint32_t RGBA hex color to Raylib Color
static inline Color HexToColor(unsigned int hex) {
    // hex is 0xRRGGBBAA
    Color c;
    c.r = (unsigned char)((hex >> 24) & 0xFF);
    c.g = (unsigned char)((hex >> 16) & 0xFF);
    c.b = (unsigned char)((hex >> 8) & 0xFF);
    c.a = (unsigned char)(hex & 0xFF);
    return c;
}

// ------------------------------------------------------------------------------
// 1. Core Windowing & Lifecycle
// ------------------------------------------------------------------------------

void RzSetLogLevel(int level) {
    SetTraceLogLevel(level);
}

void RzInitWindow(int width, int height, const char* title) {
    InitWindow(width, height, title);
}

void RzCloseWindow(void) {
    CloseWindow();
}

int RzWindowShouldClose(void) {
    return WindowShouldClose() ? 1 : 0;
}

int RzIsWindowReady(void) {
    return IsWindowReady() ? 1 : 0;
}

void RzSetTargetFPS(int fps) {
    SetTargetFPS(fps);
}

int RzGetFPS(void) {
    return GetFPS();
}

double RzGetFrameTime(void) {
    return (double)GetFrameTime();
}

double RzGetTime(void) {
    return GetTime();
}

int RzIsHeadless(void) {
    if (getenv("ROZE_GAME_BENCHMARK") || getenv("ROZE_HEADLESS")) return 1;
    const char* disp = getenv("DISPLAY");
    const char* wayland = getenv("WAYLAND_DISPLAY");
    if (!disp && !wayland) return 1;
    return 0;
}

// ------------------------------------------------------------------------------
// 2. Frame & 2D Drawing
// ------------------------------------------------------------------------------

void RzBeginDrawing(void) {
    BeginDrawing();
}

void RzEndDrawing(void) {
    EndDrawing();
}

void RzClear(unsigned int color_hex) {
    ClearBackground(HexToColor(color_hex));
}

void RzClearRGBA(int r, int g, int b, int a) {
    Color c = { (unsigned char)r, (unsigned char)g, (unsigned char)b, (unsigned char)a };
    ClearBackground(c);
}

void RzDrawText(const char* text, int posX, int posY, int fontSize, unsigned int color_hex) {
    DrawText(text, posX, posY, fontSize, HexToColor(color_hex));
}

void RzDrawFPS(int posX, int posY) {
    DrawFPS(posX, posY);
}

void RzDrawScore(int score, int posX, int posY) {
    char buf[64];
    snprintf(buf, sizeof(buf), "Score: %d", score);
    DrawText(buf, posX, posY, 18, HexToColor(0xFFFFFFFF));
}

void RzDrawHealth(int health, int posX, int posY) {
    char buf[64];
    snprintf(buf, sizeof(buf), "Hull Integrity: %d%%", health);
    DrawText(buf, posX, posY, 16, HexToColor(0x00FF7FFF));
}

void RzPrintStat(const char* label, int val, const char* suffix) {
    printf("%s%d%s\n", label, val, suffix);
}

void RzDrawRectangle(float posX, float posY, float width, float height, unsigned int color_hex) {
    Rectangle rec = { posX, posY, width, height };
    DrawRectangleRec(rec, HexToColor(color_hex));
}

void RzDrawRectangleLines(float posX, float posY, float width, float height, unsigned int color_hex) {
    DrawRectangleLines((int)posX, (int)posY, (int)width, (int)height, HexToColor(color_hex));
}

void RzDrawCircle(float centerX, float centerY, float radius, unsigned int color_hex) {
    Vector2 center = { centerX, centerY };
    DrawCircleV(center, radius, HexToColor(color_hex));
}

void RzDrawCircleLines(float centerX, float centerY, float radius, unsigned int color_hex) {
    DrawCircleLines((int)centerX, (int)centerY, radius, HexToColor(color_hex));
}

void RzDrawLine(float startX, float startY, float endX, float endY, unsigned int color_hex) {
    Vector2 start = { startX, startY };
    Vector2 end = { endX, endY };
    DrawLineV(start, end, HexToColor(color_hex));
}

void RzDrawTriangle(float v1x, float v1y, float v2x, float v2y, float v3x, float v3y, unsigned int color_hex) {
    Vector2 v1 = { v1x, v1y };
    Vector2 v2 = { v2x, v2y };
    Vector2 v3 = { v3x, v3y };
    DrawTriangle(v1, v2, v3, HexToColor(color_hex));
}

// ------------------------------------------------------------------------------
// 3. 3D Mode & Geometry
// ------------------------------------------------------------------------------

void RzBeginMode3D(float posX, float posY, float posZ,
                   float targetX, float targetY, float targetZ,
                   float upX, float upY, float upZ,
                   float fovy) {
    Camera3D camera = { 0 };
    camera.position = (Vector3){ posX, posY, posZ };
    camera.target = (Vector3){ targetX, targetY, targetZ };
    camera.up = (Vector3){ upX, upY, upZ };
    camera.fovy = fovy;
    camera.projection = CAMERA_PERSPECTIVE;
    BeginMode3D(camera);
}

void RzEndMode3D(void) {
    EndMode3D();
}

void RzDrawCube(float x, float y, float z, float width, float height, float length, unsigned int color_hex) {
    Vector3 position = { x, y, z };
    DrawCube(position, width, height, length, HexToColor(color_hex));
}

void RzDrawCubeWires(float x, float y, float z, float width, float height, float length, unsigned int color_hex) {
    Vector3 position = { x, y, z };
    DrawCubeWires(position, width, height, length, HexToColor(color_hex));
}

void RzDrawSphere(float x, float y, float z, float radius, unsigned int color_hex) {
    Vector3 position = { x, y, z };
    DrawSphere(position, radius, HexToColor(color_hex));
}

void RzDrawGrid(int slices, float spacing) {
    DrawGrid(slices, spacing);
}

// ------------------------------------------------------------------------------
// 4. Input Handling
// ------------------------------------------------------------------------------

int RzIsKeyDown(int key) {
    return IsKeyDown(key) ? 1 : 0;
}

int RzIsKeyPressed(int key) {
    return IsKeyPressed(key) ? 1 : 0;
}

int RzIsMouseButtonDown(int button) {
    return IsMouseButtonDown(button) ? 1 : 0;
}

int RzIsMouseButtonPressed(int button) {
    return IsMouseButtonPressed(button) ? 1 : 0;
}

int RzGetMouseX(void) {
    return GetMouseX();
}

int RzGetMouseY(void) {
    return GetMouseY();
}

// ------------------------------------------------------------------------------
// 5. Audio & Procedural Sound Effects (miniaudio)
// ------------------------------------------------------------------------------

void RzInitAudio(void) {
    InitAudioDevice();
}

void RzCloseAudio(void) {
    CloseAudioDevice();
}

int RzIsAudioReady(void) {
    return IsAudioDeviceReady() ? 1 : 0;
}

void RzSetVolume(float volume) {
    SetMasterVolume(volume);
}

// Synthetic Laser Sound: Pitch sweep downward (880 Hz -> 220 Hz)
void RzPlayLaserSound(void) {
    if (!IsAudioDeviceReady()) return;
    int sampleRate = 44100;
    int duration_ms = 120;
    int sampleCount = (sampleRate * duration_ms) / 1000;
    short* samples = (short*)malloc(sampleCount * sizeof(short));
    if (!samples) return;

    for (int i = 0; i < sampleCount; i++) {
        double t = (double)i / (double)sampleRate;
        double freq = 880.0 - (660.0 * (double)i / (double)sampleCount);
        double amp = 0.4 * (1.0 - (double)i / (double)sampleCount);
        samples[i] = (short)(32767.0 * amp * sin(2.0 * 3.1415926535 * freq * t));
    }

    Wave wave = { 0 };
    wave.frameCount = sampleCount;
    wave.sampleRate = sampleRate;
    wave.sampleSize = 16;
    wave.channels = 1;
    wave.data = samples;
    Sound sound = LoadSoundFromWave(wave);
    PlaySound(sound);
    free(samples);
}

// Synthetic Explosion Sound: Low-frequency noise burst with exponential decay
void RzPlayExplosionSound(void) {
    if (!IsAudioDeviceReady()) return;
    int sampleRate = 44100;
    int duration_ms = 350;
    int sampleCount = (sampleRate * duration_ms) / 1000;
    short* samples = (short*)malloc(sampleCount * sizeof(short));
    if (!samples) return;

    for (int i = 0; i < sampleCount; i++) {
        double decay = 1.0 - ((double)i / (double)sampleCount);
        double noise = ((double)rand() / (double)RAND_MAX) * 2.0 - 1.0;
        double lowFreq = sin(2.0 * 3.1415926535 * 65.0 * ((double)i / (double)sampleRate));
        double sampleVal = (0.75 * noise + 0.25 * lowFreq) * decay * 0.45;
        samples[i] = (short)(32767.0 * sampleVal);
    }

    Wave wave = { 0 };
    wave.frameCount = sampleCount;
    wave.sampleRate = sampleRate;
    wave.sampleSize = 16;
    wave.channels = 1;
    wave.data = samples;
    Sound sound = LoadSoundFromWave(wave);
    PlaySound(sound);
    free(samples);
}

// Synthetic Powerup / Chime Sound: Arpeggiated ascending chime
void RzPlayPowerupSound(void) {
    if (!IsAudioDeviceReady()) return;
    int sampleRate = 44100;
    int duration_ms = 220;
    int sampleCount = (sampleRate * duration_ms) / 1000;
    short* samples = (short*)malloc(sampleCount * sizeof(short));
    if (!samples) return;

    for (int i = 0; i < sampleCount; i++) {
        double t = (double)i / (double)sampleRate;
        double freq = (i < sampleCount / 2) ? 587.33 : 880.00; // D5 -> A5
        double amp = 0.35 * (1.0 - (double)i / (double)sampleCount);
        samples[i] = (short)(32767.0 * amp * sin(2.0 * 3.1415926535 * freq * t));
    }

    Wave wave = { 0 };
    wave.frameCount = sampleCount;
    wave.sampleRate = sampleRate;
    wave.sampleSize = 16;
    wave.channels = 1;
    wave.data = samples;
    Sound sound = LoadSoundFromWave(wave);
    PlaySound(sound);
    free(samples);
}

// Synthetic Tone Generator
void RzPlayTone(int frequency, int duration_ms) {
    if (!IsAudioDeviceReady()) return;
    int sampleRate = 44100;
    int sampleCount = (sampleRate * duration_ms) / 1000;
    if (sampleCount <= 0) return;
    short* samples = (short*)malloc(sampleCount * sizeof(short));
    if (!samples) return;

    for (int i = 0; i < sampleCount; i++) {
        double t = (double)i / (double)sampleRate;
        double amp = 0.3 * (1.0 - (double)i / (double)sampleCount);
        samples[i] = (short)(32767.0 * amp * sin(2.0 * 3.1415926535 * (double)frequency * t));
    }

    Wave wave = { 0 };
    wave.frameCount = sampleCount;
    wave.sampleRate = sampleRate;
    wave.sampleSize = 16;
    wave.channels = 1;
    wave.data = samples;
    Sound sound = LoadSoundFromWave(wave);
    PlaySound(sound);
    free(samples);
}

// ------------------------------------------------------------------------------
// 6. High-Performance Game Entity Pools & Particle System (60 FPS)
// ------------------------------------------------------------------------------

#define MAX_PROJECTILES 64
#define MAX_ASTEROIDS 32
#define MAX_PARTICLES 256

typedef struct {
    int active;
    float x;
    float y;
    float vx;
    float vy;
    float lifetime;
} RzProjectile;

typedef struct {
    int active;
    float x;
    float y;
    float vx;
    float vy;
    float radius;
    int health;
    unsigned int color;
} RzAsteroid;

typedef struct {
    int active;
    float x;
    float y;
    float vx;
    float vy;
    float life;
    float max_life;
    unsigned int color;
    float size;
} RzParticle;

static RzProjectile g_projectiles[MAX_PROJECTILES];
static RzAsteroid g_asteroids[MAX_ASTEROIDS];
static RzParticle g_particles[MAX_PARTICLES];

void RzSpawnParticle(float x, float y, float vx, float vy, unsigned int color, float life, float size) {
    for (int i = 0; i < MAX_PARTICLES; i++) {
        if (!g_particles[i].active) {
            g_particles[i].active = 1;
            g_particles[i].x = x;
            g_particles[i].y = y;
            g_particles[i].vx = vx;
            g_particles[i].vy = vy;
            g_particles[i].color = color;
            g_particles[i].life = life;
            g_particles[i].max_life = life;
            g_particles[i].size = size;
            break;
        }
    }
}

void RzUpdateParticles(double dt) {
    for (int i = 0; i < MAX_PARTICLES; i++) {
        if (g_particles[i].active) {
            g_particles[i].x += g_particles[i].vx * (float)dt;
            g_particles[i].y += g_particles[i].vy * (float)dt;
            g_particles[i].life -= (float)dt;
            if (g_particles[i].life <= 0.0f) {
                g_particles[i].active = 0;
            }
        }
    }
}

void RzDrawParticles(void) {
    for (int i = 0; i < MAX_PARTICLES; i++) {
        if (g_particles[i].active) {
            float alpha = g_particles[i].life / g_particles[i].max_life;
            if (alpha < 0.0f) alpha = 0.0f;
            Color base = HexToColor(g_particles[i].color);
            base.a = (unsigned char)(base.a * alpha);
            DrawCircleV((Vector2){ g_particles[i].x, g_particles[i].y }, g_particles[i].size * alpha, base);
        }
    }
}

int RzGetParticleCount(void) {
    int c = 0;
    for (int i = 0; i < MAX_PARTICLES; i++) if (g_particles[i].active) c++;
    return c;
}

void RzSpawnProjectile(float x, float y, float vx, float vy) {
    for (int i = 0; i < MAX_PROJECTILES; i++) {
        if (!g_projectiles[i].active) {
            g_projectiles[i].active = 1;
            g_projectiles[i].x = x;
            g_projectiles[i].y = y;
            g_projectiles[i].vx = vx;
            g_projectiles[i].vy = vy;
            g_projectiles[i].lifetime = 1.8f;
            break;
        }
    }
}

void RzUpdateProjectiles(double dt) {
    for (int i = 0; i < MAX_PROJECTILES; i++) {
        if (g_projectiles[i].active) {
            g_projectiles[i].x += g_projectiles[i].vx * (float)dt;
            g_projectiles[i].y += g_projectiles[i].vy * (float)dt;
            g_projectiles[i].lifetime -= (float)dt;
            if (g_projectiles[i].lifetime <= 0.0f || g_projectiles[i].y < -20.0f || g_projectiles[i].y > 700.0f) {
                g_projectiles[i].active = 0;
            }
        }
    }
}

void RzDrawProjectiles(void) {
    for (int i = 0; i < MAX_PROJECTILES; i++) {
        if (g_projectiles[i].active) {
            DrawRectangleRec((Rectangle){ g_projectiles[i].x - 2.0f, g_projectiles[i].y - 8.0f, 4.0f, 16.0f }, HexToColor(0xFF2222FF));
            DrawCircleV((Vector2){ g_projectiles[i].x, g_projectiles[i].y - 8.0f }, 3.0f, HexToColor(0xFFFFFFEE));
        }
    }
}

int RzGetProjectileCount(void) {
    int c = 0;
    for (int i = 0; i < MAX_PROJECTILES; i++) if (g_projectiles[i].active) c++;
    return c;
}

void RzSpawnAsteroid(float x, float y, float vx, float vy, float radius, int health, unsigned int color) {
    for (int i = 0; i < MAX_ASTEROIDS; i++) {
        if (!g_asteroids[i].active) {
            g_asteroids[i].active = 1;
            g_asteroids[i].x = x;
            g_asteroids[i].y = y;
            g_asteroids[i].vx = vx;
            g_asteroids[i].vy = vy;
            g_asteroids[i].radius = radius;
            g_asteroids[i].health = health;
            g_asteroids[i].color = color;
            break;
        }
    }
}

void RzUpdateAsteroids(double dt, int screen_w, int screen_h) {
    for (int i = 0; i < MAX_ASTEROIDS; i++) {
        if (g_asteroids[i].active) {
            g_asteroids[i].x += g_asteroids[i].vx * (float)dt * 60.0f;
            g_asteroids[i].y += g_asteroids[i].vy * (float)dt * 60.0f;
            float r = g_asteroids[i].radius;
            if (g_asteroids[i].x < -r) g_asteroids[i].x = (float)screen_w + r;
            if (g_asteroids[i].x > (float)screen_w + r) g_asteroids[i].x = -r;
            if (g_asteroids[i].y < -r) g_asteroids[i].y = (float)screen_h + r;
            if (g_asteroids[i].y > (float)screen_h + r) g_asteroids[i].y = -r;
        }
    }
}

void RzDrawAsteroids(void) {
    for (int i = 0; i < MAX_ASTEROIDS; i++) {
        if (g_asteroids[i].active) {
            DrawCircleV((Vector2){ g_asteroids[i].x, g_asteroids[i].y }, g_asteroids[i].radius, HexToColor(g_asteroids[i].color));
            DrawCircleLines((int)g_asteroids[i].x, (int)g_asteroids[i].y, g_asteroids[i].radius, HexToColor(0xFFFFFFFF));
        }
    }
}

int RzGetAsteroidCount(void) {
    int c = 0;
    for (int i = 0; i < MAX_ASTEROIDS; i++) if (g_asteroids[i].active) c++;
    return c;
}

int RzCheckBulletAsteroidCollisions(void) {
    int hits = 0;
    for (int b = 0; b < MAX_PROJECTILES; b++) {
        if (!g_projectiles[b].active) continue;
        for (int a = 0; a < MAX_ASTEROIDS; a++) {
            if (!g_asteroids[a].active) continue;
            float dx = g_projectiles[b].x - g_asteroids[a].x;
            float dy = g_projectiles[b].y - g_asteroids[a].y;
            float dist_sq = dx * dx + dy * dy;
            float r = g_asteroids[a].radius;
            if (dist_sq < (r * r)) {
                // Bullet hit asteroid
                g_projectiles[b].active = 0;
                g_asteroids[a].health -= 1;
                hits++;
                // Spawn hit spark particles
                for (int p = 0; p < 8; p++) {
                    float angle = (float)p * (3.14159f * 2.0f / 8.0f);
                    float spd = 60.0f + (float)(p * 15);
                    RzSpawnParticle(g_projectiles[b].x, g_projectiles[b].y, cosf(angle) * spd, sinf(angle) * spd, 0xFFAA00FF, 0.4f, 4.0f);
                }
                if (g_asteroids[a].health <= 0) {
                    g_asteroids[a].active = 0;
                    // Explosion burst
                    for (int p = 0; p < 16; p++) {
                        float angle = (float)p * (3.14159f * 2.0f / 16.0f);
                        float spd = 40.0f + (float)(p * 12);
                        RzSpawnParticle(g_asteroids[a].x, g_asteroids[a].y, cosf(angle) * spd, sinf(angle) * spd, 0xFF5500FF, 0.6f, 6.0f);
                    }
                    RzPlayExplosionSound();
                } else {
                    RzPlayTone(440, 60);
                }
                break;
            }
        }
    }
    return hits;
}

int RzCheckShipAsteroidCollision(float ship_x, float ship_y, float ship_radius) {
    for (int a = 0; a < MAX_ASTEROIDS; a++) {
        if (!g_asteroids[a].active) continue;
        float dx = ship_x - g_asteroids[a].x;
        float dy = ship_y - g_asteroids[a].y;
        float dist_sq = dx * dx + dy * dy;
        float combined_r = ship_radius + g_asteroids[a].radius;
        if (dist_sq < (combined_r * combined_r)) {
            // Ship hit asteroid
            for (int p = 0; p < 10; p++) {
                float angle = (float)p * (3.14159f * 2.0f / 10.0f);
                RzSpawnParticle(ship_x, ship_y, cosf(angle) * 70.0f, sinf(angle) * 70.0f, 0xFF0033FF, 0.5f, 5.0f);
            }
            RzPlayTone(220, 100);
            return 1;
        }
    }
    return 0;
}

void RzResetFleet(void) {
    for (int i = 0; i < MAX_PROJECTILES; i++) g_projectiles[i].active = 0;
    for (int i = 0; i < MAX_ASTEROIDS; i++) g_asteroids[i].active = 0;
    for (int i = 0; i < MAX_PARTICLES; i++) g_particles[i].active = 0;

    // Spawn 6 initial asteroids
    RzSpawnAsteroid(150.0f, 100.0f,  1.2f,  0.8f, 28.0f, 2, 0xFFD700FF);
    RzSpawnAsteroid(650.0f, 120.0f, -1.0f,  0.7f, 32.0f, 3, 0xFFA500FF);
    RzSpawnAsteroid(200.0f, 480.0f,  0.9f, -1.1f, 24.0f, 2, 0xFF00FFFF);
    RzSpawnAsteroid(600.0f, 450.0f, -0.8f, -0.9f, 30.0f, 3, 0x87CEEBFF);
    RzSpawnAsteroid(400.0f,  80.0f,  0.5f,  1.3f, 26.0f, 2, 0x00FF7FFF);
    RzSpawnAsteroid(700.0f, 300.0f, -1.3f, -0.4f, 34.0f, 3, 0xFF6347FF);
}
