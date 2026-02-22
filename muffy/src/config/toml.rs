use super::{ConfigError, SerializableConfig};
use std::{
    fs::read_to_string,
    path::{Path, PathBuf},
};

/// Reads a configuration file recursively.
pub fn read_config(path: &Path) -> Result<SerializableConfig, ConfigError> {
    let mut stack = Vec::new();
    read_config_inner(path, &mut stack)
}

fn read_config_inner(
    path: &Path,
    stack: &mut Vec<PathBuf>,
) -> Result<SerializableConfig, ConfigError> {
    let canonical_path = path.canonicalize()?;

    if let Some(position) = stack.iter().position(|item| item == &canonical_path) {
        let mut cycle = stack[position..].to_vec();
        cycle.push(canonical_path);
        return Err(ConfigError::CircularConfigExtends(cycle));
    }

    stack.push(canonical_path.clone());

    let result = (|| {
        let contents = read_to_string(&canonical_path)?;
        let mut config: SerializableConfig = ::toml::from_str(&contents)?;

        if let Some(extend_path) = config.extend().map(ToOwned::to_owned) {
            let parent_path = if extend_path.is_absolute() {
                extend_path
            } else {
                canonical_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(extend_path)
            };
            let mut parent = read_config_inner(&parent_path, stack)?;
            parent.merge(config);
            config = parent;
        }

        Ok(config)
    })();

    stack.pop();

    result
}

#[cfg(test)]
mod tests {
    use super::{ConfigError, read_config};
    use crate::config::compile_config;
    use pretty_assertions::assert_eq;
    use std::{
        fs::{create_dir_all, write},
    };
    use tempfile::tempdir;

    #[test]
    fn read_config_merge_extends() {
        let temp_directory = tempdir().unwrap();
        let base_path = temp_directory.path().join("base.toml");
        let middle_path = temp_directory.path().join("middle.toml");
        let child_path = temp_directory.path().join("child.toml");

        write(
            &base_path,
            r#"
concurrency = 1
[sites.default]
roots = ["https://example.com/"]
max_redirects = 5
"#,
        )
        .unwrap();
        write(
            &middle_path,
            r#"
extend = "base.toml"
concurrency = 2
[sites.default]
max_redirects = 10
"#,
        )
        .unwrap();
        write(
            &child_path,
            r#"
extend = "middle.toml"
[sites.default]
recurse = true
"#,
        )
        .unwrap();

        let config = compile_config(read_config(&child_path).unwrap()).unwrap();

        assert_eq!(config.concurrency().global(), Some(2));

        let site = config.sites().get("example.com").unwrap();

        assert_eq!(site[0].1.max_redirects(), 10);
        assert!(site[0].1.recursive());
        assert_eq!(
            config.roots().collect::<Vec<_>>(),
            vec!["https://example.com/"]
        );
    }

    #[test]
    fn read_config_resolve_relative_paths() {
        let temp_directory = tempdir().unwrap();
        let base_path = temp_directory.path().join("base.toml");
        let nested_path = temp_directory.path().join("nested");
        let child_path = nested_path.join("child.toml");

        create_dir_all(&nested_path).unwrap();
        write(
            &base_path,
            r#"
concurrency = 5
sites = {}
"#,
        )
        .unwrap();
        write(
            &child_path,
            r#"
extend = "../base.toml"
sites = {}
"#,
        )
        .unwrap();

        let config = compile_config(read_config(&child_path).unwrap()).unwrap();

        assert_eq!(config.concurrency().global(), Some(5));
    }

    #[test]
    fn read_config_detect_circular_extends() {
        let temp_directory = tempdir().unwrap();
        let first_path = temp_directory.path().join("first.toml");
        let second_path = temp_directory.path().join("second.toml");

        write(
            &first_path,
            r#"
extend = "second.toml"
sites = {}
"#,
        )
        .unwrap();
        write(
            &second_path,
            r#"
extend = "first.toml"
sites = {}
"#,
        )
        .unwrap();

        let result = read_config(&first_path);

        assert!(matches!(
            result,
            Err(ConfigError::CircularConfigExtends(_))
        ));
    }
}
