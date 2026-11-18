# Render every examples/**/*.diag, examples/**/*.seq, examples/**/*.tree,
# examples/**/*.tmap and examples/**/*.mfst file.
#
# Normally invoked through the top-level Makefile:
#   make examples
#   make examples TYPE=html
#
# It can also be used directly:
#   make -C examples
#   make -C examples TYPE=png
#   make -C examples clean

TYPE ?= svg
SWIMFLOW ?= ../target/release/swimflow

SUPPORTED_TYPES := svg png pdf html
ifeq ($(filter $(TYPE),$(SUPPORTED_TYPES)),)
$(error TYPE must be one of: $(SUPPORTED_TYPES))
endif

DIAG_SOURCES := $(shell find . -type f -name '*.diag' -print | LC_ALL=C sort)
SEQ_SOURCES := $(shell find . -type f -name '*.seq' -print | LC_ALL=C sort)
TREE_SOURCES := $(shell find . -type f -name '*.tree' -print | LC_ALL=C sort)
TMAP_SOURCES := $(shell find . -type f -name '*.tmap' -print | LC_ALL=C sort)
MFST_SOURCES := $(shell find . -type f -name '*.mfst' -print | LC_ALL=C sort)
SOURCES := $(DIAG_SOURCES) $(SEQ_SOURCES) $(TREE_SOURCES) $(TMAP_SOURCES) $(MFST_SOURCES)
OUTPUTS := $(DIAG_SOURCES:.diag=.$(TYPE)) $(SEQ_SOURCES:.seq=.$(TYPE)) \
	$(TREE_SOURCES:.tree=.$(TYPE)) $(TMAP_SOURCES:.tmap=.$(TYPE)) \
	$(MFST_SOURCES:.mfst=.$(TYPE))
# `clean` removes every output format, not only the one selected for this run.
GENERATED := $(foreach ext,svg png pdf html,\
	$(DIAG_SOURCES:.diag=.$(ext)) $(SEQ_SOURCES:.seq=.$(ext)) \
	$(TREE_SOURCES:.tree=.$(ext)) $(TMAP_SOURCES:.tmap=.$(ext)) \
	$(MFST_SOURCES:.mfst=.$(ext)))

.DEFAULT_GOAL := all
.PHONY: all clean list

all: $(OUTPUTS)

list:
	@printf '%s\n' $(SOURCES)

%.$(TYPE): %.diag
	$(SWIMFLOW) "$<" -f "$(TYPE)" -o "$@"

%.$(TYPE): %.seq
	$(SWIMFLOW) "$<" -f "$(TYPE)" -o "$@"

%.$(TYPE): %.tree
	$(SWIMFLOW) "$<" -f "$(TYPE)" -o "$@"

%.$(TYPE): %.tmap
	$(SWIMFLOW) "$<" -f "$(TYPE)" -o "$@"

%.$(TYPE): %.mfst
	$(SWIMFLOW) "$<" -f "$(TYPE)" -o "$@"

clean:
	$(RM) $(GENERATED)
