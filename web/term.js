class TerminalManager {
    constructor(containerId) {
        this.container = document.getElementById(containerId);
        this.term = new Terminal({
            cursorBlink: true,
            fontSize: 14,
            fontFamily: "'SF Mono', 'Fira Code', 'Consolas', monospace",
            theme: {
                background: '#1e1e1e',
                foreground: '#d4d4d4',
                cursor: '#d4d4d4',
                selectionBackground: '#264f78',
                black: '#000000',
                red: '#cd3131',
                green: '#0dbc79',
                yellow: '#e5e510',
                blue: '#2472c8',
                magenta: '#bc3fbc',
                cyan: '#11a8cd',
                white: '#e5e5e5',
                brightBlack: '#666666',
                brightRed: '#f14c4c',
                brightGreen: '#23d18b',
                brightYellow: '#f5f543',
                brightBlue: '#3b8eea',
                brightMagenta: '#d670d6',
                brightCyan: '#29b8db',
                brightWhite: '#ffffff',
            },
        });

        this.fitAddon = new FitAddon.FitAddon();
        this.term.loadAddon(this.fitAddon);

        try {
            this.webglAddon = new WebglAddon.WebglAddon();
            this.term.loadAddon(this.webglAddon);
        } catch (e) {
            console.warn('WebGL addon not available, falling back to canvas');
        }

        this.term.open(this.container);
        this.fitAddon.fit();

        window.addEventListener('resize', () => this.resize());

        this.term.onResize(({ cols, rows }) => {
            if (this.onResizeCallback) {
                this.onResizeCallback(cols, rows);
            }
        });

        this.onInputCallback = null;
        this.onResizeCallback = null;

        this.term.onData((data) => {
            if (this.onInputCallback) {
                this.onInputCallback(data);
            }
        });
    }

    write(data) {
        this.term.write(data);
    }

    onInput(callback) {
        this.onInputCallback = callback;
    }

    onResize(callback) {
        this.onResizeCallback = callback;
    }

    resize() {
        if (this.fitAddon) {
            try {
                this.fitAddon.fit();
            } catch (e) {
                console.warn('Fit error:', e);
            }
        }
    }

    getCols() {
        return this.term.cols;
    }

    getRows() {
        return this.term.rows;
    }

    focus() {
        this.term.focus();
    }
}
