local filer_api = require("lazy-filer.filer_win")
local myui = require("my-ui")

local api = vim.api

return {
    focus_on_last_active_win = function()
        myui.close_all()
        myui.focus_on_last_active_win()
    end,

    open_filer_win = function()
        filer_api.open_win()
    end,

    update_filer_buf = function(start_line, end_line, items)
        filer_api.update_buf(start_line, end_line, items)
    end,
}
