# Lazy-filer

Lazy-filer is a filer plugin built on the top of Rust ecosystem. It is lazy and asynchronous, and light-weight.


# Requirements

- Rust (>= 1.88.0)
- OS that supports the [`nix` crate](https://crates.io/crates/nix)

# Install

After `nvim-router.nvim` detects that all of dependencies, which are specified in `opts.ns` of `nvim-router` itself, are `setup`'d, then it automatically runs `cargo build --release` and spawns a plugin-client process.
The first build may take a long time.

Once spawning you can open a lazy-filer window and manipulate with the filesystem.

## Lazy.nvim

### Config

```lua
{
    -- Dependencies
    { "naughie/glocal-states.nvim", lazy = true },
    { "naughie/my-ui.nvim", lazy = true },

    {
        "naughie/nvim-router.nvim",
        lazy = false,
        opts = function(plugin)
            return {
                plugin_dir = plugin.dir,
                ns = { "lazy-filer" },
            }
        end,
    },

    {
        'naughie/lazy-filer.nvim',
        lazy = false,
        opts = function(plugin)
            return {
                plugin_dir = plugin.dir,
                border = {
                    -- Highlight group for the border of floating windows.
                    -- Defaults to FloatBorder
                    hl_group = "FloatBorder",
                },
                rpc_ns = "lazy-filer",

                -- { {mode}, {lhs}, {rhs}, {opts} } (see :h vim.keymap.set())
                -- We accept keys of require('lazy-filer').fn as {rhs}
                keymaps = {
                    global = {
                        -- Open a filer window.
                        -- You must call spawn_filer() before this.
                        { 'n', '<C-e>', 'new_filer' },

                        -- Focus on the filer window if it is already opened.
                        { 'n', '<C-f>', 'move_to_filer' },
                    },

                    -- Keymaps on a filer window
                    filer = {
                        -- If the file under the cursor line is a regular file, then open it.
                        -- If the file under the cursor line is a directory, then expand it.
                        { 'n', 'o', 'open_or_expand' },
                        { 'n', '<CR>', 'open_or_expand' },

                        -- Display the parent directory. It does not change the working directory.
                        { 'n', 'u', 'move_to_parent' },

                        -- Open a subwindow to create a new file into the directory under the cursor line.
                        -- See new_entry keymaps below.
                        { 'n', '<C-n>', 'open_new_entry_win' },

                        -- Delete a file under the cursor line.
                        -- You will be confirmed to delete the file.
                        -- Type y to delete, type n or <ESC> to cancel.
                        { 'n', 'd', 'open_delete_entry_win' },

                        -- Execute readdir(2) and refresh the filer window.
                        { 'n', 'r', 'refresh' },

                        -- Open a subwindow to rename a file under the cursor line.
                        -- See rename_entry keymaps below.
                        { 'n', 'm', 'open_rename_entry_win' },

                        -- Close the filer window.
                        { 'n', 'q', 'close_filer' },

                        -- If there is a subwindow (for new_entry or rename_entry), then focus on it.
                        { { 'n', 'i' }, '<C-j>', 'move_to_subwin' },
                    },

                    -- When open_new_entry_win(), it opens the subwindow to enter the filename.
                    -- Keymaps on this subwindow, independent of other subwindows.
                    --
                    -- If you are creating a regular file, type a filename.
                    -- If you are creating a directory, type a filename with a trailing slash.
                    new_entry = {
                        -- Create a new file.
                        { { 'n', 'i' }, '<CR>', 'create_entry' },

                        -- Cancel, get back to the filer window.
                        { 'n', 'q', 'close_subwin' },

                        -- Get back to the filer window, without closing the subwindow.
                        { { 'n', 'i' }, '<C-k>', 'move_to_filer' },
                    },

                    -- When open_rename_entry_win(), it opens the subwindow to enter the filename.
                    -- Keymaps on this subwindow, independent of other subwindows.
                    --
                    -- Type the new path, relative to the original file (not relative to the working directory).
                    rename_entry = {
                        -- Rename a file.
                        { { 'n', 'i' }, '<CR>', 'rename_entry' },

                        -- Cancel, get back to the filer window.
                        { 'n', 'q', 'close_subwin' },

                        -- Get back to the filer window, without closing the subwindow.
                        { { 'n', 'i' }, '<C-k>', 'move_to_filer' },
                    },
                },
            }
        end,
    },
}
```

