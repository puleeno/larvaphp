-- examples/modules/shorten.lua
-- Standalone short links service in pure Lua (no PHP required)
-- Includes dashboard UI and JSON API for third-party integration.

local links = {}
local counter = 0

local function gen_code()
    counter = counter + 1
    return larva.crypto.b64encode(tostring(counter)):sub(1,6)
end

-- Dashboard
larva.route("GET", "/dashboard", function(req)
    local rows = ""
    for code, url in pairs(links) do
        rows = rows .. "<tr><td>" .. code .. "</td><td>" .. url .. "</td></tr>"
    end
    local html = "<h1>Shorten Links Dashboard</h1><table border=1><tr><th>Code</th><th>URL</th></tr>" .. rows .. "</table>"
    html = html .. "<h2>Create</h2><form method=POST action=/api/shorten><input name=url placeholder='https://...'><button>Create</button></form>"
    return { status = 200, body = html }
end)

-- JSON API for third parties
larva.route("POST", "/api/shorten", function(req)
    local url = req.body or ""
    local code = gen_code()
    links[code] = url
    return { status = 200, body = '{"code":"' .. code .. '","short_url":"/' .. code .. '"}' }
end)

larva.route("GET", "/api/list", function(req)
    local out = "["
    local first = true
    for code, url in pairs(links) do
        if not first then out = out .. "," end
        first = false
        out = out .. '{"code":"' .. code .. '","url":"' .. url .. '"}'
    end
    out = out .. "]"
    return { status = 200, body = out }
end)

-- Redirect
larva.route("GET", "/:code", function(req)
    local code = (req.path or ""):match("/(.+)")
    local url = links[code]
    if url and url ~= "" then
        return { status = 302, headers = { Location = url } }
    end
    return { status = 404, body = "Not found" }
end)

-- Home
larva.route("GET", "/", function(req)
    return { status = 302, headers = { Location = "/dashboard" } }
end)
