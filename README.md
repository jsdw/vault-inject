# vault-inject

A utility for injecting secrets from Vault into environment variables, and then running the provided command with access to those environment variables.

## Examples

This example plucks two secrets out of vault, `FOO` and `BAR`, and prints them both (after base64 encoding and reversing BAR):

```
vault-inject \
    --command 'echo $FOO, $BAR' \
    --vault-url http://localhost:8200 \
    --secret 'FOO = /secret/foo/bar/secret_password' \
    --secret 'BAR = /cubbyhole/wibble/cubby1 | base64 | rev' \
    --token s.MtuPWVqhK0J743iB3ZgKeRmC
```

Here's another example which will prompt you for your LDAP username and password, and obtains a secret to login to some PostgresQL DB:

```
vault-inject \
    --command 'psql -U postgres -d mydb -h localhost' \
    --auth-type ldap \
    --vault-url http://localhost:8200 \
    --secret 'PGPASSWORD = /secret/foo/bar/dev_db_password'
```

You can provide `--username` or the env var `VAULT_INJECT_USERNAME` if you'd like to not have to enter it every time. Most other arguments can also be provided as environment variables, too.

The primary use case of this tool is to create bash functions or aliases, so that you have fast access to commands that would otherwise require secrets to be manually provided each time. For example, one might add the following snippet to their `~/.bash_profile`. They could then type `dev_db` in order to access a database, injecting secrets as necessary and prompting for vault login credentials if necessary (they will, by default, be cached):

```
dev_db() {
    vault-inject \
        --command 'psql -U postgres -d mydb -h localhost' \
        --auth-type ldap \
        --vault-url http://localhost:8200 \
        --secret 'PGPASSWORD = /secret/foo/bar/dev_db_password'
}
```

To capture multiple secrets from a single path, you can use `{name}` style template parameters in environment variable names and secret names at the end of the path. You can also use the `--each` CLI paramater to perform a command against each secret we capture. This example captures all secrets from the path `/foo/bar` and echoes them out:

```
vault-inject \
    --secret '{secret} = /secret/foo/bar/{secret}'
    --each 'echo $secret_key=$secret_value'
```

Within `--each`, `$secret_key` is each environment variable name assigned in the `--secret` command. `$secret_value` is is corresponding value (also available as `$secret`).

One use case for this is exporting secrets as environment variables within the current process. Sub-processes can't alter the parent environment variables easily, but we can return the values we want and `eval` them into the environment by putting something like the following into your `.bash_profile` and then running `set_env_vars`:

```
set_env_vars() {
    export $(vault-inject \
        --secret '{secret} = /secret/foo/bar/{secret}'
        --each 'echo "$secret_key=$secret_value"')
}
```

Template parameters are pretty flexible. Another use-case of them is to only capture secrets whose keys match certain patterns. The following example finds all secrets matching `foo_{a}_{b}` (eg `foo_bar_wibble` or `foo_1_2` but not `other_bar_wibble`) and puts them in environment variables which recombine whatever matches `{a}` and `{b}` into a new name:

```
vault-inject \
    --secret 'SECRET_{b}_{a} = /secret/foo/bar/foo_{a}_{b}'
    --each 'echo $secret_key=$secret_value'
```

## Other details

This tool caches the auth tokens it obtains locally, so that you don't need to re-authenticate every time. To disable this feature, the following flags are provided:
- `--no-cache`: disable all reading and writing from the cache.
- `--no-cache-read`: disable reading from the cache (the resulting token will be written, still).
- `--no-cache-write`: disable writing to the cache (but we'll still read a token from it if possible).

You can pipe the result of running this tool to others for further processing. All informational output is piped to `stderr`, and the exit code will be non-zero if the secrets cannot be successfully obtained and processed.

Run `vault-inject --help` for more information about the available flags and options.

Supported auth types:
- **userpass**: Username & Password authentication.
- **token**: Token absed authentication.
- **ldap**: LDAP authentication.

Supported secret stores:
- **KV2**: Key-Value store (version 2).
- **Cubbyhole**: Cubbyhole store.

# Installation

## From pre-built binaries

Prebuilt compressed binaries are available [here](https://github.com/jsdw/vault-inject/releases/latest). Download the compressed `.tar.gz` file for your OS/architecture and decompress it (on MacOS, this is automatic if you double-click the downloaded file).

If you like, you can download and decompress the latest release on the commandline:

### Installing a MacOS binary

Run:

```
curl -L https://github.com/jsdw/vault-inject/releases/download/v0.5.0/vault-inject-v0.5.0-x86_64-apple-darwin.tar.gz | tar -xz
```

You'll end up with a `vault-inject` binary in your current folder. The examples assume that you have placed this into your `$PATH` so that it can be called from anywhere.

### Installing a Linux binary

For Linux, you can use a binary which is dynamically linked against the GNU libc, or a fully static binary using the musl libc implementation. If you don't know which one to use, either is probably fine.

For the fully static musl binary, run:

```
curl -L https://github.com/jsdw/vault-inject/releases/download/v0.5.0/vault-inject-v0.5.0-x86_64-unknown-linux-musl.tar.gz | tar -xz
```

For the GNU binary, run:

```
curl -L https://github.com/jsdw/vault-inject/releases/download/v0.5.0/vault-inject-v0.5.0-x86_64-unknown-linux-gnu.tar.gz | tar -xz
```

In any case, you'll end up with a `vault-inject` binary in your current folder. The examples assume that you have placed this into your `$PATH` so that it can be called from anywhere.

## From source

This is probably the simplest way to build a binary for your current OS. Run the following:

```
# Install Rust (You'll need v1.42 or later):
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Compile and install vault-inject (here, v0.5.0):
cargo install --git https://github.com/jsdw/vault-inject.git --tag v0.5.0 --force
```

This installs the latest version of `vault-inject` into a local `.cargo/bin` folder that the rust installation will have prompted you to add to your `$PATH`. The `--force` command overwrites any existing `vault-inject` binary in this folder; you can ditch it if you don't want this behaviour.

## From source via docker

You can lean on docker images to build a Linux or MacOS binary without installing Rust locally.

You'll need to clone this repo locally to run these commands. All commands assume that this folder is the current working directory.

For convenience, commands have been packaged into a small `docker-build.sh` script.

### Building a Linux binary

Run one of the following (for either a GNU or musl binary):

```
docker-build.sh linux-gnu
docker-build.sh linux-musl
```

The binary is created at `target/x86_64-unknown-linux-{gnu|musl}/vault-inject`. Put that binary wherever you'd like (eg. into a `$PATH` such as `/usr/bin`).

### Building a MacOS binary

Using arcane black magic, we can also build a MacOS binary (with the target triplet `x86_64-apple-darwin`). To do so, run the following:

```
docker-build.sh mac
```

The binary is created at `target/x86_64-apple-darwin/release/vault-inject`. Put that binary wherever you'd like (eg. into a `$PATH` such as `/usr/bin`).
