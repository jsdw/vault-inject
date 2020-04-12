# The Release Process

Assuming we've committed/merged the code we'd like to be in the latest version, do the following to actually release the new version:

1. Bump version in Cargo.toml
2. Run `cargo check && cargo test` and ensure nonzero exit code
4. Ensure `README.md` references the latest version throughout
5. Create github release with `git tag VERSION` and then `git push --tags`

Assuming a manual release and no CI, also do the following:

6. Run `./docker-build all --release` to build binaries in root folder
7. Upload the binaries to the release via "Releases -> VERSION -> Edit Tag"