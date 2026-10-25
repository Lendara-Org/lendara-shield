-include .env
export

API_DIR := apps/api

# Prints a notice for targets whose code lands in a later PR of the plan.
define pending
	@echo "make $@: not available yet (arrives in $(1), see CLAUDE.md section 11)"
endef

.PHONY: help install dev dev-api dev-web db-up db-down migrate setup-testnet \
	fmt lint typecheck test test-testnet coverage codegen build

help: ## List available targets
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "}; {printf "  %-15s %s\n", $$1, $$2}'

install: ## Install JS dependencies and git hooks
	pnpm install

db-up: ## Start local Postgres and wait until healthy
	docker compose up -d --wait postgres

db-down: ## Stop local Postgres (data volume is kept)
	docker compose down

dev: ## Run API and web together
	$(call pending,H2 and H7)

dev-api: ## Run the API
	$(call pending,H2)

dev-web: ## Run the web app
	$(call pending,H7)

migrate: ## Apply database migrations
	$(call pending,H2)

setup-testnet: ## Create and fund testnet accounts, deploy the pool (idempotent)
	$(call pending,H4)

fmt: ## Format all code
	$(call pending,H2)

lint: ## Lint all code
	pnpm lint

typecheck: ## Type-check TS packages
	pnpm typecheck

test: ## Run all tests except testnet ones
	pnpm test

test-testnet: ## Run end-to-end tests against Stellar testnet (needs .env)
	$(call pending,H4)

coverage: ## Coverage report for the API
	$(call pending,H2)

codegen: ## Generate TS types from openapi.yaml
	$(call pending,H3)

build: ## Build everything
	pnpm build
