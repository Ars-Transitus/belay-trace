BINDIR ?= $(HOME)/.local/bin
PYTHON ?= python3

.PHONY: build deploy

build:
	$(PYTHON) scripts/build-local.py $(INSTALL_ARGS)

deploy: INSTALL_ARGS = --bindir "$(BINDIR)"
deploy: build
