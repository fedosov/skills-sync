# macOS Code Signing & Notarization

Guide for setting up Apple code signing and notarization for SkillsSync Desktop.

> **Status:** not yet configured. The app currently triggers Gatekeeper
> ("damaged and can't be opened") because the `.app` bundle is unsigned.

## Prerequisites

- Apple Developer Account ($99/year) — <https://developer.apple.com/programs/>
- Xcode command-line tools (`xcode-select --install`)
- A **Developer ID Application** certificate exported as `.p12`

## 1. Entitlements

Create `platform/apps/skillssync-desktop/src-tauri/Entitlements.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.app-sandbox</key>
  <false/>
  <key>com.apple.security.cs.allow-jit</key>
  <true/>
  <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
  <true/>
  <key>com.apple.security.cs.allow-dyld-environment-variables</key>
  <true/>
</dict>
</plist>
```

`allow-jit` and `allow-unsigned-executable-memory` are required by the
WebView/JavaScriptCore runtime that Tauri uses.

## 2. Tauri Configuration

Add the `macOS` section to `bundle` in `tauri.conf.json`:

```jsonc
"bundle": {
  // ...existing keys...
  "macOS": {
    "entitlements": "Entitlements.plist",
    "hardenedRuntime": true,       // default, shown for clarity
    "signingIdentity": "-"         // ad-hoc; CI overrides via env
  }
}
```

Key: `bundle.macOS` (capital **OS**) — this is the Tauri v2 schema.

## 3. Environment Variables for CI

| Variable | Description |
|---|---|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12` |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAM_ID)` |
| `APPLE_ID` | Apple ID email for notarization |
| `APPLE_PASSWORD` | App-specific password (generate at appleid.apple.com) |
| `APPLE_TEAM_ID` | 10-character team identifier |

## 4. GitHub Actions Workflow (Snippet)

```yaml
- name: Import code-signing certificate
  uses: apple-actions/import-codesign-certs@v3
  with:
    p12-file-base64: ${{ secrets.APPLE_CERTIFICATE }}
    p12-password: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}

- name: Build & sign Tauri app
  uses: tauri-apps/tauri-action@v0
  env:
    APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
    APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
    APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
    APPLE_ID: ${{ secrets.APPLE_ID }}
    APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
    APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
```

`tauri-action` handles codesign + notarization automatically when these
environment variables are present.

## 5. Local Signing (Manual)

```bash
# Sign with Developer ID
codesign --force --deep --options runtime \
  --entitlements platform/apps/skillssync-desktop/src-tauri/Entitlements.plist \
  --sign "Developer ID Application: Your Name (TEAM_ID)" \
  "target/release/bundle/macos/SkillsSync Desktop.app"

# Create ZIP for notarization
ditto -c -k --keepParent \
  "target/release/bundle/macos/SkillsSync Desktop.app" \
  SkillsSync.zip

# Submit for notarization
xcrun notarytool submit SkillsSync.zip \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" \
  --wait

# Staple the ticket
xcrun stapler staple "target/release/bundle/macos/SkillsSync Desktop.app"
```

## References

- [Tauri v2 — Code Signing](https://v2.tauri.app/distribute/sign/macos/)
- [Apple — Notarizing macOS Software Before Distribution](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [apple-actions/import-codesign-certs](https://github.com/apple-actions/import-codesign-certs)
