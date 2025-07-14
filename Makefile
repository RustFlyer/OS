# Makefile for building and managing the OS kernel for RISC-V architecture

# ======================
# Build Configuration Variables
# ======================

# Target architecture
export ARCH = riscv64
# export ARCH = loongarch64
export TESTCASE_LIBC = musl
# export TESTCASE_LIBC = glibc
export SUBMIT = false


# Docker image name for development environment
# Kernel package/output name
# Bootloader selection (default BIOS for QEMU)
# Target archeitecture full name
DOCKER_NAME = os
PACKAGE_NAME = kernel
BOOTLOADER = default

SOFTWARE_DIR = ../software

ifeq ($(ARCH), riscv64)
	export TARGET = riscv64gc-unknown-none-elf
else ifeq ($(ARCH), loongarch64)
	export TARGET = loongarch64-unknown-none
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
export DEBUG = 

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
FS_IMG := $(FS_IMG_DIR)/$(ARCH)-sdcard.img

ifeq ($(ARCH), riscv64)
	QEMU_ARGS := -m 1G
	QEMU_ARGS += -machine virt
	QEMU_ARGS += -nographic
	QEMU_ARGS += -bios $(BOOTLOADER)
	QEMU_ARGS += -kernel $(KERNEL_ELF)
	QEMU_ARGS += -smp $(SMP)
	QEMU_ARGS += -drive file=$(FS_IMG),if=none,format=raw,id=x0
	QEMU_ARGS += -device virtio-blk-device,drive=x0
	QEMU_ARGS += -device virtio-net-device,netdev=net0
	QEMU_ARGS += -netdev user,id=net0

	GDB = riscv64-unknown-elf-gdb
	GDB_ARGS = riscv:rv64
endif

ifeq ($(ARCH), loongarch64)
	QEMU_ARGS := -m 1G
	QEMU_ARGS += -machine virt 	
	QEMU_ARGS += -nographic
	QEMU_ARGS += -kernel $(KERNEL_ELF)
	QEMU_ARGS += -smp $(SMP)
	QEMU_ARGS += -drive file=$(FS_IMG),if=none,format=raw,id=x0
	QEMU_ARGS += -device virtio-blk-pci,drive=x0
	QEMU_ARGS += -no-reboot
# QEMU_ARGS += -device virtio-net-pci,netdev=net0
# QEMU_ARGS += -netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555
	QEMU_ARGS += -rtc base=utc
# QEMU_ARGS += -drive file=disk-la.img,if=none,format=raw,id=x1
# QEMU_ARGS += -device virtio-blk-pci,drive=x1

# QEMU_ARGS += -dtb loongarch.dtb
# QEMU_ARGS += -bios uefi_bios.bin
# QEMU_ARGS += -vga none
# QEMU_ARGS += -D qemu.log -d guest_errors,unimp,in_asm
# QEMU_ARGS += -machine virt,dumpdtb=loongarch.dtb
# QEMU_ARGS += -machine virt,accel=tcg

# GDB = loongarch64-unknown-linux-gnu-gdb
	GDB = loongarch64-linux-gnu-gdb
	GDB_ARGS = Loongarch64
endif


PHONY := all0
all0: $(KERNEL_ELF) $(KERNEL_ASM) $(USER_APPS)


$(KERNEL_ELF): build
$(KERNEL_ASM): $(KERNEL_ELF)
	$(OBJDUMP) $(DISASM_ARGS) $(KERNEL_ELF) > $(KERNEL_ASM)
	@echo "Updated: $(KERNEL_ASM)"


PHONY += build2docker
build2docker:
	docker build -t ${DOCKER_NAME} .


PHONY += docker
docker:
	docker run --privileged --rm -it --network="host" -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} bash


PHONY += kernel
kernel:
	cd kernel && make build


PHONY += build
build: user kernel


PHONY += run
run: build
	$(QEMU) $(QEMU_ARGS)


PHONY += clean
clean:
	cargo clean
	-rm -rf $(TARGET_DIR)/
	-rm -rf fsimg/
	-rm kernel-rv kernel-la


