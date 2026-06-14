(function() {
    const params = new URLSearchParams(window.location.search);
    const token = params.get('token');
    const permission = params.get('permission') || 'rw';

    if (!token) {
        window.location.href = '/';
        return;
    }

    let sessionId = '';
    let userId = '';
    let userPermission = permission;

    const ws = new WSClient();
    const term = new TerminalManager('terminal-container');
    const files = new FileManager(ws, 'file-tree');

    const onlineCountEl = document.getElementById('online-count');
    const sessionNameEl = document.getElementById('session-name');
    const disconnectOverlay = document.getElementById('disconnect-overlay');
    const disconnectText = document.getElementById('disconnect-text');
    const toast = document.getElementById('toast');
    const fileDrawer = document.getElementById('file-drawer');

    let onlineUsers = 0;
    const pendingFileOpens = new Set();

    function createMessage(type, payload) {
        return {
            type: type,
            session_id: sessionId,
            payload: payload || {}
        };
    }

    function showToast(msg, cls) {
        toast.textContent = msg;
        toast.className = 'toast ' + cls;
        setTimeout(() => {
            toast.classList.add('hidden');
        }, 3000);
    }

    function updateOnlineCount() {
        onlineCountEl.textContent = onlineUsers + ' online';
    }

    ws.onMessage = function(data) {
        let msg;
        try {
            msg = JSON.parse(data);
        } catch (e) {
            return;
        }

        switch (msg.type) {
            case 'browser:connected':
                sessionId = msg.session_id;
                userId = msg.payload.user_id;
                sessionNameEl.textContent = 'Session: ' + sessionId.substring(0, 8);
                term.focus();
                term.onResize((cols, rows) => {
                    ws.send(createMessage('terminal:resize', {
                        cols: cols,
                        rows: rows
                    }));
                });
                term.onInput((data) => {
                    const bytes = new TextEncoder().encode(data); const b64 = btoa(String.fromCharCode(...bytes));
                    ws.send(createMessage('terminal:input', {
                        data: b64
                    }));
                });
                break;

            case 'terminal:output':
                try {
                    const binaryStr = atob(msg.payload.data);
                    const bytes = Uint8Array.from(binaryStr, c => c.charCodeAt(0));
                    const decoded = new TextDecoder().decode(bytes);
                    term.write(decoded);
                } catch (e) {
                    console.error('Failed to decode terminal output', e);
                }
                break;

            case 'session:users':
                onlineUsers = (msg.payload.users || []).length;
                updateOnlineCount();
                break;

            case 'fs:result':
                if (pendingFileOpens.has(msg.payload._mcp_request_id)) {
                    pendingFileOpens.delete(msg.payload._mcp_request_id);
                    const editorContent = document.getElementById('editor-content');
                    const fileEditor = document.getElementById('file-editor');
                    if (msg.payload.success) {
                        try {
                            const binaryStr = atob(msg.payload.content || '');
                            const bytes = Uint8Array.from(binaryStr, c => c.charCodeAt(0));
                            editorContent.value = new TextDecoder().decode(bytes);
                        } catch (e) {
                            editorContent.value = '[Binary file]';
                        }
                    } else {
                        showToast(msg.payload.error || 'Failed to read file', 'error');
                    }
                    fileEditor.classList.remove('hidden');
                } else {
                    files.handleFsResult(msg.payload);
                }
                break;

            case 'session:agent_disconnect':
                disconnectText.textContent =
                    'The remote session has ended.';
                disconnectOverlay.classList.remove('hidden');
                ws.close();
                break;

            case 'error':
                if (msg.payload.code === 'AUTH_INVALID_TOKEN') {
                    showToast('Invalid or expired token', 'error');
                    setTimeout(() => window.location.href = '/', 2000);
                } else if (msg.payload.code === 'PERMISSION_DENIED') {
                    showToast('Permission denied: read-only access', 'error');
                } else {
                    showToast(msg.payload.message || 'Error', 'error');
                }
                break;
        }
    };

    ws.onClose = function() {
        if (!disconnectOverlay.classList.contains('hidden')) return;
        disconnectText.textContent = 'Connection lost. Reconnecting...';
        disconnectOverlay.classList.remove('hidden');
    };

    ws.onOpen = function() {
        disconnectOverlay.classList.add('hidden');
        ws.send(JSON.stringify({
            type: 'browser:join',
            payload: { token: token, permission: userPermission }
        }));
    };

    const wsUrl =
        (location.protocol === 'https:' ? 'wss://' : 'ws://') +
        location.host + '/ws';
    ws.connect(wsUrl);

    // Toolbar buttons
    document.getElementById('copy-token-btn').addEventListener('click', () => {
        navigator.clipboard.writeText(token).then(() => {
            showToast('Token copied!', 'success');
        }).catch(() => {
            const input = document.createElement('input');
            input.value = token;
            document.body.appendChild(input);
            input.select();
            document.execCommand('copy');
            document.body.removeChild(input);
            showToast('Token copied!', 'success');
        });
    });

    document.getElementById('toggle-files-btn').addEventListener('click', () => {
        const isHidden = fileDrawer.classList.contains('hidden');
        if (isHidden) {
            fileDrawer.classList.remove('hidden');
            files.init();
        } else {
            fileDrawer.classList.add('hidden');
        }
        term.resize();
    });

    document.getElementById('close-files-btn').addEventListener('click', () => {
        fileDrawer.classList.add('hidden');
        term.resize();
    });

    document.getElementById('disconnect-ok-btn').addEventListener('click', () => {
        window.location.href = '/';
    });

    files.onFileOpen = function(path, name) {
        const fileOpenId = 'fo-' + Date.now();
        pendingFileOpens.add(fileOpenId);
        ws.send(createMessage('fs:read', { path: path, _mcp_request_id: fileOpenId }));

        const editorFilename = document.getElementById('editor-filename');
        const editorContent = document.getElementById('editor-content');
        const fileEditor = document.getElementById('file-editor');

        editorFilename.textContent = name;
        editorContent.dataset.path = path;

        document.getElementById('editor-save-btn').onclick = () => {
            const content = editorContent.value;
            const bytes = new TextEncoder().encode(content);
            const b64 = btoa(String.fromCharCode(...bytes));
            ws.send(createMessage('fs:write', {
                path: path,
                content: b64
            }));
            showToast('File saved', 'success');
            fileEditor.classList.add('hidden');
        };

        document.getElementById('editor-close-btn').onclick = () => {
            fileEditor.classList.add('hidden');
            pendingFileOpens.clear();
        };
    };
})();
