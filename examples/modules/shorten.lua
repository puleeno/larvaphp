-- examples/modules/shorten.lua
-- Standalone short links service in pure Lua (no PHP required)

local links = {}

larva.route("GET", "/", function(req)
    return { status = 200, body = "<h1>Shorten Links</h1><form method=POST action=/shorten><input name=url><button>Shorten</button></form>" }
end)

larva.route("POST", "/shorten", function(req)
    local code = larva.crypto.b64encode(req.body or "x"):sub(1,6)
    links[code] = req.body or ""
    return { status = 200, body = "Short code: " .. code }
end)

larva.route("GET", "/:code", function(req)
    local code = req.path:match("/(.+)")
    local url = links[code]
    if url then
        return { status = 302, headers = { Location = url } }
    end
    return { status = 404, body = "Not found" }
end)
