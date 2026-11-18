//! Probe only Cargo's test binary, never an installed or live server.
use std::fs;
use std::io::Read;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

struct Probe {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sldr-server"));
    command
        .env_clear()
        .current_dir(root)
        .env("HOME", root)
        .env("USERPROFILE", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("APPDATA", root.join("appdata"))
        .env("LOCALAPPDATA", root.join("localappdata"))
        .env("SLDR_TLS", "1")
        .env("SLDR_API_TOKEN", "synthetic-token-never-print")
        .env("SLDR_STUDIO_DIR", root.join("private-library"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn run(mut command: Command) -> Result<Probe> {
    let mut child = ChildGuard(command.spawn().context("spawn isolated server binary")?);
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            bail!("server did not exit before deadline (possible startup or private-state read)");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut stream) = child.0.stdout.take() {
        stream.read_to_string(&mut stdout)?;
    }
    if let Some(mut stream) = child.0.stderr.take() {
        stream.read_to_string(&mut stderr)?;
    }
    Ok(Probe {
        status,
        stdout,
        stderr,
    })
}

fn run_without_state(arguments: &[&str], address: &str) -> Result<Probe> {
    let root = tempfile::tempdir()?;
    let mut command = command(root.path());
    command.args(arguments).env("SLDR_SERVER_ADDR", address);
    let probe = run(command)?;
    assert_eq!(
        fs::read_dir(root.path())?.count(),
        0,
        "state created for {arguments:?}"
    );
    Ok(probe)
}

fn assert_reply(probe: &Probe, argument: &str) {
    match argument {
        "--help" | "-h" => {
            assert!(probe.status.success(), "{}", probe.stderr);
            assert!(
                probe.stdout.contains("Usage: sldr-server"),
                "{}",
                probe.stdout
            );
            assert!(probe.stdout.contains("SLDR_SERVER_ADDR"));
            assert!(probe.stderr.is_empty(), "{}", probe.stderr);
        }
        "--version" | "-V" => {
            assert!(probe.status.success(), "{}", probe.stderr);
            assert!(probe
                .stdout
                .starts_with(concat!("sldr-server ", env!("CARGO_PKG_VERSION"))));
            assert!(
                probe.stdout.contains("git "),
                "missing revision: {}",
                probe.stdout
            );
            assert!(
                probe
                    .stdout
                    .contains(concat!("target ", env!("SLDR_BUILD_TARGET"))),
                "missing target: {}",
                probe.stdout
            );
            assert!(probe.stdout.contains(env!("SLDR_GIT_HASH")));
            assert!(probe.stderr.is_empty(), "{}", probe.stderr);
        }
        _ => {
            assert_eq!(probe.status.code(), Some(2), "{}", probe.stderr);
            assert!(probe.stdout.is_empty(), "{}", probe.stdout);
            assert!(
                probe.stderr.contains("unexpected argument"),
                "{}",
                probe.stderr
            );
        }
    }
    for output in [&probe.stdout, &probe.stderr] {
        assert!(!output.contains("listening on"), "{output}");
        assert!(!output.contains("synthetic-token-never-print"), "{output}");
        assert!(!output.contains("Bearer-token auth enabled"), "{output}");
    }
}

#[test]
fn flags_exit_without_creating_config_or_certificates_or_binding() -> Result<()> {
    // Keep the candidate port occupied: even the unfixed binary cannot expose a
    // listener. Correct output/status also rules out reaching a failed bind.
    let occupied = TcpListener::bind("127.0.0.1:0")?;
    for argument in [
        "--help",
        "-h",
        "--version",
        "-V",
        "--not-a-server-option",
        "serve",
    ] {
        let probe = run_without_state(&[argument], &occupied.local_addr()?.to_string())?;
        assert_reply(&probe, argument);
    }
    Ok(())
}

#[test]
fn flags_are_handled_before_server_environment_validation() -> Result<()> {
    for argument in ["--help", "--version", "--unknown"] {
        let probe = run_without_state(&[argument], "deliberately invalid")?;
        assert_reply(&probe, argument);
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn flags_do_not_read_existing_private_config_library_or_tls_files() -> Result<()> {
    // A read of a FIFO with no writer blocks, making even otherwise invisible
    // read-only config/certificate access observable via the child deadline.
    let occupied = TcpListener::bind("127.0.0.1:0")?;
    for argument in ["--help", "--version", "--unknown"] {
        let root = tempfile::tempdir()?;
        let config_dir = root.path().join("config/sldr");
        fs::create_dir_all(&config_dir)?;
        let config = config_dir.join("config.toml");
        let cert = root.path().join("cert.pem");
        let key = root.path().join("key.pem");
        let library = root.path().join("private-library");
        fs::create_dir_all(library.join("slides"))?;
        let slide = library.join("slides/private.md");
        for path in [&config, &cert, &key, &slide] {
            assert!(Command::new("mkfifo").arg(path).status()?.success());
        }
        let mut command = command(root.path());
        command
            .arg(argument)
            .env("SLDR_SERVER_ADDR", occupied.local_addr()?.to_string())
            .env("SLDR_TLS_CERT", &cert)
            .env("SLDR_TLS_KEY", &key);
        assert_reply(&run(command)?, argument);
        // Also probe a readable config pointing at read-trap library/TLS files.
        // This catches code that skips config for flags but still scans a library.
        fs::remove_file(&config)?;
        fs::write(
            &config,
            format!(
                "[config]\nlibrary = '{}'\n[presentations]\nslide_dir = '{}'\n",
                library.display(),
                library.join("slides").display()
            ),
        )?;
        let mut command = self::command(root.path());
        command
            .arg(argument)
            .env("SLDR_SERVER_ADDR", occupied.local_addr()?.to_string())
            .env("SLDR_TLS_CERT", &cert)
            .env("SLDR_TLS_KEY", &key);
        assert_reply(&run(command)?, argument);
        assert!(!root.path().join("data").exists());
        assert!(!root.path().join("sldr").exists());
    }
    Ok(())
}

#[test]
fn extra_arguments_are_rejected_even_after_an_informational_flag() -> Result<()> {
    for arguments in [["--help", "--unknown"], ["--version", "serve"]] {
        let probe = run_without_state(&arguments, "deliberately invalid")?;
        assert_reply(&probe, "--unknown");
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn non_utf8_argument_is_rejected_without_startup() -> Result<()> {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir()?;
    let mut command = command(root.path());
    command
        .arg(std::ffi::OsString::from_vec(vec![b'-', b'-', 0xff]))
        .env("SLDR_SERVER_ADDR", "deliberately invalid");
    assert_reply(&run(command)?, "--unknown");
    assert_eq!(fs::read_dir(root.path())?.count(), 0);
    Ok(())
}

#[test]
fn no_arguments_still_use_the_server_environment() -> Result<()> {
    let probe = run_without_state(&[], "deliberately invalid")?;
    assert!(!probe.status.success());
    assert!(
        probe.stderr.contains("Invalid SLDR_SERVER_ADDR"),
        "{}",
        probe.stderr
    );
    Ok(())
}
