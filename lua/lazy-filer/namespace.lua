local ns = { value = "lazy-filer" }

local ns_rpc = require("nvim-router").rpc(ns.value)

return {
    update = function(new_ns)
        ns.value = new_ns
        ns_rpc.update_ns(new_ns)
    end,

    get_info = function(plugin_root)
        return {
            path = plugin_root .. "/lazy-filer.rs",
            handler = "NeovimHandler",
            ns = ns.value,
        }
    end,

    rpc = ns_rpc,
}
