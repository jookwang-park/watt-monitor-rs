# Watt Monitor - Makefile
#
# Usage:
#   make              - Build release binary
#   make install      - Install to ~/.local (user install)
#   make install PREFIX=/usr DESTDIR=/tmp/pkg  - For packaging
#   make uninstall    - Remove installed files
#   make enable       - Enable daemon service (user)
#   make disable      - Disable daemon service (user)
#   make clean        - Clean build artifacts

CARGO = cargo
PREFIX ?= $(HOME)/.local
DESTDIR ?=
BINDIR = $(DESTDIR)$(PREFIX)/bin
SYSTEMD_USER_DIR = $(DESTDIR)$(HOME)/.config/systemd/user
OPENRC_DIR = $(DESTDIR)/etc/init.d

.PHONY: all build install uninstall enable disable clean help

all: build

build:
	$(CARGO) build --release

install: build
	@echo "Installing to PREFIX=$(PREFIX), DESTDIR=$(DESTDIR)"
	install -Dm755 target/release/watt-monitor $(BINDIR)/watt-monitor
	install -Dm644 systemd/watt-monitor.service $(SYSTEMD_USER_DIR)/watt-monitor.service
ifeq ($(PREFIX),/usr)
	install -Dm755 openrc/watt-monitor $(OPENRC_DIR)/watt-monitor
endif
	@echo ""
	@echo "Installation complete!"
	@echo "Run 'make enable' to start the battery logger daemon."
	@echo "Then run: watt-monitor"

uninstall:
	rm -f $(BINDIR)/watt-monitor
	rm -f $(SYSTEMD_USER_DIR)/watt-monitor.service
ifeq ($(PREFIX),/usr)
	rm -f $(OPENRC_DIR)/watt-monitor
endif
	@echo "Uninstallation complete!"

enable:
	systemctl --user daemon-reload
	systemctl --user enable --now watt-monitor.service
	@echo "Battery logger daemon enabled."

disable:
	systemctl --user disable --now watt-monitor.service
	@echo "Battery logger daemon disabled."

clean:
	$(CARGO) clean

help:
	@echo "Watt Monitor - Makefile targets:"
	@echo ""
	@echo "  make              Build release binary"
	@echo "  make install      Install to ~/.local (user install)"
	@echo "  make uninstall    Remove installed files"
	@echo "  make enable       Enable daemon service"
	@echo "  make disable      Disable daemon service"
	@echo "  make clean        Clean build artifacts"
	@echo ""
