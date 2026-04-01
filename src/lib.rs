#![doc = include_str!("../README.md")]

#[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
use redis::TlsMode;
use redis::{
    Client, ErrorKind, RedisConnectionInfo, RedisError, RedisResult, cluster::ClusterClient,
};

/// A universal Redis client that works with both standalone Redis and Redis Cluster.
///
/// Wraps either a [`redis::Client`] or a [`redis::cluster::ClusterClient`], similar to
/// go-redis's `UniversalClient`.
///
/// # Examples
///
/// ```no_run
/// use redis::AsyncCommands;
/// use redis_universal_client::UniversalClient;
///
/// # async fn example() -> redis::RedisResult<()> {
/// // Standalone Redis
/// let client = UniversalClient::open(vec!["redis://127.0.0.1:6379"])?;
/// let mut conn = client.get_connection().await?;
/// conn.set::<_, _, ()>("key", "value").await?;
/// let val: String = conn.get("key").await?;
///
/// // Redis Cluster (multiple addresses)
/// let client = UniversalClient::open(vec![
///     "redis://127.0.0.1:7000",
///     "redis://127.0.0.1:7001",
/// ])?;
/// let mut conn = client.get_connection().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub enum UniversalClient {
    Client(Client),
    Cluster(ClusterClient),
}

impl UniversalClient {
    pub async fn get_connection(&self) -> RedisResult<UniversalConnection> {
        match self {
            Self::Client(cli) => cli
                .get_multiplexed_async_connection()
                .await
                .map(UniversalConnection::Client),
            Self::Cluster(cli) => cli
                .get_async_connection()
                .await
                .map(|c| UniversalConnection::Cluster(Box::new(c))),
        }
    }

    /// Creates a [`UniversalClient`] from a list of addresses.
    ///
    /// - 1 address: creates a standalone [`redis::Client`]
    /// - Multiple addresses: creates a [`redis::cluster::ClusterClient`]
    ///
    /// To force cluster mode with a single address, use [`UniversalBuilder`] instead.
    pub fn open<T: redis::IntoConnectionInfo + Clone>(
        addrs: Vec<T>,
    ) -> RedisResult<UniversalClient> {
        let mut addrs = addrs;

        if addrs.is_empty() {
            return Err(RedisError::from((
                ErrorKind::InvalidClientConfig,
                "No address specified",
            )));
        }

        if addrs.len() == 1 {
            Client::open(addrs.remove(0)).map(Self::Client)
        } else {
            ClusterClient::new(addrs).map(Self::Cluster)
        }
    }
}

/// Builder for [`UniversalClient`] with explicit control over cluster mode and credentials.
///
/// Unlike [`UniversalClient::open`], the builder lets you force cluster mode
/// regardless of the number of addresses, and set ACL username/password
/// programmatically rather than embedding them in the URL.
///
/// # Examples
///
/// ```no_run
/// use redis_universal_client::UniversalBuilder;
///
/// # fn example() -> redis::RedisResult<()> {
/// // Force cluster mode with a single address
/// let client = UniversalBuilder::new(vec!["redis://127.0.0.1:7000".to_string()])
///     .cluster(true)
///     .build()?;
///
/// // Standalone Redis with ACL credentials
/// let client = UniversalBuilder::new(vec!["redis://127.0.0.1:6379".to_string()])
///     .username("alice")
///     .password("secret")
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct UniversalBuilder<T> {
    addrs: Vec<T>,
    cluster: bool,
    username: Option<String>,
    password: Option<String>,
    #[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
    tls: Option<TlsMode>,
}

impl<T> UniversalBuilder<T> {
    pub fn new(addrs: Vec<T>) -> UniversalBuilder<T> {
        UniversalBuilder {
            addrs,
            cluster: false,
            username: None,
            password: None,
            #[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
            tls: None,
        }
    }

    pub fn cluster(mut self, flag: bool) -> UniversalBuilder<T> {
        self.cluster = flag;
        self
    }

