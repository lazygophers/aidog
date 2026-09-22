PRODUCT_NAME := aidog
APP_NAME     := AiDog
TAURI_DIR    := src-tauri
INSTALL_DIR  := /Applications
APP_BUNDLE   := $(TAURI_DIR)/target/release/bundle/macos/$(APP_NAME).app
INSTALLED    := $(INSTALL_DIR)/$(APP_NAME).app

# Flutter 壳：与 Tauri 壳并存期用不同的安装名。两者的 bundle id 都是 com.aidog.desktop
# （Flutter 壳最终要顶替 Tauri 壳，所以身份故意相同），目录名不错开就会互相覆盖。
FLUTTER_DIR      := flutter
FLUTTER_APP_NAME := $(APP_NAME)-Flutter
FLUTTER_BUNDLE   := $(FLUTTER_DIR)/build/macos/Build/Products/Release/$(APP_NAME).app
FLUTTER_INSTALLED := $(INSTALL_DIR)/$(FLUTTER_APP_NAME).app

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

.PHONY: run-flutter
run-flutter: ## Start Flutter dev shell; lib/**.dart 存盘即自动热重载
	@printf "$(GREEN)▶ Starting Flutter dev shell (存盘自动热重载)...$(RESET)\n"
	node scripts/flutter-dev.mjs

.PHONY: build
build: ## Build frontend (tsc && vite build)
	@printf "$(CYAN)▶ Building frontend...$(RESET)\n"
	yarn build

.PHONY: build-flutter
build-flutter: ## Build Flutter app (debug) —— 编译期自检，比 release 快得多
	@printf "$(CYAN)▶ Building Flutter app (debug)...$(RESET)\n"
	cd $(FLUTTER_DIR) && flutter build macos --debug
	@printf "$(CYAN)✔ App → $(FLUTTER_DIR)/build/macos/Build/Products/Debug/$(APP_NAME).app$(RESET)\n"

.PHONY: build-all
build-all: build build-flutter ## 两个壳都编一遍（并存期用）

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

.PHONY: release-flutter
release-flutter: ## Build Flutter release app + 可分发 zip → $(FLUTTER_BUNDLE)
	@printf "$(GREEN)▶ Building Flutter release app...$(RESET)\n"
	cd $(FLUTTER_DIR) && flutter build macos --release
	@test -d "$(FLUTTER_BUNDLE)" || { printf "$(BOLD)❌ build 产物缺失: $(FLUTTER_BUNDLE)$(RESET)\n"; exit 1; }
	@# 与 release.yml 的打包方式**逐字一致**：Sparkle 只认 ditto 打的 zip
	@# （`--sequesterRsrc --keepParent`），换成 zip(1) 会丢资源分叉导致校验失败。
	@# 本地出包与 CI 出包不一致的话，本地验过的东西不代表线上那份。
	@cd $(FLUTTER_DIR)/build/macos/Build/Products/Release && \
		ditto -c -k --sequesterRsrc --keepParent $(APP_NAME).app \
		"$(APP_NAME)-Flutter-v$$(tr -d '[:space:]' < $(CURDIR)/.version)-macos.zip"
	@printf "$(GREEN)✔ App → $(FLUTTER_BUNDLE)$(RESET)\n"
	@printf "$(GREEN)✔ Zip → $(FLUTTER_DIR)/build/macos/Build/Products/Release/$(APP_NAME)-Flutter-v$$(tr -d '[:space:]' < .version)-macos.zip$(RESET)\n"

##@ Maintenance

.PHONY: check
check: ## Run TypeScript + Rust type checks
	@printf "$(CYAN)▶ TypeScript check...$(RESET)\n"
	npx tsc --noEmit
	@printf "$(CYAN)▶ Rust check...$(RESET)\n"
	cd $(TAURI_DIR) && cargo check --workspace --all-targets

.PHONY: lint
lint: ## Run linters
	@printf "$(CYAN)▶ Registry schema check...$(RESET)\n"
	yarn check:registry
	@printf "$(CYAN)▶ Modal centering check...$(RESET)\n"
	yarn check:modal
	@printf "$(CYAN)▶ Design token check...$(RESET)\n"
	yarn check:tokens
	@printf "$(CYAN)▶ Flutter settings schema check...$(RESET)\n"
	yarn check:flutter-schema
	@printf "$(CYAN)▶ Flutter icon drift check...$(RESET)\n"
	node scripts/gen-flutter-icons.mjs --check
	@printf "$(CYAN)▶ Flutter underline check...$(RESET)\n"
	node scripts/check-flutter-underline.mjs
	@printf "$(CYAN)▶ UI parity check（React 的文案 key 在 Flutter 侧必须都有）...$(RESET)\n"
	node scripts/check-ui-parity.mjs
	@printf "$(CYAN)▶ Flutter analyze...$(RESET)\n"
	cd $(FLUTTER_DIR) && flutter analyze
	@printf "$(CYAN)▶ Linting...$(RESET)\n"
	cd $(TAURI_DIR) && cargo clippy --workspace --all-targets -- -D warnings

