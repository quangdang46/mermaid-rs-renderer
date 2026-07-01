use std::io::Write;
use std::process::{Command, Stdio};

fn mmdr() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mmdr"))
}

fn assert_svg_size(svg: &str, width: &str, height: &str) {
    let root = svg
        .split('>')
        .next()
        .expect("SVG output should include a root element");
    assert!(
        root.contains(&format!("width=\"{width}\"")),
        "expected width={width:?} in root element: {root}"
    );
    assert!(
        root.contains(&format!("height=\"{height}\"")),
        "expected height={height:?} in root element: {root}"
    );
}

#[test]
fn cli_width_height_affect_stdout_svg() {
    let mut child = mmdr()
        .args(["--width", "321", "--height", "123", "--input", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run mmdr");
    child
        .stdin
        .as_mut()
        .expect("failed to open stdin")
        .write_all(b"flowchart TD\n  A-->B\n")
        .expect("failed to write stdin");
    let output = child.wait_with_output().expect("failed to wait for mmdr");

    assert!(
        output.status.success(),
        "mmdr failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = String::from_utf8(output.stdout).expect("SVG stdout should be UTF-8");
    assert_svg_size(&svg, "321", "123");
}

#[test]
fn cli_width_height_affect_file_svg() {
    let dir = std::env::temp_dir().join(format!(
        "mermaid-rs-renderer-cli-test-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("failed to create temp dir");
    let input = dir.join("input.mmd");
    let output_path = dir.join("output.svg");
    std::fs::write(&input, "flowchart TD\n  A-->B\n").expect("failed to write input");

    let output = mmdr()
        .args([
            "--width",
            "654",
            "--height",
            "456",
            "--input",
            input.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run mmdr");

    assert!(
        output.status.success(),
        "mmdr failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = std::fs::read_to_string(&output_path).expect("failed to read SVG output");
    assert_svg_size(&svg, "654", "456");

    let _ = std::fs::remove_file(input);
    let _ = std::fs::remove_file(output_path);
    let _ = std::fs::remove_dir(dir);
}

/// Issue #73: named theme presets selectable via --theme.
#[test]
fn cli_theme_flag_selects_builtin_presets() {
    for (name, expected_bg) in [
        ("dark", "#333333"),
        ("forest", "#ffffff"),
        ("neutral", "#ffffff"),
        ("default", "#FFFFFF"),
    ] {
        let mut child = mmdr()
            .args(["--theme", name, "--input", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to run mmdr");
        child
            .stdin
            .as_mut()
            .expect("failed to open stdin")
            .write_all(b"flowchart TD\n  A-->B\n")
            .expect("failed to write stdin");
        let output = child.wait_with_output().expect("failed to wait for mmdr");
        assert!(
            output.status.success(),
            "mmdr --theme {name} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let svg = String::from_utf8(output.stdout).expect("SVG stdout should be UTF-8");
        assert!(
            svg.contains(&format!("fill=\"{expected_bg}\"")),
            "--theme {name} should set background {expected_bg}"
        );
    }

    // Dark preset should color nodes with its primary color.
    let mut child = mmdr()
        .args(["--theme", "dark", "--input", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run mmdr");
    child
        .stdin
        .as_mut()
        .expect("failed to open stdin")
        .write_all(b"flowchart TD\n  A-->B\n")
        .expect("failed to write stdin");
    let output = child.wait_with_output().expect("failed to wait for mmdr");
    let svg = String::from_utf8(output.stdout).expect("SVG stdout should be UTF-8");
    assert!(
        svg.contains("#1f2020"),
        "dark theme should use #1f2020 node fills"
    );
}

/// Issue #73: unknown preset names should fail with a helpful error.
#[test]
fn cli_theme_flag_rejects_unknown_names() {
    let mut child = mmdr()
        .args(["--theme", "does-not-exist", "--input", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run mmdr");
    child
        .stdin
        .as_mut()
        .expect("failed to open stdin")
        .write_all(b"flowchart TD\n  A-->B\n")
        .expect("failed to write stdin");
    let output = child.wait_with_output().expect("failed to wait for mmdr");
    assert!(
        !output.status.success(),
        "unknown theme names should be rejected"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown theme preset"),
        "error should mention the unknown preset: {stderr}"
    );
}

/// Issue #73: themeVariables from the config file must still override the
/// preset selected via --theme.
#[test]
fn cli_theme_flag_composes_with_theme_variables() {
    let dir = std::env::temp_dir().join("mmdr-theme-flag-test");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let config_path = dir.join("config.json");
    std::fs::write(
        &config_path,
        r##"{"themeVariables": {"primaryColor": "#123456"}}"##,
    )
    .expect("write config");

    let mut child = mmdr()
        .args([
            "--theme",
            "dark",
            "--configFile",
            config_path.to_str().expect("utf-8 path"),
            "--input",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run mmdr");
    child
        .stdin
        .as_mut()
        .expect("failed to open stdin")
        .write_all(b"flowchart TD\n  A-->B\n")
        .expect("failed to write stdin");
    let output = child.wait_with_output().expect("failed to wait for mmdr");
    assert!(
        output.status.success(),
        "mmdr failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = String::from_utf8(output.stdout).expect("SVG stdout should be UTF-8");
    assert!(
        svg.contains("#123456"),
        "themeVariables.primaryColor should override the preset"
    );
    assert!(
        svg.contains("fill=\"#333333\""),
        "non-overridden dark preset values (background) should remain"
    );
}