    /// Set the ACL username for authentication (Redis 6.0+).
    pub fn username(mut self, username: impl Into<String>) -> UniversalBuilder<T> {
        self.username = Some(username.into());
        self
    }

    /// Set the password for authentication.
    pub fn password(mut self, password: impl Into<String>) -> UniversalBuilder<T> {
        self.password = Some(password.into());
        self
    }

    /// Enable TLS. Use [`TlsMode::Secure`] to verify certificates (recommended)
    /// or [`TlsMode::Insecure`] to skip verification.
    ///
    /// Requires the `tls-native-tls` or `tls-rustls` feature.
    #[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
    pub fn tls(mut self, mode: TlsMode) -> UniversalBuilder<T> {
        self.tls = Some(mode);
        self
    }

    pub fn build(self) -> RedisResult<UniversalClient>
    where
        T: redis::IntoConnectionInfo + Clone,
    {
        let UniversalBuilder {
            mut addrs,
            cluster,
            username,
            password,
            #[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
            tls,
        } = self;

        if addrs.is_empty() {
            return Err(RedisError::from((
                ErrorKind::InvalidClientConfig,
                "No address specified",
            )));
        }

        if cluster {
            let mut builder = ClusterClient::builder(addrs);
            if let Some(u) = username {
                builder = builder.username(u);
            }
            if let Some(p) = password {
                builder = builder.password(p);
            }
            #[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
            if let Some(mode) = tls {
                builder = builder.tls(mode);
            }
            builder.build().map(UniversalClient::Cluster)
        } else if username.is_some() || password.is_some() || {
            #[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
            {
                tls.is_some()
            }
            #[cfg(not(any(feature = "tls-native-tls", feature = "tls-rustls")))]
            {
                false
            }
        } {
            let conn_info = addrs.remove(0).into_connection_info()?;
            let orig = conn_info.redis_settings();
            let mut redis_info = RedisConnectionInfo::default()
                .set_db(orig.db())
                .set_protocol(orig.protocol());
            if let Some(u) = username {
                redis_info = redis_info.set_username(u);
            }
            if let Some(p) = password {
                redis_info = redis_info.set_password(p);
            }
            let conn_info = conn_info.set_redis_settings(redis_info);
            #[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
            let conn_info = if let Some(mode) = tls {
                apply_tls_to_conn_info(conn_info, mode)?
            } else {
                conn_info
            };
            Client::open(conn_info).map(UniversalClient::Client)
        } else {
            Client::open(addrs.remove(0)).map(UniversalClient::Client)
        }
    }
}

/// Converts a `ConnectionInfo` with a plain TCP address to TLS by replacing
/// `ConnectionAddr::Tcp` with `ConnectionAddr::TcpTls`.
///
/// If the address is already TLS or is a Unix socket, it is left unchanged.
#[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
fn apply_tls_to_conn_info(
    conn_info: redis::ConnectionInfo,
    mode: TlsMode,
) -> RedisResult<redis::ConnectionInfo> {
    let insecure = mode == TlsMode::Insecure;
    let new_addr = match conn_info.addr() {
        redis::ConnectionAddr::Tcp(host, port) => redis::ConnectionAddr::TcpTls {
            host: host.clone(),
            port: *port,
            insecure,
            tls_params: None,
        },
        // Already TLS or Unix socket — leave as-is
        other => other.clone(),
    };
    Ok(conn_info.set_addr(new_addr))
}

/// Async multiplexed connection for both standalone and cluster Redis.
///
/// Wraps either a [`redis::aio::MultiplexedConnection`] or a
/// [`redis::cluster_async::ClusterConnection`]. Implements [`redis::aio::ConnectionLike`],
/// so all [`redis::AsyncCommands`] work transparently.
///
/// Both variants are `Clone + Send + Sync`.
#[derive(Clone)]
pub enum UniversalConnection {
    Client(redis::aio::MultiplexedConnection),
    Cluster(Box<redis::cluster_async::ClusterConnection>),
}

#[cfg(test)]
impl UniversalClient {
    fn is_client(&self) -> bool {
        matches!(self, Self::Client(_))
    }

