#!/usr/bin/env bash
#
# ccmanager installation script
# Usage: curl -fsSL https://raw.githubusercontent.com/NormalUhr/ccmanager/main/scripts/install.sh | bash
#
# Environment variables:
#   CCMANAGER_VERSION      - Pin a specific version (e.g., v0.1.13)
#   CCMANAGER_INSTALL_DIR  - Override install directory (default: /usr/local/bin or ~/.local/bin)
#
# Examples:
#   CCMANAGER_VERSION=v0.1.13 bash install.sh
#   CCMANAGER_INSTALL_DIR=/opt/bin bash install.sh
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
	echo -e "${BLUE}==>${NC} $1"
}

log_success() {
	echo -e "${GREEN}==>${NC} $1"
}

log_warning() {
	echo -e "${YELLOW}==>${NC} $1"
}

log_error() {
	echo -e "${RED}Error:${NC} $1" >&2
}

# Detect OS and architecture
detect_platform() {
	local os arch

	case "$(uname -s)" in
	Darwin)
		os="darwin"
		;;
	Linux)
		os="linux"
		;;
	*)
		log_error "Unsupported operating system: $(uname -s)"
		echo ""
		echo "ccmanager supports macOS and Linux."
		echo "For other platforms, try building from source with Cargo:"
		echo "  cargo install ccmanager"
		echo ""
		exit 1
		;;
	esac

	case "$(uname -m)" in
	x86_64 | amd64)
		arch="amd64"
		;;
	aarch64 | arm64)
		arch="arm64"
		;;
	*)
		log_error "Unsupported architecture: $(uname -m)"
		echo ""
		echo "ccmanager prebuilt binaries are available for amd64 and arm64."
		echo "For other architectures, try building from source with Cargo:"
		echo "  cargo install ccmanager"
		echo ""
		exit 1
		;;
	esac

	echo "${os}-${arch}"
}

