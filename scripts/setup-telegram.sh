#!/usr/bin/env bash
#
# tgcryptfs -- Telegram API credential setup
#
# Walks through obtaining and configuring Telegram API credentials
# required for tgcryptfs to communicate with Telegram's storage layer.
#
# Usage:
#   ./scripts/setup-telegram.sh

set -euo pipefail

# -- Constants ----------------------------------------------------------------

CONFIG_DIR="${HOME}/.config/tgcryptfs"
ENV_FILE="${CONFIG_DIR}/.env"
TELEGRAM_URL="https://my.telegram.org"

# -- Color output -------------------------------------------------------------

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

info()    { printf "${BLUE}[info]${RESET}    %s\n" "$*"; }
success() { printf "${GREEN}[ok]${RESET}      %s\n" "$*"; }
warn()    { printf "${YELLOW}[warn]${RESET}    %s\n" "$*"; }
error()   { printf "${RED}[error]${RESET}   %s\n" "$*" >&2; }
fatal()   { error "$@"; exit 1; }
header()  { printf "\n${BOLD}%s${RESET}\n" "$*"; }
step()    { printf "\n${CYAN}Step %s:${RESET} ${BOLD}%s${RESET}\n" "$1" "$2"; }
dim()     { printf "${DIM}%s${RESET}\n" "$*"; }

# -- Pre-flight checks --------------------------------------------------------

header "tgcryptfs -- Telegram API Setup"
printf "\n"
info "This script will help you obtain Telegram API credentials."
info "You need a Telegram account with a verified phone number."
info "The process takes about 2 minutes."
printf "\n"

# Check if credentials already exist
if [ -f "$ENV_FILE" ]; then
    if grep -q "TG_API_ID" "$ENV_FILE" 2>/dev/null && grep -q "TG_API_HASH" "$ENV_FILE" 2>/dev/null; then
        warn "Existing credentials found at ${ENV_FILE}"
        printf "\n"
        printf "  Do you want to overwrite them? [y/N] "
        read -r overwrite
        case "$overwrite" in
            [yY]|[yY][eE][sS]) ;;
            *)
                info "Keeping existing credentials. Exiting."
                exit 0
                ;;
        esac
    fi
fi

# -- Step 1: Open Telegram developer portal -----------------------------------

step "1" "Open the Telegram Developer Portal"
printf "\n"
info "You need to visit: ${TELEGRAM_URL}"
info "Log in with your Telegram phone number."
printf "\n"

# Attempt to open browser automatically
opened_browser=false
if [ "$(uname -s)" = "Darwin" ]; then
    if open "$TELEGRAM_URL" 2>/dev/null; then
        opened_browser=true
    fi
elif command -v xdg-open >/dev/null 2>&1; then
    if xdg-open "$TELEGRAM_URL" 2>/dev/null; then
        opened_browser=true
    fi
elif command -v wslview >/dev/null 2>&1; then
    if wslview "$TELEGRAM_URL" 2>/dev/null; then
        opened_browser=true
    fi
fi

if [ "$opened_browser" = true ]; then
    success "Browser opened to ${TELEGRAM_URL}"
else
    info "Please open this URL in your browser:"
    printf "\n    ${BOLD}%s${RESET}\n\n" "$TELEGRAM_URL"
fi

printf "  Press Enter when you have the page open... "
read -r

# -- Step 2: Navigate to API Development Tools --------------------------------

step "2" "Navigate to API Development Tools"
printf "\n"
info "After logging in, you should see your Telegram account page."
info "Look for a link or section called:"
printf "\n"
printf "    ${BOLD}\"API development tools\"${RESET}\n"
printf "\n"
dim "  +--------------------------------------------------+"
dim "  |  Telegram Account                                 |"
dim "  |                                                   |"
dim "  |  Your Telegram Core                               |"
dim "  |                                                   |"
dim "  |  > API development tools    <-- Click this        |"
dim "  |  > Delete account                                 |"
dim "  +--------------------------------------------------+"
printf "\n"
info "Click on 'API development tools' to proceed."
printf "\n"
printf "  Press Enter when you see the API app form... "
read -r

# -- Step 3: Create an API application ----------------------------------------

step "3" "Create your API application"
printf "\n"
info "If you have never created an app before, you will see a form."
info "If you already have an app, skip ahead -- your credentials will be shown."
printf "\n"
info "Fill in the form with these values:"
printf "\n"
printf "    ${BOLD}App title:${RESET}      tgcryptfs\n"
printf "    ${BOLD}Short name:${RESET}     tgcryptfs\n"
printf "    ${BOLD}URL:${RESET}            (leave blank)\n"
printf "    ${BOLD}Platform:${RESET}       Desktop\n"
printf "    ${BOLD}Description:${RESET}    Encrypted filesystem storage\n"
printf "\n"
dim "  The app title and short name are for your reference only."
dim "  They do not affect tgcryptfs functionality."
dim "  Use any values you prefer if the above are taken."
printf "\n"
info "Click 'Create application' to submit the form."
printf "\n"
printf "  Press Enter when your app is created and you can see your credentials... "
read -r

