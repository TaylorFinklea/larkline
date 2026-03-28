-- Emoji Picker — search emoji by name, copy to clipboard.

-- Common emoji set (curated for daily use).
local EMOJIS = {
    { "👍", "thumbs up, like, yes, ok, approve" },
    { "👎", "thumbs down, dislike, no, reject" },
    { "😀", "grinning face, happy, smile" },
    { "😂", "laughing, tears of joy, lol, funny" },
    { "😅", "sweat smile, nervous, awkward" },
    { "😊", "blush, happy, warm, kind" },
    { "😎", "cool, sunglasses, confident" },
    { "😢", "cry, sad, tear" },
    { "😡", "angry, mad, rage" },
    { "🤔", "thinking, hmm, wonder, consider" },
    { "🤷", "shrug, idk, whatever, dunno" },
    { "🙏", "pray, please, thanks, hope" },
    { "🎉", "party, celebrate, congrats, tada" },
    { "🎊", "confetti, celebration" },
    { "🔥", "fire, hot, lit, awesome" },
    { "💯", "hundred, perfect, score" },
    { "❤️", "heart, love, red heart" },
    { "💔", "broken heart, heartbreak" },
    { "⭐", "star, favorite, bookmark" },
    { "✨", "sparkles, magic, new, shiny" },
    { "💡", "lightbulb, idea, tip" },
    { "⚡", "lightning, fast, zap, electric" },
    { "🚀", "rocket, launch, deploy, ship" },
    { "✅", "check, done, complete, yes" },
    { "❌", "x, no, wrong, delete, cancel" },
    { "⚠️", "warning, alert, caution" },
    { "🐛", "bug, insect, error" },
    { "🔧", "wrench, fix, tool, settings" },
    { "🔨", "hammer, build, construct" },
    { "📦", "package, box, ship, deploy" },
    { "📋", "clipboard, paste, copy" },
    { "📝", "memo, note, write, edit" },
    { "📌", "pin, important, bookmark" },
    { "🔗", "link, url, chain, connect" },
    { "🔒", "lock, secure, private, encrypted" },
    { "🔓", "unlock, open, public" },
    { "🔑", "key, password, auth, secret" },
    { "🏠", "home, house" },
    { "💻", "laptop, computer, code, dev" },
    { "📱", "phone, mobile, app" },
    { "🌐", "globe, web, internet, world" },
    { "☁️", "cloud, weather, server" },
    { "🎯", "target, goal, bullseye, aim" },
    { "📊", "chart, stats, data, analytics" },
    { "⏰", "alarm, clock, time, deadline" },
    { "🗓️", "calendar, date, schedule" },
    { "☕", "coffee, break, morning" },
    { "🍺", "beer, cheers, friday" },
    { "🍕", "pizza, food, lunch" },
    { "👀", "eyes, look, review, watch" },
    { "💬", "speech bubble, comment, chat, message" },
    { "📢", "megaphone, announce, broadcast" },
    { "🤖", "robot, bot, ai, automation" },
    { "🐍", "snake, python" },
    { "🦀", "crab, rust" },
    { "🐳", "whale, docker, container" },
    { "🌿", "herb, branch, git, green" },
    { "⬆️", "up arrow, upgrade, increase" },
    { "⬇️", "down arrow, download, decrease" },
    { "➡️", "right arrow, next, forward" },
    { "⬅️", "left arrow, back, previous" },
}

lark.register({
    on_run = function()
        local items = {}
        for _, entry in ipairs(EMOJIS) do
            local emoji, keywords = entry[1], entry[2]
            items[#items + 1] = {
                label = emoji .. "  " .. keywords:match("^([^,]+)"),
                detail = keywords,
                icon = emoji,
                copy_text = emoji,
                actions = {
                    { label = "Copy", kind = "clipboard", args = { emoji } },
                },
            }
        end
        return { title = "Emoji — " .. #items .. " emoji", items = items }
    end,
})
