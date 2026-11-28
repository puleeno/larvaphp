-- init.lua
local M = {}
M.links = {}
M.counter = 0
local CHARS = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
function M.gen_code()
    M.counter = M.counter + 1
    local code = ""
    for i = 1, 6 do
        code = code .. CHARS:sub(math.random(1, 62), math.random(1, 62))
    end
    -- ensure not duplicate
    while M.links[code] do
        code = ""
        for i = 1, 6 do code = code .. CHARS:sub(math.random(1, 62), math.random(1, 62)) end
    end
    return code
end
_G.shorten = M
require("routes")
require("dashboard")
