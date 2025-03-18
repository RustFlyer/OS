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

# Number of CPU cores
# Target board (QEMU emulator)
# Build mode (debug/release)
# Logging level (trace/debug/info/warn/error/off)
# Conditionally compile `when_debug` macro
export SMP = 4
export BOARD = qemu
export MODE = debug
export LOG = trace
export DEBUG = off

# ======================
# Toolchain Configuration
# ======================

QEMU = qemu-system-riscv64
GDB = riscv64-unknown-elf-gdb
OBJDUMP = rust-objdump --arch-name=riscv64
OBJCOPY = rust-objcopy --binary-architecture=riscv64
PAGER = less

# ======================
# QEMU Emulator Arguments
# ======================

DISASM_ARGS = -d
QEMU_ARGS = -machine virt \
			-nographic \
			-bios $(BOOTLOADER) \
			-kernel $(KERNEL_ELF) \
			-smp $(SMP)


# ======================
# File Path Configuration
# ======================

TARGET_DIR := target/$(TARGET)/$(MODE)
KERNEL_ELF := $(TARGET_DIR)/$(PACKAGE_NAME)
KERNEL_ASM := $(TARGET_DIR)/$(PACKAGE_NAME).asm

USER_APPS_DIR := ./user/src/bin
USER_APPS := $(wildcard $(USER_APPS_DIR)/*.rs)
USER_ELFS := $(patsubst $(USER_APPS_DIR)/%.rs, $(TARGET_DIR)/%, $(USER_APPS))
USER_BINS := $(patsubst $(USER_APPS_DIR)/%.rs, $(TARGET_DIR)/%.bin, $(USER_APPS))

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

# Build Docker development image
PHONY += build2docker
build2docker:
	@docker build -t ${DOCKER_NAME} .

# Start interactive Docker container with current directory mounted
PHONY += docker
docker:
	@docker run --rm -it --network="host" -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} bash

# ======================
# Build System Targets
# ======================

# Install required Rust toolchain components
PHONY += env
env:
	@(cargo install --list | grep "cargo-binutils" > /dev/null 2>&1) || cargo install cargo-binutils

# Main build target: compiles the kernel
PHONY += build
build: env user
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

# View kernel disassembly (supports pager navigation)
PHONY += disasm
disasm: $(KERNEL_ASM)
	@cat $(KERNEL_ASM) | $(PAGER)

# ======================
# Debugging Targets
# ======================

# Start QEMU in debug server mode (port 1234)
PHONY += gdbserver
gdbserver: all
	@$(QEMU) $(QEMU_ARGS) -s -S

# Connect GDB to running QEMU instance
PHONY += gdbclient
gdbclient: all
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


PHONY += user
user:
	@echo "building user..."
	@cd user && make build
	@$(foreach elf, $(USER_ELFS), $(OBJCOPY) $(elf) --strip-all -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(elf));)
	@echo "building user finished"

# ======================
# Final PHONY Declaration
# ======================

.PHONY: $(PHONY)

