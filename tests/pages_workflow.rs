//! The website is built, checked in a real browser and deployed by
//! `.github/workflows/pages.yml`. Deploying publishes to the internet, so the
//! rules that keep it safe are tested: only a push to master deploys, and
//! nothing deploys unless the site was built and its browser checks passed.

use std::fs;
use std::path::Path;

/// The Pages workflow, or `None` in the published crate, which leaves `.github` out.
fn workflow() -> Option<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/pages.yml");
    if !path.parent().is_some_and(Path::exists) {
        return None;
    }
    Some(fs::read_to_string(path).expect("a .github/workflows/pages.yml"))
}

/// The text of one job, from its name to the next job (or the end).
fn job<'a>(text: &'a str, name: &str) -> &'a str {
    let after = text
        .split(&format!("\n  {name}:"))
        .nth(1)
        .unwrap_or_else(|| panic!("a `{name}` job in pages.yml"));
    after.split("\n  deploy:").next().unwrap_or(after)
}

#[test]
fn the_site_is_built_from_the_real_example_and_checked_in_a_browser() {
    let Some(text) = workflow() else { return };
    let build = job(&text, "build");
    for step in [
        "wasm32-unknown-unknown",
        "cargo build --release --example web_art --target wasm32-unknown-unknown",
        "cargo build --release --example web_games --target wasm32-unknown-unknown",
        "cargo run --example build_site",
        "cargo run --release --example check_site -- --require",
    ] {
        assert!(build.contains(step), "the build job must run `{step}`");
    }
}

#[test]
fn both_wasm_modules_are_built_before_the_site_is_built_and_checked() {
    let Some(text) = workflow() else { return };
    let build = job(&text, "build");
    let at = |needle: &str| {
        build
            .find(needle)
            .unwrap_or_else(|| panic!("the build job must run `{needle}`"))
    };
    let art = at("--example web_art --target");
    let games = at("--example web_games --target");
    let site = at("cargo run --example build_site");
    let check = at("cargo run --release --example check_site -- --require");
    assert!(
        art < site && games < site,
        "build both wasm modules before the site"
    );
    assert!(site < check, "check the site after it is built");
}

#[test]
fn the_site_is_uploaded_only_after_the_checks_and_has_all_three_pages() {
    let Some(text) = workflow() else { return };
    let build = job(&text, "build");
    assert!(
        build.find("check_site").unwrap() < build.find("upload-pages-artifact").unwrap(),
        "upload only a checked site"
    );
    assert!(
        build.contains("path: target/site"),
        "upload the whole site directory: landing, art and games"
    );
}

#[test]
fn pull_requests_build_and_check_the_site_but_never_deploy() {
    let Some(text) = workflow() else { return };
    assert!(
        text.contains("pull_request:"),
        "check the site on pull requests"
    );
    let deploy = text.split("\n  deploy:").nth(1).expect("a `deploy` job");
    assert!(
        deploy.contains("github.event_name == 'push'") && deploy.contains("refs/heads/master"),
        "deploy must be limited to pushes to master"
    );
    assert!(
        deploy.contains("needs: build"),
        "deploy waits for the checks"
    );
    assert!(deploy.contains("actions/deploy-pages"));
    assert!(
        deploy.contains("github-pages"),
        "deploy into the github-pages environment"
    );
}

#[test]
fn the_deploy_job_has_the_permissions_pages_needs_and_the_build_job_does_not() {
    let Some(text) = workflow() else { return };
    let deploy = text.split("\n  deploy:").nth(1).expect("a `deploy` job");
    assert!(deploy.contains("pages: write") && deploy.contains("id-token: write"));
    let build = job(&text, "build");
    assert!(
        !build.contains("pages: write"),
        "building a pull request needs no write access"
    );
}

#[test]
fn only_pushes_upload_the_site_and_only_one_deploy_runs_at_a_time() {
    let Some(text) = workflow() else { return };
    assert!(text.contains("actions/upload-pages-artifact"));
    assert!(text.contains("concurrency:") && text.contains("group: pages"));
}
