.DEFAULT_GOAL := help

# === Config ===
PRODUCT_NAME := AiDog
TAURI_DIR    := src-tauri

# === Colors ===
CYAN  := \033[36m
GREEN := \033[32m
BOLD  := \033[1m
RESET := \033[0m

##@ Development

.PHONY: run
run: ## Start dev server with hot reload (frontend + Rust HMR)
	@printf "$(GREEN)▶ Starting Tauri dev server...$(RESET)\n"
	cargo tauri dev

.PHONY: dev-frontend
dev-frontend: ## Start only the frontend dev server (Vite HMR)
	@printf "$(GREEN)▶ Starting Vite dev server...$(RESET)\n"
	yarn dev

##@ Build

.PHONY: release
release: ## Build release bundle for current platform
	@printf "$(GREEN)▶ Building $(PRODUCT_NAME) release...$(RESET)\n"
	cargo tauri build
	@printf "$(CYAN)✓ Release artifacts:$(RESET)\n"
	@ls -lh $(TAURI_DIR)/target/release/bundle/ 2>/dev/null || true

.PHONY: release-dmg
release-dmg: ## Build macOS .dmg only
	@printf "$(GREEN)▶ Building macOS DMG...$(RESET)\n"
	cargo tauri build --bundles dmg
	@ls -lh $(TAURI_DIR)/target/release/bundle/dmg/ 2>/dev/null || true

.PHONY: release-msi
release-msi: ## Build Windows .msi only (cross-compile)
	@printf "$(GREEN)▶ Building Windows MSI...$(RESET)\n"
	cargo tauri build --bundles msi --target x86_64-pc-windows-msvc
	@ls -lh $(TAURI_DIR)/target/x86_64-pc-windows-msvc/release/bundle/msi/ 2>/dev/null || true

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

##@ Help

.PHONY: help
help: ## Show this help
	@printf "$(BOLD)$(PRODUCT_NAME) - Available Commands$(RESET)\n\n"
	@awk 'BEGIN {FS = ":.*##"; printf ""} /^[a-zA-Z_-]+:.*?##/ { printf "  $(GREEN)%-16s$(RESET) %s\n", $$1, $$2 } /^##@/ { printf "\n$(BOLD)  %s$(RESET)\n", substr($$0, 5) } ' $(MAKEFILE_LIST)
