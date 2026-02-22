use super::{ConfigError, SerializableConfig};
use std::path::Path;
use tokio::fs::{canonicalize, read_to_string};

/// Reads a configuration file recursively.
pub async fn read_config(path: &Path) -> Result<SerializableConfig, ConfigError> {
    let mut stack = Vec::new();
    let mut configs = Vec::new();
    let mut path = path.to_path_buf();

    loop {
        path = canonicalize(&path).await?;

        if let Some(index) = stack.iter().position(|item| item == &path) {
            let mut cycle = stack[index..].to_vec();
            cycle.push(path);
            return Err(ConfigError::CircularConfigFiles(cycle));
        }

        stack.push(path.clone());

        let contents = read_to_string(&path).await?;
        let config: SerializableConfig = ::toml::from_str(&contents)?;
        let extend_path = config.extend().map(ToOwned::to_owned);
        configs.push(config);

        let Some(extend_path) = extend_path else {
            break;
        };
        path = if extend_path.is_absolute() {
            extend_path
        } else {
            path.parent()
                .unwrap_or_else(|| Path::new("."))
                .join(extend_path)
        };
    }

    let mut configs = configs.into_iter().rev();
    let mut merged = configs.next().unwrap_or_default();

    for config in configs {
        merged.merge(config);
    }

    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::{ConfigError, read_config};
    use crate::config::compile_config;
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use std::{
        env::temp_dir,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tokio::fs::{create_dir_all, remove_dir_all, write};

    async fn create_temp_directory() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = temp_dir().join(format!("muffy-test-{}-{unique}", std::process::id()));
        create_dir_all(&directory).await.unwrap();
        directory
    }

    #[tokio::test]
    async fn read_config_merge_extends() {
        let directory = create_temp_directory().await;
        let base_path = directory.join("base.toml");
        let middle_path = directory.join("middle.toml");
        let child_path = directory.join("child.toml");

        write(
            &base_path,
            indoc! {r#"
                concurrency = 1
                [sites.default]
                roots = ["https://example.com/"]
                max_redirects = 5
            "#},
        )
        .await
        .unwrap();
        write(
            &middle_path,
            indoc! {r#"
                extend = "base.toml"
                concurrency = 2
                [sites.default]
                max_redirects = 10
            "#},
        )
        .await
        .unwrap();
        write(
            &child_path,
            indoc! {r#"
                extend = "middle.toml"
                [sites.default]
                recurse = true
            "#},
        )
        .await
        .unwrap();

        let config = compile_config(read_config(&child_path).await.unwrap()).unwrap();

        assert_eq!(config.concurrency().global(), Some(2));

        let site = config.sites().get("example.com").unwrap();

        assert_eq!(site[0].1.max_redirects(), 10);
        assert!(site[0].1.recursive());
        assert_eq!(
            config.roots().collect::<Vec<_>>(),
            vec!["https://example.com/"]
        );
        remove_dir_all(&directory).await.unwrap();
    }

    #[tokio::test]
    async fn read_config_resolve_relative_paths() {
        let directory = create_temp_directory().await;
        let base_path = directory.join("base.toml");
        let nested_path = directory.join("nested");
        let child_path = nested_path.join("child.toml");

        create_dir_all(&nested_path).await.unwrap();
        write(
            &base_path,
            indoc! {r#"
                concurrency = 5
                sites = {}
            "#},
        )
        .await
        .unwrap();
        write(
            &child_path,
            indoc! {r#"
                extend = "../base.toml"
                sites = {}
            "#},
        )
        .await
        .unwrap();

        let config = compile_config(read_config(&child_path).await.unwrap()).unwrap();

        assert_eq!(config.concurrency().global(), Some(5));
        remove_dir_all(&directory).await.unwrap();
    }

    #[tokio::test]
    async fn read_config_detect_circular_extends() {
        let directory = create_temp_directory().await;
        let first_path = directory.join("first.toml");
        let second_path = directory.join("second.toml");

        write(
            &first_path,
            indoc! {r#"
                extend = "second.toml"
                sites = {}
            "#},
        )
        .await
        .unwrap();
        write(
            &second_path,
            indoc! {r#"
                extend = "first.toml"
                sites = {}
            "#},
        )
        .await
        .unwrap();

        let result = read_config(&first_path).await;

        assert!(matches!(result, Err(ConfigError::CircularConfigFiles(_))));
        remove_dir_all(&directory).await.unwrap();
    }
}
