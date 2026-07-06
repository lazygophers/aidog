PRODUCT_NAME := aidog
TAURI_DIR    := src-tauri

BOLD  := \033[1m
CYAN  := \033[36m
GREEN := \033[32m
RESET := \033[0m

# 签名私钥 fallback: env 未设则读 ~/.tauri/aidog.key 内容 (updater artifact 签名所需)
TAURI_SIGNING_PRIVATE_KEY ?= $(shell cat $(HOME)/.tauri/aidog.key 2>/dev/null)
export TAURI_SIGNING_PRIVATE_KEY

##@ Build

.PHONY: run
run: ## Start dev server with hot reload (frontend + Rust HMR)
	@printf "$(GREEN)▶ Starting Tauri dev server...$(RESET)\n"
	yarn tauri dev

.PHONY: build
build: ## Build frontend (tsc && vite build)
	@printf "$(CYAN)▶ Building frontend...$(RESET)\n"
	yarn build

.PHONY: release
release: ## Build local installer for current platform → $(TAURI_DIR)/target/release/bundle/
	@printf "$(GREEN)▶ Building release installer ($(PRODUCT_NAME))...$(RESET)\n"
	yarn tauri build
	@printf "$(GREEN)✔ Bundles → $(TAURI_DIR)/target/release/bundle/$(RESET)\n"

.PHONY: release-debug
release-debug: ## Build installer with debug symbols (faster, larger)
	@printf "$(GREEN)▶ Building debug installer ($(PRODUCT_NAME))...$(RESET)\n"
	yarn tauri build --debug
	@printf "$(GREEN)✔ Bundles → $(TAURI_DIR)/target/debug/bundle/$(RESET)\n"

##@ Maintenance

.PHONY: check
check: ## Run TypeScript + Rust type checks
	@printf "$(CYAN)▶ TypeScript check...$(RESET)\n"
	npx tsc --noEmit
	@printf "$(CYAN)▶ Rust check...$(RESET)\n"
	cd $(TAURI_DIR) && cargo check

.PHONY: lint
lint: ## Run linters
	@printf "$(CYAN)▶ Linting...$(RESET)\n"
	cd $(TAURI_DIR) && cargo clippy -- -D warnings

.PHONY: clean
clean: ## Remove build artifacts
	@printf "$(CYAN)▶ Cleaning...$(RESET)\n"
	rm -rf dist
	cd $(TAURI_DIR) && cargo clean

.PHONY: install
install: ## Install frontend dependencies
	yarn install

##@ Pricing

.PHONY: prices-sync
prices-sync: ## Sync model prices/max_tokens → src-tauri/defaults/models.json (single entry, runs all platform scrapers)
	@printf "$(GREEN)▶ Aggregating model pricing → src-tauri/defaults/models.json...$(RESET)\n"
	cd scripts/pricing && uv run python aggregate.py

##@ Help

.PHONY: help
help: ## Show this help
	@printf "$(BOLD)$(PRODUCT_NAME) - Available Commands$(RESET)\n\n"
	@awk 'BEGIN {FS = ":.*##"; printf ""} /^[a-zA-Z_-]+:.*?##/ { printf "  $(GREEN)%-16s$(RESET) %s\n", $$1, $$2 } /^##@/ { printf "\n$(BOLD)  %s$(RESET)\n", substr($$0, 5) } ' $(MAKEFILE_LIST)
