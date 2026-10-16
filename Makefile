CYAN    := \033[36m
BOLD    := \033[1m
RESET   := \033[0m

# trunk needs a toolchain that has the wasm32 target; rustup's goes first.
export PATH := $(HOME)/.cargo/bin:$(PATH)

# Set this to the repository name when publishing to GitHub Pages.
PUBLIC_URL   ?= /
PORT         ?= 8088
PREVIEW_PORT ?= 8089
# 0.0.0.0 to reach the preview from a phone on the same network.
ADDRESS      ?= 127.0.0.1
FIGURES      ?= figures

.PHONY: help dev preview build test lint fmt update figures vectors clean

help: ## Show this help
	@printf "$(BOLD)cube$(RESET) -- a Rubik's cube in 3D and as a net, drawn in SVG\n"
	@awk 'BEGIN {FS = ":.*?## "} \
	    /^# ── / { gsub(/^# ── | ─*$$/, ""); printf "\n$(BOLD)%s$(RESET)\n", $$0; next } \
	    /^[a-zA-Z0-9_-]+:.*?## / { printf "  $(CYAN)%-16s$(RESET) %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@printf "\n"

# ── Develop ──────────────────────────────────────────────────────────────
dev: ## Serve with live reload, rebuilding on every change
	trunk serve --address $(ADDRESS) --port $(PORT)

preview: ## Serve the release build the way GitHub Pages will
	@printf "$(BOLD)http://$(ADDRESS):$(PREVIEW_PORT)$(PUBLIC_URL)$(RESET)\n"
	trunk serve --release --no-autoreload --public-url "$(PUBLIC_URL)" \
	    --address $(ADDRESS) --port $(PREVIEW_PORT)

build: ## Build the release bundle into dist/ (PUBLIC_URL=/cube/ for Pages)
	trunk build --release --public-url "$(PUBLIC_URL)"
	@du -sh dist

test: ## Run the unit, property and SageMath comparison tests
	cargo test --workspace

lint: ## Clippy and a formatting check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo fmt --all --check

fmt: ## Format everything
	cargo fmt --all

update: ## Update dependencies to their latest compatible versions
	cargo update

# ── Export ───────────────────────────────────────────────────────────────
figures: ## Write sample figures with the command line exporter
	@mkdir -p $(FIGURES)
	cargo run --release -p cube-export -- --mode net --numbers \
	    -o $(FIGURES)/net-numbered.svg
	cargo run --release -p cube-export -- -s "R U R' U'" \
	    -o $(FIGURES)/sexy-move.svg
	cargo run --release -p cube-export -- -s "R U R' U'" --mode strip \
	    -o $(FIGURES)/sexy-move-steps.svg
	cargo run --release -p cube-export -- -s "R U R' U'" --mode anim --steps 8 \
	    -o $(FIGURES)/sexy-move-anim.svg
	cargo run --release -p cube-export -- -s "R U R' U'" --mode player \
	    -o $(FIGURES)/sexy-move.html
	@ls -lh $(FIGURES)

vectors: ## Regenerate the SageMath ground truth (needs sage on PATH)
	sage docs/gen_vectors.sage | python3 docs/vectors_to_rust.py - \
	    crates/cube-core/tests/data/sage_data.rs

# ── Housekeeping ─────────────────────────────────────────────────────────
clean: ## Remove build output
	cargo clean
	rm -rf dist $(FIGURES)
