# Modules Architecture (LarvaPHP v2)

## Overview

## Architecture Diagram

```mermaid
flowchart TD
    A[HTTP Request] --> B{Router}
    B --> C[Lua Module Routes]
    B --> D[WASM Module Routes]
    B --> E[PHP VHost Handler]
    C --> F[LuaJIT Runtime]
    D --> G[WASM Sandbox]
    F --> H[Response]
    G --> H
    E --> H
```


LarvaPHP v2 supports two native module forms:

1. **Lua modules** (embedded LuaJIT via `mlua`) — flexible, can intervene at
   any layer of the request pipeline.
2. **WASM modules** (WebAssembly) — sandboxed extensions.

Modules can register HTTP routes, hooks, middleware, and workers without
requiring PHP application code.

## Loading a Lua module

```lua
-- example.lua
larva.route("GET", "/api/hello", function(req)
    return { status = 200, body = "Hello from Lua" }
end)
```

Loaded via `larva_php::modules::lua::load_lua_module(path)`.

## Security considerations (CRITICAL)

- Lua modules run in-process with full host privileges. Only load modules from
  trusted sources.
- Never load untrusted `.lua` or `.wasm` files from user uploads.
- Validate and sandbox module paths; restrict to a configured modules directory.
- WASM modules are the safer option for untrusted code (sandboxed).
- Disable module loading in production unless explicitly configured.
- Audit all module code; modules can read/write filesystem and network.
- Use role-based access to the panel's module management endpoints.


## Lua SDK Reference

Modules interact only through the `larva` global (sandboxed — no `io`/`os`/`debug`):

- `larva.route(method, path, handler)` — register HTTP route. Handler receives
  `req` table `{method, path, body}` and returns `{status, body, headers}`.
- `larva.on_request(handler)` / `larva.on_response(handler)` — hooks.
- `larva.panel_page(slug, title)` — register a panel page.
- `larva.crypto.b64encode(s)` / `b64decode(s)` — base64.
- `larva.template.render(tpl, vars)` — `{{key}}` substitution.
- Multi-file modules: directory of `.lua` files, `init.lua` loaded first, `require()` available.

## WASM SDK

WASM modules use host functions provided by LarvaPHP (same concept set) instead
of direct OS access. Loader is a stub pending implementation.
