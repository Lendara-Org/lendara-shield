-include .env
export

API_DIR := apps/api
POOL_CLI_DIR := apps/pool-cli

# Prints a notice for targets whose code lands in a later PR of the plan.
define pending
	@echo "make $@: not available yet (arrives in $(1), see CLAUDE.md section 11)"
endef

.PHONY: help install dev dev-api dev-web stage-web-sdk db-up db-down migrate setup-testnet \
	fmt fmt-check lint lint-api lint-pool-cli lint-js lint-openapi typecheck test test-api test-pool-cli \
	test-js test-web-e2e test-testnet coverage deny codegen build build-api build-pool-cli build-js

help: ## List available targets
	@grep -hE '^[a-z0-9-]+:.*## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "}; {printf "  %-15s %s\n", $$1, $$2}'

install: ## Install JS dependencies and git hooks
	pnpm install

db-up: ## Start local Postgres and wait until healthy
	docker compose up -d --wait postgres

db-down: ## Stop local Postgres (data volume is kept)
	docker compose down

dev: ## Run API and web together
	$(MAKE) -j2 dev-api dev-web

dev-api: ## Run the API (applies pending migrations first)
	cd $(API_DIR) && cargo run

dev-web: stage-web-sdk ## Run the web app (http://localhost:3000)
	pnpm --filter @lendara/shield-web dev

stage-web-sdk: ## Serve the pool SDK and this deployment to the browser (apps/web/public/spp)
	scripts/stage-web-sdk.sh

migrate: ## Apply database migrations and exit
	cd $(API_DIR) && cargo run --quiet -- migrate

setup-testnet: ## Create and fund testnet accounts, deploy the pool (idempotent)
	scripts/setup-testnet.sh

fmt: ## Format all code
	cd $(API_DIR) && cargo fmt
	cd $(POOL_CLI_DIR) && cargo fmt

fmt-check: ## Fail if any code is not formatted
	cd $(API_DIR) && cargo fmt --check
	cd $(POOL_CLI_DIR) && cargo fmt --check

lint: lint-api lint-pool-cli lint-js lint-openapi ## Lint all code

lint-api: ## Clippy with warnings as errors
	cd $(API_DIR) && cargo clippy --all-targets --all-features -- -D warnings

lint-pool-cli: ## Clippy on the pool CLI with warnings as errors
	cd $(POOL_CLI_DIR) && cargo clippy --all-targets -- -D warnings

lint-js: ## Lint TS packages
	pnpm lint

lint-openapi: ## Validate openapi.yaml (the API contract)
	pnpm exec redocly lint openapi.yaml

typecheck: ## Type-check TS packages
	pnpm typecheck

test: test-api test-pool-cli test-js ## Run all tests except testnet ones (needs `make db-up`)

test-api: ## API tests against local Postgres (DATABASE_URL)
	cd $(API_DIR) && cargo test

test-pool-cli: ## Unit tests of the pool CLI
	cd $(POOL_CLI_DIR) && cargo test

test-js: ## Test TS packages
	pnpm test

test-web-e2e: ## Smoke test of the built web app in Chromium (`make build-js` first)
	pnpm --filter @lendara/shield-web test:e2e

test-testnet: ## Check the testnet setup on-chain, read-only (needs .env)
	scripts/verify-testnet.sh
	cd $(API_DIR) && cargo test --test testnet_asp --test testnet_anchor -- --ignored

coverage: ## API coverage summary; `main.rs` is wiring only and excluded
	cd $(API_DIR) && cargo llvm-cov --ignore-filename-regex 'src/main\.rs' --summary-only

deny: ## Licenses, advisories and sources of the Rust dependencies
	cd $(API_DIR) && cargo deny check
	cd $(POOL_CLI_DIR) && cargo deny check

codegen: ## Generate TS types from openapi.yaml into packages/api-types
	pnpm --filter @lendara/shield-api-types codegen

build: build-api build-pool-cli build-js ## Build everything

build-api: ## Release build of the API
	cd $(API_DIR) && cargo build --release --locked

build-pool-cli: ## Release build of the pool CLI (the API runs it)
	cd $(POOL_CLI_DIR) && cargo build --release --locked

build-js: ## Build TS packages
	pnpm build
