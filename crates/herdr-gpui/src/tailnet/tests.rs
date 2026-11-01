use super::*;
#[test]
fn helper_arguments_refuse_duplicates_and_zero_ports() {
    let mut words = [
        "--endpoint",
        "/tmp/p",
        "--engine",
        "engine",
        "--network-hex",
        "7461696c2e74732e6e6574",
        "--peer-hex",
        "6e313233",
        "--generation",
        "1",
        "--port",
        "22",
    ]
    .map(std::ffi::OsString::from);
    words[1] = std::env::temp_dir().join("proxy-test").into_os_string();
    assert!(helper::parse(&words).is_ok());
    let mut duplicate = words.to_vec();
    duplicate.extend(["--port".into(), "22".into()]);
    assert!(helper::parse(&duplicate).is_err());
    let mut zero = words;
    zero[11] = "0".into();
    assert!(helper::parse(&zero).is_err());
}
