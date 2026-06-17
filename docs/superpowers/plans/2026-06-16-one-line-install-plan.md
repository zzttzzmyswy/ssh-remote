# One-Line Agent Install + GitHub Link — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace verbose Agent guide and download cards on homepage with a single `curl | sh` install command; add GitHub link in top-right corner.

**Architecture:** Relay serves a bash install script at `/agent/install` (embedded via `include_str!`). The script auto-detects arch, tries multiple download sources with China mainland proxy fallback, downloads to `/dev/shm` (or `/tmp`), executes with `exec`, and cleans up via `trap`. Homepage shows the command with a copy button.

**Tech Stack:** Bash (install script), Rust (axum route handler), HTML/CSS (frontend)

---

### Task 1: Create install script + relay handler + route

**Files:**
- Create: `web/install.sh`
- Modify: `src/relay/mod.rs`

- [ ] **Step 1: Create web/install.sh**

Create `web/install.sh` with this exact content:

```bash
#!/bin/sh
set -e

# shell-remote one-line agent install script
# DO NOT run directly — use: curl -fsSL <relay>/agent/install | sh

RELAY_URL="__RELAY_URL__"

ARCH=$(uname -m)
case "$ARCH" in
    x86_64|amd64)   BIN_ARCH="x86_64" ;;
    aarch64|arm64)  BIN_ARCH="aarch64" ;;
    armv7l|armv7)   BIN_ARCH="armv7" ;;
    *) echo "[shell-remote] unsupported architecture: $ARCH"; exit 1 ;;
esac

TMPDIR="/dev/shm"
if [ ! -w "$TMPDIR" ]; then
    TMPDIR="${TMPDIR:-/tmp}"
fi
BIN="$TMPDIR/shell-remote-$$"

BASE="https://github.com/zzttzzmyswy/shell-remote/releases/latest/download"
URLS="
${BASE}/shell-remote-${BIN_ARCH}
https://edgeone.gh-proxy.com/${BASE}/shell-remote-${BIN_ARCH}
https://hk.gh-proxy.com/${BASE}/shell-remote-${BIN_ARCH}
https://gh-proxy.com/${BASE}/shell-remote-${BIN_ARCH}
https://gh.llkk.cc/${BASE}/shell-remote-${BIN_ARCH}
"

echo "[shell-remote] downloading for $ARCH ($BIN_ARCH)..."

for url in $URLS; do
    if curl -fsSL --connect-timeout 5 --max-time 60 -o "$BIN" "$url" 2>/dev/null; then
        echo "[shell-remote] downloaded via $(echo "$url" | cut -d/ -f3)"
        break
    fi
done

if [ ! -f "$BIN" ] || [ ! -s "$BIN" ]; then
    echo "[shell-remote] download failed — all sources unreachable"
    exit 1
fi

chmod +x "$BIN"

cleanup() {
    rm -f "$BIN"
    echo "[shell-remote] cleaned up"
}
trap cleanup EXIT INT TERM

echo "[shell-remote] starting agent..."
exec "$BIN" agent --relay-url "$RELAY_URL" "$@"
```

- [ ] **Step 2: Verify the script is valid bash**

Run: `bash -n web/install.sh`
Expected: no output (syntax OK)

- [ ] **Step 3: Add install_script_handler to src/relay/mod.rs**

Add this handler function (before the `start()` function, after `bin_handler`):

```rust
pub async fn install_script_handler(
    State(_state): State<Arc<SharedState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .filter(|v| *v == "https")
        .map(|_| "https")
        .unwrap_or("http");
    let relay_url = format!("{}://{}", proto, host);

    let script = include_str!("../../web/install.sh")
        .replace("__RELAY_URL__", &relay_url);

    (axum::http::StatusCode::OK,
     [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
     script)
}
```

- [ ] **Step 4: Register route in start()**

In the `start()` function, add the route after the other `/agent/` routes:

```rust
.route("/agent/install", get(install_script_handler))
```

- [ ] **Step 5: Compile check**

