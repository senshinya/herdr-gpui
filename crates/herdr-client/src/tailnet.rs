//! Explicit SSH paths for the app's embedded Tailnet. Synthetic targets keep
//! an ordinary SSH device with the same name independent of an embedded one.
mod error;
use crate::Result;
pub use error::Error;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, OnceLock, PoisonError, RwLock},
    time::Duration,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct ProxyRequest {
    pub generation: u64,
    pub engine: String,
    pub network: String,
    pub peer: String,
    pub port: u16,
}

impl From<&Binding> for ProxyRequest {
    fn from(binding: &Binding) -> Self {
        Self {
            generation: 0,
            engine: binding.engine.clone(),
            network: binding.network.clone(),
            peer: binding.peer_id.clone(),
            port: binding.port,
        }
    }
}

const SUFFIX: &str = ".herdr-tailnet.invalid";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub engine: String,
    pub network: String,
    pub original_target: String,
    pub peer_id: String,
    pub port: u16,
    pub user: String,
}

impl Binding {
    /// Resolve only the user's local SSH configuration, without connecting.
    pub fn new(engine: String, network: String, peer_id: String, target: &str) -> Result<Self> {
        let destination = crate::resolve_destination(target)?;
        let binding = Self {
            engine,
            network,
            original_target: target.into(),
            peer_id,
            port: destination.port,
            user: destination.user,
        };
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> Result<()> {
        if self.engine.len() != 64
            || !self.engine.bytes().all(|b| b.is_ascii_hexdigit())
            || self.network.is_empty()
            || self.network.len() > 256
            || self.network.chars().any(char::is_control)
            || self.peer_id.is_empty()
            || self.peer_id.len() > 256
            || self.peer_id.chars().any(char::is_control)
            || self.user.is_empty()
            || self.user.len() > 256
            || self
                .user
                .chars()
                .any(|c| c.is_control() || c.is_whitespace() || "@:/'\"".contains(c))
            || self.port == 0
        {
            return Err(Error::Invalid.into());
        }
        crate::catalog::validate_target(&self.original_target)?;
        Ok(())
    }

    fn digest(&self) -> String {
        Sha256::digest(format!(
            "{}\0{}\0{}",
            self.engine, self.network, self.peer_id
        ))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
    }

    pub fn target(&self) -> String {
        let digest = self.digest();
        format!(
            "ssh://{}@ht-{}.{}{}:{}",
            self.user,
            &digest[..32],
            &digest[32..],
            SUFFIX,
            self.port
        )
    }

    pub fn trust_key(&self) -> String {
        format!("herdr-tailnet-{}", self.digest())
    }
}

#[derive(Clone, Debug)]
pub struct Route {
    binding: Binding,
    endpoint: PathBuf,
    executable: PathBuf,
    configuration: Arc<Mutex<Option<Arc<tempfile::TempPath>>>>,
    directory: PathBuf,
    generation: fn() -> u64,
}

impl Route {
    /// The private file captures the original alias's identity/user options;
    /// the synthetic target is never looked up in DNS. Background only.
    pub fn prepare(
        binding: Binding,
        endpoint: PathBuf,
        executable: PathBuf,
        directory: &Path,
    ) -> Result<Self> {
        let route = Self::deferred(binding, endpoint, executable, directory)?;
        route.configuration()?;
        Ok(route)
    }

    /// Register a saved route without process or file IO during app startup.
    /// SSH workers prepare the original alias's configuration on first use.
    pub fn deferred(
        binding: Binding,
        endpoint: PathBuf,
        executable: PathBuf,
        directory: &Path,
    ) -> Result<Self> {
        binding.validate()?;
        if !endpoint.is_absolute() || !executable.is_absolute() || !directory.is_absolute() {
            return Err(Error::Invalid.into());
        }
        Ok(Self {
            generation: || 0,
            binding,
            endpoint,
            executable,
            directory: directory.to_path_buf(),
            configuration: Arc::new(Mutex::new(None)),
        })
    }

    fn configuration(&self) -> Result<Arc<tempfile::TempPath>> {
        let mut cached = self
            .configuration
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(configuration) = cached.as_ref() {
            return Ok(configuration.clone());
        }
        let mut command = crate::environment::command("ssh");
        command.args(["-G", "--", &self.binding.original_target]);
        let (status, output) = crate::ssh::run(&mut command, Duration::from_secs(15), || false)?;
        if !status.success() {
            return Err(Error::Resolve.into());
        }
        let text = std::str::from_utf8(&output).map_err(|_| Error::Resolve)?;
        validate_ssh_config(text)?;
        let mut file = tempfile::Builder::new()
            .prefix("ssh-")
            .tempfile_in(&self.directory)
            .map_err(Error::Io)?;
        let original = self
            .binding
            .original_target
            .strip_prefix("ssh://")
            .unwrap_or(&self.binding.original_target);
        let original = original.rsplit('@').next().unwrap_or(original);
        let original = original
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap_or(original);
        let original = if original.contains(':') && !original.contains("::") {
            original.split(':').next().unwrap_or(original)
        } else {
            original
        };
        render_configuration(&mut file, text, original)?;
        file.flush().map_err(Error::Io)?;
        let configuration = Arc::new(file.into_temp_path());
        *cached = Some(configuration.clone());
        Ok(configuration)
    }

