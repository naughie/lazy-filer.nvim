local ui = require("lazy-filer.states").ui
local hl = require("lazy-filer.highlight")
local myui = require("my-ui")

local api = vim.api

local tmp_guicursor = nil
local nocursor = "n:LazyFilerNoCursor"

local guicursor_info = vim.api.nvim_get_option_info("guicursor")
local default_guicursor = guicursor_info and guicursor_info.default

local function unset_guicursor()
    local current_guicursor = vim.o.guicursor
    if current_guicursor ~= nocursor then tmp_guicursor = current_guicursor end

    vim.o.guicursor = nocursor
end

local function restore_guicursor()
    if not tmp_guicursor or tmp_guicursor == "" then
        vim.o.guicursor = default_guicursor
    else
        vim.o.guicursor = tmp_guicursor
    end
    tmp_guicursor = nil
end

return {
    focus_on_last_active_win = function()
        myui.close_all()
        myui.focus_on_last_active_win()
    end,

    open_filer_win = function()
        if ui.main.get_win() then
            ui.main.focus()
        else
            ui.main.open_float(function(win, buf)
                api.nvim_set_option_value("cursorline", true, { win = win })

                unset_guicursor()

                local augroup = api.nvim_create_augroup("NaughieLazyFileUnsetrCursor", { clear = true })
                local tab = api.nvim_get_current_tabpage()

                api.nvim_create_autocmd("WinEnter", {
                    group = augroup,
                    buffer = buf,
                    callback = function()
                        vim.schedule(function()
                            -- nvim_open_win may trigger WinEnter when opening another window
                            local current_buf = api.nvim_get_current_buf()
                            if current_buf ~= buf then return end

                            unset_guicursor()
                        end)
                    end,
                })

                api.nvim_create_autocmd("WinLeave", {
                    group = augroup,
                    buffer = buf,
                    callback = restore_guicursor,
                })
            end)
        end
    end,

    set_filer_lines = function(start_line, end_line, lines)
        ui.main.create_buf()
        ui.main.set_lines(start_line, end_line, false, lines)
    end,

    set_highlight = function(ranges)
        local buf = ui.main.get_buf()
        if not buf then return end

        for _, range in ipairs(ranges) do
            local fn = hl.set_extmark[range.hl]
            if fn then fn(buf, range) end
        end
    end,
}
