use config::DatabaseConfig;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

fn db_config(port: u16) -> DatabaseConfig {
    DatabaseConfig {
        driver: config::DatabaseDriver::Postgres,
        database_url: format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres"),
        max_connections: 2,
        connect_retry_attempts: 2,
        connect_retry_delay_ms: 1000,
        encrypt: false,
    }
}

#[tokio::test]
async fn create_user_should_persist() {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let config = db_config(port);

    migration::run(&config).await.unwrap();

    let repo = repo::connect(&config).await.unwrap();

    let user = repo
        .create("alice@example.com", "Alice", "user", true)
        .await
        .unwrap();

    assert_eq!(user.email, "alice@example.com");
    assert_eq!(user.display_name, "Alice");
    assert_eq!(user.role, "user");
    assert!(user.email_verified);
}

#[tokio::test]
async fn find_by_id_should_return_user() {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let config = db_config(port);

    migration::run(&config).await.unwrap();

    let repo = repo::connect(&config).await.unwrap();
    let created = repo
        .create("bob@example.com", "Bob", "user", true)
        .await
        .unwrap();

    let found = repo.find_by_id(created.id).await.unwrap();

    assert!(found.is_some());
    assert_eq!(found.unwrap().email, "bob@example.com");
}

#[tokio::test]
async fn find_by_id_not_found() {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let config = db_config(port);

    migration::run(&config).await.unwrap();

    let repo = repo::connect(&config).await.unwrap();

    let result = repo.find_by_id(uuid::Uuid::new_v4()).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn delete_user_should_remove() {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let config = db_config(port);

    migration::run(&config).await.unwrap();

    let repo = repo::connect(&config).await.unwrap();
    let created = repo
        .create("carol@example.com", "Carol", "user", true)
        .await
        .unwrap();

    repo.delete(created.id).await.unwrap();

    let found = repo.find_by_id(created.id).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn list_users_should_paginate() {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let config = db_config(port);

    migration::run(&config).await.unwrap();

    let repo = repo::connect(&config).await.unwrap();
    repo.create("a@example.com", "A", "user", true)
        .await
        .unwrap();
    repo.create("b@example.com", "B", "user", true)
        .await
        .unwrap();

    let (users, total) = repo.list(1, 10).await.unwrap();
    assert!(total >= 2);
    assert!(!users.is_empty());
}

#[tokio::test]
async fn health_check_should_pass() {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let config = db_config(port);

    migration::run(&config).await.unwrap();

    let repo = repo::connect(&config).await.unwrap();
    repo.health_check().await.unwrap();
}

#[tokio::test]
async fn jit_identity_flow() {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let config = db_config(port);

    migration::run(&config).await.unwrap();

    let repo = repo::connect(&config).await.unwrap();

    let user = repo
        .create("jit@example.com", "JIT User", "user", true)
        .await
        .unwrap();

    let identity = repo
        .create_identity(
            user.id,
            "oidc",
            "https://accounts.google.com",
            "google-12345",
        )
        .await
        .unwrap();

    assert_eq!(identity.provider, "oidc");
    assert_eq!(identity.issuer, "https://accounts.google.com");
    assert_eq!(identity.external_sub, "google-12345");
    assert_eq!(identity.user_id, user.id);

    let found = repo
        .find_identity("oidc", "https://accounts.google.com", "google-12345")
        .await
        .unwrap();
    assert!(found.is_some());

    let (found_user, _) = repo
        .find_by_identity("oidc", "https://accounts.google.com", "google-12345")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found_user.email, "jit@example.com");
}
