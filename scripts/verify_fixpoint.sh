#!/usr/bin/env bash
# ==============================================================================
# 🌹 ROZE 3-STAGE BOOTSTRAP FIXPOINT IDEMPOTENCE VERIFIER (M6-T06)
# ==============================================================================
# Verifies mathematical bootstrap closure of the pure Roze self-hosted compiler:
#   Stage 0 (Autonomous Seed C) -> compiles -> Stage 1 (Roze)
#   Stage 1 (Roze)              -> compiles -> Stage 2 (Roze)
#   Stage 2 (Roze)              -> compiles -> Stage 3 (Roze)
#
# Idempotence criteria:
#   1. diff -u bin/roze_stage2.c bin/roze_stage3.c == 0 (Source Closure)
#   2. cmp bin/roze_stage2 bin/roze_stage3 == 0 (Binary Executable Closure)
#   3. sha256(roze_stage2) == sha256(roze_stage3) (Bit-for-Bit Identity)
#   4. Functional Parity across all compiler flags (--parse-only, --check-only,
#      --emit-ssa, --target-elf, --target-macho, --target-pe, --standalone-bin)
# ==============================================================================
set -euo pipefail

# ANSI color formatting
BOLD="\033[1m"
GREEN="\033[32m"
BLUE="\033[34m"
CYAN="\033[36m"
YELLOW="\033[33m"
RED="\033[31m"
RESET="\033[0m"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SELF_DIR="$ROOT_DIR/demos/self_hosted"
BIN_DIR="$SELF_DIR/bin"
SRC_DIR="$SELF_DIR/src"
TEST_DIR="$SELF_DIR/tests"
COMPILER_SRC="$SRC_DIR/roze_compiler.roze"
SEED_C="$ROOT_DIR/bootstrap/roze_seed.c"
VERIFY_TMP="$BIN_DIR/fixpoint_verify_tmp"

echo -e "${BOLD}${CYAN}================================================================================${RESET}"
echo -e "${BOLD}${CYAN}🌹 ROZE 3-STAGE BOOTSTRAP FIXPOINT IDEMPOTENCE VERIFICATION (M6-T06)${RESET}"
echo -e "${BOLD}${CYAN}================================================================================${RESET}"
echo -e "${BOLD}Target: Mathematically verify 0-byte difference between Stage 2 and Stage 3${RESET}"
echo -e "${BOLD}Working Directory: ${ROOT_DIR}${RESET}"
echo -e "${BOLD}Compiler Source:   ${COMPILER_SRC}${RESET}"
echo ""

mkdir -p "$BIN_DIR"
mkdir -p "$VERIFY_TMP"

# Nanosecond timing helper
get_nanos() {
    date +%s%N
}

# ------------------------------------------------------------------------------
# STEP 0: Bundle Single-Source Roze Self-Hosted Compiler
# ------------------------------------------------------------------------------
echo -e "${BOLD}${BLUE}📦 [Step 0/4] Bundling pure Roze compiler translation unit...${RESET}"
T0_START=$(get_nanos)
make -C "$SELF_DIR" bundle > /dev/null
T0_END=$(get_nanos)
T0_MS=$(( (T0_END - T0_START) / 1000000 ))
LINE_COUNT=$(wc -l < "$COMPILER_SRC")
echo -e "   ${GREEN}✓${RESET} Bundled ${BOLD}${COMPILER_SRC}${RESET} (${LINE_COUNT} lines in ${T0_MS} ms)"

# ------------------------------------------------------------------------------
# STEP 1: Stage 0 (Autonomous Seed) -> Stage 1 Compiler
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}${BLUE}🌱 [Step 1/4] Compiling Stage 0 Seed -> Stage 1 Compiler...${RESET}"
if [ ! -f "$SEED_C" ]; then
    echo -e "${RED}❌ Error: Seed file ${SEED_C} not found!${RESET}"
    exit 1
fi

