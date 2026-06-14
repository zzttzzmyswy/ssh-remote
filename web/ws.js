class WSClient {
    constructor() {
        this.ws = null;
        this.url = null;
        this.onMessage = null;
        this.onOpen = null;
        this.onClose = null;
        this.reconnectDelay = 1000;
        this.maxReconnectDelay = 30000;
        this.shouldReconnect = false;
    }

    connect(url) {
        this.url = url;
        this.shouldReconnect = true;
        this._connect();
    }

    _connect() {
        if (this.ws) {
            this.ws.onclose = null;
            this.ws.close();
        }

        this.ws = new WebSocket(this.url);

        this.ws.onopen = () => {
            console.log('WebSocket connected');
            this.reconnectDelay = 1000;
            if (this.onOpen) this.onOpen();
        };

        this.ws.onmessage = (event) => {
            if (this.onMessage) this.onMessage(event.data);
        };

        this.ws.onclose = () => {
            console.log('WebSocket closed');
            if (this.onClose) this.onClose();
            if (this.shouldReconnect) {
                this._scheduleReconnect();
            }
        };

        this.ws.onerror = (err) => {
            console.error('WebSocket error:', err);
        };
    }

    _scheduleReconnect() {
        console.log(`Reconnecting in ${this.reconnectDelay}ms...`);
        setTimeout(() => {
            if (this.shouldReconnect) {
                this._connect();
                this.reconnectDelay = Math.min(
                    this.reconnectDelay * 2,
                    this.maxReconnectDelay
                );
            }
        }, this.reconnectDelay);
    }

    send(msg) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            if (typeof msg === 'object') {
                this.ws.send(JSON.stringify(msg));
            } else {
                this.ws.send(msg);
            }
        }
    }

    close() {
        this.shouldReconnect = false;
        if (this.ws) {
            this.ws.close();
        }
    }
}
