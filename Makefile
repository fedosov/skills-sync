SHELL := /bin/bash

ROOT_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
PLATFORM_DIR := $(ROOT_DIR)/platform
UI_DIR := $(PLATFORM_DIR)/apps/skillssync-desktop/ui

.PHONY: lint lint-fix lint-rust lint-fix-rust lint-ui lint-fix-ui

lint: lint-rust lint-ui

lint-fix: lint-fix-rust lint-fix-ui lint

lint-rust:
	cd "$(PLATFORM_DIR)" && cargo fmt --all --check
	cd "$(PLATFORM_DIR)" && cargo clippy --workspace --all-targets -- -D warnings

lint-fix-rust:
	cd "$(PLATFORM_DIR)" && cargo fmt --all

lint-ui:
	cd "$(UI_DIR)" && npm run lint

lint-fix-ui:
	cd "$(UI_DIR)" && npm run lint:fix
