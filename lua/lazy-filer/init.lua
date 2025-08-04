local M = { rpc = {} }

local mkstate = require("glocal-states")
local myui = require("my-ui")

local api = vim.api
local uv = vim.uv

local states = { jobid = mkstate.global() }
local ui = myui.declare_ui({
    main = {
        setup_buf = function(buf)
            local opts = { buffer = buf, silent = true }
            vim.keymap.set('n', 'o', M.fn.open_or_expand, opts)
            vim.keymap.set('n', '<CR>', M.fn.open_or_expand, opts)
            vim.keymap.set('n', 'q', ':q<CR>', opts)
        end,
    },
})

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

local function build_and_spawn_filer()
    vim.system({ "cargo", "build", "--release" }, {}, function()
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

function M.setup(opts)
    api.nvim_create_user_command('LazyFilerBuild', build_and_spawn_filer, { nargs = 0 })
    vim.keymap.set('n', '<C-e>', new_filer, { silent = true })

    plugin_root = opts.root_dir

    setup_autocmd()
end

M.fn = {
    new_filer = new_filer,
    open_file = open_file,
    expand_dir = expand_dir,
    open_or_expand = open_or_expand,
}

return M
