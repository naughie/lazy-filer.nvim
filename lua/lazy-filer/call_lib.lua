local states = require("lazy-filer.states")
local rpc = require("lazy-filer.namespace").rpc

local ui = states.ui

return {
    create_entry = function(dir_line_idx, fname)
        rpc.notify("create_entry", dir_line_idx, fname)
    end,

    delete_entry = function(dir_line_idx)
        if dir_line_idx == 0 then return end
        rpc.notify("delete_entry", dir_line_idx)
    end,

    expand_dir = function(line_idx)
        if line_idx == 1 then return end
        rpc.notify("expand_dir", line_idx - 1)
    end,

    get_dir = function(line_idx)
        local dir = rpc.request("get_dir", line_idx - 1)
        if dir == vim.NIL then dir = nil end
        return { name = dir, idx = line_idx - 1 }
    end,

    get_file_path = function(line_idx)
        local file = rpc.request("get_file_path", line_idx - 1)
        if file == vim.NIL then file = nil end
        return { name = file, idx = line_idx - 1 }
    end,

    move_to_parent = function(cwd)
        rpc.notify("move_to_parent", cwd)
    end,

    new_filer = function(cwd)
        rpc.notify("new_filer", cwd)
    end,

    open_file = function(line_idx)
        if line_idx == 1 then return end
        rpc.notify("open_file", line_idx - 1)
    end,

    open_or_expand = function(line_idx)
        if line_idx == 1 then return end
        rpc.notify("open_or_expand", line_idx - 1)
    end,

    refresh = function(cwd)
        rpc.notify("refresh", cwd)
    end,

    rename_entry = function(dir_line_idx, new_path, cwd)
        if dir_line_idx == 0 then return end
        rpc.notify("rename_entry", dir_line_idx, cwd, new_path)
    end,
}