# Compile seed C compiler into roze_bootstrap
T_SEED_START=$(get_nanos)
gcc -O2 -w -s "$SEED_C" -o "$BIN_DIR/roze_bootstrap" -lm
T_SEED_END=$(get_nanos)
T_SEED_MS=$(( (T_SEED_END - T_SEED_START) / 1000000 ))
SEED_SIZE=$(stat -c%s "$BIN_DIR/roze_bootstrap")
SEED_HASH=$(sha256sum "$BIN_DIR/roze_bootstrap" | cut -d' ' -f1)
echo -e "   ${GREEN}✓${RESET} Built Stage 0 seed compiler: ${BOLD}bin/roze_bootstrap${RESET} (${SEED_SIZE} bytes in ${T_SEED_MS} ms)"
echo -e "     SHA-256: ${CYAN}${SEED_HASH}${RESET}"

# Use roze_bootstrap to compile Stage 1
T_S1_START=$(get_nanos)
"$BIN_DIR/roze_bootstrap" "$COMPILER_SRC" -o "$BIN_DIR/roze_stage1" > /dev/null
T_S1_END=$(get_nanos)
T_S1_MS=$(( (T_S1_END - T_S1_START) / 1000000 ))
S1_C_SIZE=$(stat -c%s "$BIN_DIR/roze_stage1.c")
S1_BIN_SIZE=$(stat -c%s "$BIN_DIR/roze_stage1")
S1_C_HASH=$(sha256sum "$BIN_DIR/roze_stage1.c" | cut -d' ' -f1)
S1_BIN_HASH=$(sha256sum "$BIN_DIR/roze_stage1" | cut -d' ' -f1)
echo -e "   ${GREEN}✓${RESET} Built Stage 1 compiler: ${BOLD}bin/roze_stage1${RESET} (${S1_BIN_SIZE} bytes in ${T_S1_MS} ms)"
echo -e "     Emitted C: ${S1_C_SIZE} bytes | SHA-256: ${CYAN}${S1_C_HASH}${RESET}"
echo -e "     Binary:    ${S1_BIN_SIZE} bytes | SHA-256: ${CYAN}${S1_BIN_HASH}${RESET}"

# ------------------------------------------------------------------------------
# STEP 2: Stage 1 Compiler -> Stage 2 Compiler
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}${BLUE}⚡ [Step 2/4] Compiling Stage 1 -> Stage 2 Compiler...${RESET}"
T_S2_START=$(get_nanos)
"$BIN_DIR/roze_stage1" "$COMPILER_SRC" -o "$BIN_DIR/roze_stage2" > /dev/null
T_S2_END=$(get_nanos)
T_S2_MS=$(( (T_S2_END - T_S2_START) / 1000000 ))
S2_C_SIZE=$(stat -c%s "$BIN_DIR/roze_stage2.c")
S2_BIN_SIZE=$(stat -c%s "$BIN_DIR/roze_stage2")
S2_C_HASH=$(sha256sum "$BIN_DIR/roze_stage2.c" | cut -d' ' -f1)
S2_BIN_HASH=$(sha256sum "$BIN_DIR/roze_stage2" | cut -d' ' -f1)
echo -e "   ${GREEN}✓${RESET} Built Stage 2 compiler: ${BOLD}bin/roze_stage2${RESET} (${S2_BIN_SIZE} bytes in ${T_S2_MS} ms)"
echo -e "     Emitted C: ${S2_C_SIZE} bytes | SHA-256: ${CYAN}${S2_C_HASH}${RESET}"
echo -e "     Binary:    ${S2_BIN_SIZE} bytes | SHA-256: ${CYAN}${S2_BIN_HASH}${RESET}"

