local M = {}

local states = require("lazy-filer.states")
local ui = states.ui

local rpc_call = require("lazy-filer.call_lib")

local api = vim.api

local augroup = api.nvim_create_augroup("NaughieLazyFilerSubwin", { clear = true })

local companion_keymaps = {
    new_entry = nil,
    rename_entry = nil,
}

function M.set_keymaps(keymaps)
    companion_keymaps.new_entry = keymaps.new_entry
    companion_keymaps.rename_entry = keymaps.rename_entry
end

local function get_line_idx()
    local win = ui.main.get_win()
    if not win then return end

    local cursor = api.nvim_win_get_cursor(win)
    return cursor[1]
end

M.create_entry = {
    open_win = function()
        if not ui.companion.get_buf() then
            ui.companion.create_buf(function(buf)
                if companion_keymaps.new_entry then
                    companion_keymaps.new_entry(buf)
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

        ui.companion.open_float(function(win)
            api.nvim_create_autocmd("WinClosed", {
                group = augroup,
                pattern = tostring(win),
                callback = function()
                    ui.companion.delete_buf()
                    states.tmp_create_entry_states.dir = nil
                end,
            })
        end)

        local win = ui.companion.get_win()
        if not win then return end
        api.nvim_win_set_cursor(win, { 3, 0 })
        vim.cmd("startinsert")
    end,

    exec = function()
        vim.cmd("stopinsert")
        if not states.tmp_create_entry_states.dir then return end

        local line_idx = states.tmp_create_entry_states.dir.idx

        local lines = ui.companion.lines(2, 3, false)
        local fname = lines[1]

        ui.companion.close()
        ui.main.focus()

        rpc_call.create_entry(line_idx, fname)
    end,
}

M.delete_entry = {
    open_win = function()
        local line_idx = get_line_idx()

        local file = rpc_call.get_file_path(line_idx)

        local buf = api.nvim_create_buf(false, true)

        local prompt = {
            "",
            "",
            "    Delete an entry: " .. file.name,
            "       Are you sure? [y/N]",
            "",
            "",
        }

        local width = 0
        for _, line in ipairs(prompt) do
            width = math.max(width, vim.fn.strwidth(line))
        end
        width = width + 4
        local height = #prompt

        local top = math.floor((vim.o.lines - height) / 2)
        local left = math.floor((vim.o.columns - width) / 2)

        api.nvim_buf_set_lines(buf, 0, -1, false, prompt)

        local win = vim.api.nvim_open_win(buf, true, {
            relative = "editor",
            style = "minimal",
            width = width,
            height = height,
            row = top,
            col = left,
            border = "rounded",
        })

        local close = function()
            api.nvim_win_close(win, true)
            api.nvim_buf_delete(buf, { force = true })
        end
        local confirm = function()
            close()
            rpc_call.delete_entry(file.idx)
        end

        vim.keymap.set('n', 'y', confirm, { buffer = buf, silent = true })
        vim.keymap.set('n', 'n', close, { buffer = buf, silent = true })
        vim.keymap.set('n', '<ESC>', close, { buffer = buf, silent = true })
        vim.keymap.set('n', 'q', close, { buffer = buf, silent = true })
    end,
}

M.rename_entry = {
    open_win = function()
        if not ui.companion.get_buf() then
            ui.companion.create_buf(function(buf)
                if companion_keymaps.rename_entry then
                    companion_keymaps.rename_entry(buf)
                end
            end)
        end

        local line_idx = get_line_idx()

        local file = rpc_call.get_file_path(line_idx)
        states.tmp_rename_entry_states.file = file

        ui.companion.set_lines(0, -1, false, {
            "Rename an entry: " .. file.name,
            "",
        })

        ui.companion.open_float(function(win)
            local cwd = vim.uv.cwd()
            states.tmp_rename_entry_states.cwd = cwd
            local parent = vim.fs.dirname(states.tmp_rename_entry_states.file.name)
            vim.uv.chdir(parent)

            api.nvim_create_autocmd("WinClosed", {
                group = augroup,
                pattern = tostring(win),
                callback = function()
                    ui.companion.delete_buf()
                    vim.uv.chdir(states.tmp_rename_entry_states.cwd)
                    states.tmp_rename_entry_states = { file = nil, cwd = nil }
                end,
            })
        end)

        local win = ui.companion.get_win()
        if not win then return end
        api.nvim_win_set_cursor(win, { 2, 0 })
        vim.cmd("startinsert")
    end,

    exec = function()
        vim.cmd("stopinsert")
        if not states.tmp_rename_entry_states.file then return end
        local cwd = states.dir_displayed.get()
        if not cwd then return end

        local line_idx = states.tmp_rename_entry_states.file.idx

        local lines = ui.companion.lines(1, 2, false)
        local path = lines[1]

        ui.companion.close()
        ui.main.focus()

        rpc_call.rename_entry(line_idx, path, cwd)
    end,
}

return M