Run: `cargo check -p shell-remote 2>&1 | rg "error"`
Expected: zero errors

- [ ] **Step 6: Commit**

```bash
git add web/install.sh src/relay/mod.rs
git commit -m "feat: add /agent/install endpoint serving embedded install script"
```

---

### Task 2: Update homepage HTML + CSS

**Files:**
- Modify: `web/index.html`
- Modify: `web/style.css`

- [ ] **Step 1: Read the current files**

Read `web/index.html` and `web/style.css` to understand the current structure.

- [ ] **Step 2: Delete Agent guide + download sections from index.html**

Remove these HTML blocks from `web/index.html`:
- The entire `<div class="guide-section">...</div>` block (current lines ~28-61)
- The entire `<div class="download-section" id="download-section">...</div>` block (current lines ~63-71)
- The download cards JavaScript block (current lines ~116-174, the `(async () => { const cards = ... })()` block, including all the `ARCHS`, `hasLocal`, `for (const a of ARCHS)` code)

IMPORTANT: Keep the `relayHost` substitution logic (current lines ~110-114) and the connect form logic.

- [ ] **Step 3: Add install section HTML**

After the `</div>` closing tag of `.connect-container`, add:

```html
    <!-- Install Section -->
    <div class="install-section">
        <h2>一键安装 Agent</h2>
        <p>在目标机器上执行以下命令即可自动部署 Agent：</p>
        <div class="install-cmd">
            <code>$ curl -fsSL <span class="relay-host">https://host</span>/agent/install | sh</code>
            <button class="copy-btn" onclick="copyInstallCmd()">copy</button>
        </div>
        <p class="install-note">自动检测架构 → 智能选择下载源（含中国大陆代理） → 内存执行 → 不留残留文件</p>
    </div>
```

- [ ] **Step 4: Add GitHub link HTML**

At the top of `<body>`, or right after `<body class="connect-page">`, add:

```html
    <a class="github-link" href="https://github.com/zzttzzmyswy/shell-remote" target="_blank" rel="noopener" title="GitHub">
        <svg width="24" height="24" viewBox="0 0 16 16" fill="currentColor">
            <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/>
        </svg>
    </a>
```

- [ ] **Step 5: Add copyInstallCmd JavaScript**

In the `<script>` block, add this function:

```javascript
function copyInstallCmd() {
    const cmd = 'curl -fsSL ' + location.protocol + '//' + location.host + '/agent/install | sh';
    navigator.clipboard.writeText(cmd).then(() => {
        const btn = document.querySelector('.install-cmd .copy-btn');
        btn.textContent = 'copied!';
        btn.classList.add('copied');
        setTimeout(() => { btn.textContent = 'copy'; btn.classList.remove('copied'); }, 2000);
    });
}
```

- [ ] **Step 6: Add CSS styles**

Add these styles to `web/style.css`:

```css
/* GitHub link */
.github-link {
    position: absolute;
    top: 1.5rem;
    right: 1.5rem;
    color: var(--text-muted);
    opacity: 0.7;
    transition: opacity 0.2s;
}
.github-link:hover { opacity: 1; color: var(--text-primary); }

/* Install section */
.install-section {
    max-width: 600px;
    margin: 3rem auto;
    padding: 2rem;
    border: 1px solid var(--border-color);
    border-radius: 12px;
    text-align: center;
}
.install-section h2 {
    font-size: 1.25rem;
    margin-bottom: 0.75rem;
    color: var(--text-primary);
}
.install-section p {
    color: var(--text-muted);
    font-size: 0.88rem;
    margin-bottom: 1rem;
}
.install-cmd {
    display: flex;
    align-items: center;
    background: #1e1e2e;
    border-radius: 8px;
    padding: 0.75rem 1rem;
    margin: 0 0 0.75rem 0;
    font-family: ui-monospace, monospace;
    font-size: 0.82rem;
    gap: 0.5rem;
}
.install-cmd code {
    flex: 1;
    color: #cdd6f4;
    overflow-x: auto;
    white-space: nowrap;
    scrollbar-width: none;
}
.install-cmd code::-webkit-scrollbar { display: none; }
.install-cmd .copy-btn {
    flex-shrink: 0;
    background: #45475a;
    color: #cdd6f4;
    border: none;
    border-radius: 4px;
    padding: 0.25rem 0.6rem;
    font-size: 0.72rem;
    cursor: pointer;
    transition: background 0.2s;
}
.install-cmd .copy-btn:hover { background: #585b70; }
.install-cmd .copy-btn.copied { background: #40a02b; }

.install-note {
    color: var(--text-muted);
    font-size: 0.78rem;
    margin: 0;
}
```

