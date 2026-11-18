# swimflow developer Makefile.
#
# Common workflows:
#   make debug               build target/debug/swimflow
#   make release             build target/release/swimflow
#   make install             build and copy the binary to $HOME/bin
#   make examples            render every example to SVG
#   make examples TYPE=html  render every example to an HTML bundle
#   make clean               remove generated example outputs
#   make clean-all           remove generated outputs and target/

CARGO ?= cargo
TYPE ?= svg
BINDIR ?= $(HOME)/bin
ENV_FILE ?=
RELEASE_BIN := $(CURDIR)/target/release/swimflow

.DEFAULT_GOAL := help
.PHONY: help all debug release install serve examples clean clean-all

help:
	@printf '%s\n' \
		'swimflow build targets:' \
		'  make debug               Build the debug binary (target/debug/swimflow)' \
		'  make release             Build the optimized binary (target/release/swimflow)' \
		'  make install             Build and copy the binary to $$HOME/bin' \
		'                           (override the directory with BINDIR=...)' \
		'  make serve               Run the live-preview HTTP server (reads .env)' \
		'                           (point at another file with ENV_FILE=path)' \
		'  make examples            Render every example to SVG' \
		'  make examples TYPE=html  Render every example to HTML' \
		'  make clean               Remove generated example outputs' \
		'  make clean-all           Also remove target/ (cargo clean)' \
		'' \
		'TYPE may be svg (default), png, pdf or html.'

all: help

debug:
	$(CARGO) build

release:
	$(CARGO) build --release

install: release
	install -d "$(BINDIR)"
	install -m 0755 "$(RELEASE_BIN)" "$(BINDIR)/swimflow"

# Run the live-preview server. Configuration comes from .env (see
# .env.example); pass ENV_FILE=path to use another file. The server has no
# write access to the sources: edits are only rendered, and the Download
# button exports the current state to a new file.
serve: release
	$(RELEASE_BIN) --serve $(if $(ENV_FILE),--env-file "$(ENV_FILE)",)

# Use the release binary so the generated showcase matches what users install.
# The dependency makes a fresh checkout work with a single command.
examples: release
	$(MAKE) -C examples TYPE="$(TYPE)" SWIMFLOW="$(RELEASE_BIN)"

clean:
	$(MAKE) -C examples clean

clean-all: clean
	$(CARGO) clean
