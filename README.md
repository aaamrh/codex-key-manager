# Codex Key Manager

Windows 桌面工具，用于保存并快速切换多组 Codex API 配置。

Codex 配置目录是应用级设置，只需设置一次；每个账号只保存名称、API Key 和 base URL。

切换时只修改：

- `auth.json` 中的 `OPENAI_API_KEY`
- `config.toml` 中 `[model_providers.OpenAI]` 的 `base_url`

配置保存在当前用户的 `%APPDATA%\codex-key-manager\profiles.json`。API Key 为明文，与 Codex 的 `auth.json` 存储方式一致。

支持通过 JSON 文件导入、导出应用设置和全部账号。导入时可选择是否覆盖本机 Codex 目录；同 ID 账号会更新，新 ID 账号会追加。导出文件包含明文 API Key，请妥善保管。旧版数组格式会自动迁移。

导出格式：

```json
{
  "version": 2,
  "application": {
    "id": "codex",
    "directory": "C:\\Users\\Admin\\.codex"
  },
  "profiles": [
    {
      "id": "profile-id",
      "name": "工作账号",
      "apiKey": "sk-...",
      "baseUrl": "https://api.example.com"
    }
  ]
}
```

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
