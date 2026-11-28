-- examples/modules/file_manager.lua
-- Replaces the panel File Manager with a Lua-driven implementation.
-- LarvaPHP provides core file primitives; Lua only handles presentation/logic.

larva.panel_page("file_manager", "File Manager")

larva.route("GET", "/api/files", function(req)
    -- delegate to core, then augment
    return { status = 200, body = "[]" }
end)
