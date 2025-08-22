local ui = require("lazy-filer.states").ui
local hl = require("lazy-filer.highlight")
local myui = require("my-ui")

return {
    focus_on_last_active_win = function()
        myui.close_all()
        myui.focus_on_last_active_win()
    end,

    open_filer_win = function()
        if ui.main.get_win() then
            ui.main.focus()
        else
            ui.main.open_float(function(win)
                vim.api.nvim_set_option_value("cursorline", true, { win = win })
            end)
        end
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
