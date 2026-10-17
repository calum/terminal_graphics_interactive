//! Publishing to crates.io cannot be undone, so the rules that keep it safe
//! are tested: the token only ever comes from repository secrets, and a
//! publish can only happen from a push to master after the tests passed.

use std::fs;
use std::path::Path;

/// The CI workflow, or `None` in the published crate, which leaves `.github` out.
fn workflow() -> Option<String> {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml")).ok()
}

#[test]
fn the_registry_token_comes_from_secrets_never_from_plain_variables() {
    let Some(text) = workflow() else { return };
    assert!(
        text.contains("secrets.CARGO_REGISTRY_TOKEN"),
        "the publish job must read the token from secrets"
    );
    assert!(
        !text.contains("vars.CARGO_REGISTRY_TOKEN"),
        "variables are plain text and not masked in logs; use a secret"
    );
}

#[test]
fn publishing_only_happens_on_a_push_to_master_after_the_tests() {
    let Some(text) = workflow() else { return };
    let job = text
        .split("\n  publish:")
        .nth(1)
        .expect("a `publish` job in ci.yml");
    assert!(
        job.contains("github.event_name == 'push'") && job.contains("refs/heads/master"),
        "publish must be limited to pushes to master"
    );
    assert!(
        job.contains("needs: [test, msrv]"),
        "publish must wait for the tests"
    );
    assert!(job.contains("cargo publish"));
}

#[test]
fn a_missing_secret_skips_publishing_with_a_warning_instead_of_failing_master() {
    let Some(text) = workflow() else { return };
    assert!(
        text.contains("::warning::"),
        "explain how to fix a missing token"
    );
}

#[test]
fn the_release_is_only_tagged_if_it_was_actually_published() {
    let Some(text) = workflow() else { return };
    assert!(
        text.contains("steps.publish.outputs.published == 'true'"),
        "tag only after a successful publish"
    );
}

#[test]
fn github_and_the_contributor_notes_stay_out_of_the_published_crate() {
    let manifest =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    assert!(
        manifest.contains("exclude"),
        "Cargo.toml needs an exclude list"
    );
    assert!(manifest.contains(".github/*"));
    assert!(manifest.contains("CLAUDE.md"));
}
