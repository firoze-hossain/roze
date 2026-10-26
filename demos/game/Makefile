# ==============================================================================
# 🌹 Cosmic Roze: Galactic Defender - Makefile (ADV-04)
# ==============================================================================

ROZE ?= ../../target/debug/roze
BUILD_DIR ?= bin
TARGET ?= $(BUILD_DIR)/cosmic_roze

.PHONY: all build run benchmark clean

all: build

$(ROZE):
	@echo "🔧 Building Roze compiler..."
	cd ../.. && cargo build --bin roze

build: $(ROZE)
	@mkdir -p $(BUILD_DIR)
	@echo "🎮 Building Cosmic Roze Game Engine Demo via Roze C-FFI (Raylib)..."
	$(ROZE) build src/main.roze \
		--target native \
		-o $(BUILD_DIR) \
		--link src/raylib_bridge.c \
		--link raylib/lib/libraylib.a \
		--link m \
		--link pthread \
		--link dl
	@mv -f main $(TARGET) 2>/dev/null || true
	@rm -f main.o 2>/dev/null || true
	@echo "✅ Game binary built: $(TARGET)"

run: build
	@echo "🚀 Launching Cosmic Roze: Galactic Defender..."
	./$(TARGET)

benchmark: build
	@echo "⚡ Running 180-frame automated headless simulation benchmark..."
	ROZE_GAME_BENCHMARK=1 ./$(TARGET)

clean:
	@rm -rf $(BUILD_DIR) main main.o
	@echo "🧹 Cleaned game demo build artifacts."
