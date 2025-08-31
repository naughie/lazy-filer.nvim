local M = {}

local api = vim.api

local ns = api.nvim_create_namespace("NaughieLazyFilerCallLua")

local default_hl = {
    directory = { link = "Directory" },
    metadata = { link = "Comment" },
    regular = { link = "Normal" },
    exec = { link = "Special" },
    no_read = { link = "Error" },
    no_exec_dir = { link = "Error" },
    other_file = { link = "Comment" },
    link_to = { link = "Comment" },
    indent = { link = "Comment" },
}

local hl_names = {
    directory = "LazyFilerDirectory",
    metadata = "LazyFilerMetadata",
    regular = "LazyFilerRegular",
    exec = "LazyFilerExec",
    no_read = "LazyFilerNoRead",
    no_exec_dir = "LazyFilerNoExecDir",
    other_file = "LazyFilerOther",
    link_to = "LazyFilerLinkTo",
    indent = "LazyFilerIndent",
}

function M.set_highlight_groups(opts)
    for key, hl in pairs(hl_names) do
        if opts and opts[key] then
            api.nvim_set_hl(0, hl, opts[key])
        else
            api.nvim_set_hl(0, hl, default_hl[key])
        end
    end
end

M.set_extmark = {}

for key, hl in pairs(hl_names) do
    M.set_extmark[key] = function(buf, opts)
        if opts.virt_text then
            api.nvim_buf_set_extmark(buf, ns, opts.line, opts.col or 0, {
                virt_text = { { opts.virt_text, hl } },
                virt_text_pos = opts.pos,
                hl_mode = "combine",
                invalidate = true,
            })
        else
            api.nvim_buf_set_extmark(buf, ns, opts.line, opts.start_col, {
                end_row = opts.line,
                end_col = opts.end_col,
                hl_group = hl,
            })
        end
    end
end

M.set_extmark.empty_line = function(buf, opts)
    api.nvim_buf_set_extmark(buf, ns, opts.line, 0, {
        virt_lines = { { { "", "Comment" } } },
        invalidate = true,
    })
end

api.nvim_set_hl(0, "LazyFilerNoCursor", { reverse = true, blend = 100 })

return M