PHONY += disasm
disasm: $(KERNEL_ASM)
	cat $(KERNEL_ASM) | $(PAGER)


PHONY += gdbserver
gdbserver: all0
	$(QEMU) $(QEMU_ARGS) -s -S


PHONY += gdbclient
gdbclient:
	$(GDB) -ex 'file $(KERNEL_ELF)' \
			-ex 'set arch $(GDB_ARGS)' \
			-ex 'target remote localhost:1234'


PHONY += run-docker
run-docker:
	docker run --rm -it --network="host" -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} make run


PHONY += user
user:
	cd user && make build

PHONY += copy-software
copy-software:
	@echo "Copying software directories into testcase..."
	-rm -rf ./testcase/loongarch64 ./testcase/riscv64
	-mkdir -p ./testcase
	-cp -r ../software/loongarch64 ./testcase/
	-cp -r ../software/riscv64 ./testcase/
	@echo "Finished copying software."

PHONY += fs-img
fs-img: user copy-software
	@echo "building fs-img ext4..."
	@echo $(FS_IMG)
	rm -rf $(FS_IMG)
	mkdir -p $(FS_IMG_DIR)
	dd if=/dev/zero of=$(FS_IMG) bs=1K count=524288 status=progress
	mkfs.ext4 -F $(FS_IMG)
	mkdir -p emnt
	sudo mount -t ext4 -o loop $(FS_IMG) emnt
	sudo cp -r $(USER_ELFS) emnt/

	-sudo cp -r testcase/$(ARCH)/$(TESTCASE_LIBC)/basic/* emnt/
	-sudo cp -r testcase/$(ARCH)/$(TESTCASE_LIBC)/busybox/* emnt/
	-sudo cp -r testcase/$(ARCH)/$(TESTCASE_LIBC)/lua/* emnt/
	-sudo cp -r testcase/$(ARCH)/$(TESTCASE_LIBC)/libc-test/* emnt/
	-sudo cp -r testcase/$(ARCH)/$(TESTCASE_LIBC)/iozone/* emnt/
	-sudo cp -r testcase/$(ARCH)/$(TESTCASE_LIBC)/iperf/* emnt/
	-sudo cp -r testcase/$(ARCH)/$(TESTCASE_LIBC)/netperf/* emnt/
	-sudo cp -r testcase/$(ARCH)/$(TESTCASE_LIBC)/libcbench/* emnt/
	-sudo cp -r testcase/$(ARCH)/$(TESTCASE_LIBC)/lmbench/* emnt/
	-sudo cp -r testcase/$(ARCH)/$(TESTCASE_LIBC)/vim/* emnt/
	-sudo cp -r testcase/$(ARCH)/$(TESTCASE_LIBC)/git/* emnt/

	sudo cp -r img-data/common/* emnt/
	sudo cp -r img-data/$(ARCH)/* emnt/
	sudo chmod -R 755 emnt/
	sudo umount emnt
	sudo rm -rf emnt
	@echo "building fs-img finished"
	@echo "Attention: cp error may be ignored"


PHONY += fs-img-submit-rv
fs-img-submit-rv:
	@echo "building fs-img-submit-rv ext4..."
	make fs-img-submit ARCH=riscv64
	@echo "building fs-img-submit-rv finished"

PHONY += fs-img-submit-la
fs-img-submit-la:
	@echo "building fs-img-submit-la ext4..."
	make fs-img-submit ARCH=loongarch64
	@echo "building fs-img-submit-la finished"


PHONY += fs-img-submit
fs-img-submit: user copy-software
	@echo $(FS_IMG)
	rm -rf $(FS_IMG)
	mkdir -p $(FS_IMG_DIR)
	dd if=/dev/zero of=$(FS_IMG) bs=1K count=524288 status=progress
	mkfs.ext4 -F $(FS_IMG)
	mkdir -p emnt
	mount -t ext4 -o loop $(FS_IMG) emnt
	cp -r $(USER_ELFS) emnt/
	cp -r img-data/common/* emnt/
	cp -r img-data/$(ARCH)/* emnt/
	chmod -R 755 emnt/
	umount emnt
	rm -rf emnt
	@echo "Attention: cp error may be ignored"


PHONY += all
all:
	-rm -r .cargo
	-mkdir .cargo
	cp submit/config.toml .cargo/

	-rm -rf vendor
	tar xf submit/vendor.tar.gz

	make build ARCH=riscv64 LOG= MODE=release
	cp target/riscv64gc-unknown-none-elf/release/kernel kernel-rv
# make fs-img-submit-rv
# cp fsimg/riscv64-sdcard.img disk-rv.img

	-rm -rf vendor/
	tar xf submit/vendor.tar.gz

	make build ARCH=loongarch64 LOG= MODE=release
	cp target/loongarch64-unknown-none/release/kernel kernel-la
# make fs-img-submit-la
# cp fsimg/loongarch64-sdcard.img disk-la.img


PHONY += rkernel
rkernel: rkernel-run


PHONY += rkernel-build
rkernel-build:
	make kernel-build ARCH=riscv64
	cp $(KERNEL_ELF) kernel-rv


PHONY += rkernel-run
rkernel-run:
	make rkernel-run-wrapped ARCH=riscv64


PHONY += rkernel-run-wrapped
rkernel-run-wrapped: rkernel-build
	$(QEMU) -machine virt -kernel $(KERNEL_ELF) -m 1G -nographic -smp 1 -bios default -drive file=sdcard-rv.img,if=none,format=raw,id=x0 \
                    -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 -no-reboot -device virtio-net-device,netdev=net -netdev user,id=net \
                    -rtc base=utc 
# -drive file=disk-rv.img,if=none,format=raw,id=x1 -device virtio-blk-device,drive=x1,bus=virtio-mmio-bus.1


PHONY += lkernel
lkernel: lkernel-run


PHONY += lkernel-build
lkernel-build:
	make kernel-build ARCH=loongarch64
	cp $(KERNEL_ELF) kernel-la


PHONY += lkernel-run
lkernel-run:
	make lkernel-run-wrapped ARCH=loongarch64


PHONY += lkernel-run-wrapped
lkernel-run-wrapped: lkernel-build
	$(QEMU) -kernel $(KERNEL_ELF) -m 1G -nographic -smp 1 -drive file=sdcard-la.img,if=none,format=raw,id=x0 \
                        -device virtio-blk-pci,drive=x0 -no-reboot  -device virtio-net-pci,netdev=net0 \
                        -netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555 \
                        -rtc base=utc 
# -drive file=disk-la.img,if=none,format=raw,id=x1 -device virtio-blk-pci,drive=x1


PHONY += kernel-build
kernel-build:
	make user kernel


PHONY += rkernel-debug
rkernel-debug:
	make rkernel-debug-wrapped ARCH=riscv64


PHONY += rkernel-debug-wrapped
rkernel-debug-wrapped: rkernel-build
	$(QEMU) -machine virt -kernel $(KERNEL_ELF) -m 1G -nographic -smp 1 -bios default -drive file=sdcard-rv.img,if=none,format=raw,id=x0 \
                    -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 -no-reboot -device virtio-net-device,netdev=net -netdev user,id=net \
                    -rtc base=utc -s -S
# -drive file=disk-rv.img,if=none,format=raw,id=x1 -device virtio-blk-device,drive=x1,bus=virtio-mmio-bus.1


PHONY += lkernel-debug
lkernel-debug:
	make lkernel-debug-wrapped ARCH=loongarch64


PHONY += lkernel-debug-wrapped
lkernel-debug-wrapped: lkernel-build
	$(QEMU) -kernel $(KERNEL_ELF) -m 1G -nographic -smp 1 -drive file=sdcard-la.img,if=none,format=raw,id=x0 \
                        -device virtio-blk-pci,drive=x0 -no-reboot  -device virtio-net-pci,netdev=net0 \
                        -netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555 \
                        -rtc base=utc -s -S
# -drive file=disk-la.img,if=none,format=raw,id=x1 -device virtio-blk-pci,drive=x1

PHONY += unzip
unzip:
	-rm -rf /vendor
	tar xf submit/vendor.tar.gz


.PHONY: $(PHONY)

