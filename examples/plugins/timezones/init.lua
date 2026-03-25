-- Timezones — show current time across major zones.

local zones = {
    { tz = "UTC",               label = "UTC" },
    { tz = "US/Eastern",        label = "US Eastern" },
    { tz = "US/Central",        label = "US Central" },
    { tz = "US/Pacific",        label = "US Pacific" },
    { tz = "Europe/London",     label = "London" },
    { tz = "Europe/Berlin",     label = "Berlin" },
    { tz = "Asia/Tokyo",        label = "Tokyo" },
    { tz = "Australia/Sydney",  label = "Sydney" },
    { tz = "Asia/Kolkata",      label = "India (IST)" },
}

lark.register({
    on_run = function()
        local items = {}
        for _, z in ipairs(zones) do
            -- Use env-prefixed date for the target timezone.
            local tz_out = lark.exec("env", { "TZ=" .. z.tz, "date", "+%H:%M  %Z  %Y-%m-%d" })
            local time_str = tz_out and tz_out:gsub("%s+$", "") or "?"

            items[#items + 1] = {
                label = time_str,
                detail = z.label,
                icon = "🕐",
                copy_text = z.label .. ": " .. time_str,
                actions = {
                    { label = "Copy time", kind = "clipboard", args = { time_str } },
                },
            }
        end

        return { title = "Timezones", items = items }
    end,
})
