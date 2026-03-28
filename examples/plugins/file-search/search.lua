-- File Search — find files by name using fd (falls back to find).

lark.register({
    on_run = function()
        if lark.form_values then
            local query = lark.form_values.query or ""
            if query == "" then
                return {
                    title = "File Search",
                    items = { { label = "No query entered", icon = "!" } },
                }
            end

            -- Try fd first (faster, respects .gitignore), fall back to find.
            -- Use LARK_CWD (set by lark.nvim) if available, otherwise $HOME.
            local search_root = lark.env("LARK_CWD") or os.getenv("HOME") or "/"
            local raw = lark.exec("fd", { "--max-results", "50", "--color", "never", query, search_root })
            if not raw or raw == "" then
                raw = lark.exec("find", { search_root, "-maxdepth", "5", "-iname", "*" .. query .. "*", "-not", "-path", "*/.*" })
            end

            if not raw or raw == "" then
                return {
                    title = "File Search",
                    items = { { label = "No files found for: " .. query, icon = "📭" } },
                }
            end

            local items = {}
            for path in raw:gmatch("[^\n]+") do
                local name = path:match("([^/]+)$") or path
                local dir = path:match("^(.*)/[^/]+$") or ""
                local home = os.getenv("HOME") or ""
                if home ~= "" and dir:sub(1, #home) == home then
                    dir = "~" .. dir:sub(#home + 1)
                end
                items[#items + 1] = {
                    label = name,
                    detail = dir,
                    icon = "📄",
                    copy_text = path,
                    actions = {
                        { label = "Open in Finder", kind = "shell", args = { "open", "-R", path } },
                        { label = "Open in $EDITOR", kind = "shell", args = { os.getenv("EDITOR") or "vim", path } },
                        { label = "Copy path", kind = "clipboard", args = { path } },
                        { label = "Copy filename", kind = "clipboard", args = { name } },
                    },
                }
                if #items >= 50 then break end
            end

            return { title = "File Search — " .. #items .. " results", items = items }
        end

        return {
            title = "File Search",
            form = {
                fields = {
                    {
                        id = "query",
                        label = "Search",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "e.g. config.toml, *.lua, README",
                    },
                },
                submit_label = "Search",
            },
        }
    end,
})
