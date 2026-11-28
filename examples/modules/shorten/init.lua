-- init.lua
local M = {}
M.links = {}
M.counter = 0
local CHARS = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
function M.gen_code()
    M.counter = M.counter + 1
    local n = M.counter
    local code = ""
    for i = 1, 6 do
        code = code .. CHARS:sub((n % 62) + 1, (n % 62) + 1)
        n = math.floor(n / 62)
    end
    return code
end
_G.shorten = M
require("routes")
require("dashboard")
