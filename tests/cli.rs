use std::process::Command;

use tempfile::tempdir;

fn slintx() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slintx"))
}

#[test]
fn new_creates_a_buildable_basic_project_without_git() {
    let temporary = tempdir().expect("temporary directory");
    let destination = temporary.path().join("hello");
    let output = slintx()
        .args([
            "new",
            "Hello_App",
            "--path",
            destination.to_str().expect("UTF-8 test path"),
            "--vcs",
            "none",
        ])
        .output()
        .expect("run slintx");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(destination.join("Cargo.toml").is_file());
    assert!(destination.join("build.rs").is_file());
    assert!(destination.join("ui/app-window.slint").is_file());
    let manifest =
        std::fs::read_to_string(destination.join("Cargo.toml")).expect("read generated manifest");
    assert!(manifest.contains("name = \"hello-app\""));

    let doctor = slintx()
        .args(["doctor", "--skip-cargo-check"])
        .current_dir(&destination)
        .output()
        .expect("run doctor in generated project");
    assert!(
        doctor.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&doctor.stdout),
        String::from_utf8_lossy(&doctor.stderr)
    );
    assert!(String::from_utf8_lossy(&doctor.stdout).contains("Slint dependencies"));
}

#[test]
fn new_creates_workspace_template_and_refuses_existing_destination() {
    let temporary = tempdir().expect("temporary directory");
    let destination = temporary.path().join("workspace-app");
    let path_arg = destination.to_str().expect("UTF-8 test path");
    let output = slintx()
        .args([
            "new",
            "workspace-app",
            "--template",
            "workspace",
            "--path",
            path_arg,
            "--vcs",
            "none",
        ])
        .output()
        .expect("run slintx");
    assert!(output.status.success());
    assert!(
        destination
            .join("apps/desktop/ui/app-window.slint")
            .is_file()
    );
    assert!(destination.join("crates/app-core/src/lib.rs").is_file());
    let doctor = slintx()
        .args(["doctor", "--skip-cargo-check"])
        .current_dir(&destination)
        .output()
        .expect("run doctor in generated workspace");
    assert!(
        doctor.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&doctor.stdout),
        String::from_utf8_lossy(&doctor.stderr)
    );
    assert!(String::from_utf8_lossy(&doctor.stdout).contains("Slint dependencies"));

    let sentinel = destination.join("keep.txt");
    std::fs::write(&sentinel, "keep").expect("write sentinel");
    let output = slintx()
        .args(["new", "workspace-app", "--path", path_arg, "--vcs", "none"])
        .output()
        .expect("run slintx");
    assert!(!output.status.success());
    assert_eq!(
        std::fs::read_to_string(sentinel).expect("read sentinel"),
        "keep"
    );
}

#[test]
fn init_adds_files_without_overwriting_existing_content() {
    let temporary = tempdir().expect("temporary directory");
    let project = temporary.path().join("existing-app");
    std::fs::create_dir(&project).expect("create project directory");
    let output = slintx()
        .args(["init", "--template", "basic"])
        .current_dir(&project)
        .output()
        .expect("run init");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.join("Cargo.toml").is_file());
    assert!(project.join("src/main.rs").is_file());

    let manifest = project.join("Cargo.toml");
    std::fs::write(&manifest, "keep this manifest").expect("write existing manifest");
    let output = slintx()
        .args(["init", "--template", "basic"])
        .current_dir(&project)
        .output()
        .expect("run conflicting init");
    assert!(!output.status.success());
    assert_eq!(
        std::fs::read_to_string(manifest).expect("read existing manifest"),
        "keep this manifest"
    );

    let conflicting_project = temporary.path().join("conflicting-app");
    std::fs::create_dir(&conflicting_project).expect("create conflicting project");
    std::fs::write(
        conflicting_project.join("Cargo.toml"),
        "preserve user content",
    )
    .expect("write conflicting file");
    let output = slintx()
        .args(["init"])
        .current_dir(&conflicting_project)
        .output()
        .expect("run conflicting init");
    assert!(!output.status.success());
    assert_eq!(
        std::fs::read_to_string(conflicting_project.join("Cargo.toml"))
            .expect("read conflicting file"),
        "preserve user content"
    );
    assert!(
        !conflicting_project.join("src/main.rs").exists(),
        "a collision must be detected before writing any template file"
    );
}

