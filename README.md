# vault-inject

A utility for injecting secrets from Vault into environment variables, and then running the provided command with access to those environment variables. This is particularly useful for creating shell aliases to common commands which automatically pull the relevant credentials from vault on execution.

Here's an example that creates an alias to the `psql` command (PostgresQL repl), pulling the password from vault:

```
alias db='vault-inject "psql -U jsdw -d my_db" --username jsdw --vault-url "http://path.to.vault" --auth-type userpass --secret 'PGPASSWORD=kv2://db/mydb/password''
```

Running `db` would then ask for my vault password (and would assume that my vault username was 'jsdw') and would then login to my psql database using the password stored in Vault's KV2 store at `/db/mydb` with the key `password`.