-- Weather: Current — current conditions from wttr.in with location support.

local function weather_icon(code)
    local c = tonumber(code) or 0
    if c == 113 then return "☀" end   -- sunny
    if c == 116 then return "⛅" end  -- partly cloudy
    if c == 119 or c == 122 then return "☁" end  -- cloudy/overcast
    if c >= 176 and c <= 185 then return "🌧" end -- light rain/drizzle
    if c >= 200 and c <= 232 then return "⛈" end  -- thunder
    if c >= 263 and c <= 302 then return "🌦" end -- drizzle
    if c >= 308 and c <= 359 then return "🌧" end -- rain
    if c >= 368 and c <= 395 then return "🌨" end -- snow
    return "🌤"
end

local function get_location()
    local saved = lark.store.get("weather_location")
    if saved and saved ~= "" then return saved end
    return nil -- auto-detect
end

local function fetch_weather(location)
    local url = "https://wttr.in/"
    if location then
        url = url .. location:gsub(" ", "+")
    end
    url = url .. "?format=j1"
    return lark.http.get(url, { timeout = 8 })
end

lark.register({
    on_run = function()
        local location = get_location()
        local resp = fetch_weather(location)

        if resp.status ~= 200 then
            return {
                title = "Weather",
                items = { { label = "Failed to fetch weather", detail = "HTTP " .. resp.status, icon = "!" } },
            }
        end

        local ok, data = pcall(lark.json.decode, resp.body)
        if not ok or not data.current_condition then
            return {
                title = "Weather",
                items = { { label = "Failed to parse response", icon = "!" } },
            }
        end

        local cc = data.current_condition[1]
        local area = "Unknown"
        if data.nearest_area and data.nearest_area[1] then
            local na = data.nearest_area[1]
            local city = na.areaName and na.areaName[1] and na.areaName[1].value or ""
            local region = na.region and na.region[1] and na.region[1].value or ""
            if city ~= "" then
                area = region ~= "" and (city .. ", " .. region) or city
            end
        end

        local desc = "Unknown"
        if cc.weatherDesc and cc.weatherDesc[1] then
            desc = cc.weatherDesc[1].value
        end

        local code = cc.weatherCode or "0"
        local icon = weather_icon(code)

        local items = {
            { label = desc, detail = "Current condition", icon = icon },
            { label = cc.temp_F .. "°F / " .. cc.temp_C .. "°C", detail = "Temperature", icon = "🌡" },
            { label = "Feels like " .. cc.FeelsLikeF .. "°F / " .. cc.FeelsLikeC .. "°C", detail = "Wind chill / heat index", icon = "🤔" },
            { label = cc.humidity .. "%", detail = "Humidity", icon = "💧" },
            { label = cc.windspeedMiles .. " mph " .. cc.winddir16Point, detail = "Wind", icon = "💨" },
            { label = cc.visibility .. " mi", detail = "Visibility", icon = "👁" },
            { label = (cc.uvIndex or "?"), detail = "UV Index", icon = "☀" },
        }

        -- Sunrise/sunset from astronomy data.
        if data.weather and data.weather[1] and data.weather[1].astronomy then
            local astro = data.weather[1].astronomy[1]
            if astro then
                items[#items + 1] = {
                    label = "↑ " .. (astro.sunrise or "?") .. "  ↓ " .. (astro.sunset or "?"),
                    detail = "Sunrise / Sunset",
                    icon = "🌅",
                }
            end
        end

        return { title = "Weather — " .. area, items = items }
    end,
})
