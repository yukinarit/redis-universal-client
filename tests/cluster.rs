use redis::AsyncCommands;
use redis_universal_client::{UniversalBuilder, UniversalClient};
use std::sync::OnceLock;
use testcontainers_modules::testcontainers::{
    core::{ContainerPort, WaitFor},
    runners::SyncRunner,
    Container, GenericImage, ImageExt,
};

fn should_run() -> bool {
    std::env::var("RUN_CLUSTER_TESTS").is_ok()
}

const BASE_PORT: u16 = 7100;

fn cluster_container() -> &'static Container<GenericImage> {
    static CONTAINER: OnceLock<Container<GenericImage>> = OnceLock::new();
    CONTAINER.get_or_init(|| {
        GenericImage::new("grokzen/redis-cluster", "7.0.10")
            .with_exposed_port(ContainerPort::Tcp(BASE_PORT))
            .with_exposed_port(ContainerPort::Tcp(BASE_PORT + 1))
            .with_exposed_port(ContainerPort::Tcp(BASE_PORT + 2))
            .with_exposed_port(ContainerPort::Tcp(BASE_PORT + 3))
            .with_exposed_port(ContainerPort::Tcp(BASE_PORT + 4))
            .with_exposed_port(ContainerPort::Tcp(BASE_PORT + 5))
            .with_wait_for(WaitFor::message_on_stdout("Cluster state changed: ok"))
            .with_env_var("IP", "0.0.0.0")
            .with_env_var("INITIAL_PORT", BASE_PORT.to_string())
            .with_mapped_port(BASE_PORT, ContainerPort::Tcp(BASE_PORT))
            .with_mapped_port(BASE_PORT + 1, ContainerPort::Tcp(BASE_PORT + 1))
            .with_mapped_port(BASE_PORT + 2, ContainerPort::Tcp(BASE_PORT + 2))
            .with_mapped_port(BASE_PORT + 3, ContainerPort::Tcp(BASE_PORT + 3))
            .with_mapped_port(BASE_PORT + 4, ContainerPort::Tcp(BASE_PORT + 4))
            .with_mapped_port(BASE_PORT + 5, ContainerPort::Tcp(BASE_PORT + 5))
            .with_startup_timeout(std::time::Duration::from_secs(60))
            .start()
            .unwrap()
    })
}

fn ensure_cluster() -> bool {
    if !should_run() {
        return false;
    }
    // Initialize on a separate thread to avoid "block_on inside runtime" panic
    std::thread::scope(|s| {
        s.spawn(|| {
            cluster_container();
        })
        .join()
        .unwrap();
    });
    true
}

fn cluster_addrs() -> Vec<String> {
    (BASE_PORT..=BASE_PORT + 5)
        .map(|p| format!("redis://127.0.0.1:{}", p))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn cluster_open_set_get() {
    if !ensure_cluster() {
        return;
    }

    let addrs = cluster_addrs();
    let client = UniversalClient::open(addrs).unwrap();
    let mut conn = client.get_connection().await.unwrap();

    let _: () = conn.set("cluster_key", "cluster_value").await.unwrap();
    let val: String = conn.get("cluster_key").await.unwrap();
    assert_eq!(val, "cluster_value");
}

#[tokio::test(flavor = "multi_thread")]
async fn cluster_builder_force_cluster() {
    if !ensure_cluster() {
        return;
    }

    let client = UniversalBuilder::new(vec![format!("redis://127.0.0.1:{}", BASE_PORT)])
        .cluster(true)
        .build()
        .unwrap();
    let mut conn = client.get_connection().await.unwrap();

    let _: () = conn.set("builder_ck", "builder_cv").await.unwrap();
    let val: String = conn.get("builder_ck").await.unwrap();
    assert_eq!(val, "builder_cv");
}

#[tokio::test(flavor = "multi_thread")]
async fn cluster_keys_different_slots() {
    if !ensure_cluster() {
        return;
    }

    let addrs = cluster_addrs();
    let client = UniversalClient::open(addrs).unwrap();
    let mut conn = client.get_connection().await.unwrap();

    // Keys with different hash tags route to different slots
    let _: () = conn.set("{a}key", "val_a").await.unwrap();
    let _: () = conn.set("{b}key", "val_b").await.unwrap();
    let _: () = conn.set("{c}key", "val_c").await.unwrap();

    let va: String = conn.get("{a}key").await.unwrap();
    let vb: String = conn.get("{b}key").await.unwrap();
    let vc: String = conn.get("{c}key").await.unwrap();

    assert_eq!(va, "val_a");
    assert_eq!(vb, "val_b");
    assert_eq!(vc, "val_c");
}
