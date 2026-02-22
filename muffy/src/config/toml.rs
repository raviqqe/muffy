use super::{ConfigError, SerializableConfig};
use std::path::Path;
use tokio::fs::{canonicalize, read_to_string};

/// Reads a configuration file recursively.
pub async fn read_config(path: &Path) -> Result<SerializableConfig, ConfigError> {
    let mut paths = vec![];
    let mut previous_path = canonicalize(path).await?;
    let mut config = read_bare_config(path).await?;

    while let Some(path) = config.extend() {
        let path = canonicalize(
            previous_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(path),
        )
        .await?;

        if let Some(index) = paths.iter().position(|item| item == &path) {
            paths.push(path);
            return Err(ConfigError::CircularConfigFiles(paths[index..].to_vec()));
        }

        paths.push(path.clone());
        let mut parent = read_bare_config(&path).await?;
        parent.merge(config);
        config = parent;
        previous_path = path;
    }

    Ok(config)
}

async fn read_bare_config(path: &Path) -> Result<SerializableConfig, ConfigError> {
    Ok(toml::from_str(&read_to_string(&path).await?)?)
}

#[cfg(test)]
mod tests {
    use super::{ConfigError, read_config};
    use crate::config::compile_config;
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;
    use tokio::fs::{create_dir_all, write};

    #[tokio::test]
    async fn merge_configs() {
        let directory = tempdir().unwrap();
        let directory = directory.path();
        let base_file = directory.join("base.toml");
        let middle_file = directory.join("middle.toml");
        let child_file = directory.join("child.toml");

        write(
            &base_file,
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
            &middle_file,
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
            &child_file,
            indoc! {r#"
                extend = "middle.toml"
                [sites.default]
                recurse = true
            "#},
        )
        .await
        .unwrap();

        let config = compile_config(read_config(&child_file).await.unwrap()).unwrap();

        assert_eq!(config.concurrency().global(), Some(2));

        let site = config.sites().get("example.com").unwrap();

        assert_eq!(site[0].1.max_redirects(), 10);
        assert!(site[0].1.recursive());
        assert_eq!(
            config.roots().collect::<Vec<_>>(),
            vec!["https://example.com/"]
        );
    }

    #[tokio::test]
    async fn resolve_relative_files() {
        let directory = tempdir().unwrap();
        let directory = directory.path();
        let base_file = directory.join("base.toml");
        let sub_directory = directory.join("nested");
        let child_file = sub_directory.join("child.toml");

        create_dir_all(&sub_directory).await.unwrap();
        write(
            &base_file,
            indoc! {r#"
                concurrency = 5
                sites = {}
            "#},
        )
        .await
        .unwrap();
        write(
            &child_file,
            indoc! {r#"
                extend = "../base.toml"
                sites = {}
            "#},
        )
        .await
        .unwrap();

        let config = compile_config(read_config(&child_file).await.unwrap()).unwrap();

        assert_eq!(config.concurrency().global(), Some(5));
    }

    #[tokio::test]
    async fn detect_circular_extends() {
        let directory = tempdir().unwrap();
        let directory = directory.path();
        let first_file = directory.join("first.toml");
        let second_file = directory.join("second.toml");

        write(
            &first_file,
            indoc! {r#"
                extend = "second.toml"
                sites = {}
            "#},
        )
        .await
        .unwrap();
        write(
            &second_file,
            indoc! {r#"
                extend = "first.toml"
                sites = {}
            "#},
        )
        .await
        .unwrap();

        let result = read_config(&first_file).await;

        assert!(matches!(result, Err(ConfigError::CircularConfigFiles(_))));
    }
}
