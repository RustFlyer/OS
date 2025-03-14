ifeq ($(MODE), release)
	MODE_ARG = --release
endif

ifeq ($(DEBUG), on)
	DEBUG_ARG = --cfg ondebug
endif

RUSTFLAGS := -Ccode-model=large
RUSTFLAGS := $(RUSTFLAGS) $(DEBUG_ARG)

PHONY := all
all: build

PHONY += env
env:
	@(cargo install --list | grep "cargo-binutils" > /dev/null 2>&1) || cargo install cargo-binutils

PHONY += build
build: env
	@RUSTFLAGS="$(RUSTFLAGS)" cargo build $(MODE_ARG) 

.PHONY: $(PHONY)
