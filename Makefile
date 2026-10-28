.DEFAULT_GOAL := help
SHELL := /bin/bash

DATA_DIR := wow-data
DATA     := $(DATA_DIR)/Data
WASM     := web/dist/wenilla_bg.wasm

.PHONY: help build start native lobby fetch-data fetch-legion dist toolchain wasm clean

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

fetch-data: ## Download the game data (a WoW 1.12.1 client) from archive.org
	./scripts/fetch-data.sh

fetch-legion: ## Download Legion 7.3.5's game files (71 GB, checked); STREAM=1 reads archive.org as needed instead
	./scripts/fetch-legion.sh $(if $(STREAM),--stream,)

toolchain: ## Install the wasm toolchain (wasi-sdk, wasm-bindgen, binaryen)
	./scripts/web-setup.sh

wasm: ## Build the browser client into web/dist/
	./scripts/web-build.sh

build: fetch-data toolchain wasm ## Fetch data + install toolchain + build the wasm

start: ## Serve the game at http://127.0.0.1:8090 (run `make build` first)
	@test -f $(WASM) || { echo "  not built yet — run: make build"; exit 1; }
	cargo run --release -p wenilla-host -- --www web/dist --data $(DATA)

lobby: ## Run a standalone multiplayer lobby server on :7777 (no game data needed)
	cargo run --release -p survivors-server -- --bind 0.0.0.0:7777

native: fetch-data ## Build + run the native desktop version (with Legion's data if fetched)
	WOW_DATA=$(DATA) cargo run --release -p warcraft-survivors

dist: ## Build the native game for players into dist/azeroth-survivors/ (no game data)
	./scripts/dist.sh

clean: ## Remove the fetched game data and wasm build output
	rm -rf web/dist $(DATA_DIR)
