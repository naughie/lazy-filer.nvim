local states = require("lazy-filer.states")
local ns = require("lazy-filer.namespace")

local ui = states.ui

local rpcnotify = ns.rpc.notify
local rpcrequest = ns.rpc.request

local function get_or_create_buf()
    ui.main.create_buf()
    return ui.main.get_buf()
end

return {
    create_entry = function(dir_line_idx, fname)
        local buf = get_or_create_buf()
        rpcnotify("create_entry", buf, dir_line_idx, fname)
    end,

    delete_entry = function(dir_line_idx)
        local buf = get_or_create_buf()
        rpcnotify("delete_entry", buf, dir_line_idx)
    end,

    expand_dir = function(line_idx)
        local buf = get_or_create_buf()
        rpcnotify("expand_dir", buf, line_idx - 1)
    end,

    get_dir = function(line_idx)
        local dir = rpcrequest("get_dir", line_idx - 1)
        return { name = dir, idx = line_idx - 1 }
    end,

    get_file_path = function(line_idx)
        local file = rpcrequest("get_file_path", line_idx - 1)
        return { name = file, idx = line_idx - 1 }
    end,

    move_to_parent = function(cwd)
        local buf = get_or_create_buf()
        rpcnotify("move_to_parent", buf, cwd)
    end,

    new_filer = function(cwd)
        local buf = get_or_create_buf()
        rpcnotify("new_filer", buf, cwd)
    end,

    open_file = function(line_idx)
        rpcnotify("open_file", line_idx - 1)
    end,

    open_or_expand = function(line_idx)
        local buf = get_or_create_buf()

        rpcnotify("open_or_expand", buf, line_idx - 1)
    end,

    refresh = function(cwd)
        local buf = get_or_create_buf()
        rpcnotify("refresh", buf, cwd)
    end,

    rename_entry = function(dir_line_idx, new_path, cwd)
        local buf = get_or_create_buf()
        rpcnotify("rename_entry", buf, dir_line_idx, cwd, new_path)
    end,
}