# ------------------------------------------------------------------------------
# STEP 3: Stage 2 Compiler -> Stage 3 Compiler
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}${BLUE}🔬 [Step 3/4] Compiling Stage 2 -> Stage 3 Compiler...${RESET}"
T_S3_START=$(get_nanos)
"$BIN_DIR/roze_stage2" "$COMPILER_SRC" -o "$BIN_DIR/roze_stage3" > /dev/null
T_S3_END=$(get_nanos)
T_S3_MS=$(( (T_S3_END - T_S3_START) / 1000000 ))
S3_C_SIZE=$(stat -c%s "$BIN_DIR/roze_stage3.c")
S3_BIN_SIZE=$(stat -c%s "$BIN_DIR/roze_stage3")
S3_C_HASH=$(sha256sum "$BIN_DIR/roze_stage3.c" | cut -d' ' -f1)
S3_BIN_HASH=$(sha256sum "$BIN_DIR/roze_stage3" | cut -d' ' -f1)
echo -e "   ${GREEN}✓${RESET} Built Stage 3 compiler: ${BOLD}bin/roze_stage3${RESET} (${S3_BIN_SIZE} bytes in ${T_S3_MS} ms)"
echo -e "     Emitted C: ${S3_C_SIZE} bytes | SHA-256: ${CYAN}${S3_C_HASH}${RESET}"
echo -e "     Binary:    ${S3_BIN_SIZE} bytes | SHA-256: ${CYAN}${S3_BIN_HASH}${RESET}"

# ------------------------------------------------------------------------------
# STEP 4: Mathematically Verify Bootstrap Closure (Idempotence)
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}${BLUE}🎯 [Step 4/4] Verifying Mathematical Fixpoint Closure & Parity...${RESET}"

# 1. Diff of C output
echo -n "   1. Checking emitted C code identity (roze_stage2.c vs roze_stage3.c)... "
if diff -u "$BIN_DIR/roze_stage2.c" "$BIN_DIR/roze_stage3.c" > "$VERIFY_TMP/c_diff.patch"; then
    echo -e "${GREEN}${BOLD}PASSED (0 bytes diff)${RESET}"
else
    echo -e "${RED}${BOLD}FAILED! Diff found:${RESET}"
    cat "$VERIFY_TMP/c_diff.patch"
    exit 1
fi

# 2. Diff of Binary output
echo -n "   2. Checking native executable identity (roze_stage2 vs roze_stage3)... "
if cmp -s "$BIN_DIR/roze_stage2" "$BIN_DIR/roze_stage3"; then
    echo -e "${GREEN}${BOLD}PASSED (0 bytes diff)${RESET}"
else
    DIFF_COUNT=$(cmp -l "$BIN_DIR/roze_stage2" "$BIN_DIR/roze_stage3" | wc -l)
    echo -e "${RED}${BOLD}FAILED (${DIFF_COUNT} differing bytes)!${RESET}"
    exit 1
fi

# 3. SHA-256 Checksum Comparison
echo -n "   3. Cryptographic SHA-256 verification (C Source)... "
if [ "$S2_C_HASH" = "$S3_C_HASH" ]; then
    echo -e "${GREEN}${BOLD}IDENTICAL (${S2_C_HASH})${RESET}"
else
    echo -e "${RED}${BOLD}HASH MISMATCH!${RESET}"
    exit 1
fi

echo -n "   4. Cryptographic SHA-256 verification (Native Binary)... "
if [ "$S2_BIN_HASH" = "$S3_BIN_HASH" ]; then
    echo -e "${GREEN}${BOLD}IDENTICAL (${S2_BIN_HASH})${RESET}"
else
    echo -e "${RED}${BOLD}HASH MISMATCH!${RESET}"
    exit 1
fi

# ------------------------------------------------------------------------------
# STEP 5: Functional Parity Across Compiler Flags
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}${BLUE}🧪 [Step 5] Verifying Full Compiler CLI Flag Parity...${RESET}"

# 1. --parse-only parity
echo -n "   1. Pure Roze AST Parser (--parse-only)... "
"$BIN_DIR/roze_stage2" --parse-only "$TEST_DIR/06_frontend_grammar.roze" > "$VERIFY_TMP/s2_parse.log"
"$BIN_DIR/roze_stage3" --parse-only "$TEST_DIR/06_frontend_grammar.roze" > "$VERIFY_TMP/s3_parse.log"
diff -u "$VERIFY_TMP/s2_parse.log" "$VERIFY_TMP/s3_parse.log"
echo -e "${GREEN}${BOLD}PARITY VERIFIED${RESET}"

