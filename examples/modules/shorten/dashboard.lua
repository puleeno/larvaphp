-- dashboard.lua
local M = _G.shorten
larva.route("GET", "/dashboard", function(req)
    local rows = ""
    for code, url in pairs(M.links) do
        rows = rows .. "<tr><td>" .. code .. "</td><td>" .. url .. "</td></tr>"
    end
    return { status = 200, body = "<h1>Dashboard</h1><table border=1><tr><th>Code</th><th>URL</th></tr>" .. rows .. "</table>" }
end)
larva.route("GET", "/", function(req)
    return { status = 302, headers = { Location = "/dashboard" } }
end)
