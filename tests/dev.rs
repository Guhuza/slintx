use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::tempdir;

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn dev_rebuilds_and_restarts_after_a_rust_source_change() {
    let project = tempdir().expect("temporary project directory");
    let target = tempdir().expect("temporary Cargo target directory");
    let log_path = project.path().join("slintx.log");
    std::fs::create_dir(project.path().join("src")).expect("create source directory");
    std::fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = \"slintx-dev-smoke\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo manifest");
    std::fs::write(
        project.path().join("slintx.toml"),
        "schema-version = 1\ntemplate = \"basic\"\ntemplate-version = 1\ndefault-package = \"slintx-dev-smoke\"\n",
    )
    .expect("write SlintX config");
    write_app(project.path(), "first");
    let lock_status = Command::new("cargo")
        .args(["generate-lockfile", "--manifest-path"])
        .arg(project.path().join("Cargo.toml"))
        .current_dir(project.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("generate Cargo lockfile");
    assert!(
        lock_status.success(),
        "Cargo failed to generate the lockfile"
    );

    let log_file = std::fs::File::create(&log_path).expect("create SlintX log");
    let child = Command::new(env!("CARGO_BIN_EXE_slintx"))
        .args(["dev", "--manifest-path"])
        .arg(project.path().join("Cargo.toml"))
        .current_dir(project.path())
        .stdout(Stdio::from(log_file.try_clone().expect("clone SlintX log")))
        .stderr(Stdio::from(log_file))
        .env("RUST_LOG", "debug")
        .env("CARGO_TARGET_DIR", target.path())
        .spawn()
        .expect("start slintx dev");
    let mut child = ChildGuard(child);
    let marker = project.path().join("launches.txt");
    wait_for_launches(&mut child.0, &marker, 1);
    thread::sleep(Duration::from_secs(2));

    write_app(project.path(), "second");
    wait_for_launches(&mut child.0, &marker, 2);
    let launches_before_failed_build =
        wait_for_stable_launch_count(&mut child.0, &marker, &log_path, Duration::from_secs(2));

    std::fs::write(
        project.path().join("src/main.rs"),
        "fn main() { this is invalid; }\n",
    )
    .expect("write invalid Rust source");
    thread::sleep(Duration::from_secs(3));
    let launches = std::fs::read_to_string(&marker)
        .expect("read launch marker")
        .lines()
        .count();
    assert_eq!(
        launches, launches_before_failed_build,
        "a failed rebuild must not launch an old artifact"
    );
}

fn write_app(project: &std::path::Path, value: &str) {
    let source = format!(
        r#"fn main() {{
    use std::io::Write as _;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("launches.txt")
        .expect("open launch marker");
    writeln!(file, "{value}").expect("write launch marker");
}}
"#
    );
    std::fs::write(project.join("src/main.rs"), source).expect("write app source");
}

fn wait_for_launches(child: &mut Child, marker: &std::path::Path, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if let Some(status) = child.try_wait().expect("inspect slintx dev process") {
            panic!("slintx dev exited early with {status}");
        }
        let launches = std::fs::read_to_string(marker)
            .map(|contents| contents.lines().count())
            .unwrap_or_default();
        if launches >= expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected} app launches"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

fn wait_for_stable_launch_count(
    child: &mut Child,
    marker: &std::path::Path,
    log_path: &std::path::Path,
    stable_for: Duration,
) -> usize {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut previous_count = 0;
    let mut unchanged_since = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("inspect slintx dev process") {
            panic!("slintx dev exited early with {status}");
        }
        let count = std::fs::read_to_string(marker)
            .map(|contents| contents.lines().count())
            .unwrap_or_default();
        if count != previous_count {
            previous_count = count;
            unchanged_since = Instant::now();
        } else if unchanged_since.elapsed() >= stable_for {
            return count;
        }
        assert!(
            Instant::now() < deadline,
            "application launch count did not stabilize; SlintX log:\n{}",
            last_log_lines(log_path, 100)
        );
        thread::sleep(Duration::from_millis(100));
    }
}

fn last_log_lines(path: &std::path::Path, count: usize) -> String {
    let contents = std::fs::read_to_string(path).unwrap_or_default();
    contents
        .lines()
        .rev()
        .take(count)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}
