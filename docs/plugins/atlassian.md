# Atlassian (Jira + Confluence) plugin

A single plugin covers both Atlassian Cloud products. Two auth paths are
supported; pick whichever fits.

## Commands

| Quickkey | Command | Notes |
|---|---|---|
| `jmi` | My Issues | Open Jira issues assigned to you. |
| `jsp` | Active Sprint | Issues in the active scrum sprint of your first board. |
| `jtr` | Triage Queue | Unassigned To-Do issues in the default project. |
| `jnw` | New Jira Issue | Form-driven create. |
| `jtx` | Transition Issue | Form prompts for issue key, lists transitions, applies on click. |
| `jcm` | Comment on Issue | Two-field form: issue key + body. |
| `cre` | Recent Pages | Recently updated Confluence pages. |
| `csr` | Search Confluence | CQL full-text search. |
| `cmy` | My Pages | Pages you authored, newest first. |
| `cnw` | New Page | Create a Confluence page (plain text → `<p>` storage format). |

## Auth — pick one

### Option 1: API token (simplest)

1. Create an API token at <https://id.atlassian.com/manage-profile/security/api-tokens>.
2. Set the secrets:
   ```sh
   lark secret set ATLASSIAN_EMAIL       # your Atlassian account email
   lark secret set ATLASSIAN_API_TOKEN   # the token from step 1
   ```
3. Set the host in plugin settings (TUI: Plugin Manager → Atlassian → Settings):
   - **`atlassian_host`** = `acme.atlassian.net` (no `https://`, no trailing slash)
4. Optional: set `default_project_key` for the Triage and New-Issue commands.

That's it. Hit `jmi` and you should see your issues.

### Option 2: OAuth 2.0 (browser flow)

Better UX (no copy-paste tokens) but takes one extra setup step.

1. **One-time gate:** the OAuth `client_id` must be configured. Either:
   - Wait until larkline ships with a baked `client_id` (registered by the
     maintainer), or
   - Register your own public OAuth 2.0 (3LO) app at
     <https://developer.atlassian.com/console/myapps/> and export
     `LARKLINE_ATLASSIAN_CLIENT_ID=<your-client-id>` before running login.

   Required scopes: `read:jira-work`, `read:jira-user`, `write:jira-work`,
   `read:confluence-content.all`, `read:confluence-user`,
   `write:confluence-content`, `offline_access`.

   Required callback URL: `http://127.0.0.1` (Atlassian allows any port if the
   host matches).
2. Run the login:
   ```sh
   lark atlassian login
   ```
   Browser opens, you approve, the redirect lands on `127.0.0.1:<ephemeral>`,
   and the token is stored in your macOS Keychain. If you have multiple
   Atlassian sites, the CLI will prompt to pick one.
3. Verify:
   ```sh
   lark atlassian status
   ```
4. Hit `jmi` — should now use OAuth automatically.

### Auth precedence

When both are configured, **API token wins**. Useful for a temporary override
when an OAuth refresh token is misbehaving — set the env vars in your current
shell and the plugin will use them just for that session.

## `lark atlassian` subcommands

Plumbing for the OAuth path. Plugins call these via `lark.exec`; you rarely
need to invoke them directly.

| Command | Behavior |
|---|---|
| `lark atlassian login` | Browser-based authorization. One-time. |
| `lark atlassian token` | Print a valid access token. Refreshes silently if expired. Empty stdout + exit 1 when not signed in. |
| `lark atlassian cloudid` | Print the active cloud id from Keychain. |
| `lark atlassian site` | Print the human-facing site URL. Used by plugins for "Open in browser" actions. |
| `lark atlassian status` | Show signed-in account + cloud + token-validity countdown. |
| `lark atlassian logout` | Wipe Keychain + cache. Idempotent. |

## Plugin settings

Open the Plugin Manager (`P` key in the TUI), find Atlassian, edit settings:

| Setting | Purpose |
|---|---|
| `atlassian_host` | Base host for API-token mode (e.g. `acme.atlassian.net`). Ignored in OAuth mode. |
| `default_project_key` | Used by Triage and pre-fills New Jira Issue. |
| `max_results` | Default page size for list commands (25 / 50 / 100). |

## Where state lives

| Item | Location |
|---|---|
| `ATLASSIAN_EMAIL`, `ATLASSIAN_API_TOKEN` | macOS Keychain via `lark secret set`, or `~/.config/larkline/.env` |
| OAuth refresh token | macOS Keychain (`ATLASSIAN_REFRESH_TOKEN`) |
| OAuth cloud id + email + site URL | macOS Keychain (`ATLASSIAN_CLOUDID`, `ATLASSIAN_ACCOUNT_EMAIL`, `ATLASSIAN_SITE_URL`) |
| OAuth access token + expiry (1-hour cache) | `$XDG_CACHE_HOME/larkline/atlassian-access.json` (mode 0600) |
| Plugin settings | `lark.store` (per-plugin JSON in `$XDG_DATA_HOME/larkline/stores/atlassian.json`) |

## Troubleshooting

**"Not signed in to Atlassian"** — Either set the API-token env vars + host, or
run `lark atlassian login`. The error PluginOutput has a chain action that
launches the OAuth flow directly.

**"401 Unauthorized — token revoked or expired"** — In API-token mode: the
token was deleted at id.atlassian.com or your account changed. Generate a new
one. In OAuth mode: refresh token revoked. Run `lark atlassian login` again.

**"403 Forbidden — account lacks permission"** — Your account doesn't have the
specific scope this command needs. For OAuth, the consent screen requested all
scopes upfront, so this typically means the Jira/Confluence admin restricted
the project. For API-token, the token inherits your full account permissions.

**"Atlassian session expired"** — Refresh token was revoked at the Atlassian
side. `lark atlassian login` to re-auth.

**Multiple cloud sites** — `lark atlassian login` prompts to pick one. To
switch, run `lark atlassian logout` then `lark atlassian login` again. (A
dedicated `switch` subcommand is a v0.12.x backlog item.)

**Linux / Windows** — OAuth requires macOS Keychain (via the `security` CLI).
On other platforms, use the API-token path: store secrets in
`~/.config/larkline/.env` (KEY=VALUE per line).

**ADF rendering looks wrong** — Atlassian Document Format covers ~6 common node
types in our reducer (paragraph, heading, text, bulletList, orderedList,
codeBlock, link). Unsupported nodes render as `[unsupported: <type>]`. File an
issue if you hit one in practice.

## Storage format vs ADF

- **Jira** uses **ADF** (Atlassian Document Format, JSON-based). The `comment`
  and `new-issue` commands wrap user-typed plain text in a minimal ADF doc
  (`paragraph` + `text` + `hardBreak` nodes).
- **Confluence** uses **storage format** (HTML/XML-flavored). The `new-page`
  command wraps plain text paragraphs in `<p>...</p>` and `\n` → `<br/>`. Users
  who paste raw storage XML (anything starting with `<`) get it through
  un-transformed.
