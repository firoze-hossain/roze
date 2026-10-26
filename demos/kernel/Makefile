# ==============================================================================
# 🌹 Roze OS Kernel Makefile
# ==============================================================================

ROZE ?= cargo run --manifest-path ../../Cargo.toml --bin roze --
SRC = src/main.roze
KERNEL_BIN = main
KERNEL_QEMU = main.qemu
ISO_IMG = roze_os.iso
ISO_DIR = build/iso

.PHONY: all build iso run run-iso clean

all: build

build: $(SRC)
	@echo "🔧 Building Roze 64-bit Freestanding Kernel..."
	$(ROZE) build --target freestanding $(SRC)

iso: build
	@echo "💿 Creating Bootable Multiboot2 ISO Image..."
	@mkdir -p $(ISO_DIR)/boot/grub
	@cp $(KERNEL_BIN) $(ISO_DIR)/boot/kernel.bin
	@cp grub.cfg $(ISO_DIR)/boot/grub/grub.cfg
	grub-mkrescue -o $(ISO_IMG) $(ISO_DIR) 2>/dev/null
	@echo "✅ Bootable ISO created: $(ISO_IMG)"

run: build
	@echo "🚀 Booting Roze OS in QEMU (Direct Multiboot Kernel)..."
	qemu-system-x86_64 \
		-kernel $(KERNEL_QEMU) \
		-vga std \
		-serial stdio \
		-device isa-debug-exit,iobase=0xf4,iosize=0x04

run-iso: iso
	@echo "🚀 Booting Roze OS in QEMU (GRUB Multiboot2 CD-ROM)..."
	qemu-system-x86_64 \
		-cdrom $(ISO_IMG) \
		-vga std \
		-serial stdio \
		-device isa-debug-exit,iobase=0xf4,iosize=0x04

clean:
	@echo "🧹 Cleaning kernel artifacts..."
	rm -rf $(KERNEL_BIN) $(KERNEL_QEMU) *.o build $(ISO_IMG)
