ifeq ($(MODE), release)
	MODE_ARGS = --release
endif

ifeq ($(ARCH), riscv64)
	CARGO_ARGS = --target riscv64gc-unknown-none-elf
else ifeq ($(ARCH), loongarch64)
	CARGO_ARGS = --target loongarch64-unknown-none
else
	$(error Unsupported architecture: $(ARCH). Supported architectures are riscv64 and loongarch64.)
endif

PHONY += build
build:
	touch src/main.rs
	cargo build $(MODE_ARGS) $(CARGO_ARGS) --offline

.PHONY: $(PHONY)
