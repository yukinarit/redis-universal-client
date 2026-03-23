# redis-universal-client

Simple wrapper around `redis::Client` and `redis::cluster::ClusterClient`, inspired by go-redis [UniversalClient](https://pkg.go.dev/github.com/redis/go-redis/v9#UniversalClient).

Provides a single `UniversalClient` type that works with both standalone Redis and Redis Cluster, using async multiplexed connections under the hood.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
redis-universal-client = "0.0.3"
```

### Standalone Redis

```rust
use redis::AsyncCommands;
use redis_universal_client::UniversalClient;

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    let client = UniversalClient::open(vec!["redis://127.0.0.1:6379"])?;
    let mut conn = client.get_connection().await?;

    conn.set("key", "value").await?;
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

    conn.set("key", "value").await?;
    let val: String = conn.get("key").await?;
    println!("{}", val);
    Ok(())
}
```

### Builder

Use `UniversalBuilder` to force cluster mode even with a single address:

```rust
use redis_universal_client::UniversalBuilder;

let client = UniversalBuilder::new(vec!["redis://127.0.0.1:7000".to_string()])
    .cluster(true)
    .build()?;
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
cargo test --test single_redis

# Integration tests with Redis Cluster (requires Docker)
RUN_CLUSTER_TESTS=1 cargo test --test cluster_redis
```

## License

BSD-3-Clause
