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