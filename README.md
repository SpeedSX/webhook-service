# Webhook Test Service

A web service that provides webhook testing capabilities and a web interface for monitoring webhook requests.

## Features

- **Webhook Reception**: Accepts HTTP requests at `/{token}` endpoints
- **Request Storage**: Stores incoming webhook requests with full metadata
- **CLI Compatibility**: Provides the exact API that WebhookUI-CLI expects
- **Web Interface**: User-friendly web UI for testing and monitoring
- **Token Management**: Generate, list, and delete webhook tokens
- **Real-time Logs**: View webhook request logs through the web interface

## API Endpoints

### Webhook Endpoints
- `POST/GET/PUT/DELETE /{token}` - Webhook endpoint (accepts any HTTP method)
- `GET /{token}/log/{count}` - Retrieve webhook logs (CLI compatible)

### Management Endpoints
- `POST /api/tokens` - Generate new webhook token
- `GET /api/tokens` - List all tokens
- `DELETE /api/tokens/{token}` - Delete a token and its logs

### Web Interface
- `GET /` - Web interface for testing and monitoring

## Quick Start

1. **Install Dependencies**:
   ```bash
   cd webhook-service
   cargo build --release
   ```

2. **Run the Service**:
   ```bash
   cargo run
   ```

3. **Access the Web Interface**:
   Open http://localhost:3000 in your browser

4. **Use with WebhookUI-CLI**:
   ```bash
   # Update your CLI config to point to this service
   # In config.toml or config.local.toml:
   base_url = "http://localhost:3000"
   
   # Generate a token
   webhook generate
   
   # Monitor webhooks
   webhook monitor --token <your-token>
   ```

## Usage with WebhookUI-CLI

1. **Update CLI Configuration**:
   Edit `config.toml` or `config.local.toml` in your CLI project:
   ```toml
   [webhook]
   base_url = "http://localhost:3000"
   ```

2. **Generate a Token**:
   ```bash
   webhook generate
   ```

3. **Monitor Webhooks**:
   ```bash
   webhook monitor --token <generated-token>
   ```

4. **View Logs**:
   ```bash
   webhook logs --token <generated-token>
   ```

## Web Interface Features

- **Token Management**: Create, view, and delete webhook tokens
- **Webhook Testing**: Send test webhook requests with custom headers and body
- **Log Viewing**: Browse webhook request logs with detailed information
- **Real-time Updates**: Refresh logs to see new incoming requests

## Database

The service uses SQLite for data storage. The database file (`webhook_service.db`) is created automatically in the service directory.

## Configuration

The service runs on `0.0.0.0:3000` by default. To change the port or host, modify the `main.rs` file:

```rust
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
```

## Development

### Project Structure
```
webhook-service/
├── src/
│   ├── main.rs              # Main application and routes
│   ├── models.rs            # Data models (compatible with CLI)
│   ├── database.rs          # Database operations
│   ├── web_interface.html   # Web interface HTML
│   ├── style.css           # Web interface styles
│   └── script.js           # Web interface JavaScript
├── Cargo.toml              # Dependencies
└── README.md               # This file
```

### Building
```bash
cargo build --release
```

### Running in Development
```bash
cargo run
```

## Compatibility

This service is designed to work seamlessly with the WebhookUI-CLI tool. It implements the exact API structure that the CLI expects:

- Webhook requests are stored with the same data structure
- The logs endpoint returns data in the format expected by the CLI
- All field names and data types match the CLI's models

## License

MIT License - same as the WebhookUI-CLI project.
