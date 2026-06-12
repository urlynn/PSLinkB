module("luci.controller.pslinkb", package.seeall)

local REDIRECT_DOMAINS = {
    "contribute.live-video.net",
    "global-contribute.live-video.net",
    "tmi.twitch.tv",
    "irc.twitch.tv",
    "live.twitch.tv",
}

local REDIRECT_CONF = "/etc/dnsmasq.d/pslinkb.conf"

function index()
    entry({"admin", "services", "pslinkb"}, alias("admin", "services", "pslinkb", "status"), _("PSLinkB"), 50).dependent = false
    entry({"admin", "services", "pslinkb", "status"},      call("action_status"),      _("Status"),  1).pollinterval = 5
    entry({"admin", "services", "pslinkb", "status-json"},  call("action_status_json")).sysauth = true
    entry({"admin", "services", "pslinkb", "dns-status"},   call("action_dns_status")).sysauth = true
    entry({"admin", "services", "pslinkb", "dns-peek"},     call("action_dns_peek")).sysauth = true
    entry({"admin", "services", "pslinkb", "config"},      cbi("pslinkb/config"),     _("Config"),  2)
    entry({"admin", "services", "pslinkb", "log"},         call("action_log"),         _("Log"),     3).pollinterval = 1
    entry({"admin", "services", "pslinkb", "log-text"},    call("action_log_text")).sysauth = true
    entry({"admin", "services", "pslinkb", "auth"},   call("action_auth"), _("Auth"),   4)


    entry({"admin", "services", "pslinkb", "ctl_start"},   call("action_ctl_start")).leaf = true
    entry({"admin", "services", "pslinkb", "ctl_stop"},    call("action_ctl_stop")).leaf = true
    entry({"admin", "services", "pslinkb", "ctl_restart"}, call("action_ctl_restart")).leaf = true


    entry({"admin", "services", "pslinkb", "redirect_on"},  call("action_redirect_on")).sysauth = true
    entry({"admin", "services", "pslinkb", "redirect_off"}, call("action_redirect_off")).sysauth = true


    entry({"pslinkb-dns-on"},  call("action_redirect_on")).leaf = true
    entry({"pslinkb-dns-off"}, call("action_redirect_off")).leaf = true


    entry({"admin", "services", "pslinkb", "ctl_clear_log"}, call("action_clear_log")).leaf = true


    entry({"admin", "services", "pslinkb", "dns-test-real"},  call("action_dns_test_real")).sysauth = true
    entry({"admin", "services", "pslinkb", "dns-test-fake"},  call("action_dns_test_fake")).sysauth = true
    entry({"admin", "services", "pslinkb", "dns-test-file"},  call("action_dns_test_file")).sysauth = true
    entry({"admin", "services", "pslinkb", "dns-test-delay"}, call("action_dns_test_delay")).sysauth = true
    entry({"admin", "services", "pslinkb", "dns-sim-on"},     call("action_dns_sim_on")).sysauth = true
    entry({"admin", "services", "pslinkb", "dns-sim-off"},    call("action_dns_sim_off")).sysauth = true
    entry({"admin", "services", "pslinkb", "dns-sim-status"}, call("action_dns_sim_status")).sysauth = true
    entry({"admin", "services", "pslinkb", "dns-status"},      call("action_dns_status")).sysauth = true
end


local function is_running()
    return luci.sys.call("pidof pslinkb >/dev/null") == 0
end

local function lan_ip()
    local ip = luci.sys.exec("uci get network.lan.ipaddr 2>/dev/null"):match("^(%S+)")
    return ip or "192.168.1.1"
end

local function dns_redirect_enabled()
    return luci.sys.call("test -f " .. REDIRECT_CONF) == 0
end

local function dns_redirect_status()
    if not dns_redirect_enabled() then
        return "inactive"
    end
    local test_domain = REDIRECT_DOMAINS[1]
    local result = luci.sys.exec("nslookup " .. test_domain .. " 2>/dev/null | grep -oE '([0-9]+%.[0-9]+%.[0-9]+%.[0-9]+)' | tail -1"):match("^(%S+)")
    local router = lan_ip()
    if result and result == router then
        return "active"
    elseif result then
        return "mismatch:" .. result
    else
        return "noresolv"
    end
