# vault-inject

A utility for injecting secrets from Vault into environment variables, and then running the provided command with access to those environment variables. Instead of having to manually login to vault and fetch the password(s) you need to run some command, you can wrap the command in `vault-inject`, which will prompt you for your vault credentials (LDAP, username-password or a token are supported) and then run the command, providing it the relevant secrets as environment variables of your choosing.

Here's how to create a function which just echoes a couple of secrets to stdout from a locally running version of Vault; one from the KV2 store and one from the Cubbyhole store. The latter secret is base64 encoded and reversed (you can pipe secret output through any number of commands) before being provided to the `echo` command:

```
echo_foo_bar () {
    vault-inject \
        --command 'echo $FOO, $BAR' \
        --auth-type token \
        --vault-url http://localhost:8200 \
        --secret 'FOO = /secret/foo/bar/secret_password' \
        --secret 'BAR = /cubbyhole/wibble/cubby1 | base64 | rev' \
        --token s.MtuPWVqhK0J743iB3ZgKeRmC
}
```

Most of the commands to `vault-inject` can be provided as environment variables to help save repetition incase you are defining lots of similar functions. run `vault-inject --help` for more details about the arguments that you can provide. The only required arguments are `--command` and `--vault-url`; the rest have sensible defaults but can be set to increase automation or work with non-standard Vault setups.

Here's another example which will prompt you for your LDAP password, and obtains a secret to login to some PostgresQL DB:

```
psql_dev_db () {
    vault-inject \
        --command 'psql -U postgres -d mydb -h localhost' \
        --auth-type ldap \
        --vault-url http://localhost:8200 \
        --secret 'PGPASSWORD = /secret/foo/bar/dev_db_password'
}
```

Most of the environment variables that can be provided to this command are prefixed by `VAULT_INJECT_`, with the exeption of `VAULT_ADDR` which is used to provide the URL to a Vault instance. This is for compatibility with the `vault` CLI tool which uses the same.

# Installation

## From source

You can compile `vault-inject` from source.

First, go to [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install) and install Rust.

Then to install a release of `vault-inject` (here, v0.3.0), run the following:

```
cargo install --git https://github.com/jsdw/vault-inject.git --tag v0.3.0 --force
```

This installs the latest version of `vault-inject` into a local `.cargo/bin` folder that the rust installation will have prompted you to add to your `$PATH`. The `--force` command overwrites any existing `vault-inject` binary in this folder; you can ditch it if you don't want this behaviour.