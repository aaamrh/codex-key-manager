# Codex Key Manager

Windows 桌面工具，用于保存并快速切换多组 Codex API 配置。

切换时只修改：

- `auth.json` 中的 `OPENAI_API_KEY`
- `config.toml` 中 `[model_providers.OpenAI]` 的 `base_url`

配置保存在当前用户的 `%APPDATA%\codex-key-manager\profiles.json`。API Key 为明文，与 Codex 的 `auth.json` 存储方式一致。

关闭窗口后应用隐藏到 Windows 系统托盘。双击托盘图标可恢复窗口，右键菜单可打开或退出。

## 开发

```powershell
npm install
npm run tauri dev
```

## 构建

```powershell
npm run tauri build
```

安装程序输出到 `src-tauri\target\release\bundle\nsis`。