# Download and install from GitHub releases
install_from_release() {
	log_info "Installing ccmanager from GitHub releases..."

	local platform=$1
	local tmp_dir
	tmp_dir=$(mktemp -d)
	trap 'rm -rf "$tmp_dir"' EXIT

	# Get latest release version (or use override)
	local version="${CCMANAGER_VERSION:-}"

	if [ -z "$version" ]; then
		log_info "Fetching latest release..."
		local latest_url="https://api.github.com/repos/NormalUhr/ccmanager/releases/latest"
		local release_json

		if command -v curl &>/dev/null; then
			release_json=$(curl -fsSL --retry 3 --retry-connrefused --connect-timeout 10 --max-time 30 "$latest_url")
		elif command -v wget &>/dev/null; then
			release_json=$(wget --tries=3 --timeout=30 -qO- "$latest_url")
		else
			log_error "Neither curl nor wget found. Please install one of them."
			exit 1
		fi

		version=$(echo "$release_json" | grep '"tag_name"' | sed -E 's/.*"tag_name": "([^"]+)".*/\1/')

		if [ -z "$version" ]; then
			log_error "Failed to fetch latest version"
			echo ""
			echo "This might be due to network issues or GitHub API rate limits."
			echo "You can specify a version manually:"
			echo "  CCMANAGER_VERSION=v0.1.13 bash install.sh"
			echo ""
			exit 1
		fi
	fi

	log_info "Installing version: $version"

	# Download URL
	local archive_name="ccmanager-${platform}.tar.gz"
	local download_url="https://github.com/NormalUhr/ccmanager/releases/download/${version}/${archive_name}"

	log_info "Downloading $archive_name..."

	cd "$tmp_dir"
	if command -v curl &>/dev/null; then
		if ! curl -fsSL --retry 3 --retry-connrefused --connect-timeout 10 --max-time 120 -o "$archive_name" "$download_url"; then
			log_error "Download failed"
			echo ""
			echo "The release may not have a prebuilt binary for your platform."
			echo "Try installing with Cargo instead:"
			echo "  cargo install ccmanager"
			echo ""
			cd - >/dev/null || cd "$HOME"
			exit 1
		fi
	elif command -v wget &>/dev/null; then
		if ! wget --tries=3 --timeout=120 -q -O "$archive_name" "$download_url"; then
			log_error "Download failed"
			echo ""
			echo "The release may not have a prebuilt binary for your platform."
			echo "Try installing with Cargo instead:"
			echo "  cargo install ccmanager"
			echo ""
			cd - >/dev/null || cd "$HOME"
			exit 1
		fi
	fi

	# Download and verify checksum
	log_info "Verifying checksum..."
	local checksum_file="ccmanager-${platform}.sha256"
	local checksum_url="https://github.com/NormalUhr/ccmanager/releases/download/${version}/${checksum_file}"

	if command -v curl &>/dev/null; then
		if ! curl -fsSL --retry 3 --retry-connrefused --connect-timeout 10 --max-time 30 -o "$checksum_file" "$checksum_url"; then
			log_error "Failed to download checksum file"
			cd - >/dev/null || cd "$HOME"
			exit 1
		fi
	elif command -v wget &>/dev/null; then
		if ! wget --tries=3 --timeout=30 -q -O "$checksum_file" "$checksum_url"; then
			log_error "Failed to download checksum file"
			cd - >/dev/null || cd "$HOME"
			exit 1
		fi
	fi

	# Verify checksum using sha256sum (Linux) or shasum (macOS)
	if command -v sha256sum &>/dev/null; then
		if ! sha256sum -c "$checksum_file" &>/dev/null; then
			log_error "Checksum verification failed"
			echo ""
			echo "The downloaded file may be corrupted or tampered with."
			echo "Please try again or report this issue."
			echo ""
			cd - >/dev/null || cd "$HOME"
			exit 1
		fi
	elif command -v shasum &>/dev/null; then
		if ! shasum -a 256 -c "$checksum_file" &>/dev/null; then
			log_error "Checksum verification failed"
			echo ""
			echo "The downloaded file may be corrupted or tampered with."
			echo "Please try again or report this issue."
			echo ""
			cd - >/dev/null || cd "$HOME"
			exit 1
		fi
	else
		log_warning "Neither sha256sum nor shasum found, skipping checksum verification"
	fi

	log_success "Checksum verified"

	# Extract archive
	log_info "Extracting archive..."
	if ! tar -xzf "$archive_name"; then
		log_error "Failed to extract archive"
		exit 1
	fi

	# Determine install location (with override support)
	local install_dir="${CCMANAGER_INSTALL_DIR:-}"
	if [ -z "$install_dir" ]; then
		if [[ -w /usr/local/bin ]]; then
			install_dir="/usr/local/bin"
		else
			install_dir="$HOME/.local/bin"
			mkdir -p "$install_dir"
		fi
	fi

	# Check for existing installation
	if [ -f "$install_dir/ccmanager" ]; then
		local existing_version
		existing_version=$("$install_dir/ccmanager" --version 2>/dev/null | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+' || echo "unknown")
		log_info "Existing installation found: $existing_version"
		log_info "Upgrading to: $version"
	fi

	# Install binary atomically
	log_info "Installing to $install_dir..."
	local tmp_binary="$install_dir/ccmanager.tmp.$$"

	if [[ -w "$install_dir" ]]; then
		cp ccmanager "$tmp_binary"
		chmod +x "$tmp_binary"
		mv -f "$tmp_binary" "$install_dir/ccmanager"
	else
		if ! sudo cp ccmanager "$tmp_binary"; then
			log_error "Failed to install to $install_dir (sudo required)"
			exit 1
		fi
		sudo chmod +x "$tmp_binary"
		sudo mv -f "$tmp_binary" "$install_dir/ccmanager"
	fi

	# Remove macOS quarantine attribute if present
	if [[ "$(uname -s)" == "Darwin" ]] && command -v xattr &>/dev/null; then
		xattr -d com.apple.quarantine "$install_dir/ccmanager" 2>/dev/null || true
	fi

	log_success "ccmanager installed to $install_dir/ccmanager"

	# If the install dir isn't on PATH, automatically add it to the
	# user's shell rc and tell them how to activate it in the current
	# shell. (We can't modify the parent shell's PATH from a piped
	# install script — only the rc file modification persists.)
	if [[ ":$PATH:" != *":$install_dir:"* ]]; then
		configure_shell_path "$install_dir"
	fi

	cd - >/dev/null || cd "$HOME"

	# Return the install directory via a global variable
	INSTALL_DIR="$install_dir"
}

# Append `$install_dir` to the user's shell PATH by editing their rc
# file. Idempotent — a marker comment prevents double-adding on
# repeated installs. Sets the global `ACTIVATE_HINT` to the exact
# command the user should run to make ccmanager available in their
# current shell session.
configure_shell_path() {
	local install_dir="$1"
	local shell_name
	shell_name=$(basename "${SHELL:-bash}")

	local rc_files=()
	local rc_export

	case "$shell_name" in
	bash)
		# Linux: ~/.bashrc is read by interactive non-login shells.
		# macOS Terminal launches login shells by default → ~/.bash_profile.
		# Append to both when present to cover both cases.
		rc_files+=("$HOME/.bashrc")
		if [[ "$(uname -s)" == "Darwin" ]]; then
			rc_files+=("$HOME/.bash_profile")
		fi
		rc_export="export PATH=\"$install_dir:\$PATH\""
		;;
	zsh)
		rc_files+=("$HOME/.zshrc")
		rc_export="export PATH=\"$install_dir:\$PATH\""
		;;
	fish)
		rc_files+=("$HOME/.config/fish/config.fish")
		rc_export="set -gx PATH $install_dir \$PATH"
		;;
	*)
		log_warning "Unknown shell ($shell_name) — please add this to your shell profile manually:"
		echo "  export PATH=\"$install_dir:\$PATH\""
		echo ""
		return
		;;
	esac

	local marker="# Added by ccmanager installer"
	local touched_file=""
	for rc in "${rc_files[@]}"; do
		# Skip if our marker is already present (idempotent across re-runs)
		if [ -f "$rc" ] && grep -Fq "$marker" "$rc"; then
			touched_file="$rc"
			continue
		fi
		mkdir -p "$(dirname "$rc")"
		printf '\n%s\n%s\n' "$marker" "$rc_export" >> "$rc"
		touched_file="$rc"
	done

	if [ -z "$touched_file" ]; then
		# Should not happen, but cover the case.
		log_warning "Couldn't add $install_dir to PATH automatically. Add it manually:"
		echo "  $rc_export"
		echo ""
		return
	fi

	log_success "Added $install_dir to PATH in $touched_file"

	# Stash the activation command so main() can print it at the very end.
	ACTIVATE_HINT="source $touched_file"
}

