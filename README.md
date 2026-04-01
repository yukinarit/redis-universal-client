# redis-universal-client

Simple wrapper around `redis::Client` and `redis::cluster::ClusterClient`, inspired by go-redis [UniversalClient](https://pkg.go.dev/github.com/redis/go-redis/v9#UniversalClient).

Provides a single `UniversalClient` type that works with both standalone Redis and Redis Cluster, using async multiplexed connections under the hood.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
redis-universal-client = "0.3.0"
```

### Standalone Redis

```rust
use redis::AsyncCommands;
use redis_universal_client::UniversalClient;

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    let client = UniversalClient::open(vec!["redis://127.0.0.1:6379"])?;
    let mut conn = client.get_connection().await?;

    let _: () = conn.set("key", "value").await?;
    let val: String = conn.get("key").await?;
    println!("{}", val);
    Ok(())
}
```

### Redis Cluster

When multiple addresses are provided, `open` automatically creates a cluster client:

```rust
use redis::AsyncCommands;
use redis_universal_client::UniversalClient;

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    let client = UniversalClient::open(vec![
        "redis://127.0.0.1:7000",
        "redis://127.0.0.1:7001",
        "redis://127.0.0.1:7002",
    ])?;
    let mut conn = client.get_connection().await?;

    let _: () = conn.set("key", "value").await?;
    let val: String = conn.get("key").await?;
    println!("{}", val);
    Ok(())
}
```

### Builder

Use `UniversalBuilder` for explicit control over cluster mode, credentials, and TLS:

```rust
use redis_universal_client::UniversalBuilder;

// Force cluster mode with a single address
let client = UniversalBuilder::new(vec!["redis://127.0.0.1:7000".to_string()])
    .cluster(true)
    .build()?;

// ACL authentication (Redis 6.0+)
let client = UniversalBuilder::new(vec!["redis://127.0.0.1:6379".to_string()])
    .username("alice")
    .password("secret")
    .build()?;

// TLS (requires the tls-rustls or tls-native-tls feature)
let client = UniversalBuilder::new(vec!["redis://127.0.0.1:6380".to_string()])
    .tls(redis::TlsMode::Secure)
    .build()?;
```

## Features

All TLS features use the same names as the underlying `redis` crate.

| Feature | Description |
|---|---|
| `tls-native-tls` | TLS via native-tls (OS certificate store) |
| `tls-rustls` | TLS via rustls (native OS certificate store) |
| `tls-rustls-insecure` | rustls with support for `TlsMode::Insecure` (skips certificate verification) |
| `tls-rustls-webpki-roots` | rustls with Mozilla's WebPKI root certificates instead of the OS store |
| `tokio-native-tls-comp` | Alias for `tls-native-tls` |
| `tokio-rustls-comp` | Alias for `tls-rustls` |

TLS can also be enabled without these features by using a `rediss://` URL directly:

```toml
[dependencies]
redis-universal-client = { version = "0.3.0", features = ["tls-rustls"] }
```

```rust
// rediss:// enables TLS (secure); rediss://<host>/#insecure skips cert verification
let client = UniversalClient::open(vec!["rediss://127.0.0.1:6380"])?;
```

## How it works

| Addresses | `open` behavior | `UniversalBuilder` behavior |
|-----------|----------------|-----------------------------|
| 1 address | `Client` (standalone) | Depends on `.cluster()` flag |
| N addresses | `ClusterClient` | Depends on `.cluster()` flag |

`UniversalConnection` implements `redis::aio::ConnectionLike`, so you can use all `redis::AsyncCommands` on it. Both variants use async multiplexed connections (`MultiplexedConnection` / `cluster_async::ClusterConnection`) which are `Clone + Send + Sync`.

## Testing

```bash
# Unit tests (no Docker required)
cargo test --lib

# Integration tests with standalone Redis (requires Docker)
cargo test --test single

# Integration tests with ACL authentication (requires Docker)
cargo test --test acl

# Integration tests with Redis Cluster (requires Docker)
RUN_CLUSTER_TESTS=1 cargo test --test cluster
```

## License

BSD-3-Clause