- [ ] **Step 7: Build check**

Run: `cargo check -p shell-remote 2>&1 | rg "error"`
Expected: zero errors

- [ ] **Step 8: Commit**

```bash
git add web/index.html web/style.css
git commit -m "feat: replace guide+download with one-line install command + GitHub link"
```

---

### Task 3: Tests + final verify

**Files:**
- Modify: `src/relay/mod.rs` (tests section)

- [ ] **Step 1: Add test for install_script_handler**

In `src/relay/mod.rs` tests, add:

```rust
#[tokio::test]
async fn test_install_script_handler_returns_script() {
    let state = Arc::new(SharedState::new("".into(), None, 100 * 1024 * 1024));
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("host", "example.com:3000".parse().unwrap());
    let resp = install_script_handler(State(state), headers).await.into_response();
    assert_eq!(resp.status(), 200);
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let text = std::str::from_utf8(&body).unwrap();
    assert!(text.contains("RELAY_URL=\"http://example.com:3000\""));
    assert!(text.contains("agent --relay-url"));
    assert!(text.contains("#!/bin/sh"));
}

#[tokio::test]
async fn test_install_script_handler_https_forwarded() {
    let state = Arc::new(SharedState::new("".into(), None, 100 * 1024 * 1024));
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("host", "example.com".parse().unwrap());
    headers.insert("x-forwarded-proto", "https".parse().unwrap());
    let resp = install_script_handler(State(state), headers).await.into_response();
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let text = std::str::from_utf8(&body).unwrap();
    assert!(text.contains("RELAY_URL=\"https://example.com\""));
}

#[tokio::test]
async fn test_install_script_handler_default_host() {
    let state = Arc::new(SharedState::new("".into(), None, 100 * 1024 * 1024));
    let headers = axum::http::HeaderMap::new();
    let resp = install_script_handler(State(state), headers).await.into_response();
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let text = std::str::from_utf8(&body).unwrap();
    assert!(text.contains("RELAY_URL=\"http://localhost\""));
}
```

- [ ] **Step 2: Update test_relay_router_builds_without_error**

Add the install route to the test:

```rust
.route("/agent/install", get(install_script_handler))
```

- [ ] **Step 3: Update test_all_web_assets_accessible**

Add `"install.sh"` to the assets array in the test.

- [ ] **Step 4: Run all tests**

Run: `cargo test -p shell-remote 2>&1 | tail -5`
Expected: all tests pass

- [ ] **Step 5: Run clippy + fmt**

Run: `cargo clippy -p shell-remote -- -D warnings 2>&1`
Expected: zero warnings

Run: `cargo fmt -p shell-remote`

- [ ] **Step 6: Commit**

```bash
git add src/relay/mod.rs
git commit -m "test: add tests for install_script_handler, update route/assets tests"
```

---

### Task 4: Release build + cleanup

**Files:** None new

- [ ] **Step 1: Release build**

Run: `cargo build --release 2>&1 | tail -3`
Expected: Finished release profile

- [ ] **Step 2: Verify script endpoint locally** (optional manual smoke test)

If relay is running: `curl -s localhost:3000/agent/install | head -5`

- [ ] **Step 3: Final commit if any cleanups needed**

```bash
git add -A && git commit -m "chore: final cleanup for one-line install feature"
```
