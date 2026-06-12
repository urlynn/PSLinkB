local m = Map("pslinkb", translate("PSLinkB Config"))
m.title = ""

m.description = [[<h2 name="title">]] .. translate("PSLinkB") .. [[</h2>
<script>(function(){var t=document.querySelector('h2[name="title"]');var m=document.getElementById('tabmenu');if(t&&m&&m.parentNode){m.parentNode.insertBefore(t,m)}})();</script>
<style>
#cbi-pslinkb .cbi-map-descr { display: none !important; }
#cbi-pslinkb .cbi-section h3 { margin-bottom: 0.5rem; }
#cbi-pslinkb .cbi-value br { display: none; }
</style>]]

s1 = m:section(NamedSection, "live", "live", translate("Live Configuration"))
s1:option(Value, "room_id", translate("Room ID"), translate("Live Room ID"))
s1:option(Value, "area_v2", translate("Area ID"), translate("Default 237 - Single Player - Console Game"))
s1:option(Value, "title", translate("Title"), translate("Leave empty for original title"))
o4 = s1:option(Value, "live_mode", translate("Live Mode"), translate("Auto - One-Click Start | Manual - Manual Control"))
o4:value("auto")
o4:value("manual")

s2 = m:section(NamedSection, "auth", "auth", translate("Authentication"))
s2:option(Value, "cookie", translate("Cookie"), translate("Format: SESSDATA=xxx; bili_jct=xxx")).password = true

m.on_after_commit = function()
    luci.sys.call("uci commit pslinkb")
end

return m
