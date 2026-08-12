# Codex Key Manager

Windows 桌面工具，用于按应用保存并快速切换多组 API 账号。

界面按“应用 → 账号”组织。每个应用实例单独保存类型、名称、配置目录和账号；目前已实现 Codex 适配器，后续增加 Claude Code 时不需要修改存储层级和主界面。

切换时只修改：

- `auth.json` 中的 `OPENAI_API_KEY`
- `config.toml` 中 `[model_providers.OpenAI]` 的 `base_url`

配置保存在当前用户的 `%APPDATA%\codex-key-manager\profiles.json`。API Key 为明文，与 Codex 的 `auth.json` 存储方式一致。

支持通过 JSON 文件导入、导出全部应用和账号。导入时可选择是否导入配置目录；同 ID 应用及账号会合并。导出文件包含明文 API Key，请妥善保管。v1 数组格式和 v2 单应用格式会自动迁移。

导出格式：

```json
{
  "version": 3,
  "applications": [
    {
      "id": "application-uuid",
      "name": "Codex 工作",
      "kind": "codex",
      "directory": "C:\\Users\\Admin\\.codex",
      "profiles": [
        {
          "id": "profile-id",
          "name": "工作账号",
          "apiKey": "sk-...",
          "baseUrl": "https://api.example.com"
        }
      ]
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
