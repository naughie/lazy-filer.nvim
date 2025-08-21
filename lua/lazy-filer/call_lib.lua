local states = require("lazy-filer.states")
local ui = states.ui

local rpcnotify = vim.rpcnotify
local rpcrequest = vim.rpcrequest

local function get_or_create_buf()
    ui.main.create_buf()
    return ui.main.get_buf()
end

return {
    create_entry = function(dir_line_idx, fname)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()

        rpcnotify(jobid, "create_entry", buf, dir_line_idx, fname)
    end,

    delete_entry = function(dir_line_idx)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()

        rpcnotify(jobid, "delete_entry", buf, dir_line_idx)
    end,

    expand_dir = function(line_idx)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()

        rpcnotify(jobid, "expand_dir", buf, line_idx - 1)
    end,

    get_dir = function(line_idx)
        local jobid = states.jobid.get()
        if not jobid then return end

        local dir = rpcrequest(jobid, "get_dir", line_idx - 1)
        return { name = dir, idx = line_idx - 1 }
    end,

    get_file_path = function(line_idx)
        local jobid = states.jobid.get()
        if not jobid then return end

        local file = rpcrequest(jobid, "get_file_path", line_idx - 1)
        return { name = file, idx = line_idx - 1 }
    end,

    move_to_parent = function(cwd)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()
        rpcnotify(jobid, "move_to_parent", buf, cwd)
    end,

    new_filer = function(cwd)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()
        rpcnotify(jobid, "new_filer", buf, cwd)
    end,

    open_file = function(line_idx)
        local jobid = states.jobid.get()
        if not jobid then return end

        rpcnotify(jobid, "open_file", line_idx - 1)
    end,

    open_or_expand = function(line_idx)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()

        rpcnotify(jobid, "open_or_expand", buf, line_idx - 1)
    end,

    refresh = function(cwd)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()

        rpcnotify(jobid, "refresh", buf, cwd)
    end,

    rename_entry = function(dir_line_idx, new_path, cwd)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()

        rpcnotify(jobid, "rename_entry", buf, dir_line_idx, cwd, new_path)
    end,
}
