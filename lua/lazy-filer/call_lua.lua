local ui = require("lazy-filer.states").ui
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
            ui.main.open_float()
        end
    end,
}
