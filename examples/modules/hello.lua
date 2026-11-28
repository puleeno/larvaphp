-- examples/modules/hello.lua
-- Standalone API service in pure Lua (no PHP required)

larva.route("GET", "/api/hello", function(req)
    return { status = 200, body = "Hello from Lua!" }
end)

larva.on_request(function(req)
    -- can inspect/modify request
    return req
end)
