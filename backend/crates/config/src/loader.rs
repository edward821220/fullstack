use crate::{AppConfig, Error, Result};
use figment::{
    Figment,
    providers::{Env, Format, Yaml},
};
use std::path::PathBuf;

const CONFIG_DIR_ENV: &str = "APP_CONFIG_DIR";
const DEFAULT_CONFIG_DIR: &str = "config";

#[derive(Debug)]
pub struct ConfigDir {
    dir: PathBuf,
}

impl ConfigDir {
    pub fn resolve(cli_override: Option<PathBuf>) -> Self {
        let dir = cli_override
            .or_else(|| std::env::var_os(CONFIG_DIR_ENV).map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_DIR));
        Self { dir }
    }

    pub fn default_yaml(&self) -> PathBuf {
        self.dir.join("default.yaml")
    }

    pub fn local_yaml(&self) -> PathBuf {
        self.dir.join("local.yaml")
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_dir = ConfigDir::resolve(None);
        Self::load_from_dir(&config_dir)
    }

    pub fn load_with_config_dir(dir: impl Into<PathBuf>) -> Result<Self> {
        let config_dir = ConfigDir::resolve(Some(dir.into()));
        Self::load_from_dir(&config_dir)
    }

    fn load_from_dir(config_dir: &ConfigDir) -> Result<Self> {
        let default_path = config_dir.default_yaml();
        let local_path = config_dir.local_yaml();

        let mut figment = Figment::new().merge(Yaml::file(&default_path));

        if local_path.exists() {
            figment = figment.merge(Yaml::file(&local_path));
        }

        figment
            .merge(Env::prefixed("APP_").split("__"))
            .extract()
            .map_err(|e| Error::Load {
                source: Box::new(e),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Helper to temporarily set an env var and restore it after the closure runs.
    /// This avoids cross-test pollution from parallel test threads.
    fn with_env<F, R>(key: &str, value: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let orig = std::env::var_os(key);
        // SAFETY: single-threaded test helper; env mutation is scoped and restored.
        unsafe { std::env::set_var(key, value) };
        let result = f();
        match orig {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
        result
    }

    /// Helper to temporarily remove an env var and restore it after the closure runs.
    fn without_env<F, R>(key: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let orig = std::env::var_os(key);
        // SAFETY: single-threaded test helper; env mutation is scoped and restored.
        unsafe { std::env::remove_var(key) };
        let result = f();
        match orig {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
        result
    }

    #[test]
    fn config_dir_resolve_defaults_to_config() {
        without_env(CONFIG_DIR_ENV, || {
            let dir = ConfigDir::resolve(None);
            assert_eq!(dir.dir, PathBuf::from("config"));
            assert_eq!(dir.default_yaml(), PathBuf::from("config/default.yaml"));
            assert_eq!(dir.local_yaml(), PathBuf::from("config/local.yaml"));
        });
    }

    #[test]
    fn config_dir_resolve_cli_overrides_default() {
        let dir = ConfigDir::resolve(Some(PathBuf::from("/etc/myapp")));
        assert_eq!(dir.dir, PathBuf::from("/etc/myapp"));
        assert_eq!(dir.default_yaml(), PathBuf::from("/etc/myapp/default.yaml"));
        assert_eq!(dir.local_yaml(), PathBuf::from("/etc/myapp/local.yaml"));
    }

    #[test]
    fn config_dir_resolve_env_fallback_when_no_cli() {
        with_env(CONFIG_DIR_ENV, "/opt/config", || {
            let dir = ConfigDir::resolve(None);
            assert_eq!(dir.dir, PathBuf::from("/opt/config"));
        });
    }

    #[test]
    fn config_dir_resolve_cli_takes_precedence_over_env() {
        with_env(CONFIG_DIR_ENV, "/from/env", || {
            let dir = ConfigDir::resolve(Some(PathBuf::from("/from/cli")));
            assert_eq!(dir.dir, PathBuf::from("/from/cli"));
        });
    }
}
