PRODUCT_NAME := aidog
APP_NAME     := AiDog
TAURI_DIR    := src-tauri
INSTALL_DIR  := /Applications
APP_BUNDLE   := $(TAURI_DIR)/target/release/bundle/macos/$(APP_NAME).app
INSTALLED    := $(INSTALL_DIR)/$(APP_NAME).app

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

.PHONY: deps
deps: ## Install frontend dependencies
	yarn install

.PHONY: install
install: ## Release build + 安装 AiDog.app 到 /Applications (自动 kill 运行中实例)
	@printf "$(GREEN)▶ Building release installer ($(PRODUCT_NAME))…$(RESET)\n"
	yarn tauri build
	@test -d "$(APP_BUNDLE)" || { printf "$(BOLD)❌ build 产物缺失: $(APP_BUNDLE)$(RESET)\n"; exit 1; }
	@printf "$(GREEN)▶ 安装 → $(INSTALLED)$(RESET)\n"
	@rm -rf "$(INSTALLED)"
	@cp -R "$(APP_BUNDLE)" "$(INSTALL_DIR)/"
	@printf "$(CYAN)▶ 检测运行中的 $(APP_NAME)…$(RESET)\n"
	@pkill -f "$(APP_NAME).app/Contents/MacOS/" 2>/dev/null \
		&& { printf "$(GREEN)✔ 已终止运行中实例，重启以加载新版本…$(RESET)\n"; sleep 1; } \
		|| printf "$(GREEN)✔ 无运行中实例，跳过$(RESET)\n"
	@printf "$(GREEN)✔ 已安装: $(INSTALLED)$(RESET)\n"

.PHONY: uninstall
uninstall: ## 从 /Applications 移除 AiDog.app
	@test -e "$(INSTALLED)" || { printf "$(GREEN)ℹ️  未安装: $(INSTALLED)$(RESET)\n"; exit 0; }
	@rm -rf "$(INSTALLED)"
	@printf "$(GREEN)🗑  已移除: $(INSTALLED)$(RESET)\n"

##@ Docs

.PHONY: presets-view
presets-view: ## Generate interactive HTML from platform-presets.json + models.json and open it
	@printf "$(GREEN)▶ Generating presets HTML...$(RESET)\n"
	@python3 scripts/presets_view/generate.py
	@printf "$(GREEN)▶ Opening...$(RESET)\n"
	@case "$$(uname -s)" in \
		Darwin) open "$(PWD)/.aidoc/presets.html" ;; \
		Linux)  (xdg-open "$(PWD)/.aidoc/presets.html" || echo "open manually: $(PWD)/.aidoc/presets.html") ;; \
		MINGW*|MSYS*|CYGWIN*) start "" "$(PWD)/.aidoc/presets.html" ;; \
		*) echo "unsupported OS, open manually: $(PWD)/.aidoc/presets.html" ;; \
	esac

##@ Help

.PHONY: help
help: ## Show this help
	@printf "$(BOLD)$(PRODUCT_NAME) - Available Commands$(RESET)\n\n"
	@awk 'BEGIN {FS = ":.*##"; printf ""} /^[a-zA-Z_-]+:.*?##/ { printf "  $(GREEN)%-16s$(RESET) %s\n", $$1, $$2 } /^##@/ { printf "\n$(BOLD)  %s$(RESET)\n", substr($$0, 5) } ' $(MAKEFILE_LIST)
