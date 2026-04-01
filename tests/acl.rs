use redis::AsyncCommands;
use redis_universal_client::{UniversalBuilder, UniversalClient};
use testcontainers_modules::testcontainers::{
    ContainerAsync, GenericImage, ImageExt, core::WaitFor, runners::AsyncRunner,
};

async fn redis_with_password(password: &str) -> (ContainerAsync<GenericImage>, String, String) {
    let container = GenericImage::new("redis", "7")
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
        .with_cmd(["--requirepass", password])
        .start()
        .await
        .unwrap();
    let host = container.get_host().await.unwrap().to_string();
    let port = container.get_host_port_ipv4(6379).await.unwrap();
    let url = format!("redis://{}:{}", host, port);
    (container, url, password.to_string())
}

/// Connect with a password only (no explicit username).
/// Redis treats this as auth for the `default` user.
#[tokio::test]
async fn builder_password_only_auth() {
    let (_container, url, password) = redis_with_password("s3cret").await;

    let client = UniversalBuilder::new(vec![url])
        .password(password)
        .build()
        .unwrap();
    let mut conn = client.get_connection().await.unwrap();

    let _: () = conn.set("pw_key", "pw_value").await.unwrap();
    let val: String = conn.get("pw_key").await.unwrap();
    assert_eq!(val, "pw_value");
}

/// Connection without credentials should fail when Redis requires a password.
#[tokio::test]
async fn connection_without_password_is_rejected() {
    let (_container, url, _) = redis_with_password("s3cret").await;

    let client = UniversalBuilder::new(vec![url]).build().unwrap();
    // Auth is checked when commands are executed, not at connection time
    let conn = client.get_connection().await;
    let result = match conn {
        Err(e) => Err(e),
        Ok(mut c) => c.set::<_, _, ()>("k", "v").await,
    };
    assert!(result.is_err(), "expected auth error, got success");
}

/// Connect with an explicit ACL username and password (Redis 6+ ACL).
///
/// Steps:
/// 1. Start Redis with a requirepass so the default user has a password.
/// 2. Connect as `default` to run `ACL SETUSER` and create user `alice`.
/// 3. Reconnect as `alice` using `.username()` + `.password()`.
#[tokio::test]
async fn builder_username_and_password_acl() {
    const ADMIN_PASS: &str = "adminpass";
    const ALICE_PASS: &str = "alicepass";

    let (_container, url, _) = redis_with_password(ADMIN_PASS).await;

    // Set up ACL user via the default (admin) connection
    let admin_url = url.replace("redis://", &format!("redis://:{}@", ADMIN_PASS));
    let admin_client = UniversalClient::open(vec![admin_url]).unwrap();
    let mut admin_conn = admin_client.get_connection().await.unwrap();

    let _: () = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("alice")
        .arg("on")
        .arg(format!(">{}", ALICE_PASS))
        .arg("~*")
        .arg("&*")
        .arg("+@all")
        .query_async(&mut admin_conn)
        .await
        .unwrap();

    // Now connect as alice
    let client = UniversalBuilder::new(vec![url])
        .username("alice")
        .password(ALICE_PASS)
        .build()
        .unwrap();
    let mut conn = client.get_connection().await.unwrap();

    let _: () = conn.set("alice_key", "alice_value").await.unwrap();
    let val: String = conn.get("alice_key").await.unwrap();
    assert_eq!(val, "alice_value");
}
