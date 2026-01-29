GO ?= go
BIN ?= jv
CMD ?= ./cmd/jv
PKG ?= ./...
CGO_ENABLED ?= 0

.PHONY: build run test fmt tidy clean

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