# Verify installation
verify_installation() {
	local install_dir="$1"

	# Verify the binary exists and is executable
	if [ ! -x "$install_dir/ccmanager" ]; then
		log_error "ccmanager binary not found or not executable at $install_dir/ccmanager"
		exit 1
	fi

	# Test the binary works
	if ! "$install_dir/ccmanager" --version &>/dev/null; then
		log_error "ccmanager binary exists but failed to run"
		exit 1
	fi

	log_success "ccmanager is installed and ready!"
	echo ""
	"$install_dir/ccmanager" --version
	echo ""
}

# Print the next-steps message. Shape depends on whether the user
# needs to activate the new PATH or whether `ccmanager` already works.
print_next_steps() {
	if [ -n "${ACTIVATE_HINT:-}" ]; then
		echo "${YELLOW}One more step:${NC} make ccmanager available in your current shell."
		echo ""
		echo "  Easiest — restart your shell:"
		echo "    exec \$SHELL"
		echo ""
		echo "  Or load just this update:"
		echo "    $ACTIVATE_HINT"
		echo ""
		echo "After that:"
	else
		echo "Get started:"
	fi
	echo "  ccmanager        # browse conversation history"
	echo "  ccmanager --help # see all options"
	echo ""
	echo "Documentation: https://github.com/NormalUhr/ccmanager"
	echo ""
}

# Main installation flow
main() {
	echo ""
	echo "ccmanager installer"
	echo ""

	log_info "Detecting platform..."
	local platform
	platform=$(detect_platform)
	log_info "Platform: $platform"

	# Download and install
	install_from_release "$platform"

	# Verify
	verify_installation "$INSTALL_DIR"

	# Tell the user what to do next.
	print_next_steps
}

main "$@"
