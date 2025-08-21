local mkstate = require("glocal-states")
local myui = require("my-ui")

local states = {
    tmp_create_entry_states = { dir = nil },
    tmp_rename_entry_states = { file = nil, cwd = nil },

    dir_displayed = mkstate.tab(),

    ui = myui.declare_ui({}),
}

return states
