use std::process::Command;
use tempfile::TempDir;

fn tdee_bin() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("tdee");
    path
}

fn setup() -> (TempDir, TempDir, String) {
    let data_dir = TempDir::new().unwrap();
    let key_dir = TempDir::new().unwrap();
    let key_path = key_dir.path().join("test-key.txt");

    use secrecy::ExposeSecret;
    let identity = age::x25519::Identity::generate();
    let pubkey = identity.to_public().to_string();
    let secret = identity.to_string().expose_secret().clone();
    let content = format!(
        "# created: 2026-01-01T00:00:00Z\n# public key: {}\n{}\n",
        pubkey, secret
    );
    std::fs::write(&key_path, content).unwrap();

    (data_dir, key_dir, key_path.to_str().unwrap().to_string())
}

fn run(data_dir: &str, key_file: &str, args: &[&str]) -> std::process::Output {
    Command::new(tdee_bin())
        .env("TDEE_DATA_DIR", data_dir)
        .env("TDEE_KEY_FILE", key_file)
        .args(args)
        .output()
        .expect("failed to execute tdee")
}

#[test]
fn integration_init_log_status() {
    let (data_dir, _key_dir, key_path) = setup();
    let dd = data_dir.path().to_str().unwrap();

    let out = run(
        dd,
        &key_path,
        &[
            "init",
            "--start-weight", "200",
            "--goal", "180",
            "--height-cm", "180",
            "--age", "35",
            "--sex", "male",
            "--start-date", "2026-08-01",
        ],
    );
    assert!(out.status.success(), "init failed: {}", String::from_utf8_lossy(&out.stderr));

    for i in 0..10 {
        let date = format!("2026-08-{:02}", i + 1);
        let weight = format!("{:.1}", 200.0 - 0.2 * i as f64);
        let kcal = "1900";
        let out = run(
            dd,
            &key_path,
            &["log", "--date", &date, "--weight", &weight, "--kcal", kcal],
        );
        assert!(
            out.status.success(),
            "log day {} failed: {}",
            i,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let out = run(dd, &key_path, &["status", "--json"]);
    assert!(out.status.success(), "status failed: {}", String::from_utf8_lossy(&out.stderr));
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("status --json is not valid JSON");
    assert!(json["tdee"].is_number(), "tdee should be a number");
    assert!(json["trend_lb"].is_number(), "trend_lb should be a number");
    assert!(json["target_kcal"].is_number(), "target_kcal should be a number");
    assert!(json["date"].is_string(), "date should be present");
}

#[test]
fn integration_log_requires_field() {
    let (data_dir, _key_dir, key_path) = setup();
    let dd = data_dir.path().to_str().unwrap();

    let out = run(
        dd,
        &key_path,
        &[
            "init",
            "--start-weight", "200",
            "--goal", "180",
            "--height-cm", "180",
            "--age", "35",
            "--sex", "male",
        ],
    );
    assert!(out.status.success());

    let out = run(dd, &key_path, &["log", "--date", "2026-08-01"]);
    assert!(!out.status.success(), "log with no fields should fail");
}

#[test]
fn integration_history() {
    let (data_dir, _key_dir, key_path) = setup();
    let dd = data_dir.path().to_str().unwrap();

    let out = run(
        dd,
        &key_path,
        &[
            "init",
            "--start-weight", "200",
            "--goal", "180",
            "--height-cm", "180",
            "--age", "35",
            "--sex", "male",
            "--start-date", "2026-08-01",
        ],
    );
    assert!(out.status.success());

    for i in 0..3 {
        let date = format!("2026-08-{:02}", i + 1);
        let weight = format!("{:.1}", 200.0 - 0.1 * i as f64);
        run(
            dd,
            &key_path,
            &["log", "--date", &date, "--weight", &weight, "--kcal", "2000"],
        );
    }

    let out = run(dd, &key_path, &["history", "--json"]);
    assert!(out.status.success());
    let json: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json.len(), 3);
}
