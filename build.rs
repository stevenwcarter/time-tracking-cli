use std::env;
use std::process::Command;

fn main() {
    if env::var("CARGO_FEATURE_WEBAPP").is_err() {
        println!("cargo:warning=Skipping build.rs because `webapp` feature is not enabled");
        return;
    }
    // Watch specific files
    println!("cargo:rerun-if-changed=site/index.html");
    println!("cargo:rerun-if-changed=site/package.json");
    println!("cargo:rerun-if-changed=site/tsconfig.json");
    println!("cargo:rerun-if-changed=site/vite.config.ts");

    // Watch directories (recursively)
    println!("cargo:rerun-if-changed=site/src");
    println!("cargo:rerun-if-changed=site/public");

    // Skip frontend build in CI/CD if environment variable is set
    if env::var("SKIP_YARN").is_ok() || env::var("CI").is_ok() {
        println!("cargo:warning=Skipping yarn build (SKIP_YARN or CI set)");
        return;
    }

    // Run `yarn build` inside the `site` directory
    let status = Command::new("yarn")
        .arg("build")
        .current_dir("site")
        .status()
        .expect("failed to run yarn build");

    if !status.success() {
        panic!("yarn build failed");
    }
}
