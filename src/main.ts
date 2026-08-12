import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

type Profile = {
  id: string;
  name: string;
  apiKey: string;
  baseUrl: string;
};

type AppState = {
  directory: string;
  profiles: Profile[];
  activeId: string | null;
};

type ImportPreview = {
  directory: string | null;
  profileCount: number;
};

const emptyProfile = (): Profile => ({
  id: "",
  name: "",
  apiKey: "",
  baseUrl: "",
});

let state: AppState = { directory: "", profiles: [], activeId: null };
let editing = emptyProfile();
let editingApplication = false;
let busy = false;
let toastTimer = 0;

const app = document.querySelector<HTMLDivElement>("#app")!;

function escapeHtml(value: string): string {
  const element = document.createElement("div");
  element.textContent = value;
  return element.innerHTML;
}

function escapeAttribute(value: string): string {
  return escapeHtml(value).replace(/"/g, "&quot;").replace(/'/g, "&#39;");
}

function maskedKey(value: string): string {
  return value.length < 12 ? "********" : `${value.slice(0, 7)}********${value.slice(-4)}`;
}

function firstAvailableProfile(nextState: AppState): Profile | undefined {
  return nextState.profiles.find((profile) => profile.id === nextState.activeId)
    ?? nextState.profiles[0];
}

function render(): void {
  app.innerHTML = `
    <main class="shell">
      <header>
        <div><p class="eyebrow">CODEX</p><h1>账号配置</h1></div>
        <div class="header-actions">
          <button class="secondary" id="import-profiles" ${busy ? "disabled" : ""}>导入</button>
          <button class="secondary" id="export-profiles" ${busy || state.profiles.length === 0 ? "disabled" : ""}>导出</button>
          <button class="secondary" id="new-profile" ${busy ? "disabled" : ""}>新建配置</button>
          <button class="text danger" id="exit-app" ${busy ? "disabled" : ""}>退出</button>
        </div>
      </header>
      <section class="application-bar">
        <div>
          <p class="eyebrow">APPLICATION</p>
          <div class="application-title"><strong>Codex</strong><span>${escapeHtml(state.directory || "尚未设置目录")}</span></div>
        </div>
        <button class="secondary" id="edit-application" ${busy ? "disabled" : ""}>修改目录</button>
      </section>
      <section class="workspace">
        <aside aria-label="已保存配置">
          <div class="section-heading"><h2>已保存</h2><span>${state.profiles.length}</span></div>
          <div class="profile-list">
            ${state.profiles.length === 0 ? '<p class="empty">还没有配置</p>' : ""}
            ${state.profiles.map((profile) => `
              <article class="profile ${profile.id === state.activeId ? "active" : ""} ${profile.id === editing.id ? "selected" : ""}" data-profile-id="${escapeAttribute(profile.id)}" tabindex="0">
                <div class="profile-title">
                  <strong>${escapeHtml(profile.name)}</strong>
                  ${profile.id === state.activeId ? "<span>当前</span>" : ""}
                </div>
                <p>${escapeHtml(profile.baseUrl)}</p>
                <p class="key">${escapeHtml(maskedKey(profile.apiKey))}</p>
                <div class="profile-actions">
                  <button class="primary apply" data-id="${escapeAttribute(profile.id)}" ${busy || profile.id === state.activeId ? "disabled" : ""}>切换</button>
                  <button class="text edit" data-id="${escapeAttribute(profile.id)}" ${busy ? "disabled" : ""}>编辑</button>
                  <button class="text danger delete" data-id="${escapeAttribute(profile.id)}" ${busy ? "disabled" : ""}>删除</button>
                </div>
              </article>`).join("")}
          </div>
        </aside>
        ${editingApplication ? `
        <form id="application-form">
          <div class="section-heading form-heading">
            <div><p class="eyebrow">APPLICATION</p><h2>Codex 应用设置</h2></div>
          </div>
          <label>Codex 配置目录
            <input name="directory" required autocomplete="off" value="${escapeAttribute(state.directory)}" placeholder="C:\\Users\\Admin\\.codex" />
            <small>只设置一次，所有账号都切换这个目录中的 auth.json 和 config.toml</small>
          </label>
          <div class="form-actions">
            <button class="primary" type="submit" ${busy ? "disabled" : ""}>${busy ? "处理中..." : "保存应用设置"}</button>
            <button class="secondary" type="button" id="cancel-application">取消</button>
          </div>
        </form>` : `
        <form id="profile-form">
          <div class="section-heading form-heading">
            <div><p class="eyebrow">${editing.id ? "EDIT" : "NEW"}</p><h2>${editing.id ? "编辑配置" : "新建配置"}</h2></div>
          </div>
          <label>配置名称
            <input name="name" required autocomplete="off" value="${escapeAttribute(editing.name)}" placeholder="例如：工作账号" />
          </label>
          <label>OPENAI_API_KEY
            <div class="secret-input">
              <input name="apiKey" type="password" required autocomplete="off" value="${escapeAttribute(editing.apiKey)}" placeholder="sk-..." />
              <label class="show-key"><input type="checkbox" id="show-key" /> 显示</label>
            </div>
          </label>
          <label>base_url
            <input name="baseUrl" type="url" required autocomplete="off" value="${escapeAttribute(editing.baseUrl)}" placeholder="https://api.example.com" />
          </label>
          <div class="form-actions">
            <button class="primary" type="submit" ${busy ? "disabled" : ""}>${busy ? "处理中..." : "保存配置"}</button>
            ${editing.id ? '<button class="secondary" type="button" id="cancel-edit">取消</button>' : ""}
          </div>
        </form>`}
      </section>
      <div id="toast" role="status" aria-live="polite"></div>
    </main>`;
  bindEvents();
}

function formProfile(): Profile {
  const data = new FormData(document.querySelector<HTMLFormElement>("#profile-form")!);
  return {
    id: editing.id,
    name: String(data.get("name") ?? ""),
    apiKey: String(data.get("apiKey") ?? ""),
    baseUrl: String(data.get("baseUrl") ?? ""),
  };
}

function showToast(message: string, error = false): void {
  window.clearTimeout(toastTimer);
  const toast = document.querySelector<HTMLDivElement>("#toast")!;
  toast.textContent = message;
  toast.className = error ? "visible error" : "visible";
  toastTimer = window.setTimeout(() => toast.className = "", 3000);
}

async function run(
  action: () => Promise<AppState>,
  success: string,
  selectAfter: (nextState: AppState) => Profile | undefined,
): Promise<boolean> {
  busy = true;
  render();
  try {
    state = await action();
    const selected = selectAfter(state) ?? firstAvailableProfile(state);
    editing = selected ? { ...selected } : emptyProfile();
    busy = false;
    render();
    showToast(success);
    return true;
  } catch (error) {
    busy = false;
    render();
    showToast(String(error), true);
    return false;
  }
}

async function importProfiles(): Promise<void> {
  try {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "JSON 配置", extensions: ["json"] }],
    });
    if (path) {
      const preview = await invoke<ImportPreview>("preview_import", { path });
      if (!window.confirm(`导入文件包含 ${preview.profileCount} 个账号，继续？`)) return;
      let importDirectory = false;
      if (preview.directory && preview.directory !== state.directory) {
        importDirectory = window.confirm(
          `导入文件包含应用目录：\n${preview.directory}\n\n是否同时覆盖当前 Codex 目录？\n选择“取消”将保留本机目录。`,
        );
      }
      await run(
        () => invoke("import_profiles", { path, importDirectory }),
        "配置已导入",
        firstAvailableProfile,
      );
    }
  } catch (error) {
    showToast(String(error), true);
  }
}

