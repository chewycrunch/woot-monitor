---
parent: high-level-design
prefix: CONFIG
---

# Config

## Context and Design Philosophy

Config owns what is adjustable, where each value comes from, and how a bad configuration announces itself.

The organising question is what a value *belongs to*. Webhook endpoints, keywords and ASINs describe what the operator wants told to them — they are the same wherever the monitor runs, and they change when intent changes. Addresses, credentials and cadence describe where this particular instance runs — they differ between a laptop and a server, and they change when the deployment changes. The first kind lives in a file; the second arrives from the environment. Neither is compiled into the binary or baked into an image.

## Sources and precedence

Values resolve through three layers, each overriding the one below:

1. Environment variables, prefixed to avoid collision with unrelated variables
2. The configuration file
3. Compiled-in defaults

A value with a compiled-in default is optional everywhere. A value without one must appear in the file.

An environment variable set to an empty string means *unset*, and resolution falls through to the file and then the default. An empty address or credential can only fail, and an empty assignment is a common way to express absence in a deployment file. **The implementation diverges: an empty variable is currently taken as a literal empty value.**

An empty list of webhook entries is valid. The monitor runs and notifies nobody, which is a legitimate way to keep it polling while every channel is turned off.

The file is read from a path relative to the working directory, which in a container is the directory the configuration is mounted into. Locally, a dotfile is loaded into the environment at startup before any value is read, so a developer can set operational values without exporting them; no such file exists in the image, and a deployment supplies them through the container's environment instead.

## What lives where

| Value | Source | Default |
|---|---|---|
| Webhook entries — name, endpoints, keywords, ASINs | File only | None; required |
| Search API key | Environment, or file | Compiled-in |
| Poll interval | Environment, or file | Compiled-in |
| Sidecar address | Environment, or file | Compiled-in, suited to a local sidecar |
| Sidecar auth key | Environment, or file | Compiled-in, matching the sidecar image's own default |
| Log filter and format | Environment only | Compiled-in |

The search API key is an upstream credential that expires on the upstream's schedule rather than the operator's. Placing it behind an environment variable means a rotation is a deployment edit and a restart, not a rebuild — and it is the reason the environment layer exists at all rather than the file being the only source.

Log filtering and format are consumed by the logging framework directly and never reach the configuration structure.

## Failure behaviour

A missing or malformed configuration file stops the process before any polling begins, naming the path and the underlying error. The path is relative, so the common cause is a missing mount, and the message has to be enough to diagnose that from a crash line alone.

This is deliberately louder than the rest of the system. A configuration error is unrecoverable — no retry will fix a file that is absent — whereas a network error is transient by default. Stopping immediately also means an operator sees the failure at deploy time rather than discovering later that notifications never arrived.

A configured endpoint that is not a well-formed URL is a configuration error and is rejected at load, alongside a missing or malformed file. Accepting it instead would defer the failure to delivery, where it recurs silently for every offer and is never retried — the quietest possible home for a typo.

An unknown key in the file is ignored rather than rejected, which is what makes a renamed key silently stop working. Renamed keys therefore keep their previous names as accepted aliases until deployments have caught up.

## Deployment shape

The monitor image declares a health check that invokes the monitor binary in a health mode. That mode reads the liveness signal detection emits and nothing else — no configuration file, no environment, no proxy list — because the staleness margin it needs is recorded in the signal itself. A deployment that changes the poll interval therefore needs no change to the check, and no check can fail for a reason unrelated to whether polling is working. The signal's location is part of the image's contract rather than a configurable value.

The published images contain no configuration. Both the configuration file and the proxy list are supplied at runtime, which is what allows one image to serve every deployment. A deployment that forgets the configuration mount fails immediately and loudly; one that forgets the proxy mount runs unproxied, visible only as a count at startup.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| File format | TOML | YAML; JSON | JSON has no comments, and a hand-edited config needs them. The maintained YAML libraries are forks of an archived crate. |
| Layering | Environment over file over default | File only; environment only | Structured values like a keyword list are unwieldy in environment variables; deployment values are unwieldy in a file that is otherwise identical everywhere. |
| Health check command | The monitor binary in a health mode | A shell one-liner over the signal file; an HTTP endpoint the check polls | A shell check puts the staleness arithmetic in a Dockerfile string, where no test in the crate can reach it. An endpoint adds a listening socket to a process that otherwise only makes outbound requests. |
| Config in the image | Never baked | Bake a default and override by mount | A baked file means an image carries one deployment's endpoints, and a missing mount silently runs against them instead of failing. |
| Bad config | Stop the process | Warn and continue with defaults | Continuing means notifications silently stop; no retry fixes an absent file. |
| Malformed endpoint | Rejected at load | Accepted, failing at delivery | A typo caught at deploy time is loud and immediate; the same typo caught at delivery is silent and permanent. |
| Renamed keys | Accepted as aliases | Hard rename | Unknown keys are ignored, so a hard rename stops a channel silently — the worst failure shape available. |
| Env prefix | Project-specific prefix | Bare names | An unprefixed environment source maps every variable in the process into the configuration namespace. |
| Empty env var | Treated as unset | Taken literally | An empty address or credential cannot succeed, and an empty assignment reads as absence in a deployment file. |
| Empty webhook list | Valid | Rejected as pointless | Turning every channel off is a legitimate state, and rejecting it would make the config file's only required key also its only unremovable one. |
| Local dotfile | Loaded at startup, absent in the image | Required everywhere; not used at all | It removes friction locally and is inert where it does not exist. [inferred] |

## Open Questions & Future Decisions

### Resolved
1. ✅ Webhook entries belong in the file; deployment values belong in the environment.
2. ✅ A missing configuration file is fatal; a missing proxy list is not.

### Deferred
1. The implementation treats an empty environment variable as a literal value rather than as unset.
2. The implementation does not yet validate endpoints at load.
3. Aliases for the previous webhook key names are carried indefinitely; there is no signal telling an operator a deployment still uses them.
4. The poll interval accepts any value, including zero, which would poll as fast as the upstream answers.
5. The configuration file path is fixed relative to the working directory and cannot be overridden, so the mount location is part of the image's contract.
6. The proxy list is not part of the configuration structure and follows its own path and format.

## References

- `docs/high-level-design.md` — segment boundaries and tenets.
- `docs/intent/fetching/fetching-design.md` — the proxy list and sidecar key contract.
- `docs/intent/routing/routing-design.md` — what webhook entries mean.
