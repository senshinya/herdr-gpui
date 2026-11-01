# Embedded Tailscale

Herdr UI can join one Tailnet itself. It does not require a system Tailscale
client, create a system VPN interface, or change the system client's network.

The embedded Tailnet IP belongs to Herdr's userspace node. Other Tailnet devices
can reach local TCP and UDP services through that IP: IPv4 maps to `127.0.0.1`
and IPv6 to `::1`, preserving the destination port. For example,
`http://<Herdr-Tailnet-IPv4>:8080` reaches a service listening on
`127.0.0.1:8080` or `0.0.0.0:8080`. The service must already be running and
listening on that loopback address or a wildcard address; services bound only
to a LAN address are not reached. Local services see a loopback source address,
so services that trust localhost also become reachable under the node's Tailnet
access rules. Existing service authentication still applies.

This forwarding starts with the embedded connection and closes on disconnect,
sign out, or app exit. It shares a limit of 64 incoming TCP/UDP sessions;
idle UDP sessions expire after two minutes. It does not provide subnet routing,
create a system VPN interface, or start SSH, web, or Herdr services.

Open **Settings > Tailscale**, choose a device name, and click **Connect**.
When the authorization link is ready, Herdr opens your browser once for that
connection attempt. **Continue in browser** remains available to reopen it.
Authorize the new Herdr node; networks requiring device approval show that
state until an administrator approves it.
An optional HTTPS control-server URL supports a custom coordination server.

The bottom status bar shows **Tailnet** independently of the selected device's
daemon or SSH connection: off, disconnected, connecting, disconnecting, sign-in
required, awaiting device approval, connected, or failed. Click it to open
Tailscale settings. It reads the existing network snapshot and does not start a
connection or disconnect on click. Narrow windows keep this item and turn
auxiliary status-bar buttons into icons, hiding CPU, GPU and network-quality
readouts to leave room for connection and usage information.
When the terminal column is especially narrow, an enabled network keeps its
icon and short state while auxiliary buttons and the version label give way.
The Off item may be hidden there when the embedded network is disabled.

**Add Device > Tailnet Devices** appears only while the embedded network is
connected and lists its online, unexpired peers. Selecting a peer fills its
Tailscale IP, preferring IPv4 and otherwise using an IPv6 SSH URI. Edit the
SSH target to choose your username or port,
for example `ssh://me@100.64.0.42:2222`. The selection retains the
peer's stable identity; editing the address does not select a different peer.
Use **Choose a different device** to go back to the source picker.
**Enter SSH address manually** is a separate action outside the Nearby and
Tailnet tabs. It clears the Tailnet selection and opens the connection form.
Manually entered addresses use ordinary SSH through the system network, even
when the address is a Tailnet IP; only selecting a Tailnet peer enables the
embedded route.
While connected, **Settings > Tailscale > Devices** lists the network’s devices
in a separate searchable view, with an optional **Online only** filter.
Nearby Devices uses Bonjour and SSH configuration only; it does not execute
`tailscale status` or read the system daemon's LocalAPI.

The selected machine must already have SSH access and a compatible Herdr
installation. The dialog probes SSH and asks before trusting a new host key.
A machine needing interactive SSH setup or Herdr installation must be prepared
separately before adding it through the embedded network. Network access rules
still apply, and an online peer is not a guarantee that its SSH port is reachable.
Windows peers are listed too; their SSH and Herdr support is checked when added.

Saved Tailnet devices use a client-specific internal SSH target. Their original
target and network/peer identity are saved in Herdr UI's device settings. Open
these devices through Herdr UI; another standalone SSH or Herdr process does not
have this process's embedded network. Connections, scripts, Git, transfers and
port forwards started through the client's SSH transport share this path.
Host-key records and SSH masters are isolated from ordinary SSH targets.
An unavailable embedded connection fails rather than trying a system route.

**Disconnect** closes the embedded network and its streams, retaining the
identity. **Sign out** asks for confirmation, closes the connection and removes
the saved identity. Neither action changes a separate system Tailscale client.
Sign out before changing the control server or credential store. An enabled connection is restored on application launch. Only one Herdr process
may own an identity at a time; another process reports that it is busy. After an
owner exits or crashes, retry explicitly in the other process.

Identity state uses Herdr's credential store. A nonsecret private marker tracks whether an identity must be signed out before its server/store changes. **Allow private file for
credentials** explicitly permits a private local file on Unix. Unsigned macOS
builds require this option because they cannot use the release's Keychain
identity. Without a configured store, Connect explains this before starting the
node. A failed startup releases the node and keeps its error visible. If no
identity was saved, adjust the settings and retry Connect directly; Disconnect
and Sign out are not required first. A saved identity still requires sign out
before changing its control server or credential store.
Windows uses Credential Manager and has no plaintext fallback.
The settings file stores no node key or login URL. CLI status and peer replies
exclude login URLs and paginate peers in groups of at most 64.

```toml
[tailscale]
enabled = false
hostname = "herdr-ui"
control_url = ""
allow_plaintext_credentials = false
```

Building from source needs Go 1.26.7 and a C compiler. The pinned SagerNet Tailscale module
and checksums live in `native/tailscale`. macOS and Linux link a static archive;
Windows ships `herdr_tailscale.dll` beside the app. On Windows run
`python scripts/setup-tailscale-cgo.py DIRECTORY` and put the printed directory
on PATH to install the checksum-pinned LLVM MinGW compiler for CGO. Rust still
uses the repository's normal MSVC toolchain. `CC_<target_with_underscores>` can
select an existing target C compiler instead. The Go module never downloads a
new Go toolchain during a Cargo build. `just test-tailscale` runs its race tests.
Release third-party notices include the Go runtime and pinned module licenses.
