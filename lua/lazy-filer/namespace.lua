local M = {}

local default_ns = "lazy-filer"

local router = require("nvim-router")

M.rpc = {
    notify = function() end,
    request = function() end,
}

function M.register(plugin_root, new_ns)
    local info = {
        path = plugin_root .. "/lazy-filer.rs",
        handler = "NeovimHandler",
    }

    if new_ns then
        info.ns = new_ns
    else
        info.ns = default_ns
    end

    local rpc = router.register(info)
    M.rpc.notify = rpc.notify
    M.rpc.request = rpc.request
end

return M