.PHONY: test-flutter
test-flutter: ## Run the Flutter test suite (单独一条：全量要 10 分钟以上，不塞进 lint)
	@printf "$(CYAN)▶ Flutter tests...$(RESET)\n"
	cd $(FLUTTER_DIR) && flutter test

.PHONY: clean
clean: ## Remove build artifacts
	@printf "$(CYAN)▶ Cleaning...$(RESET)\n"
	rm -rf dist
	cd $(TAURI_DIR) && cargo clean

.PHONY: deps
deps: ## Install frontend dependencies
	yarn install

.PHONY: version-bump
version-bump: ## Auto bump patch version and sync manifests; override: VERSION=0.1.13
	@if [ -n "$(VERSION)" ]; then \
		node scripts/sync-version.mjs --set "$(VERSION)"; \
	else \
		node scripts/sync-version.mjs --bump; \
	fi

.PHONY: version-check
version-check: ## Verify manifests match .version
	node scripts/sync-version.mjs --check

.PHONY: install
install: ## Release build + 安装 AiDog.app 到 /Applications (自动 kill 运行中实例)
	@printf "$(GREEN)▶ Building release installer ($(PRODUCT_NAME))…$(RESET)\n"
	yarn tauri build --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'
	@test -d "$(APP_BUNDLE)" || { printf "$(BOLD)❌ build 产物缺失: $(APP_BUNDLE)$(RESET)\n"; exit 1; }
	@printf "$(GREEN)▶ 安装 → $(INSTALLED)$(RESET)\n"
	@rm -rf "$(INSTALLED)"
	@cp -R "$(APP_BUNDLE)" "$(INSTALL_DIR)/"
	@printf "$(CYAN)▶ 检测运行中的 $(APP_NAME)…$(RESET)\n"
	@pkill -f "$(APP_NAME).app/Contents/MacOS/" 2>/dev/null \
		&& { printf "$(GREEN)✔ 已终止运行中实例，重启以加载新版本…$(RESET)\n"; sleep 1; } \
		|| printf "$(GREEN)✔ 无运行中实例，跳过$(RESET)\n"
	@printf "$(GREEN)✔ 已安装: $(INSTALLED)$(RESET)\n"
	@open "$(INSTALLED)"

.PHONY: install-flutter
install-flutter: ## Flutter release build + 安装到 /Applications/$(FLUTTER_APP_NAME).app
	@printf "$(GREEN)▶ Building Flutter release app…$(RESET)\n"
	cd $(FLUTTER_DIR) && flutter build macos --release
	@test -d "$(FLUTTER_BUNDLE)" || { printf "$(BOLD)❌ build 产物缺失: $(FLUTTER_BUNDLE)$(RESET)\n"; exit 1; }
	@printf "$(GREEN)▶ 安装 → $(FLUTTER_INSTALLED)$(RESET)\n"
	@rm -rf "$(FLUTTER_INSTALLED)"
	@cp -R "$(FLUTTER_BUNDLE)" "$(FLUTTER_INSTALLED)"
	@printf "$(CYAN)▶ 检测运行中的 $(FLUTTER_APP_NAME)…$(RESET)\n"
	@# 模式带 -Flutter，匹配不到 Tauri 壳的 AiDog.app/Contents/MacOS/ —— 不碰另一个版本。
	@pkill -f "$(FLUTTER_APP_NAME).app/Contents/MacOS/" 2>/dev/null \
		&& { printf "$(GREEN)✔ 已终止运行中实例，重启以加载新版本…$(RESET)\n"; sleep 1; } \
		|| printf "$(GREEN)✔ 无运行中实例，跳过$(RESET)\n"
	@printf "$(GREEN)✔ 已安装: $(FLUTTER_INSTALLED)$(RESET)\n"
	@open "$(FLUTTER_INSTALLED)"

.PHONY: uninstall-flutter
uninstall-flutter: ## 从 /Applications 移除 $(FLUTTER_APP_NAME).app
	@test -e "$(FLUTTER_INSTALLED)" || { printf "$(GREEN)ℹ️  未安装: $(FLUTTER_INSTALLED)$(RESET)\n"; exit 0; }
	@rm -rf "$(FLUTTER_INSTALLED)"
	@printf "$(GREEN)🗑  已移除: $(FLUTTER_INSTALLED)$(RESET)\n"

.PHONY: uninstall
uninstall: ## 从 /Applications 移除 AiDog.app
	@test -e "$(INSTALLED)" || { printf "$(GREEN)ℹ️  未安装: $(INSTALLED)$(RESET)\n"; exit 0; }
	@rm -rf "$(INSTALLED)"
	@printf "$(GREEN)🗑  已移除: $(INSTALLED)$(RESET)\n"

##@ Help

.PHONY: help
help: ## Show this help
	@printf "$(BOLD)$(PRODUCT_NAME) - Available Commands$(RESET)\n\n"
	@awk 'BEGIN {FS = ":.*##"; printf ""} /^[a-zA-Z_-]+:.*?##/ { printf "  $(GREEN)%-16s$(RESET) %s\n", $$1, $$2 } /^##@/ { printf "\n$(BOLD)  %s$(RESET)\n", substr($$0, 5) } ' $(MAKEFILE_LIST)
