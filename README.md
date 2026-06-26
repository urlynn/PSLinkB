![Release](https://img.shields.io/github/v/release/urlynn/PSLinkB)
![Platform](https://img.shields.io/badge/platform-macOS%20|%20Linux%20|%20Windows%20|%20OpenWrt-blue)
![Downloads](https://img.shields.io/github/downloads/urlynn/PSLinkB/total)

**使用教程：**[PSLinkB - PS5一键自动推流B站·弹幕同步·自动开播·Rust·开源](https://b23.tv/AkiN2Jq)

#### DNS 重定向
```shell
*.global-contribute.live-video.net #hosts 不支持通配符 则为: ingest.global-contribute.live-video.net
irc.twitch.tv
```

## 安装教程
### OpenWrt

```shell
# 使用 apk
apk add --allow-untrusted pslinkb-0.3.3-x86_64-openwrt.apk # or aarch64
apk add --allow-untrusted luci-app-pslinkb-0.3.0.apk
# 使用 opkg
opkg update
opkg install pslinkb-0.3.3-x86_64-openwrt.ipk # or aarch64
opkg install luci-app-pslinkb-0.3.0.ipk
```
## Todo
- [ ] 内置 twitch 直连
- [x]  v0.3.3 - 开播无需人脸验证
- [x]  v0.3.3 - 动态更新直播标题、直播分区
- [x] v0.3.3 - 直播标题使用 PS5 标题
- [ ] 各文档补充


## Feature Flags

| Flag | 依赖 | 作用 |
|------|------|------|
| `channel-mpsc` | — | 单消费者弹幕通道 |
| `channel-broadcast` | — | 多消费者弹幕通道 |
| `cli` | `clap`, `toml`, `qrcode`, `image`, `url` | CLI + TOML + QR 扫码 |
| `openwrt` | — | UCI 配置 + 文件 IPC |
| `ffi-ffmpeg` | — | `ffi` ffmpeg 绑定 |
| `external-ffmpeg` | — | `stream copy`子进程 |
| `dns-redirect` | `hickory-proto` | 内置 DNS 代理 or 检查 |
| `protobuf-support` | `blivemsg/protobuf-support` | 详见 [blivemsg](https://crates.io/crates/blivemsg) |

> **注意**：`channel-mpsc` 与 `channel-broadcast` 互斥、`cli` 与 `openwrt` 互斥。

## 构建配置

| 平台 | Binary | Features |
|------|--------|----------|
| macOS | `pslinkb`@`FFI` | `cli`,`channel-broadcast`,`ffi-ffmpeg`,`dns-redirect`  |
| Linux portable | `pslinkb`@`FFI` | `cli`,`channel-mpsc`,`ffi-ffmpeg`,`dns-redirect` |
| Linux bundle | `pslinkb` + `stream` | `cli`,`channel-mpsc`,`external-ffmpeg`,`dns-redirect` |
| Alpine / DEB / Arch | `pslinkb` + `stream` | `cli`,`channel-mpsc`,`external-ffmpeg`,`dns-redirect` |
| OpenWRT | `pslinkb` + `stream` | `openwrt`,`channel-mpsc`,`external-ffmpeg` |
| Windows | `pslinkb` + `stream` | `cli`,`channel-broadcast`,`external-ffmpeg`,`dns-redirect` |

## 作者吐槽

> 想让 PS5 直播变得全自动化，于是决定用 Rust 来开发一套完整的方案——
>覆盖了 DNS 重写、开关播、推流、弹幕同步等全流程。
> 
> 开发完 `PSLinkB v1.0` ，因 Rust 生态里没有能用的B 站直播弹幕库，只好从头造轮子——也就是 [`blivemsg`](https://github.com/urlynn/blivemsg)。 
>最初不知道有整理好的 B 站 API 文档，就像在黑箱里摸索尝试，收到的消息先看原始数据，再一个个字段解析。经过了无数次重构，最后 `blivemsg` 这个项目，花的时间已经远超初版 `PSLinkB` 本身。
> 
> ---
> 
> 开发 RTMP 服务的时候，先用 `rtmp-rs` 写完了除推流外的全部功能，做完了才发现它不支持 push。
> 之后陆续尝试了 `odv-rtmp`、`rml-rtmp`、`mio`……全都推不上 B 站。
> 折腾许久，最后解决方案让人哭笑不得——自己精简编译了个 923KB 的 ffmpeg。
>
> 双二进制不方便，想深度集成，结果`ffmpeg-next` 不兼容新版 ffmpeg 的 API；
>而 `ffmpeg-sys-next` 的强制要求，不是你适配它的库结构，就是改它的源码。
> 与其改别人的源码来适配我们，还不如自己写呢，手写 FFI 方案就此来临。
> 
> 引入 unsafe 又带来新的问题——ffmpeg 一崩，整个进程跟着崩。
> external 模式绕了一圈，又回到了外置 ffmpeg？
>
> 最后想通了——既然只需要 stream copy 这一个功能，为何不手写 `stream_copy.c`，只链接 `avformat`、`avcodec`、`avutil` 三个库。
> 923KB → 633KB，这回儿终于满意了。

## 特别致谢

[IceNoproblem/PS5BiliDanmaku](https://github.com/IceNoproblem/PS5BiliDanmaku) - 灵感来源

[GamerNoTitle/BiliLive-Utility](https://github.com/GamerNoTitle/BiliLive-Utility) - Electron 开播模式参考

[SocialSisterYi/bilibili-API-collect]() - Biliblili API 参考
