-- Weather: Forecast — 3-day forecast with hourly breakdowns.

local function weather_icon(code)
    local c = tonumber(code) or 0
    if c == 113 then return "☀" end
    if c == 116 then return "⛅" end
    if c == 119 or c == 122 then return "☁" end
    if c >= 176 and c <= 185 then return "🌧" end
    if c >= 200 and c <= 232 then return "⛈" end
    if c >= 263 and c <= 302 then return "🌦" end
    if c >= 308 and c <= 359 then return "🌧" end
    if c >= 368 and c <= 395 then return "🌨" end
    return "🌤"
end

local function hour_label(time_str)
    -- wttr.in hourly time is "0", "300", "600", etc. (minutes from midnight * 100).
    local mins = tonumber(time_str) or 0
    local h = math.floor(mins / 100)
    if h == 0 then return "12am"
    elseif h < 12 then return h .. "am"
    elseif h == 12 then return "12pm"
    else return (h - 12) .. "pm"
    end
end

local function get_location()
    local saved = lark.store.get("weather_location")
    if saved and saved ~= "" then return saved end
    return nil
end

lark.register({
    on_run = function()
        local location = get_location()
        local url = "https://wttr.in/"
        if location then
            url = url .. location:gsub(" ", "+")
        end
        url = url .. "?format=j1"

        local resp = lark.http.get(url, { timeout = 10 })
        if resp.status ~= 200 then
            return {
                title = "Forecast",
                items = { { label = "Failed to fetch forecast", detail = "HTTP " .. resp.status, icon = "!" } },
            }
        end

        local ok, data = pcall(lark.json.decode, resp.body)
        if not ok or not data.weather then
            return {
                title = "Forecast",
                items = { { label = "Failed to parse response", icon = "!" } },
            }
        end

        local area = "Unknown"
        if data.nearest_area and data.nearest_area[1] then
            local na = data.nearest_area[1]
            local city = na.areaName and na.areaName[1] and na.areaName[1].value or ""
            if city ~= "" then area = city end
        end

        local items = {}
        for _, day in ipairs(data.weather) do
            local date = day.date or "?"
            local max_f = day.maxtempF or "?"
            local min_f = day.mintempF or "?"
            local max_c = day.maxtempC or "?"
            local min_c = day.mintempC or "?"

            -- Day header.
            items[#items + 1] = {
                label = date .. "  ↑" .. max_f .. "°F/" .. max_c .. "°C  ↓" .. min_f .. "°F/" .. min_c .. "°C",
                detail = "Daily summary",
                icon = "📅",
            }

            -- Hourly breakdowns (wttr.in gives 8 entries per day at 3h intervals).
            if day.hourly then
                for _, h in ipairs(day.hourly) do
                    local time = hour_label(h.time or "0")
                    local temp = h.tempF or "?" 
                    local temp_c = h.tempC or "?"
                    local desc = "?"
                    if h.weatherDesc and h.weatherDesc[1] then
                        desc = h.weatherDesc[1].value
                    end
                    local code = h.weatherCode or "0"
                    local precip = h.precipMM or "0"
                    local wind = h.windspeedMiles or "?"

                    local detail = desc .. "  💨" .. wind .. "mph"
                    if tonumber(precip) and tonumber(precip) > 0 then
                        detail = detail .. "  🌧" .. precip .. "mm"
                    end

                    items[#items + 1] = {
                        label = time .. "  " .. temp .. "°F/" .. temp_c .. "°C",
                        detail = detail,
                        icon = weather_icon(code),
                    }
                end
            end
        end

        return { title = "Forecast — " .. area, items = items }
    end,
})