end


function action_status()
    luci.template.render("pslinkb/status")
end


function action_status_json()

    local function r(p) return luci.sys.exec("cat /tmp/pslinkb/"..p.." 2>/dev/null"):match("^(%S+)") or "" end
    local s = r("state"); local u = r("user"); local q = r("qr_url"); local t = r("rtmp"); local str = r("stream")
    local err = luci.sys.exec("cat /tmp/pslinkb/error 2>/dev/null"):match("^(.-)%s*$") or ""
    local running = (luci.sys.call("pidof pslinkb >/dev/null") == 0)
    local streaming = (luci.sys.call("pidof pslinkb-stream >/dev/null") == 0)
    local dns_on = (luci.sys.call("test -s /tmp/pslinkb/urlynn") == 0)
    luci.http.header("Cache-Control", "no-cache, no-store, must-revalidate")
    luci.http.prepare_content("application/json")
    luci.http.write(luci.jsonc.stringify({
        state = s, user = u, qr = q, rtmp = t, error = err,
        running = running, streaming = streaming, stream = str,
        dns_on = dns_on
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


function action_redirect_on()
    local ip = lan_ip()
    local function wf(p,v) local f=io.open("/tmp/pslinkb/"..p,"w"); if f then f:write(v.."\n"); f:close() end end
    local target = luci.sys.exec("cat /tmp/pslinkb/target-ip 2>/dev/null"):match("^(%S+)") or ip

    local src = luci.sys.exec("nslookup " .. REDIRECT_DOMAINS[1] .. " 127.0.0.1 2>/dev/null|grep -oE '([0-9]+\\.[0-9]+\\.[0-9]+\\.[0-9]+)'|tail -1"):match("^(%S+)") or ""
    wf("source-ip", src)
    wf("target-ip", target)
    wf("isequal", "0")

    local conf = {}
    for _, domain in ipairs(REDIRECT_DOMAINS) do
        conf[#conf + 1] = "address=/" .. domain .. "/" .. ip
    end
    local f = io.open(REDIRECT_CONF, "w")
    if f then f:write(table.concat(conf, "\n") .. "\n"); f:close() end

    luci.sys.call("/etc/init.d/dnsmasq restart >/dev/null 2>&1")
    luci.sys.call("sleep 1")
    src = luci.sys.exec("nslookup " .. REDIRECT_DOMAINS[1] .. " 127.0.0.1 2>/dev/null|grep -oE '([0-9]+\\.[0-9]+\\.[0-9]+\\.[0-9]+)'|tail -1"):match("^(%S+)") or ""
    wf("source-ip", src)
    wf("isequal", (src == target) and "1" or "0")

    luci.http.prepare_content("application/json")
    luci.http.write_json({ok=true, source=src, target=target, isequal=(src==target) and "1" or "0"})
end

function action_redirect_off()
    os.remove(REDIRECT_CONF)
    luci.sys.call("/etc/init.d/dnsmasq restart >/dev/null 2>&1")
    local function wf(p,v) local f=io.open("/tmp/pslinkb/"..p,"w"); if f then f:write(v.."\n"); f:close() end end
    wf("isequal", "2")
    wf("source-ip", "")
    local target = luci.sys.exec("cat /tmp/pslinkb/target-ip 2>/dev/null"):match("^(%S+)") or "192.168.1.1"
    luci.http.prepare_content("application/json")
    luci.http.write_json({ok=true, source="", target=target, isequal="2"})
end


function action_dns_test_real()
    local dc = "/etc/dnsmasq.d/pslinkb.conf"
    local dns_on = (luci.sys.call("test -f " .. dc) == 0)
    local lan = luci.sys.exec("uci get network.lan.ipaddr 2>/dev/null"):match("^(%S+)") or "192.168.1.1"
    local dns_res = dns_on and luci.sys.exec("timeout 2 nslookup contribute.live-video.net 2>/dev/null|grep -oE '([0-9]+\\.[0-9]+\\.[0-9]+\\.[0-9]+)'|tail -1"):match("^(%S+)") or nil
    local dns_ok = dns_res and dns_res == lan
    luci.http.prepare_content("application/json")
    luci.http.write_json({method="real", dns_on=(dns_on==true), dns_ok=(dns_ok==true), ip=dns_res or ""})
end

function action_dns_test_fake()
    local t = (os.time() % 4)
    local states = {{true,false},{true,false},{true,true},{false,false}}
    luci.http.prepare_content("application/json")
    luci.http.write_json({method="fake", dns_on=states[t+1][1], dns_ok=states[t+1][2]})
end

function action_dns_test_file()
    local dc = "/etc/dnsmasq.d/pslinkb.conf"
    local dns_on = (luci.sys.call("test -f " .. dc) == 0)
    local s = luci.sys.exec("cat /tmp/pslinkb/dns_test_flag 2>/dev/null"):match("^(%S+)") or ""
    luci.http.prepare_content("application/json")
    luci.http.write_json({method="file", dns_on=(dns_on==true), dns_ok=(s=="ok")})
end

local SIM_CONF = "/etc/dnsmasq.d/pslinkb-sim.conf"
local SIM_DOMAIN = "httpbin.org"

function action_dns_sim_on()
    local ip = lan_ip()
    local f = io.open(SIM_CONF, "w")
    if f then f:write("address=/" .. SIM_DOMAIN .. "/" .. ip .. "\n"); f:close() end
    luci.sys.exec("/etc/init.d/dnsmasq restart >/dev/null 2>&1 &")
    luci.http.prepare_content("application/json")
    luci.http.write('{"ok":true}')
end

function action_dns_sim_off()
    os.remove(SIM_CONF)
    luci.sys.exec("/etc/init.d/dnsmasq restart >/dev/null 2>&1 &")
    luci.http.prepare_content("application/json")
    luci.http.write('{"ok":true}')
end

function action_dns_sim_status()
    local dns_on = (luci.sys.call("test -f " .. SIM_CONF) == 0)
    local lan = luci.sys.exec("uci get network.lan.ipaddr 2>/dev/null"):match("^(%S+)") or "192.168.1.1"
    local dns_res = dns_on and luci.sys.exec("timeout 2 nslookup " .. SIM_DOMAIN .. " 2>/dev/null|grep -oE '([0-9]+\\.[0-9]+\\.[0-9]+\\.[0-9]+)'|tail -1"):match("^(%S+)") or nil
    local dns_ok = dns_res and dns_res == lan
    luci.http.header("Cache-Control", "no-cache, no-store, must-revalidate")
    luci.http.prepare_content("application/json")
    luci.http.write_json({dns_on=(dns_on==true), dns_ok=(dns_ok==true), dns_res=dns_res or "", lan=lan})
end

function action_dns_status()
    local function rf(p) local f=io.open("/tmp/pslinkb/"..p,"r");if f then local v=f:read("*a"):match("^(.-)%s*$");f:close();return v or "" end;return "" end
    local source = rf("source-ip")
    local target = rf("target-ip")
    local isequal = rf("isequal")
    local dns_on = target ~= ""
    local dns_ok = isequal == "1"
    luci.http.header("Cache-Control", "no-cache, no-store, must-revalidate")
    luci.http.prepare_content("application/json")
    luci.http.write_json({dns_on=dns_on, dns_ok=dns_ok, source=source, target=target, isequal=isequal})
end

function action_dns_peek()
    local domain = "contribute.live-video.net"
    local src = luci.sys.exec("nslookup " .. domain .. " 127.0.0.1 2>/dev/null|grep -oE '([0-9]+\\.[0-9]+\\.[0-9]+\\.[0-9]+)'|tail -1"):match("^(%S+)") or ""
    local target = luci.sys.exec("cat /tmp/pslinkb/target-ip 2>/dev/null"):match("^(%S+)") or "192.168.1.1"
    luci.http.prepare_content("application/json")
    luci.http.write_json({source=src, target=target})
end
