-- routes.lua
local M = _G.shorten
larva.route("POST", "/api/shorten", function(req)
    local url = req.body or ""
    local code = M.gen_code()
    M.links[code] = url
    return { status = 200, body = '{"code":"' .. code .. '"}' }
end)
larva.route("GET", "/:code", function(req)
    local code = (req.path or ""):match("/(.+)")
    local url = M.links[code]
    if url and url ~= "" then
        return { status = 302, headers = { Location = url } }
    end
    return { status = 404, body = "Not found" }
end)
