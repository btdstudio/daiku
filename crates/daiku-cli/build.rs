//! Stamps the version `daiku version` prints (architecture D17): `v<crate version>`
//! for a release build (`DAIKU_RELEASE=1`, set by the release workflow), `dev`
//! otherwise, as Go's dev builds print.

fn main() {
    println!("cargo::rerun-if-env-changed=DAIKU_RELEASE");
    let version = if std::env::var("DAIKU_RELEASE").as_deref() == Ok("1") {
        format!(
            "v{}",
            std::env::var("CARGO_PKG_VERSION").unwrap_or_default()
        )
    } else {
        "dev".to_string()
    };
    println!("cargo::rustc-env=DAIKU_VERSION={version}");
}
