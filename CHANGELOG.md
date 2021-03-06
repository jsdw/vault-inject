# v0.5.0

- Allow template `{params}` in environment variable names and key names (at the end of paths) when getting secrets, so that you can capture and assign multiple environment variables at once using basic pattern matching.
- Add `each` CLI option which runs a command for each secret obtained (relevant details exposed in the command you provide as `$secret_key` and `$secret_value`). Any `each` command provided runs before the `command`. This makes it easy to do things like echoing out all secrets for exporting.

# v0.4.3

- Add instructions/config to cross compile to MacOS
- Lean on rustls over openssl to ease cross-compiling to MacOS

# v0.4.2

- Optimise the compile for size

# v0.4.1

- Trim the trailing newline added when piping secrets through commands

# v0.4.0

- Cache tokens by default so that you don't need to re-login
- Tweak approach to finding secret mount points to need fewer permissions
- Add CLI flags to disable the cache if necessary

# v0.3.0

- Support multiple of the same type of secret engines being mounted at the same time

# v0.2.0

- Move to async requests to allow fetching multiple secrets in parallel
- Discover secret mount points and remove the need to explicitly declare secret type in paths

# v0.1.0

Initial release