    fn is_cluster(&self) -> bool {
        matches!(self, Self::Cluster(_))
    }
}

impl redis::aio::ConnectionLike for UniversalConnection {
    fn req_packed_command<'a>(
        &'a mut self,
        cmd: &'a redis::Cmd,
    ) -> redis::RedisFuture<'a, redis::Value> {
        match self {
            Self::Client(conn) => conn.req_packed_command(cmd),
            Self::Cluster(conn) => conn.req_packed_command(cmd),
        }
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        cmd: &'a redis::Pipeline,
        offset: usize,
        count: usize,
    ) -> redis::RedisFuture<'a, Vec<redis::Value>> {
        match self {
            Self::Client(conn) => conn.req_packed_commands(cmd, offset, count),
            Self::Cluster(conn) => conn.req_packed_commands(cmd, offset, count),
        }
    }

    fn get_db(&self) -> i64 {
        match self {
            Self::Client(conn) => conn.get_db(),
            Self::Cluster(conn) => conn.get_db(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_empty_addrs_error() {
        let result = UniversalClient::open(Vec::<String>::new());
        assert!(result.is_err());
    }

    #[test]
    fn open_single_addr_is_client() {
        let result = UniversalClient::open(vec!["redis://127.0.0.1:6379"]);
        assert!(result.unwrap().is_client());
    }

    #[test]
    fn open_multiple_addrs_is_cluster() {
        let result =
            UniversalClient::open(vec!["redis://127.0.0.1:7000", "redis://127.0.0.1:7001"]);
        assert!(result.unwrap().is_cluster());
    }

    #[test]
    fn builder_empty_addrs_error() {
        let result = UniversalBuilder::new(Vec::<String>::new()).build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_cluster_true_forces_cluster() {
        let result = UniversalBuilder::new(vec!["redis://127.0.0.1:6379".to_string()])
            .cluster(true)
            .build();
        assert!(result.unwrap().is_cluster());
    }

    #[test]
    fn builder_cluster_false_uses_first_addr() {
        let result = UniversalBuilder::new(vec![
            "redis://127.0.0.1:7000".to_string(),
            "redis://127.0.0.1:7001".to_string(),
        ])
        .cluster(false)
        .build();
        assert!(result.unwrap().is_client());
    }

    #[test]
    fn builder_with_password_is_client() {
        let result = UniversalBuilder::new(vec!["redis://127.0.0.1:6379".to_string()])
            .password("secret")
            .build();
        assert!(result.unwrap().is_client());
    }

    #[test]
    fn builder_with_username_and_password_is_client() {
        let result = UniversalBuilder::new(vec!["redis://127.0.0.1:6379".to_string()])
            .username("alice")
            .password("secret")
            .build();
        assert!(result.unwrap().is_client());
    }

    #[test]
    fn builder_with_password_cluster_is_cluster() {
        let result = UniversalBuilder::new(vec![
            "redis://127.0.0.1:7000".to_string(),
            "redis://127.0.0.1:7001".to_string(),
        ])
        .password("secret")
        .cluster(true)
        .build();
        assert!(result.unwrap().is_cluster());
    }

    #[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
    #[test]
    fn builder_tls_secure_is_client() {
        let result = UniversalBuilder::new(vec!["redis://127.0.0.1:6380".to_string()])
            .tls(redis::TlsMode::Secure)
            .build();
        assert!(result.unwrap().is_client());
    }

    #[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
    #[test]
    fn builder_tls_insecure_is_client() {
        let result = UniversalBuilder::new(vec!["redis://127.0.0.1:6380".to_string()])
            .tls(redis::TlsMode::Insecure)
            .build();
        assert!(result.unwrap().is_client());
    }

    #[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
    #[test]
    fn builder_tls_cluster_is_cluster() {
        let result = UniversalBuilder::new(vec![
            "redis://127.0.0.1:7000".to_string(),
            "redis://127.0.0.1:7001".to_string(),
        ])
        .tls(redis::TlsMode::Secure)
        .cluster(true)
        .build();
        assert!(result.unwrap().is_cluster());
    }
}
