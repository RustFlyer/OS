ifeq ($(MODE), release)
	MODE_ARG = --release
endif

ifeq ($(DEBUG), on)
	DEBUG_ARG = --cfg ondebug
endif

PHONY := all
all: build

PHONY += env
env:
	@(cargo install --list | grep "cargo-binutils" > /dev/null 2>&1) || cargo install cargo-binutils

PHONY += build
build: env
	@RUSTFLAGS="$(DEBUG_ARG)" cargo build $(MODE_ARG) 

.PHONY: $(PHONY)
