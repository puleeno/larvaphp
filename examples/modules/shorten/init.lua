local M = {}
M.links = {}
M.counter = 0
local CHARS = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"

-- Ensure table exists
pcall(function() larva.db.execute("CREATE TABLE IF NOT EXISTS shorten_links (code TEXT PRIMARY KEY, url TEXT NOT NULL, clicks INTEGER DEFAULT 0, created_at TEXT DEFAULT (datetime('now')))") end)
pcall(function() larva.db.execute("ALTER TABLE shorten_links ADD COLUMN clicks INTEGER DEFAULT 0") end)

function M.gen_code()
    M.counter = M.counter + 1
    local len = 8
    local code = ""
    for i = 1, len do
        local idx = math.random(1, 62)
        code = code .. string.sub(CHARS, idx, idx)
    end
    while true do
        local rows = larva.db.query("SELECT 1 FROM shorten_links WHERE code = '" .. code .. "'")
        if #rows == 0 then break end
        code = ""
        for i = 1, 8 do code = code .. string.sub(CHARS, math.random(1,62), math.random(1,62)) end
    end
    return code
end

function M.save(code, url)
    larva.db.execute("INSERT INTO shorten_links (code, url) VALUES ('" .. code .. "', '" .. url .. "')")
end

function M.find(code)
    local rows = larva.db.query("SELECT url FROM shorten_links WHERE code = '" .. code .. "'")
    if #rows > 0 then return rows[1].url end
    return nil
end

function M.all()
    return larva.db.query("SELECT code, url, clicks FROM shorten_links ORDER BY created_at DESC")
end

_G.shorten = M
require("routes")
require("dashboard")
