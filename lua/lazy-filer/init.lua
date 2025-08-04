local M = { rpc = {} }

local mkstate = require("glocal-states")
local myui = require("my-ui")

local api = vim.api

local states = { jobid = mkstate.global() }
local ui = myui.declare_ui({})

local plugin_root = ""

local function get_command_path()
    return plugin_root .. "/lazy-filer.rs/target/release/lazy-filer"
end

local function get_or_create_buf()
    ui.main.create_buf()
    return ui.main.get_buf()
end

local function open_file()
    local jobid = states.jobid.get()
    if not jobid then return end

    local cursor = vim.api.nvim_win_get_cursor(0)
    local line_idx = cursor[1]

    vim.rpcnotify(jobid, "open_file", line_idx - 1)
end

local function expand_dir()
    local jobid = states.jobid.get()
    if not jobid then return end

    local buf = get_or_create_buf()

    local cursor = vim.api.nvim_win_get_cursor(0)
    local line_idx = cursor[1]

    vim.rpcnotify(jobid, "expand_dir", buf, line_idx - 1)
end

local function open_or_expand()
    local jobid = states.jobid.get()
    if not jobid then return end

    local buf = get_or_create_buf()

    local cursor = vim.api.nvim_win_get_cursor(0)
    local line_idx = cursor[1]

    vim.rpcnotify(jobid, "open_or_expand", buf, line_idx - 1)
end

local function move_to_parent()
    local jobid = states.jobid.get()
    if not jobid then return end

    local buf = get_or_create_buf()
    vim.rpcnotify(jobid, "move_to_parent", buf, vim.uv.cwd())
end

function M.rpc.focus_on_last_active_win()
    ui.main.close()
    myui.focus_on_last_active_win()
end

function M.rpc.open_filer_win()
    ui.main.open_float()
end

local function new_filer()
    local jobid = states.jobid.get()
    if not jobid then return end

    local buf = get_or_create_buf()
    vim.rpcnotify(jobid, "new_filer", buf, vim.uv.cwd())
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

function M.build_and_spawn_filer(root_dir)
    plugin_root = root_dir
    vim.system({ "cargo", "build", "--release" }, { cwd = plugin_root }, function()
        vim.schedule(spawn_filer)
    end)
end

local function setup_autocmd()
    local augroup = vim.api.nvim_create_augroup('NaughieLazyFiler', { clear = true })
    vim.api.nvim_create_autocmd('VimEnter', {
        group = augroup,
        callback = spawn_filer,
    })
end

M.fn = {
    new_filer = new_filer,
    move_to_parent = move_to_parent,
    open_file = open_file,
    expand_dir = expand_dir,
    open_or_expand = open_or_expand,
}

local function define_keymaps_wrap(args, default_opts)
    local opts = vim.tbl_deep_extend('force', vim.deepcopy(default_opts), args[4] or {})

    local rhs = args[3]
    if type(rhs) == 'string' and M.fn[rhs] then
        vim.keymap.set(args[1], args[2], M.fn[rhs], opts)
    else
        vim.keymap.set(args[1], args[2], rhs, opts)
    end
end

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
    end

    setup_autocmd()
end

return M
