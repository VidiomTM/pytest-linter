use pytest_linter::config::Config;
use pytest_linter::engine::LintEngine;
use std::path::PathBuf;
use std::time::Instant;

const SOAK_DIR: &str = "soak_repo";

fn get_soak_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SOAK_DIR)
}

#[test]
#[ignore]
fn soak_test_10k_files() {
    let soak_dir = get_soak_dir();
    assert!(
        soak_dir.exists(),
        "Soak repo not found at {:?}. Run: python scripts/generate-soak-repo.py --output-dir {}",
        soak_dir,
        SOAK_DIR
    );

    let engine = LintEngine::new(Config::default()).unwrap();

    let start = Instant::now();
    let violations = engine.lint_paths(&[soak_dir]).unwrap();
    let elapsed = start.elapsed();

    let file_count = std::fs::read_dir(get_soak_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "py"))
        .count();

    eprintln!(
        "Soak test: {} files, {} violations in {:.2}s ({:.0} files/s)",
        file_count,
        violations.len(),
        elapsed.as_secs_f64(),
        file_count as f64 / elapsed.as_secs_f64()
    );

    assert!(
        !violations.is_empty(),
        "Expected violations from synthetic repo"
    );

    let budget = std::time::Duration::from_secs(300);
    assert!(
        elapsed < budget,
        "Soak test exceeded time budget: {:.2}s > {}s",
        elapsed.as_secs_f64(),
        budget.as_secs()
    );
}

#[test]
#[ignore]
fn soak_test_single_file_performance() {
    let soak_dir = get_soak_dir();
    if !soak_dir.exists() {
        eprintln!("Skipping: soak repo not found");
        return;
    }

    let first_file = std::fs::read_dir(&soak_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().is_some_and(|ext| ext == "py"))
        .unwrap();

    let engine = LintEngine::new(Config::default()).unwrap();
    let start = Instant::now();
    let _violations = engine.lint_paths(&[first_file.path()]).unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "Single file lint took {:.0}ms",
        elapsed.as_millis()
    );
}
