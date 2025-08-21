local M = {}

local api = vim.api

local ns = api.nvim_create_namespace("NaughieLazyFilerCallLua")

local default_hl = {
    directory = { link = "Directory" },
    metadata = { link = "Comment" },
    regular = { link = "Normal" },
    other_file = { link = "Comment" },
    link_to = { link = "Comment" },
}

local hl_names = {
    directory = "LazyFilerDirectory",
    metadata = "LazyFilerMetadata",
    regular = "LazyFilerRegular",
    other_file = "LazyFilerOther",
    link_to = "LazyFilerLinkTo",
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
    M.set_extmark[key] = function(buf, range)
        api.nvim_buf_set_extmark(buf, ns, range.line, range.start_col, {
            end_row = range.line,
            end_col = range.end_col,
            hl_group = hl,
        })
    end
end
M.set_extmark.link_to = function(buf, opts)
    api.nvim_buf_set_extmark(buf, ns, opts.line, 0, {
        virt_text = { { opts.target, hl_names.link_to } },
        virt_text_pos = "eol",
    })
end

return M
