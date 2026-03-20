-- Weather — current conditions from wttr.in (no API key needed).
-- Demonstrates lark.http.get() and lark.json.decode().

lark.register({
    on_run = function()
        local resp = lark.http.get("https://wttr.in/?format=j1", { timeout = 5 })

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

        local items = {
            { label = desc, detail = "Current condition", icon = "C" },
            { label = cc.temp_F .. "°F / " .. cc.temp_C .. "°C", detail = "Temperature", icon = "T" },
            { label = "Feels like " .. cc.FeelsLikeF .. "°F / " .. cc.FeelsLikeC .. "°C", detail = "Wind chill / heat index", icon = "F" },
            { label = cc.humidity .. "%", detail = "Humidity", icon = "H" },
            { label = cc.windspeedMiles .. " mph " .. cc.winddir16Point, detail = "Wind", icon = "W" },
        }

        return { title = "Weather — " .. area, items = items }
    end,
})
