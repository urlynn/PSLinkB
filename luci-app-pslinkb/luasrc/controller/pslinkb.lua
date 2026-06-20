module("luci.controller.pslinkb", package.seeall)

function index()
    entry({"admin", "services", "pslinkb"}, alias("admin", "services", "pslinkb", "status"), _("PSLinkB"), 50).dependent = false
    entry({"admin", "services", "pslinkb", "status"},      call("action_status"),      _("Status"),  1)
    entry({"admin", "services", "pslinkb", "status-json"},  call("action_status_json")).sysauth = true
    entry({"admin", "services", "pslinkb", "dns-status"},   call("action_dns_status")).sysauth = true
    entry({"admin", "services", "pslinkb", "config"},      cbi("pslinkb/config"),     _("Config"),  2)
    entry({"admin", "services", "pslinkb", "log"},         call("action_log"),         _("Log"),     3).pollinterval = 1
    entry({"admin", "services", "pslinkb", "log-text"},    call("action_log_text")).sysauth = true
    entry({"admin", "services", "pslinkb", "auth"},   call("action_auth"), _("Auth"),   4)

    entry({"admin", "services", "pslinkb", "ctl_start"},   call("action_ctl_start")).leaf = true
    entry({"admin", "services", "pslinkb", "ctl_stop"},    call("action_ctl_stop")).leaf = true
    entry({"admin", "services", "pslinkb", "ctl_restart"}, call("action_ctl_restart")).leaf = true

    entry({"admin", "services", "pslinkb", "ctl_clear_log"}, call("action_clear_log")).leaf = true

    entry({"admin", "services", "pslinkb", "dns-toggle"}, call("action_dns_toggle")).sysauth = true
end


local function is_running()
    return luci.sys.call("pidof pslinkb >/dev/null") == 0
end

local function lan_ip()
    local ip = luci.sys.exec("uci get network.lan.ipaddr 2>/dev/null"):match("^(%S+)")
    return ip or "192.168.1.1"
end


function action_status()
    luci.template.render("pslinkb/status_js")
end


function action_status_json()

    local running = (luci.sys.call("pidof pslinkb >/dev/null") == 0)
    local function r(p) return running and luci.sys.exec("cat /tmp/pslinkb/"..p.." 2>/dev/null"):match("^(%S+)") or "" end
    local u = r("user"); local q = r("qr_url"); local t = r("rtmp"); local str = r("stream")
    local err = luci.sys.exec("cat /tmp/pslinkb/error 2>/dev/null"):match("^(.-)%s*$") or ""
    local streaming = false
    local pids = luci.sys.exec("pidof pslinkb-stream 2>/dev/null")
    if pids and #pids > 0 then
        for pid in pids:gmatch("%S+") do
            local stat = luci.sys.exec("cat /proc/" .. pid .. "/stat 2>/dev/null"):match("^%d+ %(%S+%) (%S)")
            if stat and stat ~= "Z" then
                streaming = true
                break
            end
        end
    end
    local stream_crashed = (not streaming and err:find("FFmpeg: Worker crashed", 1, true) ~= nil)

    local push_url = ""
    local live_mode = luci.sys.exec("uci get pslinkb.@live[0].mode 2>/dev/null"):match("^(%S+)") or "auto"
    if live_mode == "manual" then
        local stream_key = luci.sys.exec("uci get pslinkb.@live[0].stream_key 2>/dev/null"):match("^(%S+)") or ""
        if stream_key ~= "" then
            push_url = "rtmp://" .. lan_ip() .. ":1935/live/" .. stream_key
        end
    end

    luci.http.header("Cache-Control", "no-cache, no-store, must-revalidate")
    luci.http.prepare_content("application/json")
    luci.http.write(luci.jsonc.stringify({
        state = "", user = u, qr = q, rtmp = t, error = err,
        running = running, streaming = streaming, stream_crashed = stream_crashed, stream = str,
        push_url = push_url,
    }))
end


function action_log()
    luci.template.render("pslinkb/log")
end

function action_log_text()
    luci.http.prepare_content("text/plain; charset=utf-8")
    luci.http.write(luci.sys.exec("cat /tmp/pslinkb/log 2>/dev/null | tail -80"))
end


function action_auth()
    local url = luci.sys.exec("cat /tmp/pslinkb/qr_url 2>/dev/null"):match("^(%S+)")
    if url and #url > 0 then
        local ua = (luci.http.getenv("HTTP_USER_AGENT") or ""):lower()
        if ua:match("iphone") or ua:match("ipad") or ua:match("android") or ua:match("mobile") then
            luci.http.redirect(url)
        else
            local user = (luci.sys.exec("cat /tmp/pslinkb/user 2>/dev/null"):match("^(%S+)"))
            luci.template.render("pslinkb/auth_pc", {
                qr_url = url,
                is_logged_in = (user and #user > 0)
            })
        end
    else
        luci.template.render("pslinkb/auth_missing")
    end
end


function action_clear_log()
    local f = io.open("/tmp/pslinkb/log", "w"); if f then f:close() end
    luci.http.redirect(luci.dispatcher.build_url("admin/services/pslinkb/log"))
end


function action_ctl_start()
    luci.sys.call("/etc/init.d/pslinkb start >/dev/null 2>&1")
    luci.http.status(204, "No Content")
end

function action_ctl_stop()
    luci.sys.call("/etc/init.d/pslinkb stop >/dev/null 2>&1")
    luci.http.status(204, "No Content")
end

function action_ctl_restart()
    luci.sys.call("/etc/init.d/pslinkb restart >/dev/null 2>&1")
    luci.http.status(204, "No Content")
end


function action_dns_status()
    local f = io.open("/tmp/pslinkb/dns_status", "r")
    if not f then
        luci.http.prepare_content("application/json")
        luci.http.write_json({checking=false, enabled=false, target="", actual="", ok=false})
        return
    end
    local raw = f:read("*a")
    f:close()
    luci.http.header("Cache-Control", "no-cache, no-store, must-revalidate")
    luci.http.prepare_content("application/json")
    luci.http.write(raw)
end


-- ── DNS 开关 ──

function action_dns_toggle()
    local val = luci.http.formvalue("val")  -- "1" or "0"
    if val ~= "1" and val ~= "0" then
        luci.http.status(400, "Bad Request")
        return
    end
    luci.sys.call("uci set pslinkb.config.dns_redirect=" .. val)
    luci.sys.call("uci commit pslinkb")
    os.execute("kill -HUP $(pgrep pslinkb) 2>/dev/null &")
    luci.http.status(204, "No Content")
end


