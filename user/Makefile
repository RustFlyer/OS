# Building variables
FEATURES :=


CARGO_BUILD_ARGS :=
ifeq ($(MODE), release)
	CARGO_BUILD_ARGS += --release
endif
ifneq ($(FEATURES), )
	CARGO_BUILD_ARGS += --features "$(FEATURES)"
endif

# Phony targets
PHONY :=

PHONY += build
build:
	cargo build $(CARGO_BUILD_ARGS) --offline

.PHONY: build
