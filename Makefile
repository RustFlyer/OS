# Makefile for building and managing the OS kernel for RISC-V architecture

# ======================
# Build Configuration Variables
# ======================

# Target architecture
# export ARCH = riscv64
export ARCH = loongarch64

# Docker image name for development environment
# Kernel package/output name
# Bootloader selection (default BIOS for QEMU)
# Target archeitecture full name
DOCKER_NAME = os
PACKAGE_NAME = kernel
BOOTLOADER = default

ifeq ($(ARCH),riscv64)
	TARGET = riscv64gc-unknown-none-elf
else ifeq ($(ARCH),loongarch64)
	TARGET = loongarch64-unknown-none
else
	$(error "Unsupported architecture: $(ARCH).")
endif

# Number of CPU cores
# Target board (QEMU emulator)
# Build mode (debug/release)
# Logging level (trace/debug/info/warn/error/off)
# Conditionally compile `when_debug` macro
export SMP = 1
export BOARD = qemu
export MODE = debug
export LOG = trace
export DEBUG = off

QEMU = qemu-system-$(ARCH)
OBJDUMP = rust-objdump --arch-name=$(ARCH)
OBJCOPY = rust-objcopy --binary-architecture=$(ARCH)
PAGER = less

DISASM_ARGS = -d -s

TARGET_DIR := target/$(TARGET)/$(MODE)
KERNEL_ELF := $(TARGET_DIR)/$(PACKAGE_NAME)
KERNEL_ASM := $(TARGET_DIR)/$(PACKAGE_NAME).asm

USER_APPS_DIR := ./user/src/bin
USER_APPS := $(wildcard $(USER_APPS_DIR)/*.rs)
USER_ELFS := $(patsubst $(USER_APPS_DIR)/%.rs, $(TARGET_DIR)/%, $(USER_APPS))
USER_BINS := $(patsubst $(USER_APPS_DIR)/%.rs, $(TARGET_DIR)/%.bin, $(USER_APPS))


FS_IMG_DIR := fsimg
FS_IMG := $(FS_IMG_DIR)/sdcard.img

ifeq ($(ARCH),riscv64)
	QEMU_ARGS := -m 128
	QEMU_ARGS += -machine virt
	QEMU_ARGS += -nographic
	QEMU_ARGS += -bios $(BOOTLOADER)
	QEMU_ARGS += -kernel $(KERNEL_ELF)
	QEMU_ARGS += -smp $(SMP)
	QEMU_ARGS += -drive file=$(FS_IMG),if=none,format=raw,id=x0
	QEMU_ARGS += -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

	GDB = riscv64-unknown-elf-gdb
	GDB_ARGS = riscv:rv64
endif

ifeq ($(ARCH),loongarch64)
	QEMU_ARGS := -m 1G
	QEMU_ARGS += -nographic
	QEMU_ARGS += -kernel $(KERNEL_ELF)
	QEMU_ARGS += -smp $(SMP)
	QEMU_ARGS += -drive file=$(FS_IMG),if=none,format=raw,id=x0
	QEMU_ARGS += -device virtio-blk-pci,drive=x0
	QEMU_ARGS += -no-reboot
	QEMU_ARGS += -device virtio-net-pci,netdev=net0 
	QEMU_ARGS += -netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555
	QEMU_ARGS += -rtc base=utc
# QEMU_ARGS += -drive file=disk-la.img,if=none,format=raw,id=x1
# QEMU_ARGS += -device virtio-blk-pci,drive=x1 

	QEMU_ARGS += -dtb loongarch.dtb
# QEMU_ARGS += -bios uefi_bios.bin
# QEMU_ARGS += -vga none
# QEMU_ARGS += -D qemu.log -d guest_errors,unimp,in_asm
# QEMU_ARGS += -machine virt,dumpdtb=loongarch.dtb
# QEMU_ARGS += -machine virt,accel=tcg

	GDB = loongarch64-unknown-linux-gnu-gdb
	GDB_ARGS = Loongarch64
endif


PHONY := all
all: $(KERNEL_ELF) $(KERNEL_ASM) $(USER_APPS)


$(KERNEL_ELF): build
$(KERNEL_ASM): $(KERNEL_ELF)
	@$(OBJDUMP) $(DISASM_ARGS) $(KERNEL_ELF) > $(KERNEL_ASM)
	@echo "Updated: $(KERNEL_ASM)"

 
PHONY += build2docker
build2docker:
	@docker build -t ${DOCKER_NAME} .


PHONY += docker
docker:
	@docker run --rm -it --network="host" -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} bash

 
PHONY += env
env:
	@(cargo install --list | grep "cargo-binutils" > /dev/null 2>&1) || cargo install cargo-binutils


PHONY += build
build: env user
	@echo Platform: $(BOARD)
	@cd kernel && make build
	@echo "Updated: $(KERNEL_ELF)"

 
PHONY += run
run: build
	@echo $(QEMU_ARGS)
	@$(QEMU) $(QEMU_ARGS)


PHONY += clean
clean:
	@cargo clean
	@rm -rf $(TARGET_DIR)/*

 
PHONY += disasm
disasm: $(KERNEL_ASM)
	@cat $(KERNEL_ASM) | $(PAGER)

 
PHONY += gdbserver
gdbserver: all
	@$(QEMU) $(QEMU_ARGS) -s -S


PHONY += gdbclient
gdbclient: all
	@$(GDB) -ex 'file $(KERNEL_ELF)' \
			-ex 'set arch $(GDB_ARGS)' \
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
	@echo "building user finished"


PHONY += fs-img
fs-img: user
	@echo "building fs-img ext4..."
	@echo $(FS_IMG)
	@rm -rf $(FS_IMG)
	@mkdir -p $(FS_IMG_DIR)
	@dd if=/dev/zero of=$(FS_IMG) bs=1K count=524288 status=progress
	@mkfs.ext4 -F $(FS_IMG)
	@mkdir -p emnt
	@sudo mount -t ext4 -o loop $(FS_IMG) emnt
	@sudo cp -r $(USER_ELFS) emnt/
	@sudo cp -r testcase/basic/* emnt/
	@sudo cp -r testcase/busybox/* emnt/
	@sudo cp -r img-data/* emnt/
	@sudo chmod -R 755 emnt/
	@sudo umount emnt
	@sudo rm -rf emnt
	@echo "building fs-img finished"


.PHONY: $(PHONY)