# 2. --check-only parity
echo -n "   2. Pure Roze Semantic Analyzer (--check-only)... "
"$BIN_DIR/roze_stage2" --check-only "$TEST_DIR/07_semantic_types.roze" > "$VERIFY_TMP/s2_check.log"
"$BIN_DIR/roze_stage3" --check-only "$TEST_DIR/07_semantic_types.roze" > "$VERIFY_TMP/s3_check.log"
diff -u "$VERIFY_TMP/s2_check.log" "$VERIFY_TMP/s3_check.log"
echo -e "${GREEN}${BOLD}PARITY VERIFIED${RESET}"

# 3. --emit-ssa parity
echo -n "   3. Pure Roze SSA IR & Optimizer (--emit-ssa)... "
"$BIN_DIR/roze_stage2" --emit-ssa "$TEST_DIR/08_optimizer.roze" > "$VERIFY_TMP/s2_ssa.ir"
"$BIN_DIR/roze_stage3" --emit-ssa "$TEST_DIR/08_optimizer.roze" > "$VERIFY_TMP/s3_ssa.ir"
diff -u "$VERIFY_TMP/s2_ssa.ir" "$VERIFY_TMP/s3_ssa.ir"
echo -e "${GREEN}${BOLD}PARITY VERIFIED${RESET}"

# 4. --target-elf parity
echo -n "   4. Direct Machine Code ELF64 Relocatable Emitter (--target-elf)... "
"$BIN_DIR/roze_stage2" "$TEST_DIR/09_native_binary.roze" --target-elf "$VERIFY_TMP/s2_elf.o" > /dev/null
"$BIN_DIR/roze_stage3" "$TEST_DIR/09_native_binary.roze" --target-elf "$VERIFY_TMP/s3_elf.o" > /dev/null
cmp -s "$VERIFY_TMP/s2_elf.o" "$VERIFY_TMP/s3_elf.o"
echo -e "${GREEN}${BOLD}PARITY VERIFIED (0 diff)${RESET}"

# 5. --target-macho parity
echo -n "   5. Direct Machine Code Mach-O 64-bit Relocatable Emitter (--target-macho)... "
"$BIN_DIR/roze_stage2" "$TEST_DIR/09_native_binary.roze" --target-macho "$VERIFY_TMP/s2_macho.o" > /dev/null
"$BIN_DIR/roze_stage3" "$TEST_DIR/09_native_binary.roze" --target-macho "$VERIFY_TMP/s3_macho.o" > /dev/null
cmp -s "$VERIFY_TMP/s2_macho.o" "$VERIFY_TMP/s3_macho.o"
echo -e "${GREEN}${BOLD}PARITY VERIFIED (0 diff)${RESET}"

# 6. --target-pe parity
echo -n "   6. Direct Machine Code PE/COFF 64-bit Relocatable Emitter (--target-pe)... "
"$BIN_DIR/roze_stage2" "$TEST_DIR/09_native_binary.roze" --target-pe "$VERIFY_TMP/s2_pe.obj" > /dev/null
"$BIN_DIR/roze_stage3" "$TEST_DIR/09_native_binary.roze" --target-pe "$VERIFY_TMP/s3_pe.obj" > /dev/null
cmp -s "$VERIFY_TMP/s2_pe.obj" "$VERIFY_TMP/s3_pe.obj"
echo -e "${GREEN}${BOLD}PARITY VERIFIED (0 diff)${RESET}"

# 7. --standalone-bin parity & direct syscall execution
echo -n "   7. Standalone Executable Emitter & Execution (--standalone-bin)... "
"$BIN_DIR/roze_stage2" "$TEST_DIR/09_native_binary.roze" --standalone-bin "$VERIFY_TMP/s2_standalone" > /dev/null
"$BIN_DIR/roze_stage3" "$TEST_DIR/09_native_binary.roze" --standalone-bin "$VERIFY_TMP/s3_standalone" > /dev/null
cmp -s "$VERIFY_TMP/s2_standalone" "$VERIFY_TMP/s3_standalone"
OUTPUT_S2=$("$VERIFY_TMP/s2_standalone")
OUTPUT_S3=$("$VERIFY_TMP/s3_standalone")
if [ "$OUTPUT_S2" = "$OUTPUT_S3" ] && [ -n "$OUTPUT_S2" ]; then
    echo -e "${GREEN}${BOLD}PARITY & EXECUTION VERIFIED${RESET}"
