local ns = { value = "lazy-filer" }

return {
    get = function()
        return ns.value
    end,

    update = function(new_ns)
        ns.value = new_ns
    end,

    get_info = function(plugin_root)
        return {
            package = "lazy-filer-rs",
            path = plugin_root .. "/lazy-filer.rs",
            handler = "NeovimHandler",
            ns = ns.value,
        }
    end,
}
