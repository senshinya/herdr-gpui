use super::{Config, Error, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use herdr_credentials::{Store, file, keyring_read, keyring_save};
use secrecy::{ExposeSecret, SecretString};
use serde::{Serialize, Serializer, ser::SerializeMap};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs, io,
    path::PathBuf,
    sync::{Mutex, PoisonError},
};
use zeroize::Zeroizing;

/// One atomic credential record owns the complete StateStore, so logout can
/// remove every identity key without enumerating a platform's keyring.
#[derive(Debug)]
pub(super) struct Storage {
    directory: PathBuf,
    namespace: String,
    policy: Store,
    access: Mutex<()>,
    pub(super) error: Mutex<Option<Error>>,
}

impl Storage {
    pub(super) fn new(config: &Config, directory: PathBuf, namespace: String) -> Self {
        Self {
            directory,
            namespace,
            policy: Store::choose(
                config.allow_plaintext_credentials && herdr_credentials::FILE,
                herdr_credentials::KEYRING,
                false,
            ),
            access: Mutex::new(()),
            error: Mutex::new(None),
        }
    }

    pub(super) fn require_available(&self) -> Result<()> {
        if self.policy == Store::Environment {
            return Err(Error::CredentialsUnavailable);
        }
        Ok(())
    }

    pub(super) fn read(&self, key: &[u8]) -> Result<Option<Zeroizing<Vec<u8>>>> {
        let _access = self.access.lock().unwrap_or_else(PoisonError::into_inner);
        let records = self.records()?;
        records
            .get(&fingerprint(key))
            .map(|secret| {
                STANDARD
                    .decode(secret.expose_secret())
                    .map(Zeroizing::new)
                    .map_err(Error::from)
            })
            .transpose()
    }

    pub(super) fn write(&self, key: &[u8], data: Option<&[u8]>) -> Result<()> {
        let _access = self.access.lock().unwrap_or_else(PoisonError::into_inner);
        let mut records = self.records()?;
        if let Some(data) = data {
            records.insert(fingerprint(key), SecretString::from(STANDARD.encode(data)));
        } else {
            records.remove(&fingerprint(key));
        }
        if records.len() > 128 {
            return Err(Error::Limit);
        }
        struct Record<'a>(&'a BTreeMap<String, SecretString>);
        impl Serialize for Record<'_> {
            fn serialize<S: Serializer>(
                &self,
                serializer: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                let mut map = serializer.serialize_map(Some(self.0.len()))?;
                for (key, value) in self.0 {
                    map.serialize_entry(key, value.expose_secret())?;
                }
                map.end()
            }
        }
        let bytes = Zeroizing::new(serde_json::to_vec(&Record(&records))?);
        if bytes.len() > herdr_credentials::LIMIT {
            return Err(Error::Limit);
        }
        let text = String::from_utf8(bytes.to_vec()).map_err(|_| Error::Storage)?;
        self.save(Some(&SecretString::from(text)))
    }

    pub(super) fn clear(&self) -> Result<()> {
        let _access = self.access.lock().unwrap_or_else(PoisonError::into_inner);
        if !self.has_identity()? {
            return Ok(());
        }
        self.save(None)
    }

    fn records(&self) -> Result<BTreeMap<String, SecretString>> {
        let record = match self.policy {
            Store::Keyring => keyring_read("dev.herdr-ui.tailscale", &self.namespace)?,
            Store::File => file::read_checked(&self.directory, c"identity.json", validate)?,
            Store::Environment => return Err(herdr_credentials::Error::Policy.into()),
        };
        match record {
            Some(record) => Ok(serde_json::from_str(record.expose_secret())?),
            None => Ok(BTreeMap::new()),
        }
    }

    pub(super) fn has_identity(&self) -> Result<bool> {
        match fs::symlink_metadata(self.directory.join("identity.present")) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn save(&self, record: Option<&SecretString>) -> Result<()> {
        if let Some(secret) = record {
            validate(secret)?;
        }
        if record.is_some() {
            let mut options = fs::OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                options.mode(0o600);
            }
            match options.open(self.directory.join("identity.present")) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        match self.policy {
            Store::Keyring => keyring_save(
                "dev.herdr-ui.tailscale",
                "Herdr Tailscale identity",
                record,
                &self.namespace,
            )?,
            Store::File => {
                file::store_checked(&self.directory, c"identity.json", record, true, validate)?
            }
            Store::Environment => return Err(herdr_credentials::Error::Policy.into()),
        }
        if record.is_none() {
            match fs::remove_file(self.directory.join("identity.present")) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

fn validate(secret: &SecretString) -> Result<()> {
    if secret.expose_secret().len() > herdr_credentials::LIMIT {
        return Err(Error::Limit);
    }
    Ok(())
}

pub(super) fn fingerprint(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
