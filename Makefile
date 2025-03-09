# Makefile for building and managing the OS kernel for RISC-V architecture

# ======================
# Build Configuration Variables
# ======================
# Docker image name for development environment
# Kernel package/output name
# Bootloader selection (default BIOS for QEMU)
# Target architecture specification
DOCKER_NAME = my-os
PACKAGE_NAME = kernel
BOOTLOADER = default
TARGET = riscv64gc-unknown-none-elf
# Target board (QEMU emulator)
# Build mode (debug/release)
# Logging level (trace/debug/info/warn/error/off)
export BOARD = qemu
export MODE = debug
export LOG = trace
export DEBUG = off

# ======================
# Toolchain Configuration
# ======================
QEMU = qemu-system-riscv64
GDB = riscv64-elf-gdb
OBJDUMP = rust-objdump --arch-name=riscv64
OBJCOPY = rust-objcopy --binary-architecture=riscv64
PAGER ?= less

# ======================
# QEMU Emulator Arguments
# ======================
DISASM_ARGS = -d
QEMU_ARGS = -machine virt \
			 -nographic \
			 -bios $(BOOTLOADER) \
			 -kernel $(KERNEL_ELF)
	

# ======================
# File Path Configuration
# ======================
TARGET_DIR := target/$(TARGET)/$(MODE)
KERNEL_ELF := $(TARGET_DIR)/$(PACKAGE_NAME)
# be aware that make has implict rule on .S suffix
KERNEL_ASM := $(TARGET_DIR)/$(PACKAGE_NAME).asm

# ======================
# Phony Target Declaration
# ======================
PHONY := all

# ======================
# Default Build Target
# ======================
# Build both kernel ELF and disassembly file
all: $(KERNEL_ELF) $(KERNEL_ASM)

# ======================
# File Dependency Rules
# ======================
# Kernel ELF depends on build target
$(KERNEL_ELF): build

# Generate disassembly from ELF
$(KERNEL_ASM): $(KERNEL_ELF)
	@$(OBJDUMP) $(DISASM_ARGS) $(KERNEL_ELF) > $(KERNEL_ASM)
	@echo "Updated: $(KERNEL_ASM)"

# ======================
# Development Environment Targets
# ======================
PHONY += build2docker

# Build Docker development image
build2docker:
	@docker build -t ${DOCKER_NAME} .

PHONY += docker

# Start interactive Docker container with current directory mounted
docker:
	@docker run --rm -it --network="host" -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} bash


# ======================
# Build System Targets
# ======================
# Install required Rust toolchain components
PHONY += env
env:
	@(cargo install --list | grep "cargo-binutils" > /dev/null 2>&1) || cargo install cargo-binutils

PHONY += build
# Main build target: compiles the kernel
build: env
	@echo Platform: $(BOARD)
	@cd kernel && make build
	@echo "Updated: $(KERNEL_ELF)"

# ======================
# Execution & Debugging Targets
# ======================
# Run kernel in QEMU emulator
PHONY += run
run: build
	@$(QEMU) $(QEMU_ARGS)

# Clean build artifacts
PHONY += clean
clean:
	@cargo clean
	@rm -rf $(TARGET_DIR)/*

# ======================
# Diagnostic Targets
# ======================
PHONY += disasm
# View kernel disassembly (supports pager navigation)
disasm: $(KERNEL_ASM)
	@cat $(KERNEL_ASM) | $(PAGER)

# ======================
# Debugging Targets
# ======================
# Start QEMU in debug server mode (port 1234)
PHONY += gdbserver
gdbserver:
	@$(QEMU) $(QEMU_ARGS) -s -S

# Connect GDB to running QEMU instance
PHONY += gdbclient
gdbclient:
	@$(GDB) -ex 'file $(KERNEL_ELF)' \
			-ex 'set arch riscv:rv64' \
			-ex 'target remote localhost:1234'

PHONY += run-debug
run-debug:
	@make run DEBUG=on

PHONY += run-docker
run-docker:
	@docker run --rm -it --network="host" -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} make run

PHONY += run-docker-debug
run-docker-debug:
	@docker run --rm -it --network="host" -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} make run-debug

# ======================
# Final PHONY Declaration
# ======================
.PHONY: $(PHONY)
