GO ?= go
BIN ?= jv
CMD ?= ./cmd/jv
PKG ?= ./...
CGO_ENABLED ?= 0

.PHONY: build run test fmt tidy clean rust-build rust-test rust-difftest

build:
	CGO_ENABLED=$(CGO_ENABLED) $(GO) build -o $(BIN) $(CMD)

run:
	CGO_ENABLED=$(CGO_ENABLED) $(GO) run $(CMD)

test:
	$(GO) test $(PKG)

fmt:
	$(GO) fmt $(PKG)

tidy:
	$(GO) mod tidy

clean:
	rm -f $(BIN)

# --- Rust port (rust/) ---
CARGO ?= cargo
GO_REF_BIN ?= $(CURDIR)/rust/target/jv-go-reference

rust-build:
	cd rust && $(CARGO) build --release

rust-test:
	cd rust && $(CARGO) fmt --all -- --check && $(CARGO) clippy --all-targets -- -D warnings && $(CARGO) test

# Compares the Rust binary with a fresh Go build, byte for byte.
rust-difftest:
	CGO_ENABLED=0 $(GO) build -o $(GO_REF_BIN) $(CMD)
	cd rust && JV_GO_BIN=$(GO_REF_BIN) $(CARGO) test --release --test differential
