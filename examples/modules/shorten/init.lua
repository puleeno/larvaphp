-- init.lua
local M = {}
M.links = {}
M.counter = 0
local CHARS = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
function M.gen_code()
    M.counter = M.counter + 1
    local code = ""
    for i = 1, 6 do
        local idx = math.random(1, 62)
        code = code .. string.sub(CHARS, idx, idx)
    end
    while M.links[code] do
        code = ""
        for i = 1, 6 do
            local idx = math.random(1, 62)
            code = code .. string.sub(CHARS, idx, idx)
        end
    end
    return code
end
_G.shorten = M
require("routes")
require("dashboard")
