//! Targeted writes of the embedded Tailnet connection and per-device binding.
use super::{Config, LOCAL_CONFIG, write_config};
use crate::Result;
use std::{fs, io::ErrorKind, path::Path};

impl Config {
    pub(crate) fn save_tailscale(config: &herdr_remote::tailscale::Config) -> Result<()> {
        config.validate().map_err(crate::tailnet::Error::from)?;
        let (_lock, local) = Self::prepare_files(&Self::path()?)?;
        Self::save_tailscale_path(config, &local)
    }

    pub(super) fn save_tailscale_path(
        config: &herdr_remote::tailscale::Config,
        path: &Path,
    ) -> Result<()> {
        config.validate().map_err(crate::tailnet::Error::from)?;
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => LOCAL_CONFIG.into(),
            Err(error) => return Err(error.into()),
        };
        let mut document = text.parse::<toml_edit::DocumentMut>()?;
        let table = document
            .entry("tailscale")
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
            .as_table_like_mut()
            .ok_or(crate::herdr_settings::Error::Table("tailscale"))?;
        for (key, mut value) in [
            ("enabled", toml_edit::value(config.enabled)),
            ("hostname", toml_edit::value(config.hostname.as_str())),
            ("control_url", toml_edit::value(config.control_url.as_str())),
            (
                "allow_plaintext_credentials",
                toml_edit::value(config.allow_plaintext_credentials),
            ),
        ] {
            if let Some(existing) = table.get_mut(key) {
                if let (Some(old), Some(new)) = (existing.as_value(), value.as_value_mut()) {
                    *new.decor_mut() = old.decor().clone();
                }
                *existing = value;
            } else {
                table.insert(key, value);
            }
        }
        write_config(path, &document.to_string())
    }

    pub(crate) fn save_tailnet_binding(
        profile: &str,
        binding: &herdr_client::tailnet::Binding,
    ) -> Result<()> {
        binding.validate()?;
        let (_lock, path) = Self::prepare_files(&Self::path()?)?;
        let mut table = toml_edit::Table::new();
        for (key, value) in [
            ("engine", binding.engine.as_str()),
            ("network", &binding.network),
            ("original_target", &binding.original_target),
            ("peer_id", &binding.peer_id),
            ("user", &binding.user),
        ] {
            table.insert(key, toml_edit::value(value));
        }
        table.insert("port", toml_edit::value(i64::from(binding.port)));
        super::devices::set_device_key(
            profile,
            "tailnet",
            Some(toml_edit::Item::Table(table)),
            &path,
        )
    }
}
