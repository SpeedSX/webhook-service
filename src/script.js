class WebhookService {
    constructor() {
        this.baseUrl = window.location.origin;
        this.tokens = [];
        this.init();
    }

    init() {
        this.bindEvents();
        this.loadTokens();
    }

    bindEvents() {
        document.getElementById('create-token').addEventListener('click', () => this.createToken());
        document.getElementById('refresh-tokens').addEventListener('click', () => this.loadTokens());
        document.getElementById('send-webhook').addEventListener('click', () => this.sendWebhook());
        document.getElementById('load-logs').addEventListener('click', () => this.loadLogs());
        
        // Auto-refresh tokens dropdown when tokens are loaded
        document.getElementById('selected-token-logs').addEventListener('change', (e) => {
            if (e.target.value) {
                this.loadLogs();
            }
        });
    }

    async createToken() {
        try {
            const response = await fetch(`${this.baseUrl}/api/tokens`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
            });

            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            const token = await response.json();
            this.tokens.unshift(token);
            this.renderTokens();
            this.updateTokenDropdowns();
            this.showMessage('Token created successfully!', 'success');
        } catch (error) {
            this.showMessage(`Error creating token: ${error.message}`, 'error');
        }
    }

    async loadTokens() {
        try {
            const response = await fetch(`${this.baseUrl}/api/tokens`);
            
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            this.tokens = await response.json();
            this.renderTokens();
            this.updateTokenDropdowns();
        } catch (error) {
            this.showMessage(`Error loading tokens: ${error.message}`, 'error');
        }
    }

    async deleteToken(token) {
        if (!confirm('Are you sure you want to delete this token? This will also delete all associated webhook logs.')) {
            return;
        }

        try {
            const response = await fetch(`${this.baseUrl}/api/tokens/${token}`, {
                method: 'DELETE',
            });

            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            this.tokens = this.tokens.filter(t => t.token !== token);
            this.renderTokens();
            this.updateTokenDropdowns();
            this.showMessage('Token deleted successfully!', 'success');
        } catch (error) {
            this.showMessage(`Error deleting token: ${error.message}`, 'error');
        }
    }

    renderTokens() {
        const container = document.getElementById('token-list');
        
        if (this.tokens.length === 0) {
            container.innerHTML = '<div class="loading">No tokens found. Create one to get started!</div>';
            return;
        }

        container.innerHTML = this.tokens.map(token => `
            <div class="token-item">
                <div class="token-info">
                    <div class="token-value">${token.token}</div>
                    <div class="token-url">${token.webhook_url}</div>
                    <div class="token-created">Created: ${new Date(token.created_at).toLocaleString()}</div>
                </div>
                <div class="token-actions">
                    <button class="btn btn-secondary" onclick="webhookService.selectToken('${token.token}')">Select</button>
                    <button class="btn btn-danger" onclick="webhookService.deleteToken('${token.token}')">Delete</button>
                </div>
            </div>
        `).join('');
    }

    updateTokenDropdowns() {
        const logsDropdown = document.getElementById('selected-token-logs');
        logsDropdown.innerHTML = '<option value="">Select a token to view logs</option>' +
            this.tokens.map(token => 
                `<option value="${token.token}">${token.token}</option>`
            ).join('');
    }

    selectToken(token) {
        const tokenInfo = this.tokens.find(t => t.token === token);
        if (tokenInfo) {
            document.getElementById('webhook-url').value = tokenInfo.webhook_url;
            document.getElementById('selected-token-logs').value = token;
            this.loadLogs();
        }
    }

    async sendWebhook() {
        const url = document.getElementById('webhook-url').value;
        const method = document.getElementById('http-method').value;
        const body = document.getElementById('request-body').value;
        const headersText = document.getElementById('custom-headers').value;

        if (!url) {
            this.showMessage('Please select a token first', 'error');
            return;
        }

        try {
            let customHeaders = {};
            if (headersText.trim()) {
                customHeaders = JSON.parse(headersText);
            }

            const requestOptions = {
                method: method,
                headers: {
                    'Content-Type': 'application/json',
                    ...customHeaders
                }
            };

            if (body.trim() && (method === 'POST' || method === 'PUT' || method === 'PATCH')) {
                requestOptions.body = body;
            }

            const response = await fetch(url, requestOptions);
            
            this.showMessage(
                `Webhook sent! Status: ${response.status} ${response.statusText}`, 
                response.ok ? 'success' : 'error'
            );

            // Auto-refresh logs if a token is selected
            const selectedToken = document.getElementById('selected-token-logs').value;
            if (selectedToken) {
                setTimeout(() => this.loadLogs(), 1000);
            }
        } catch (error) {
            this.showMessage(`Error sending webhook: ${error.message}`, 'error');
        }
    }

    async loadLogs() {
        const token = document.getElementById('selected-token-logs').value;
        const count = document.getElementById('log-count').value;

        if (!token) {
            document.getElementById('logs-container').innerHTML = '<div class="loading">Select a token to view logs</div>';
            return;
        }

        try {
            const response = await fetch(`${this.baseUrl}/${token}/log/${count}`);
            
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            const logs = await response.json();
            this.renderLogs(logs);
        } catch (error) {
            this.showMessage(`Error loading logs: ${error.message}`, 'error');
            document.getElementById('logs-container').innerHTML = '<div class="error">Failed to load logs</div>';
        }
    }

    renderLogs(logs) {
        const container = document.getElementById('logs-container');
        
        if (logs.length === 0) {
            container.innerHTML = '<div class="loading">No webhook requests found for this token</div>';
            return;
        }

        container.innerHTML = logs.map(log => `
            <div class="log-item">
                <div class="log-header">
                    <span class="log-method method-${log.MessageObject.Method.toLowerCase()}">${log.MessageObject.Method}</span>
                    <span class="log-id">ID: ${log.Id}</span>
                    <span class="log-timestamp">${new Date(log.Date).toLocaleString()}</span>
                </div>
                <div class="log-details">
                    <div class="log-url">${log.MessageObject.Value}</div>
                    ${log.MessageObject.Body ? `
                        <div class="log-body">${this.formatJson(log.MessageObject.Body)}</div>
                    ` : ''}
                    ${Object.keys(log.MessageObject.Headers).length > 0 ? `
                        <div class="log-headers">
                            <h4>Headers:</h4>
                            <pre>${this.formatHeaders(log.MessageObject.Headers)}</pre>
                        </div>
                    ` : ''}
                </div>
            </div>
        `).join('');
    }

    formatJson(str) {
        try {
            const obj = JSON.parse(str);
            return JSON.stringify(obj, null, 2);
        } catch {
            return str;
        }
    }

    formatHeaders(headers) {
        return Object.entries(headers)
            .map(([key, values]) => `${key}: ${values.join(', ')}`)
            .join('\n');
    }

    showMessage(message, type) {
        // Remove existing messages
        const existingMessages = document.querySelectorAll('.error, .success');
        existingMessages.forEach(msg => msg.remove());

        // Create new message
        const messageDiv = document.createElement('div');
        messageDiv.className = type;
        messageDiv.textContent = message;
        
        // Insert at the top of the main content
        const main = document.querySelector('main');
        main.insertBefore(messageDiv, main.firstChild);

        // Auto-remove after 5 seconds
        setTimeout(() => {
            if (messageDiv.parentNode) {
                messageDiv.remove();
            }
        }, 5000);
    }
}

// Initialize the webhook service when the page loads
const webhookService = new WebhookService();
