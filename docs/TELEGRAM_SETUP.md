# Telegram API Setup

TGCryptFS uses the Telegram API to store encrypted data blocks in your Telegram account. This requires a Telegram API ID and API Hash, which you obtain by registering an application with Telegram.

This is a one-time setup. Your API credentials are stored locally and never transmitted anywhere except to Telegram's authentication servers.

---

## Why does TGCryptFS need API credentials?

TGCryptFS communicates directly with Telegram's MTProto API (the same protocol used by Telegram clients) to upload and download encrypted blocks. Unlike a Telegram bot, this runs as a full user client under your own account, giving you access to your personal cloud storage with no third-party intermediary.

Telegram requires all applications using their API to register and obtain credentials. This is free and takes about two minutes.

---

## Step-by-step: Obtaining API credentials

### 1. Visit the Telegram API portal

Open [https://my.telegram.org](https://my.telegram.org) in a web browser.

### 2. Log in with your phone number

Enter the phone number associated with your Telegram account in international format (e.g., `+1 555 123 4567`). Telegram will send a verification code to your Telegram app -- not via SMS.

### 3. Navigate to "API development tools"

After logging in, click **API development tools**. If this is your first time, you will see a form to create a new application.

### 4. Fill in the application form

| Field | What to enter |
|-------|---------------|
| **App title** | `TGCryptFS` (or any name you prefer) |
| **Short name** | `tgcryptfs` (3-32 characters, lowercase, no spaces) |
| **URL** | Leave blank or enter `https://github.com/hedonistic-io/tgcryptfs` |
| **Platform** | Select **Desktop** |
| **Description** | Optional. Something like "Encrypted filesystem storage" |

Click **Create application**.

### 5. Record your credentials

After creation, you will see two values:

- **App api_id** -- a numeric ID (e.g., `12345678`)
- **App api_hash** -- a 32-character hexadecimal string (e.g., `0123456789abcdef0123456789abcdef`)

You will need both of these. Do not share them publicly.

---

## Configuring TGCryptFS

There are three ways to provide your credentials to TGCryptFS. Use whichever is most convenient.

### Method 1: Setup script (recommended)

Run the interactive setup script:

```bash
./scripts/setup-telegram.sh
```

This prompts you for your API ID and API Hash, validates them, and writes the configuration file.

### Method 2: Configuration file

Create the file `~/.config/tgcryptfs/.env` with the following contents:

```
TG_API_ID=12345678
TG_API_HASH=0123456789abcdef0123456789abcdef
```

On Linux and macOS, set restrictive permissions:

```bash
mkdir -p ~/.config/tgcryptfs
chmod 700 ~/.config/tgcryptfs
nano ~/.config/tgcryptfs/.env    # add your credentials
chmod 600 ~/.config/tgcryptfs/.env
```

### Method 3: Environment variables

Set the variables directly in your shell:

```bash
export TG_API_ID=12345678
export TG_API_HASH=0123456789abcdef0123456789abcdef
```

To make this permanent, add the lines to your shell profile (`~/.bashrc`, `~/.zshrc`, or `~/.config/fish/config.fish`).

You can also pass credentials directly to the login command:

```bash
tgcryptfs auth login --api-id 12345678 --api-hash 0123456789abcdef0123456789abcdef
```

### Precedence

If credentials are provided through multiple methods, the order of precedence is:

1. Command-line arguments (`--api-id`, `--api-hash`)
2. Environment variables (`TG_API_ID`, `TG_API_HASH`)
3. Configuration file (`~/.config/tgcryptfs/.env`)

---

## Security notes

- **Keep your API Hash private.** Treat it like a password. Anyone with your API ID and Hash can create Telegram sessions that impersonate your application. They cannot access your account without also having your phone number and verification code.
- **Do not commit credentials to version control.** The `.env` file is listed in `.gitignore` by default.
- **File permissions matter.** The configuration file should be readable only by your user (`chmod 600`). The configuration directory should be accessible only by your user (`chmod 700`).
- **Revoking credentials.** If you believe your API Hash has been compromised, visit [https://my.telegram.org](https://my.telegram.org) and regenerate your application credentials. Then update your local configuration.

---

## Troubleshooting

### "Invalid API ID or API Hash"

Double-check that you copied both values correctly from [https://my.telegram.org](https://my.telegram.org). The API ID is numeric only. The API Hash is exactly 32 hexadecimal characters.

### "Phone number not recognized"

Ensure you are entering your phone number in international format with the country code (e.g., `+1` for US, `+44` for UK). Do not include leading zeros after the country code.

### "Verification code not received"

The code is sent to your **Telegram app**, not via SMS. Open Telegram on your phone or desktop and look for a message from Telegram (the "Telegram" service account). If you do not receive it within a minute, try again -- Telegram rate-limits verification attempts.

### "Two-factor authentication password required"

If you have enabled 2FA on your Telegram account, TGCryptFS will prompt you for your 2FA password after entering the verification code. This is your Telegram cloud password, not your TGCryptFS volume passphrase.

### "FLOOD_WAIT" or rate limiting

Telegram rate-limits authentication attempts. If you see a flood wait error, wait the indicated number of seconds before retrying. Avoid rapid repeated login attempts.

### "API development tools" page not showing

If you have previously created an application, you may be taken directly to your existing application details. Your API ID and Hash are displayed on that page. Telegram allows only one application per account.

### Configuration file not found

Verify the file exists at the correct path:

- Linux / macOS: `~/.config/tgcryptfs/.env`
- The directory is created by the setup script, or you can create it manually

If you are using a non-standard `XDG_CONFIG_HOME`, the configuration directory follows that variable:

```bash
$XDG_CONFIG_HOME/tgcryptfs/.env
```
