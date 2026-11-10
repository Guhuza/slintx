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

    let child = Command::new(env!("CARGO_BIN_EXE_slintx"))
        .args(["dev", "--manifest-path"])
        .arg(project.path().join("Cargo.toml"))
        .current_dir(project.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start slintx dev");
    let mut child = ChildGuard(child);
    let marker = project.path().join("launches.txt");
    wait_for_launches(&mut child.0, &marker, 1);
    thread::sleep(Duration::from_secs(2));

    write_app(project.path(), "second");
    wait_for_launches(&mut child.0, &marker, 2);

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
        launches, 2,
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
