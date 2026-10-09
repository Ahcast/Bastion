GO ?= go
.PHONY: check build test migrate
check:
	$(GO) vet ./...
	$(GO) test -race ./...
build:
	mkdir -p bin
	$(GO) build -trimpath -o bin/server ./cmd/server
	$(GO) build -trimpath -o bin/worker ./cmd/worker
	$(GO) build -trimpath -o bin/admin ./cmd/admin
test:
	$(GO) test -race ./...
migrate:
	$(GO) run ./cmd/admin migrate
