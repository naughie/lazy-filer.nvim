local states = require("lazy-filer.states")
local rpc = require("lazy-filer.namespace").rpc

local ui = states.ui

local function get_or_create_buf()
    ui.main.create_buf()
    return ui.main.get_buf()
end

return {
    create_entry = function(dir_line_idx, fname)
        local buf = get_or_create_buf()
        rpc.notify("create_entry", buf, dir_line_idx, fname)
    end,

    delete_entry = function(dir_line_idx)
        if dir_line_idx == 0 then return end
        local buf = get_or_create_buf()
        rpc.notify("delete_entry", buf, dir_line_idx)
    end,

    expand_dir = function(line_idx)
        if line_idx == 1 then return end
        local buf = get_or_create_buf()
        rpc.notify("expand_dir", buf, line_idx - 1)
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
        local buf = get_or_create_buf()
        rpc.notify("move_to_parent", buf, cwd)
    end,

    new_filer = function(cwd)
        local buf = get_or_create_buf()
        rpc.notify("new_filer", buf, cwd)
    end,

    open_file = function(line_idx)
        if line_idx == 1 then return end
        rpc.notify("open_file", line_idx - 1)
    end,

    open_or_expand = function(line_idx)
        if line_idx == 1 then return end
        local buf = get_or_create_buf()

        rpc.notify("open_or_expand", buf, line_idx - 1)
    end,

    refresh = function(cwd)
        local buf = get_or_create_buf()
        rpc.notify("refresh", buf, cwd)
    end,

    rename_entry = function(dir_line_idx, new_path, cwd)
        if dir_line_idx == 0 then return end
        local buf = get_or_create_buf()
        rpc.notify("rename_entry", buf, dir_line_idx, cwd, new_path)
    end,
}
