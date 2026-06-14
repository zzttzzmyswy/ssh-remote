class FileManager {
    constructor(wsClient, treeContainerId) {
        this.ws = wsClient;
        this.treeContainer = document.getElementById(treeContainerId);
        this.currentPath = '/';
        this.expandedDirs = new Set();
        this.onFileOpen = null;
    }

    init() {
        this.loadDirectory('.');
    }

    loadDirectory(path) {
        this.currentPath = path;
        this.ws.send({
            type: 'fs:list',
            session_id: '', // filled by relay
            payload: { path: path }
        });
    }

    handleFsResult(result) {
        if (!result.success) {
            this.treeContainer.innerHTML =
                `<div class="file-tree-item" style="color:#f44747">${result.error || 'Error'}</div>`;
            return;
        }

        this.renderTree(result.entries || []);
    }

    renderTree(entries) {
        this.treeContainer.innerHTML = '';

        if (this.currentPath !== '/' && this.currentPath !== '.') {
            const parent = document.createElement('div');
            parent.className = 'file-tree-item directory';
            parent.innerHTML = '<span class="icon">&#x2190;</span>..';
            parent.addEventListener('click', () => {
                const parts = this.currentPath.split('/').filter(Boolean);
                parts.pop();
                const parentPath = '/' + parts.join('/');
                this.loadDirectory(parentPath || '.');
            });
            this.treeContainer.appendChild(parent);
        }

        entries.forEach((entry) => {
            const item = this.createTreeItem(entry);
            this.treeContainer.appendChild(item);

            if (this.expandedDirs.has(entry.path) && entry.type === 'directory') {
                this.ws.send({
                    type: 'fs:list',
                    session_id: '',
                    payload: { path: entry.path }
                });
            }
        });
    }

    createTreeItem(entry) {
        const item = document.createElement('div');
        item.className = 'file-tree-item';
        item.dataset.path = entry.path;
        item.dataset.type = entry.type;

        const icon = entry.type === 'directory' ? '&#x1F4C1;' : '&#x1F4C4;';
        const sizeStr = entry.type === 'file'
            ? this.formatSize(entry.size) : '';

        item.innerHTML = `<span class="icon">${icon}</span>${entry.name}` +
            (sizeStr ? `<span class="size">${sizeStr}</span>` : '');

        if (entry.type === 'directory') {
            item.classList.add('directory');
        }

        item.addEventListener('click', (e) => {
            this.selectItem(item);
            if (entry.type === 'directory') {
                this.expandedDirs.add(entry.path);
                this.loadDirectory(entry.path);
            }
        });

        item.addEventListener('dblclick', (e) => {
            if (entry.type === 'file' && this.onFileOpen) {
                this.onFileOpen(entry.path, entry.name);
            }
        });

        item.addEventListener('contextmenu', (e) => {
            e.preventDefault();
            this.showContextMenu(e.clientX, e.clientY, entry);
        });

        return item;
    }

    selectItem(item) {
        this.treeContainer.querySelectorAll('.file-tree-item.selected')
            .forEach(el => el.classList.remove('selected'));
        item.classList.add('selected');
    }

    showContextMenu(x, y, entry) {
        const existing = document.querySelector('.context-menu');
        if (existing) existing.remove();

        const menu = document.createElement('div');
        menu.className = 'context-menu';
        menu.style.left = x + 'px';
        menu.style.top = y + 'px';

        const renameItem = document.createElement('div');
        renameItem.className = 'context-menu-item';
        renameItem.textContent = 'Rename';
        renameItem.addEventListener('click', () => {
            menu.remove();
            const newName = prompt('New name:', entry.name);
            if (newName && newName !== entry.name) {
                const newPath = entry.path.replace(
                    /[^/]+$/,
                    newName
                );
                this.ws.send({
                    type: 'fs:rename',
                    session_id: '',
                    payload: { from: entry.path, to: newPath }
                });
            }
        });

        const deleteItem = document.createElement('div');
        deleteItem.className = 'context-menu-item';
        deleteItem.textContent = 'Delete';
        deleteItem.style.color = '#f44747';
        deleteItem.addEventListener('click', () => {
            menu.remove();
            if (confirm(`Delete ${entry.name}?`)) {
                this.ws.send({
                    type: 'fs:delete',
                    session_id: '',
                    payload: { path: entry.path }
                });
            }
        });

        menu.appendChild(renameItem);
        if (entry.type === 'file') {
            const openItem = document.createElement('div');
            openItem.className = 'context-menu-item';
            openItem.textContent = 'Open';
            openItem.addEventListener('click', () => {
                menu.remove();
                if (this.onFileOpen) {
                    this.onFileOpen(entry.path, entry.name);
                }
            });
            menu.insertBefore(openItem, menu.firstChild);
        }
        menu.appendChild(deleteItem);

        document.body.appendChild(menu);

        const closeMenu = () => {
            menu.remove();
            document.removeEventListener('click', closeMenu);
        };
        setTimeout(() => document.addEventListener('click', closeMenu), 0);
    }

    formatSize(bytes) {
        if (bytes < 1024) return bytes + ' B';
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
        return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    }
}
