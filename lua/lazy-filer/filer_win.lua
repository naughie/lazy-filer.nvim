local M = {}

local ui = require("lazy-filer.states").ui
local hl = require("lazy-filer.highlight")

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

function M.open_win()
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
end

local metadata_text_helper = {
    ft = function(item)
        if item.is_regular then return "f" end
        if item.is_dir then return "d" end
        return "-"
    end,
    r = function(item)
        if item.read then return "r" end
        return "-"
    end,
    w = function(item)
        if item.write then return "w" end
        return "-"
    end,
    x = function(item)
        if item.exec then return "x" end
        return "-"
    end,
}
local metadata_text = function(item)
    local ft = metadata_text_helper.ft(item)
    local r = metadata_text_helper.r(item)
    local w = metadata_text_helper.w(item)
    local x = metadata_text_helper.x(item)
    return string.format("[%s%s%s%s]", ft, r, w, x)
end

local function build_buf_lines(items)
    local lines = {}
    local highlights = {}

    for i, item in ipairs(items) do
        local indent = ""
        if item.level > 0 then
            indent = "    " .. string.rep("\u{eb10}   ", item.level - 1)
        end

        local file_icon = "\u{f29c}"
        if item.is_regular then
            file_icon = "\u{f4a5}"
        elseif item.is_dir then
            file_icon = "\u{f413}"
        end
        local fname = file_icon .. " " .. item.fname

        local line = indent .. fname

        if item.is_link then
            line = line .. "@"
        end
        if item.is_regular and item.exec then
            line = line .. "*"
        end
        if item.is_dir then
            line = line .. "/"
        end

        table.insert(lines, line)

        local insert_hl = function(hl_group, opts)
            opts.line = i
            opts.hl = hl_group
            table.insert(highlights, opts)
        end

        if item.level == 0 then
            insert_hl("empty_line", {})
        end

        local indent_len = string.len(indent)
        insert_hl("indent", {
            start_col = 0,
            end_col = indent_len,
        })

        local fname_len = string.len(fname)
        local fname_hl = "other_file"
        if item.is_regular then
            if not item.read then
                fname_hl = "no_read"
            elseif item.exec then
                fname_hl = "exec"
            else
                fname_hl = "regular"
            end
        elseif item.is_dir then
            if not item.read then
                fname_hl = "no_read"
            elseif item.exec then
                fname_hl = "directory"
            else
                fname_hl = "no_exec_dir"
            end
        end
        insert_hl(fname_hl, {
            start_col = indent_len,
            end_col = indent_len + fname_len,
        })

        local metadata = metadata_text(item)
        insert_hl("metadata", {
            text = metadata,
        })

        if item.is_link and item.link_to and item.link_to ~= vim.NIL then
            local link_text = " \u{f44c} " .. item.link_to
            insert_hl("link_to", {
                text = link_text,
            })
        end
    end

    return lines, highlights
end

function M.update_buf(start_line, end_line, items)
    ui.main.create_buf()
    local buf = ui.main.get_buf()
    if not buf then return end

    local lines, highlights = build_buf_lines(items)

    api.nvim_set_option_value("modifiable", true, { buf = buf })
    ui.main.set_lines(start_line, end_line, false, lines)
    api.nvim_set_option_value("modifiable", false, { buf = buf })

    for _, opts in ipairs(highlights) do
        opts.line = opts.line + start_line - 1
        local fn = hl.set_extmark[opts.hl]
        if fn then fn(buf, opts) end
    end
end

return M
