# ==============================================================================
# 🌹 Roze AI / ML Inference Demo - Makefile (ADV-05)
# ==============================================================================

ROZE ?= ../../target/debug/roze
BUILD_DIR ?= bin
TARGET ?= $(BUILD_DIR)/roze_ai_demo

.PHONY: all build run benchmark clean

all: build

$(ROZE):
	@echo "🔧 Building Roze compiler..."
	cd ../.. && cargo build --bin roze

build: $(ROZE)
	@mkdir -p $(BUILD_DIR) models
	@echo "🧠 Building Roze AI / ML Native Executable via C-FFI..."
	$(ROZE) build src/main.roze \
		--target native \
		-o $(BUILD_DIR) \
		--link src/roze_ai.c \
		--link m
	@mv -f main $(TARGET) 2>/dev/null || true
	@rm -f main.o 2>/dev/null || true
	@echo "✅ AI/ML binary built: $(TARGET)"

run: build
	@echo "🚀 Executing Roze AI / ML Inference Suite..."
	./$(TARGET)

benchmark: build
	@echo "⚡ Executing GFLOPS Tensor Benchmark..."
	./$(TARGET)

clean:
	@rm -rf $(BUILD_DIR) models main main.o
	@echo "🧹 Cleaned AI demo build artifacts."
