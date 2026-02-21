SHELL := /bin/bash

ROOT_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
PLATFORM_DIR := $(ROOT_DIR)/platform
UI_DIR := $(PLATFORM_DIR)/apps/skillssync-desktop/ui
TAURI_DIR := $(PLATFORM_DIR)/apps/skillssync-desktop/src-tauri

.PHONY: all build run app lint lint-fix lint-rust lint-fix-rust lint-ui lint-fix-ui lint-workflows test test-rust release

all: app

app: run

build:
	if ! cargo tauri --help >/dev/null 2>&1; then \
		echo "cargo-tauri is not installed. Install it with: cargo install tauri-cli" >&2; \
		exit 1; \
	fi
	if [[ ! -d "$(UI_DIR)/node_modules" ]]; then \
		echo "Installing UI dependencies..."; \
		(cd "$(UI_DIR)" && npm install); \
	fi
	cd "$(TAURI_DIR)" && cargo tauri build --debug

run:
	"$(ROOT_DIR)/scripts/run-tauri-gui.sh"

lint: lint-rust lint-ui

lint-fix: lint-fix-rust lint-fix-ui lint

lint-rust:
	cd "$(PLATFORM_DIR)" && cargo fmt --all --check
	mkdir -p "$(UI_DIR)/dist"
	cd "$(PLATFORM_DIR)" && cargo clippy --workspace --all-targets -- -D warnings

lint-fix-rust:
	cd "$(PLATFORM_DIR)" && cargo fmt --all

lint-ui:
	cd "$(UI_DIR)" && npm run lint

lint-fix-ui:
	cd "$(UI_DIR)" && npm run lint:fix

lint-workflows:
	@if ! command -v actionlint >/dev/null 2>&1; then \
		echo "actionlint is required. Install from https://github.com/rhysd/actionlint"; \
		exit 1; \
	fi
	@if ! command -v yamllint >/dev/null 2>&1; then \
		echo "yamllint is required. Install with: pip install yamllint"; \
		exit 1; \
	fi
	actionlint
	yamllint -c .yamllint.yml .github/workflows

test: test-rust

test-rust:
	mkdir -p "$(UI_DIR)/dist"
	cd "$(PLATFORM_DIR)" && cargo test --workspace

release:
ifndef VERSION
	$(error VERSION is required. Usage: make release VERSION=0.2.0)
endif
	@if ! echo "$(VERSION)" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$$'; then \
		echo "Error: VERSION must be semver (e.g. 0.2.0)" >&2; exit 1; \
	fi
	@if ! git diff --quiet || ! git diff --cached --quiet; then \
		echo "Error: working tree is not clean. Commit or stash changes first." >&2; exit 1; \
	fi
	@if git rev-parse "v$(VERSION)" >/dev/null 2>&1; then \
		echo "Error: tag v$(VERSION) already exists." >&2; exit 1; \
	fi
	@echo "Bumping version to $(VERSION)..."
	sed -i '' 's/^version = ".*"/version = "$(VERSION)"/' "$(PLATFORM_DIR)/Cargo.toml"
	sed -i '' 's/"version": ".*"/"version": "$(VERSION)"/' "$(TAURI_DIR)/tauri.conf.json"
	sed -i '' 's/"version": ".*"/"version": "$(VERSION)"/' "$(UI_DIR)/package.json"
	cd "$(PLATFORM_DIR)" && cargo update --workspace
	git add "$(PLATFORM_DIR)/Cargo.toml" "$(PLATFORM_DIR)/Cargo.lock" \
		"$(TAURI_DIR)/tauri.conf.json" "$(UI_DIR)/package.json"
	git commit -m "release: v$(VERSION)"
	git tag "v$(VERSION)"
	git push
	git push origin "v$(VERSION)"
	@echo "Released v$(VERSION)"
