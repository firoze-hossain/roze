# ==============================================================================
# 🌹 Roze Autonomous Language Build System (M6-T05)
# ==============================================================================
# 100% Pure Roze Build System - Rust Decommissioned as Primary Build Driver
# ==============================================================================

CC ?= gcc
CFLAGS ?= -O2 -w
PREFIX ?= /usr/local

BIN_DIR = bin
BOOTSTRAP_DIR = bootstrap
SELF_HOSTED_DIR = demos/self_hosted

ROZE_BIN = $(BIN_DIR)/roze
ROZE_SEED = $(BOOTSTRAP_DIR)/roze_seed.c

.PHONY: all roze bootstrap test clean seed install uninstall help

# Default target: build autonomous compiler, verify bootstrap, and create ./roze
all: bootstrap
	@mkdir -p $(BIN_DIR)
	@cp $(SELF_HOSTED_DIR)/bin/roze_stage3 $(ROZE_BIN)
	@chmod +x $(ROZE_BIN)
	@ln -sf $(ROZE_BIN) ./roze
	@echo "=================================================================="
	@echo "🌹 Roze Autonomous Compiler Ready: ./roze ($(ROZE_BIN))"
	@echo "=================================================================="

# Autonomous 3-stage bootstrap fixpoint closure (Zero Rust Dependency)
bootstrap:
	@echo "===> Executing Pure Roze Autonomous Bootstrap Pipeline..."
	@$(MAKE) -C $(SELF_HOSTED_DIR) bundle
	@$(MAKE) -C $(SELF_HOSTED_DIR) all

# Run complete self-hosted test suites using pure Roze compiler
test: all
	@echo "===> Running All Pure Roze Test Suites via $(ROZE_BIN)..."
	@$(MAKE) -C $(SELF_HOSTED_DIR) test_programs
	@$(MAKE) -C $(SELF_HOSTED_DIR) test_frontend
	@$(MAKE) -C $(SELF_HOSTED_DIR) test_semantic
	@$(MAKE) -C $(SELF_HOSTED_DIR) test_optimizer
	@$(MAKE) -C $(SELF_HOSTED_DIR) test_native_backend
	@echo "=================================================================="
	@echo "🎉 100% Pure Roze Autonomous Test Suites Passed!"
	@echo "=================================================================="

# Regenerate bootstrap seed from latest pure Roze source
seed:
	@echo "===> Regenerating $(ROZE_SEED) from pure Roze sources..."
	@$(MAKE) -C $(SELF_HOSTED_DIR) bundle
	@$(MAKE) -C $(SELF_HOSTED_DIR) stage2
	@cp $(SELF_HOSTED_DIR)/bin/roze_stage2.c $(ROZE_SEED)
	@echo "✅ Updated $(ROZE_SEED)"

# Clean all build artifacts
clean:
	@$(MAKE) -C $(SELF_HOSTED_DIR) clean
	@rm -rf $(BIN_DIR) ./roze /tmp/roze_*

# Install Roze compiler binary to system
install: all
	mkdir -p $(DESTDIR)$(PREFIX)/bin
	cp $(ROZE_BIN) $(DESTDIR)$(PREFIX)/bin/roze
	chmod 755 $(DESTDIR)$(PREFIX)/bin/roze
	@echo "✅ Installed Roze to $(DESTDIR)$(PREFIX)/bin/roze"

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/roze
	@echo "✅ Uninstalled Roze from $(DESTDIR)$(PREFIX)/bin/roze"

help:
	@echo "🌹 Roze Build System (Pure Roze Self-Hosting)"
	@echo ""
	@echo "Targets:"
	@echo "  make              Build autonomous Roze compiler into bin/roze and ./roze"
	@echo "  make bootstrap    Execute 3-stage bootstrap fixpoint closure (0 diff)"
	@echo "  make test         Run full test suite (frontend, semantic, optimizer, native)"
	@echo "  make seed         Regenerate bootstrap/roze_seed.c from source"
	@echo "  make install      Install roze to $(PREFIX)/bin"
	@echo "  make clean        Remove all build artifacts"
