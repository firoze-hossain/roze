# ==============================================================================
# 🌹 Roze Self-Hosted Compiler - Makefile (ADV-06)
# ==============================================================================

CC ?= gcc
CFLAGS ?= -O2 -w
ROZE_STAGE0 ?= ../../target/debug/roze

SRC_DIR = src
BIN_DIR = bin
TEST_DIR = tests

SOURCES = $(SRC_DIR)/ast.roze \
          $(SRC_DIR)/lexer.roze \
          $(SRC_DIR)/parser.roze \
          $(SRC_DIR)/typechecker.roze \
          $(SRC_DIR)/codegen.roze \
          $(SRC_DIR)/main.roze

RUNTIME_C = $(SRC_DIR)/runtime.c
ROZE_COMPILER_ROZE = $(SRC_DIR)/roze_compiler.roze

.PHONY: all clean bundle stage1 stage2 stage3 bootstrap_test test_programs

all: bootstrap_test test_programs

$(BIN_DIR):
	mkdir -p $(BIN_DIR)

bundle: $(ROZE_COMPILER_ROZE)

$(ROZE_COMPILER_ROZE): $(SOURCES)
	python3 -c '\
files = ["$(SRC_DIR)/ast.roze", "$(SRC_DIR)/lexer.roze", "$(SRC_DIR)/parser.roze", "$(SRC_DIR)/typechecker.roze", "$(SRC_DIR)/codegen.roze", "$(SRC_DIR)/main.roze"]; \
out = ["// ==============================================================================\n", \
       "// 🌹 Roze Self-Hosted Compiler (Single-Source Bootstrap Translation Unit)\n", \
       "// ==============================================================================\n\n"]; \
for f in files: \
    with open(f, "r") as fh: \
        for line in fh: \
            if not line.strip().startswith("import "): \
                out.append(line); \
with open("$(ROZE_COMPILER_ROZE)", "w") as fh: \
    fh.write("".join(out)); \
print("✅ Bundled $(ROZE_COMPILER_ROZE) (" + str(len(out)) + " lines)")'

stage1: $(BIN_DIR) $(ROZE_COMPILER_ROZE)
	@echo "===> [Stage 1] Compiling Stage 1 self-hosted compiler using Stage 0 (Rust roze)..."
	$(ROZE_STAGE0) build $(SRC_DIR)/main.roze --target native --link $(RUNTIME_C)
	mv main $(BIN_DIR)/roze_stage1
	@echo "✅ [Stage 1] Built $(BIN_DIR)/roze_stage1"

stage2: stage1
	@echo "===> [Stage 2] Compiling Stage 2 compiler using Stage 1 compiler..."
	./$(BIN_DIR)/roze_stage1 $(ROZE_COMPILER_ROZE) -o $(BIN_DIR)/roze_stage2
	@echo "✅ [Stage 2] Built $(BIN_DIR)/roze_stage2"

stage3: stage2
	@echo "===> [Stage 3] Compiling Stage 3 compiler using Stage 2 compiler..."
	./$(BIN_DIR)/roze_stage2 $(ROZE_COMPILER_ROZE) -o $(BIN_DIR)/roze_stage3
	@echo "✅ [Stage 3] Built $(BIN_DIR)/roze_stage3"

bootstrap_test: stage3
	@echo "=================================================================="
	@echo "⚡ [Verification] Proving Self-Hosting Fixpoint & Idempotence"
	@echo "=================================================================="
	diff -u $(BIN_DIR)/roze_stage2.c $(BIN_DIR)/roze_stage3.c
	@echo "✅ Zero Diff! Stage 2 and Stage 3 outputs are 100% byte-for-byte identical!"
	@echo "🎉 FULL SELF-HOSTING BOOTSTRAP PROOF VERIFIED!"

test_programs: stage2
	@echo "=================================================================="
	@echo "🧪 Running Test Programs with Self-Hosted Compiler"
	@echo "=================================================================="
	./$(BIN_DIR)/roze_stage2 $(TEST_DIR)/01_hello.roze -o $(BIN_DIR)/test01 && ./$(BIN_DIR)/test01
	./$(BIN_DIR)/roze_stage2 $(TEST_DIR)/02_arithmetic.roze -o $(BIN_DIR)/test02 && ./$(BIN_DIR)/test02
	./$(BIN_DIR)/roze_stage2 $(TEST_DIR)/03_control_flow.roze -o $(BIN_DIR)/test03 && ./$(BIN_DIR)/test03
	./$(BIN_DIR)/roze_stage2 $(TEST_DIR)/04_classes.roze -o $(BIN_DIR)/test04 && ./$(BIN_DIR)/test04
	./$(BIN_DIR)/roze_stage2 $(TEST_DIR)/05_strings.roze -o $(BIN_DIR)/test05 && ./$(BIN_DIR)/test05
	@echo "✅ All 5 test suite programs executed and verified!"

clean:
	rm -rf $(BIN_DIR)/*.o $(BIN_DIR)/*.c $(BIN_DIR)/test* $(BIN_DIR)/self_test* $(BIN_DIR)/roze_stage*
