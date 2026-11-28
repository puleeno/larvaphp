-- init.lua
local M = {}
M.links = {}
M.counter = 0
function M.gen_code()
    M.counter = M.counter + 1
    return larva.crypto.b64encode(tostring(M.counter)):sub(1,6)
end
_G.shorten = M
require("routes")
require("dashboard")