#[test]
fn add_generates_components_and_pages_without_replacing_files() {
    let temporary = tempdir().expect("temporary directory");
    let destination = temporary.path().join("generated");
    let output = slintx()
        .args([
            "new",
            "generated",
            "--path",
            destination.to_str().expect("UTF-8 test path"),
            "--vcs",
            "none",
        ])
        .output()
        .expect("create project");
    assert!(output.status.success());

    for kind in ["component", "page"] {
        let output = slintx()
            .args(["add", kind, "Fancy-Widget"])
            .current_dir(&destination)
            .output()
            .expect("run add");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let directory = if kind == "component" {
            "components"
        } else {
            "pages"
        };
        let generated = destination
            .join("ui")
            .join(directory)
            .join("fancy-widget.slint");
        let contents = std::fs::read_to_string(&generated).expect("read generated UI");
        assert!(contents.contains("export component FancyWidget"));

        let output = slintx()
            .args(["add", kind, "Fancy-Widget"])
            .current_dir(&destination)
            .output()
            .expect("attempt duplicate add");
        assert!(!output.status.success());
        assert_eq!(
            std::fs::read_to_string(generated).expect("read generated UI"),
            contents
        );
    }
}

#[test]
fn installs_and_uses_a_local_external_template() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    std::fs::create_dir(&source).expect("create template source");
    std::fs::write(
        source.join(".slintx-template.toml"),
        "schema-version = 1\nname = \"demo\"\nversion = \"1.0.0\"\ndescription = \"Test template\"\n",
    )
    .expect("write template metadata");
    std::fs::write(
        source.join("Cargo.toml"),
        "[package]\nname = \"{{package_name}}\"\nversion = \"0.1.0\"\n",
    )
    .expect("write template manifest");
    std::fs::write(source.join("README.md"), "# {{project_name}}\n")
        .expect("write template readme");

    let registry_root = temporary.path().join("user-data");
    let registry_env = [
        ("LOCALAPPDATA", registry_root.as_os_str()),
        ("APPDATA", registry_root.as_os_str()),
        ("XDG_DATA_HOME", registry_root.as_os_str()),
        ("HOME", registry_root.as_os_str()),
    ];
    let mut install = slintx();
    install
        .args([
            "template",
            "add",
            source.to_str().expect("UTF-8 source path"),
            "--name",
            "demo-template",
        ])
        .envs(registry_env.iter().copied());
    let output = install.output().expect("install template");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let destination = temporary.path().join("custom-project");
    let output = slintx()
        .args([
            "new",
            "Custom_Project",
            "--template",
            "demo-template",
            "--path",
            destination.to_str().expect("UTF-8 destination"),
            "--vcs",
            "none",
        ])
        .envs(registry_env.iter().copied())
        .output()
        .expect("create from external template");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!destination.join(".slintx-template.toml").exists());
    assert!(
        std::fs::read_to_string(destination.join("Cargo.toml"))
            .expect("read generated Cargo manifest")
            .contains("name = \"custom-project\"")
    );
    assert_eq!(
        std::fs::read_to_string(destination.join("README.md")).expect("read generated readme"),
        "# Custom_Project\n"
    );
}

#[test]
fn help_and_version_are_available() {
    let help = slintx().arg("--help").output().expect("run help");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("doctor"));

    let version = slintx().arg("--version").output().expect("run version");
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("slintx"));

    let template_help = slintx()
        .args(["template", "list"])
        .output()
        .expect("list templates");
    assert!(template_help.status.success());
    assert!(String::from_utf8_lossy(&template_help.stdout).contains("workspace"));
}
