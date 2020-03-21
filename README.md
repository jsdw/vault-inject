# vault-inject

A utility for injecting secrets from Vault into environment variables, and then running the provided command with access to those environment variables.

This example plucks two secrets out of vault, `FOO` and `BAR`, and prints them both (after base64 encoding and reversing BAR):

```
echo_foo_bar () {
    vault-inject \
        --command 'echo $FOO, $BAR' \
        --vault-url http://localhost:8200 \
        --secret 'FOO = /secret/foo/bar/secret_password' \
        --secret 'BAR = /cubbyhole/wibble/cubby1 | base64 | rev' \
        --token s.MtuPWVqhK0J743iB3ZgKeRmC
}
```

Here's another example which will prompt you for your LDAP username and password, and obtains a secret to login to some PostgresQL DB:

```
psql_dev_db () {
    vault-inject \
        --command 'psql -U postgres -d mydb -h localhost' \
        --auth-type ldap \
        --vault-url http://localhost:8200 \
        --secret 'PGPASSWORD = /secret/foo/bar/dev_db_password'
}
```

You can provide `--username` or the env var `VAULT_INJECT_USERNAME` if you'd like to not have to enter it every time. Most other arguments can also be provided as environment variables, too.

This tool caches the auth tokens it obtained locally, so that you don't need to re-authenticate every time. To disable this feature, the following flags are provided:
- `--no-cache`: disable all reading and writing from the cache.
- `--no-cache-read`: disable reading from the cache (the resulting token will be written, still).
- `--no-cache-write`: disable writing to the cache (but we'll still read a token from it if possible).

You can pipe the result of running this tool to others for further processing. All error output is piped to `stderr`, and the exit code will be non-zero if the secrets cannot be successfully obtained and processed.

Run `vault-inject --help` for more information about the available flags and options.

Supported auth types:
- **userpass**: Username & Password authentication.
- **token**: Token absed authentication.
- **ldap**: LDAP authentication.

Supported secret stores:
- **KV2**: Key-Value store (version 2).
- **Cubbyhole**: Cubbyhole store.

# Installation

## Using docker

### For a Linux binary

A docker one-liner to compile a linux-compatible binary (with the target triplet `x86_64-unknown-linux-gnu`) is as follows:

```
docker run \
    -it \
    --rm \
    --user "$(id -u)":"$(id -g)" \
    -v "$PWD":/code \
    -w /code rust:1.42.0 \
    cargo build --release
```

The binary is created at `target/releases/vault-inject`. Put that binary wherever you'd like (eg. into a `$PATH` such as `/usr/bin`).

Finally, to clean up any cached bits after you've moved the binary, run `rm -rf target`.

## From source

You can compile `vault-inject` from source.

First, go to [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install) and install Rust. If you already have rust installed, run `rustup update` to update to the latest version. You'll want to be using version 1.42 or newer (which you can check using `rustc --version`).

Then to compile and install a release of `vault-inject` (here, v0.4.2), run the following:

```
cargo install --git https://github.com/jsdw/vault-inject.git --tag v0.4.2 --force
```

This installs the latest version of `vault-inject` into a local `.cargo/bin` folder that the rust installation will have prompted you to add to your `$PATH`. The `--force` command overwrites any existing `vault-inject` binary in this folder; you can ditch it if you don't want this behaviour.