    /// Capture the current network epoch when constructing each SSH operation.
    pub fn with_generation(mut self, generation: fn() -> u64) -> Self {
        self.generation = generation;
        self
    }

    pub fn binding(&self) -> &Binding {
        &self.binding
    }
}

fn routes() -> &'static RwLock<BTreeMap<String, Route>> {
    static ROUTES: OnceLock<RwLock<BTreeMap<String, Route>>> = OnceLock::new();
    ROUTES.get_or_init(Default::default)
}

pub fn install(route: Route) -> Result<()> {
    let target = route.binding.target();
    let mut routes = routes().write().unwrap_or_else(PoisonError::into_inner);
    if routes.len() >= 256 && !routes.contains_key(&target) {
        return Err(Error::Invalid.into());
    }
    routes.insert(target.clone(), route);
    drop(routes);
    crate::ssh::mux::host_keys_changed(&target);
    Ok(())
}

pub fn is_target(target: &str) -> bool {
    target
        .split('/')
        .next_back()
        .unwrap_or(target)
        .split(':')
        .next()
        .unwrap_or(target)
        .ends_with(SUFFIX)
        || target.contains(".herdr-tailnet.invalid:")
}

pub(crate) fn command(target: &str) -> Command {
    let mut command = crate::environment::command("ssh");
    command.args(options(target));
    command
}

pub(crate) fn options(target: &str) -> Vec<OsString> {
    if !is_target(target) {
        return Vec::new();
    }
    let route = routes()
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(target)
        .cloned();
    let Some(route) = route else {
        return closed_options();
    };
    let Ok(configuration) = route.configuration() else {
        return closed_options();
    };
    let mut proxy = OsString::from("ProxyCommand=");
    proxy.push(quote(route.executable.as_os_str()));
    proxy.push(" tailscale-proxy --endpoint ");
    proxy.push(quote(route.endpoint.as_os_str()));
    proxy.push(" --engine ");
    proxy.push(&route.binding.engine);
    // Opaque control-plane identities must never become shell syntax on Windows.
    for (name, value) in [
        (" --network-hex ", &route.binding.network),
        (" --peer-hex ", &route.binding.peer_id),
    ] {
        proxy.push(name);
        proxy.push(
            value
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        );
    }
    proxy.push(format!(" --generation {} --port %p", (route.generation)()));
    vec![
        "-F".into(),
        configuration.as_os_str().to_owned(),
        "-o".into(),
        proxy,
        "-o".into(),
        format!("HostKeyAlias={}", route.binding.trust_key()).into(),
        "-o".into(),
        "CheckHostIP=no".into(),
    ]
}

fn closed_options() -> Vec<OsString> {
    ["-o", "ProxyCommand=false", "-o", "ControlPath=none"]
        .map(OsString::from)
        .to_vec()
}

/// `ssh -G` emits unquoted single-argument paths. Replay them as one SSH
/// argument and resolve original-host tokens before using the synthetic name.
fn original_host_tokens(value: &str, original: &str) -> String {
    let mut result = String::new();
    let mut characters = value.chars();
    while let Some(ch) = characters.next() {
        result.push(ch);
        if ch == '%'
            && let Some(next) = characters.next()
        {
            if next == 'n' {
                result.pop();
                result.push_str(&original.replace('%', "%%"));
            } else {
                result.push(next);
            }
        }
    }
    result
}

fn render_configuration(output: &mut impl Write, text: &str, original: &str) -> Result<()> {
    validate_ssh_config(text)?;
    writeln!(output, "Host *").map_err(Error::Io)?;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(' ') else {
            continue;
        };
        if matches!(
            key,
            "host"
                | "controlpath"
                | "hostkeyalias"
                | "proxycommand"
                | "proxyjump"
                | "canonicalizehostname"
        ) {
            continue;
        }
        let value = original_host_tokens(value, original);
        if matches!(
            key,
            "identityfile"
                | "identityagent"
                | "certificatefile"
                | "pkcs11provider"
                | "securitykeyprovider"
                | "xauthlocation"
                | "revokedhostkeys"
        ) {
            let value = value.replace('\\', "\\\\").replace('"', "\\\"");
            writeln!(output, "  {key} \"{value}\"").map_err(Error::Io)?;
        } else {
            writeln!(output, "  {key} {value}").map_err(Error::Io)?;
        }
    }
    writeln!(output, "  ControlPath none\n  CanonicalizeHostname no").map_err(Error::Io)?;
    Ok(())
}

fn validate_ssh_config(text: &str) -> std::result::Result<(), Error> {
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(' ')
            && matches!(key, "proxycommand" | "proxyjump" | "hostkeyalias")
            && !matches!(value.trim(), "none" | "")
        {
            return Err(Error::Conflict);
        }
    }
    Ok(())
}

fn quote(value: &OsStr) -> OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let mut bytes = vec![b'\''];
        for byte in value.as_bytes() {
            match byte {
                b'\'' => bytes.extend_from_slice(b"'\\''"),
                b'%' => bytes.extend_from_slice(b"%%"),
                b => bytes.push(*b),
            }
        }
        bytes.push(b'\'');
        OsString::from_vec(bytes)
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        let mut wide = vec![u16::from(b'"')];
        for ch in value.encode_wide() {
            if ch == u16::from(b'%') {
                wide.push(ch);
            }
            wide.push(ch);
        }
        wide.push(u16::from(b'"'));
        OsString::from_wide(&wide)
    }
}

#[cfg(test)]
mod tests;
