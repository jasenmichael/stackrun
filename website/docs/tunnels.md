---
id: tunnels
sidebar_position: 6
title: Cloudflare tunnels for local OAuth
description: Use a named Cloudflare tunnel so local apps keep a stable HTTPS callback URL for OAuth, OIDC, and SSO. Quick tunnels are fine for a one-off share.
---

A command with no `tunnel` key has no `cloudflared` sibling.

Tunnels are especially useful when developing apps that use OAuth, OIDC, SSO, or other authentication services that require a fixed HTTPS callback URL.

Stackrun can expose your local app through a Cloudflare tunnel, letting you test authentication against your local code using the same URL registered with your auth provider. A named tunnel keeps that URL consistent between runs, so you don't need to deploy the app or constantly update callback URLs just to test login. Quick tunnels are fine for a one-off share; named hosts are what you want when the callback is fixed.

| Field | Description |
| --- | --- |
| `local` | Local origin (`http://127.0.0.1:4000`). |
| `public` | Public hostname. Set this for a named tunnel + `route dns`. Omit it for a quick tunnel. |
| `env` | Merged over `env` when tunneling is on. |
| `resource` | Cloudflare object name. Else the command's prefix/name, else `stackrun`. Unique among named tunnels. |
| `color` | Cloudflared prefix color. Default `cyan`. |
| `removeExisting` | Delete an existing named tunnel and overwrite DNS. |

The sibling log token is `[tunnel-<prefix>]` (command `prefix` if set, else sliced `name`). Example: `web` → `[web]` + `[tunnel-web]`.

**Quick** (`local` only): `cloudflared tunnel --url <local>` opens a random `*.trycloudflare.com` host. No login, no token, no DNS.

**Named** (`local` + `public`): `cloudflared tunnel create` + `route dns` + `tunnel run --url <local> <resource>`.

Named tunnels need `cert.pem` from `cloudflared tunnel login`. `removeExisting: true` on the command deletes an existing name and overwrites DNS.

Cleanup deletes the tunnel and local creds. It does not delete the CNAME. A leftover host shows Cloudflare `1016`.

`--tunnel` / `TUNNEL=true` force tunnels on. A top-level `tunnel` key is ignored. Omit `tunnel` on a command to skip its sibling.

`--tunnel` with zero `local` exits 1 before hooks. If tunneling is on and `cloudflared` is missing (or named tunnels lack `cert.pem`), stackrun exits before hooks and prints install + login steps.
