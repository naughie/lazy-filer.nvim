local M = {}

local states = require("lazy-filer.states")
local ui = states.ui

local rpc_call = require("lazy-filer.call_lib")

local subwin = require("lazy-filer.subwin")

local myui = require("my-ui")

local api = vim.api

local plugin_root = ""

local augroup = api.nvim_create_augroup("NaughieLazyFiler", { clear = true })

local function get_command_path()
    return plugin_root .. "/lazy-filer.rs/target/release/lazy-filer"
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
    api.nvim_create_autocmd("VimEnter", {
        group = augroup,
        callback = spawn_filer,
    })
end

local function get_line_idx()
    local win = ui.main.get_win()
    if not win then return end

    local cursor = api.nvim_win_get_cursor(win)
    return cursor[1]
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

    refresh = function()
        local cwd = states.dir_displayed.get()
        if cwd then
            rpc_call.refresh(cwd)
        else
            cwd = vim.uv.cwd()
            states.dir_displayed.set(cwd)
            rpc_call.refresh(cwd)
        end
    end,

    create_entry = subwin.create_entry.exec,
    open_new_entry_win = subwin.create_entry.open_win,
    open_delete_entry_win = subwin.delete_entry.open_win,
    open_rename_entry_win = subwin.rename_entry.open_win,
    rename_entry = subwin.rename_entry.exec,
    spawn_filer = spawn_filer,

    move_to_filer = function()
        ui.main.focus()
    end,
    move_to_subwin = function()
        ui.companion.focus()
    end,

    close_filer = function()
        ui.main.close()
        myui.focus_on_last_active_ui()
    end,
    close_subwin = function()
        ui.main.focus()
        ui.companion.close()
    end,
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

        subwin.set_keymaps({
            new_entry = function(buf)
                if opts.keymaps.new_entry then
                    for _, args in ipairs(opts.keymaps.new_entry) do
                        define_keymaps_wrap(args, { buffer = buf, silent = true })
                    end
                end
            end,

            rename_entry = function(buf)
                if opts.keymaps.rename_entry then
                    for _, args in ipairs(opts.keymaps.rename_entry) do
                        define_keymaps_wrap(args, { buffer = buf, silent = true })
                    end
                end
            end,
        })
    end

    if opts.border then
        ui.update_opts({ background = opts.border })
    end

    setup_autocmd()
end

return M
