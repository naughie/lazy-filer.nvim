local M = {}

local mkstate = require("glocal-states")
local myui = require("my-ui")

local api = vim.api

local states = {
    jobid = mkstate.global(),

    tmp_create_entry_states = { dir = nil },

    dir_displayed = mkstate.tab(),
}
local ui = myui.declare_ui({})

local plugin_root = ""

local companion_keymaps = {
    new_entry = {},
}

local function define_keymaps_wrap(args, default_opts)
    local opts = vim.tbl_deep_extend("force", vim.deepcopy(default_opts), args[4] or {})

    local rhs = args[3]
    if type(rhs) == "string" and M.fn[rhs] then
        vim.keymap.set(args[1], args[2], M.fn[rhs], opts)
    else
        vim.keymap.set(args[1], args[2], rhs, opts)
    end
end

local function get_command_path()
    return plugin_root .. "/lazy-filer.rs/target/release/lazy-filer"
end

local function get_or_create_buf()
    ui.main.create_buf()
    return ui.main.get_buf()
end

local rpc_call = {
    create_entry = function(dir_line_idx, fname)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()

        vim.rpcnotify(jobid, "create_entry", buf, dir_line_idx, fname)
    end,

    expand_dir = function(line_idx)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()

        vim.rpcnotify(jobid, "expand_dir", buf, line_idx - 1)
    end,

    get_dir = function(line_idx)
        local jobid = states.jobid.get()
        if not jobid then return end

        local dir = vim.rpcrequest(jobid, "get_dir", line_idx - 1)
        return { name = dir, idx = line_idx - 1 }
    end,

    move_to_parent = function(cwd)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()
        vim.rpcnotify(jobid, "move_to_parent", buf, cwd)
    end,

    new_filer = function(cwd)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()
        vim.rpcnotify(jobid, "new_filer", buf, cwd)
    end,

    open_file = function(line_idx)
        local jobid = states.jobid.get()
        if not jobid then return end

        vim.rpcnotify(jobid, "open_file", line_idx - 1)
    end,

    open_or_expand = function(line_idx)
        local jobid = states.jobid.get()
        if not jobid then return end

        local buf = get_or_create_buf()

        vim.rpcnotify(jobid, "open_or_expand", buf, line_idx - 1)
    end,
}

local function get_line_idx()
    local win = ui.main.get_win()
    if not win then return end

    local cursor = api.nvim_win_get_cursor(win)
    return cursor[1]
end

local function spawn_filer()
    if states.jobid.get() then return end
    local server_cmd = get_command_path()
    if vim.fn.executable(server_cmd) == 0 then return end

    local id = vim.fn.jobstart({ server_cmd }, { rpc = true })
    if id ~= 0 and id ~= 1 then
        states.jobid.set(id)
    end
end

local function open_new_entry_win()
    if not ui.companion.get_buf() then
        ui.companion.create_buf(function(buf)
            if companion_keymaps.new_entry then
                for _, args in ipairs(companion_keymaps.new_entry) do
                    define_keymaps_wrap(args, { buffer = buf, silent = true })
                end
            end
        end)
    end

    local line_idx = get_line_idx()

    local dir = rpc_call.get_dir(line_idx)
    states.tmp_create_entry_states.dir = dir

    ui.companion.set_lines(0, -1, false, {
        "Create a new entry in " .. dir.name,
        "Enter the new filename:",
        "",
    })

    ui.companion.open_float()

    local win = ui.companion.get_win()
    if not win then return end
    api.nvim_win_set_cursor(win, { 3, 0 })
    vim.cmd("startinsert")
end

local function create_entry()
    vim.cmd("stopinsert")
    if not states.tmp_create_entry_states.dir then return end

    local line_idx = states.tmp_create_entry_states.dir.idx
    states.tmp_create_entry_states.dir = nil

    local lines = ui.companion.lines(2, 3, false)
    local fname = lines[1]

    ui.companion.close()
    ui.main.focus()

    rpc_call.create_entry(line_idx, fname)
end

function M.build_and_spawn_filer(root_dir)
    plugin_root = root_dir
    vim.system({ "cargo", "build", "--release" }, { cwd = plugin_root }, function()
        vim.schedule(spawn_filer)
    end)
end

local function setup_autocmd()
    local augroup = vim.api.nvim_create_augroup("NaughieLazyFiler", { clear = true })
    vim.api.nvim_create_autocmd("VimEnter", {
        group = augroup,
        callback = spawn_filer,
    })
end

M.fn = {
    expand_dir = function()
        local line_idx = get_line_idx()
        rpc_call.expand_dir(line_idx)
    end,

    get_dir = function()
        local line_idx = get_line_idx()
        return rpc_call.get_dir(line_idx)
    end,

    move_to_parent = function()
        local cwd = states.dir_displayed.get()
        if not cwd then return end
        rpc_call.move_to_parent(cwd)

        local parent = vim.fs.dirname(cwd)
        states.dir_displayed.set(parent)
    end,

    new_filer = function()
        local cwd = vim.uv.cwd()
        states.dir_displayed.set(cwd)
        rpc_call.new_filer(cwd)
    end,

    open_file = function()
        local line_idx = get_line_idx()
        rpc_call.open_file(line_idx)
    end,

    open_or_expand = function()
        local line_idx = get_line_idx()
        rpc_call.open_or_expand(line_idx)
    end,

    create_entry = create_entry,
    open_new_entry_win = open_new_entry_win,
    spawn_filer = spawn_filer,

    move_to_filer = function()
        ui.main.focus()
    end,
    move_to_subwin = function()
        ui.companion.focus()
    end,

    close_filer = function()
        ui.main.close()
    end,
    close_subwin = function()
        ui.companion.close()
    end,
}

M.rpc = {
    focus_on_last_active_win = function()
        myui.close_all()
        myui.focus_on_last_active_win()
    end,

    open_filer_win = function()
        if ui.main.get_win() then
            ui.main.focus()
        else
            ui.main.open_float()
        end
    end,
}

function M.setup(opts)
    plugin_root = opts.root_dir

    if opts.keymaps then
        if opts.keymaps.global then
            for _, args in ipairs(opts.keymaps.global) do
                define_keymaps_wrap(args, { silent = true })
            end
        end

        if opts.keymaps.filer then
            ui.opts.main.setup_buf = function(buf)
                for _, args in ipairs(opts.keymaps.filer) do
                    define_keymaps_wrap(args, { buffer = buf, silent = true })
                end
            end
        end

        if opts.keymaps.new_entry then
            companion_keymaps.new_entry = opts.keymaps.new_entry
        end
    end

    setup_autocmd()
end

return M
