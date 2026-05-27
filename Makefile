.PHONY: build install-local uninstall-local clean lint test fmt fmt-check ci eval

BINARY            := bin/no-comment-hook
TARGET            := target/release/no-comment-hook
STALE_PLUGIN_LINK := $(HOME)/.claude/plugins/no-comment-hook
HERE              := $(CURDIR)

build: $(BINARY)

$(BINARY): $(TARGET)
	@mkdir -p bin
	cp $(TARGET) $(BINARY)

$(TARGET): $(shell find src -name '*.rs') Cargo.toml
	cargo build --release

install-local: build
	python3 scripts/install-hooks.py install
	@if [ -L "$(STALE_PLUGIN_LINK)" ]; then \
		unlink "$(STALE_PLUGIN_LINK)"; \
		echo "Removed stale plugin symlink: $(STALE_PLUGIN_LINK)"; \
	fi

uninstall-local:
	python3 scripts/install-hooks.py uninstall
	@if [ -L "$(STALE_PLUGIN_LINK)" ]; then \
		unlink "$(STALE_PLUGIN_LINK)"; \
		echo "Removed stale plugin symlink: $(STALE_PLUGIN_LINK)"; \
	fi

clean:
	rm -rf bin target

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo fmt --all -- --check
	cargo clippy --all-targets -- -D warnings

test:
	cargo test

eval: build
	python3 eval/run.py

ci: fmt-check lint test
