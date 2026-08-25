# Config — Specs

## Sources and precedence

- [x] **CONFIG-001**: The system shall read its configuration from a file at a fixed path relative to its working directory.
- [x] **CONFIG-002**: The system shall allow any single-valued configuration setting to be overridden by an environment variable carrying the project's prefix followed by the setting's name.
- [x] **CONFIG-006**: The system shall take the list of webhook entries from the configuration file only.
- [x] **CONFIG-003**: The system shall resolve each value from the environment first, then the configuration file, then a compiled-in default.
- [x] **CONFIG-004**: On startup, the system shall load a local environment file into its environment before resolving any configuration value.
- [ ] **CONFIG-005**: The system shall treat an environment variable set to an empty string as unset.

## Required and optional values

- [x] **CONFIG-010**: The system shall require the configuration file to declare the list of webhook entries.
- [x] **CONFIG-011**: The system shall accept an empty list of webhook entries and run without notifying any channel.
- [x] **CONFIG-012**: The system shall supply a compiled-in default for the search API key, the poll interval, the sidecar address, and the sidecar auth key.
- [x] **CONFIG-013**: Where a webhook entry omits an endpoint, keyword list, or ASIN list, the system shall treat it as absent rather than as an error.

## Failure behaviour

- [x] **CONFIG-020**: If the configuration file is absent or cannot be parsed, then the system shall stop before polling and report the path and the underlying error.
- [x] **CONFIG-021**: The system shall ignore keys in the configuration file that it does not recognise.
- [x] **CONFIG-022**: The system shall accept the webhook endpoint keys' previous names as aliases for their current names.
- [ ] **CONFIG-023**: If a configured webhook endpoint is not a well-formed URL, then the system shall stop before polling and report which entry and endpoint is at fault.

## Reporting

- [x] **CONFIG-030**: On startup, the system shall report its version, the number of webhook entries loaded, the sidecar address in use, and the poll interval in use.

## Deployment

- [x] **CONFIG-040**: The published monitor image shall contain no configuration file and no proxy list.
- [ ] **CONFIG-041**: The published monitor image shall declare a health check whose staleness margin is derived from the configured poll interval.
