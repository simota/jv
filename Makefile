GO ?= go
BIN ?= jv
CMD ?= ./cmd/jv
PKG ?= ./...

.PHONY: build run test fmt tidy clean

build:
	$(GO) build -o $(BIN) $(CMD)

run:
	$(GO) run $(CMD)

test:
	$(GO) test $(PKG)

fmt:
	$(GO) fmt $(PKG)

tidy:
	$(GO) mod tidy

clean:
	rm -f $(BIN)
