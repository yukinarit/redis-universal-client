use redis::AsyncCommands;
use redis_universal_client::{UniversalBuilder, UniversalClient};
use testcontainers_modules::{
    redis::Redis,
    testcontainers::{runners::AsyncRunner, ContainerAsync},
};

async fn redis_url() -> (ContainerAsync<Redis>, String) {
    let container = Redis::default().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(6379).await.unwrap();
    let url = format!("redis://{}:{}", host, port);
    (container, url)
}

#[tokio::test]
async fn open_set_get() {
    let (_container, url) = redis_url().await;

    let client = UniversalClient::open(vec![url]).unwrap();
    let mut conn = client.get_connection().await.unwrap();

    let _: () = conn.set("key1", "value1").await.unwrap();
    let val: String = conn.get("key1").await.unwrap();
    assert_eq!(val, "value1");
}

#[tokio::test]
async fn builder_set_get() {
    let (_container, url) = redis_url().await;

    let client = UniversalBuilder::new(vec![url]).build().unwrap();
    let mut conn = client.get_connection().await.unwrap();

    let _: () = conn.set("builder_key", "builder_value").await.unwrap();
    let val: String = conn.get("builder_key").await.unwrap();
    assert_eq!(val, "builder_value");
}

#[tokio::test]
async fn pipeline() {
    let (_container, url) = redis_url().await;

    let client = UniversalClient::open(vec![url]).unwrap();
    let mut conn = client.get_connection().await.unwrap();

    redis::pipe()
        .set("pk1", "pv1")
        .set("pk2", "pv2")
        .set("pk3", "pv3")
        .exec_async(&mut conn)
        .await
        .unwrap();

    let (v1, v2, v3): (String, String, String) = redis::pipe()
        .get("pk1")
        .get("pk2")
        .get("pk3")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(v1, "pv1");
    assert_eq!(v2, "pv2");
    assert_eq!(v3, "pv3");
}