else
    echo -e "${RED}${BOLD}FAILED EXECUTION PARITY!${RESET}"
    exit 1
fi

# 8. Complete compiler test suite execution with Stage 3 compiler
echo -n "   8. Running 5 Full Test Programs with Stage 3 Compiler... "
"$BIN_DIR/roze_stage3" "$TEST_DIR/01_hello.roze" -o "$VERIFY_TMP/test01" > /dev/null && "$VERIFY_TMP/test01" > /dev/null
"$BIN_DIR/roze_stage3" "$TEST_DIR/02_arithmetic.roze" -o "$VERIFY_TMP/test02" > /dev/null && "$VERIFY_TMP/test02" > /dev/null
"$BIN_DIR/roze_stage3" "$TEST_DIR/03_control_flow.roze" -o "$VERIFY_TMP/test03" > /dev/null && "$VERIFY_TMP/test03" > /dev/null
"$BIN_DIR/roze_stage3" "$TEST_DIR/04_classes.roze" -o "$VERIFY_TMP/test04" > /dev/null && "$VERIFY_TMP/test04" > /dev/null
"$BIN_DIR/roze_stage3" "$TEST_DIR/05_strings.roze" -o "$VERIFY_TMP/test05" > /dev/null && "$VERIFY_TMP/test05" > /dev/null
echo -e "${GREEN}${BOLD}ALL 5 EXECUTED SUCCESSFULLY${RESET}"

# Cleanup temporary verification files
rm -rf "$VERIFY_TMP"

# ------------------------------------------------------------------------------
# STEP 6: Fixpoint Summary Report
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}${CYAN}================================================================================${RESET}"
echo -e "${BOLD}${CYAN}📊 3-STAGE BOOTSTRAP FIXPOINT REPORT SUMMARY${RESET}"
echo -e "${BOLD}${CYAN}================================================================================${RESET}"
printf "%-12s | %-12s | %-14s | %-64s\n" "Stage" "Duration" "Binary Size" "SHA-256 Digest"
echo "------------------------------------------------------------------------------------------------------------------------"
printf "%-12s | %-10d ms | %-12d B | %-64s\n" "Stage 0 (Seed)" "$T_SEED_MS" "$SEED_SIZE" "$SEED_HASH"
printf "%-12s | %-10d ms | %-12d B | %-64s\n" "Stage 1 (Roze)" "$T_S1_MS" "$S1_BIN_SIZE" "$S1_BIN_HASH"
printf "%-12s | %-10d ms | %-12d B | %-64s\n" "Stage 2 (Roze)" "$T_S2_MS" "$S2_BIN_SIZE" "$S2_BIN_HASH"
printf "%-12s | %-10d ms | %-12d B | %-64s\n" "Stage 3 (Roze)" "$T_S3_MS" "$S3_BIN_SIZE" "$S3_BIN_HASH"
echo "------------------------------------------------------------------------------------------------------------------------"
echo -e "${BOLD}Emitted C Source Fixpoint:${RESET} diff -u bin/roze_stage2.c bin/roze_stage3.c == ${GREEN}${BOLD}0 bytes (IDENTICAL)${RESET}"
echo -e "${BOLD}Native Binary Fixpoint:   ${RESET} cmp bin/roze_stage2 bin/roze_stage3        == ${GREEN}${BOLD}0 bytes (IDENTICAL)${RESET}"
echo -e "${BOLD}SHA-256 Fixpoint Match:   ${RESET} ${GREEN}${BOLD}${S2_BIN_HASH}${RESET}"
echo -e "${BOLD}Mathematical Closure:     ${RESET} ${GREEN}${BOLD}PROVED: FIXPOINT CONVERGENCE ACHIEVED (100% IDEMPOTENT)${RESET}"
echo -e "${BOLD}${CYAN}================================================================================${RESET}"
echo -e "${GREEN}${BOLD}🎉 M6-T06 COMPLETE: 3-Stage Bootstrap Fixpoint Verified with Zero Byte Difference!${RESET}"
exit 0