async function exportProfiles(): Promise<void> {
  if (!window.confirm("导出文件包含明文 OPENAI_API_KEY，继续？")) return;
  try {
    busy = true;
    render();
    const exported = await invoke<boolean>("export_profiles");
    busy = false;
    render();
    if (exported) showToast("配置已导出");
  } catch (error) {
    busy = false;
    render();
    showToast(String(error), true);
  }
}

function bindEvents(): void {
  document.querySelector("#edit-application")?.addEventListener("click", () => {
    editingApplication = true;
    render();
  });
  document.querySelector("#cancel-application")?.addEventListener("click", () => {
    editingApplication = false;
    render();
  });
  document.querySelector("#application-form")?.addEventListener("submit", (event) => {
    event.preventDefault();
    const data = new FormData(event.currentTarget as HTMLFormElement);
    const directory = String(data.get("directory") ?? "");
    void run(
      () => invoke("save_application", { directory }),
      "Codex 目录已保存",
      firstAvailableProfile,
    ).then((saved) => {
      if (saved) {
        editingApplication = false;
        render();
      }
    });
  });
  document.querySelector("#import-profiles")?.addEventListener("click", () => {
    void importProfiles();
  });
  document.querySelector("#export-profiles")?.addEventListener("click", () => {
    void exportProfiles();
  });
  document.querySelector("#new-profile")?.addEventListener("click", () => {
    editingApplication = false;
    editing = emptyProfile();
    render();
  });
  document.querySelector("#exit-app")?.addEventListener("click", () => {
    if (window.confirm("退出 Codex Key Manager？")) void invoke("exit_app");
  });
  document.querySelector("#cancel-edit")?.addEventListener("click", () => {
    editing = emptyProfile();
    render();
  });
  document.querySelector<HTMLInputElement>("#show-key")?.addEventListener("change", (event) => {
    document.querySelector<HTMLInputElement>('input[name="apiKey"]')!.type =
      (event.target as HTMLInputElement).checked ? "text" : "password";
  });
  document.querySelector("#profile-form")?.addEventListener("submit", (event) => {
    event.preventDefault();
    const profile = formProfile();
    void run(
      () => invoke("save_profile", { profile }),
      "配置已保存",
      (nextState) => profile.id
        ? nextState.profiles.find((item) => item.id === profile.id)
        : nextState.profiles[nextState.profiles.length - 1],
    );
  });
  document.querySelectorAll<HTMLButtonElement>(".apply").forEach((button) => {
    button.addEventListener("click", () => {
      const id = button.dataset.id!;
      void run(
        () => invoke("apply_profile", { id }),
        "切换成功，重启 Codex 后生效",
        (nextState) => nextState.profiles.find((profile) => profile.id === id),
      );
    });
  });
  document.querySelectorAll<HTMLButtonElement>(".edit").forEach((button) => {
    button.addEventListener("click", () => {
      const profile = state.profiles.find((item) => item.id === button.dataset.id);
      if (profile) {
        editingApplication = false;
        editing = { ...profile };
        render();
      }
    });
  });
  document.querySelectorAll<HTMLButtonElement>(".delete").forEach((button) => {
    button.addEventListener("click", () => {
      const profile = state.profiles.find((item) => item.id === button.dataset.id);
      if (profile && window.confirm(`删除配置“${profile.name}”？`)) {
        void run(
          () => invoke("delete_profile", { id: profile.id }),
          "配置已删除",
          firstAvailableProfile,
        );
      }
    });
  });
  document.querySelectorAll<HTMLElement>(".profile").forEach((card) => {
    const selectCard = () => {
      const profile = state.profiles.find((item) => item.id === card.dataset.profileId);
      if (profile) {
        editingApplication = false;
        editing = { ...profile };
        render();
      }
    };
    card.addEventListener("click", (event) => {
      if (!(event.target as HTMLElement).closest("button")) selectCard();
    });
    card.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        selectCard();
      }
    });
  });
}

async function start(): Promise<void> {
  render();
  try {
    state = await invoke("get_state");
    const selected = firstAvailableProfile(state);
    editing = selected ? { ...selected } : emptyProfile();
    render();
  } catch (error) {
    showToast(String(error), true);
  }
}

void start();
