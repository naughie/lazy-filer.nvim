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
        local line_idx = get_line_idx()

        local dir = rpc_call.get_dir(line_idx)
        if not dir.name then return end
        states.tmp_create_entry_states.dir = dir

        if not ui.companion.get_buf() then
            ui.companion.create_buf(function(buf)
                if companion_keymaps.new_entry then
                    companion_keymaps.new_entry(buf)
                end
            end)
        end

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

        ui.main.focus()
        ui.companion.close()

        rpc_call.create_entry(line_idx, fname)
    end,
}

M.delete_entry = {
    open_win = function()
        local line_idx = get_line_idx()
        if line_idx == 1 then return end

        local file = rpc_call.get_file_path(line_idx)
        if not file.name then return end

        if not ui.companion.get_buf() then
            local close = function()
                ui.main.focus()
                ui.companion.close()
            end
            local confirm = function()
                close()
                rpc_call.delete_entry(file.idx)
            end

            ui.companion.create_buf(function(buf)
                vim.keymap.set('n', 'y', confirm, { buffer = buf, silent = true })
                vim.keymap.set('n', 'n', close, { buffer = buf, silent = true })
                vim.keymap.set('n', '<ESC>', close, { buffer = buf, silent = true })
                vim.keymap.set('n', 'q', close, { buffer = buf, silent = true })
            end)
        end

        local prompt = {
            "",
            "",
            "    Delete an entry: " .. file.name,
            "       Are you sure? [y/N]",
            "",
            "",
        }
        ui.companion.set_lines(0, -1, false, prompt)

        local width = 0
        for _, line in ipairs(prompt) do
            width = math.max(width, vim.fn.strwidth(line))
        end
        width = width + 4
        local height = #prompt

        local geom = {
            width = width,
            height = height,
            col = math.floor((vim.o.columns - width) / 2),
            row = math.floor((vim.o.lines - height) / 2),
        }

        ui.companion.open_float(function(win)
            api.nvim_create_autocmd("WinClosed", {
                group = augroup,
                pattern = tostring(win),
                callback = function()
                    ui.companion.delete_buf()
                end,
            })
        end, geom)
    end,
}

M.rename_entry = {
    open_win = function()
        local line_idx = get_line_idx()
        if line_idx == 1 then return end

        local file = rpc_call.get_file_path(line_idx)
        if not file.name then return end
        states.tmp_rename_entry_states.file = file

        if not ui.companion.get_buf() then
            ui.companion.create_buf(function(buf)
                if companion_keymaps.rename_entry then
                    companion_keymaps.rename_entry(buf)
                end
            end)
        end

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

        ui.main.focus()
        ui.companion.close()

        rpc_call.rename_entry(line_idx, path, cwd)
    end,
}

return M
