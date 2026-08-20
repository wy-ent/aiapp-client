# Telemetry Schema v1 — Anonymous Event Reporting Contract

> A unified, anonymous event contract reserved for incentives / ad revenue share / billing.
> The open-source side (enterprise local admin) only **produces** events and anonymously reports them to the closed-source Pro service;
> the closed-source side `aiapp-saas` receives and aggregates them, generating statistics and share/billing bills.
> Both sides evolve independently: **this file is the single source of truth**, and changes require bumping `schema_version`.

## Transport

- Method / Path: `POST {PRO_URL}/v1/telemetry`
- Request header: optional `X-Api-Key: <PRO_API_KEY>` (required when the Pro service is configured with a key)
- Protocol: HTTP/1.1, JSON body, UTF-8
- Failure policy: **silent** (open-source side is fire-and-forget, never affects the main flow); enterprises can fully
  disable it via `AIAPP_TELEMETRY=false` (fully private mode, no revenue share / cloud statistics).

## Event Body (Common Envelope)

```json
{
  "schema_version": 1,
  "type": "app_launch",
  "ts": "2026-08-18 20:30",
  "detail": { }
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `schema_version` | int | ✅ | Version of the metric definition; currently `1` |
| `type` | string | ✅ | Event type (see table below) |
| `ts` | string | ✅ | Local time `YYYY-MM-DD HH:mm` |
| `detail` | object | ✅ | Event-specific extra fields |

> Anonymity principle: **no** field that can identify a user or enterprise is carried (no user_id, no org, no IP detail).
> Events are used only for aggregated counts and totals.

## Event Types

| type | Trigger | detail fields | Purpose |
|---|---|---|---|
| `user_register` | A new user registers successfully | `{}` | Registration volume statistics |
| `app_generate` | A user generates an app successfully | `{ "template": "todo", "update": false }` | Generation volume statistics, per-template profile |
| `app_launch` | An app is opened (detail visit) | `{ "template": "todo" }` | Launch count / approximate DAU |
| `app_report` | A user reports an app | `{ "app_id": "gen_1" }` | Content moderation hotspots |
| `app_publish` | An app is published / shared | `{ "share": "public" \| "org" }` | Ecosystem activity |

## Server-Side Behavior (aiapp-pro-server)

- `POST /v1/telemetry`: lenient parsing (unrecognized types counted as `unknown`), counts aggregated by `type`,
  retains the most recent events (in-memory MVP, cap 10k entries; production uses a database / message queue).
- `GET /v1/saas/stats`: returns `{ counts, events }` for the closed-source operations dashboard.
- Event authenticity is not validated (the open-source side is trusted); for safety, rate limiting can be applied at the gateway layer.

## Evolution Rules

- Adding an event type: append to this file + `aiapp-pro-server/src/saas.rs::EVENT_TYPES`.
- Changing fields: bump `schema_version` (the Pro side can keep parsing older-version events for a while).
- Open-source side implementation: `aiapp-mb/crates/aiapp-web/src/telemetry.rs`.