# -- Step 4: Collect API ID ---------------------------------------------------

step "4" "Enter your API credentials"
printf "\n"
info "Your app page should now show two important values:"
printf "\n"
printf "    ${BOLD}App api_id:${RESET}     A numeric ID (e.g., 12345678)\n"
printf "    ${BOLD}App api_hash:${RESET}   A 32-character hex string\n"
printf "\n"
dim "  +--------------------------------------------------+"
dim "  |  App Configuration                                |"
dim "  |                                                   |"
dim "  |  App api_id:    12345678                          |"
dim "  |  App api_hash:  0123456789abcdef0123456789abcdef  |"
dim "  |  ...                                              |"
dim "  +--------------------------------------------------+"
printf "\n"

# Read API ID
while true; do
    printf "  Enter your ${BOLD}API ID${RESET} (numeric): "
    read -r api_id

    # Validate: must be a positive integer
    if [[ "$api_id" =~ ^[0-9]+$ ]] && [ "$api_id" -gt 0 ]; then
        break
    fi
    error "Invalid API ID. It must be a positive integer (e.g., 12345678)."
done

# Read API Hash (hidden input)
while true; do
    printf "  Enter your ${BOLD}API Hash${RESET} (input hidden): "
    read -rs api_hash
    printf "\n"

    # Validate: must be a 32-character hex string
    if [[ "$api_hash" =~ ^[0-9a-fA-Fa-f]{32}$ ]]; then
        break
    fi
    error "Invalid API Hash. It must be a 32-character hexadecimal string."
    dim "  Example: 0123456789abcdef0123456789abcdef"
done

printf "\n"
success "Credentials received."

# -- Step 5: Write credentials -------------------------------------------------

step "5" "Saving credentials"
printf "\n"

mkdir -p "$CONFIG_DIR"
chmod 700 "$CONFIG_DIR"

# Write the .env file with restrictive permissions
{
    printf "# tgcryptfs Telegram API credentials\n"
    printf "# Generated by setup-telegram.sh on %s\n" "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    printf "# DO NOT share these values or commit this file to version control.\n"
    printf "\n"
    printf "TG_API_ID=%s\n" "$api_id"
    printf "TG_API_HASH=%s\n" "$api_hash"
} > "$ENV_FILE"

chmod 600 "$ENV_FILE"

success "Credentials written to ${ENV_FILE}"
info "File permissions: 600 (owner read/write only)"

# -- Step 6: Verify -----------------------------------------------------------

step "6" "Verification"
printf "\n"

if [ -f "$ENV_FILE" ]; then
    stored_id="$(grep "^TG_API_ID=" "$ENV_FILE" | cut -d= -f2)"
    stored_hash="$(grep "^TG_API_HASH=" "$ENV_FILE" | cut -d= -f2)"

    if [ "$stored_id" = "$api_id" ] && [ "$stored_hash" = "$api_hash" ]; then
        success "Credentials verified successfully."
    else
        fatal "Credential verification failed. The file may be corrupted."
    fi
else
    fatal "Credentials file was not created."
fi

# Verify file permissions
perms="$(stat -f "%Lp" "$ENV_FILE" 2>/dev/null || stat -c "%a" "$ENV_FILE" 2>/dev/null)"
if [ "$perms" = "600" ]; then
    success "File permissions are correct (600)."
else
    warn "File permissions are ${perms} (expected 600). Fixing..."
    chmod 600 "$ENV_FILE"
fi

# -- Next steps ----------------------------------------------------------------

header "Setup complete"
printf "\n"
info "Your Telegram API credentials are configured."
printf "\n"
info "Next steps:"
printf "\n"
printf "  ${BOLD}1.${RESET} Authenticate with Telegram:\n"
printf "     ${CYAN}tgcryptfs auth login${RESET}\n"
printf "\n"
printf "     This will send a code to your Telegram app.\n"
printf "     Enter the code to complete authentication.\n"
printf "\n"
printf "  ${BOLD}2.${RESET} Create your first encrypted volume:\n"
printf "     ${CYAN}tgcryptfs volume create my-secure-files${RESET}\n"
printf "\n"
printf "  ${BOLD}3.${RESET} Mount and use it:\n"
printf "     ${CYAN}mkdir -p ~/mnt/secure${RESET}\n"
printf "     ${CYAN}tgcryptfs mount my-secure-files ~/mnt/secure${RESET}\n"
printf "\n"
info "For more information: tgcryptfs --help"
printf "\n"
