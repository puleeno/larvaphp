local M = _G.shorten
local function get_param(body, key)
    if not body then return "" end
    for k, v in body:gmatch("([^&=]+)=([^&=]+)") do
        if k == key then return v:gsub("%%(%x%x)", function(h) return string.char(tonumber(h,16)) end):gsub("+"," ") end
    end
    return ""
end

larva.route("POST", "/api/shorten", function(req)
    local url = get_param(req.body, "url")
    if url == "" then url = req.body or "" end
    local code = M.gen_code()
    M.links[code] = url
    return { status = 200, body = '{"code":"' .. code .. '","short_url":"/' .. code .. '"}' }
end)
larva.route("GET", "/:code", function(req)
    local code = (req.path or ""):match("/(.+)")
    local url = M.links[code]
    if url and url ~= "" then
        return { status = 302, headers = { Location = url } }
    end
    return { status = 404, body = "Not found" }
end)
