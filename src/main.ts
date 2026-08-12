import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

type Profile = {
  id: string;
  name: string;
  apiKey: string;
  baseUrl: string;
};

type Application = {
  id: string;
  name: string;
  kind: string;
  directory: string;
  profiles: Profile[];
  activeId: string | null;
};

type AppState = { applications: Application[] };

type ImportPreview = {
  applicationCount: number;
  profileCount: number;
  hasExistingDirectories: boolean;
};

const emptyProfile = (): Profile => ({ id: "", name: "", apiKey: "", baseUrl: "" });
const emptyApplication = (): Application => ({
  id: "",
  name: "",
  kind: "codex",
  directory: "",
  profiles: [],
  activeId: null,
});

let state: AppState = { applications: [] };
let selectedApplicationId = "";
let editingProfile = emptyProfile();
let editingApplication: Application | null = null;
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

function currentApplication(): Application | undefined {
  return state.applications.find((application) => application.id === selectedApplicationId)
    ?? state.applications[0];
}

function syncSelection(): void {
  const application = currentApplication();
  selectedApplicationId = application?.id ?? "";
  if (!application) {
    editingProfile = emptyProfile();
    return;
  }
  const selected = application.profiles.find((profile) => profile.id === editingProfile.id)
    ?? application.profiles.find((profile) => profile.id === application.activeId)
    ?? application.profiles[0];
  editingProfile = selected ? { ...selected } : emptyProfile();
}

function renderProfileForm(application: Application): string {
  return `
    <form id="profile-form">
      <div class="form-heading">
        <div><p class="eyebrow">${editingProfile.id ? "EDIT ACCOUNT" : "NEW ACCOUNT"}</p><h2>${editingProfile.id ? "编辑账号" : "新建账号"}</h2></div>
      </div>
      <label>账号名称
        <input name="name" required autocomplete="off" value="${escapeAttribute(editingProfile.name)}" placeholder="例如：工作账号" />
      </label>
      <label>OPENAI_API_KEY
        <div class="secret-input">
          <input name="apiKey" type="password" required autocomplete="off" value="${escapeAttribute(editingProfile.apiKey)}" placeholder="sk-..." />
          <label class="show-key"><input type="checkbox" id="show-key" /> 显示</label>
        </div>
      </label>
      <label>base_url
        <input name="baseUrl" type="url" required autocomplete="off" value="${escapeAttribute(editingProfile.baseUrl)}" placeholder="https://api.example.com" />
      </label>
      <div class="form-actions">
        <button class="primary" type="submit" ${busy ? "disabled" : ""}>${busy ? "处理中..." : "保存账号"}</button>
        ${editingProfile.id ? '<button class="secondary" type="button" id="cancel-profile">取消</button>' : ""}
      </div>
      <p class="form-context">保存到 ${escapeHtml(application.name)}</p>
    </form>`;
}

function renderApplicationForm(): string {
  const application = editingApplication ?? emptyApplication();
  return `
    <form id="application-form">
      <div class="form-heading">
        <div><p class="eyebrow">APPLICATION</p><h2>${application.id ? "编辑应用" : "添加应用"}</h2></div>
      </div>
      <label>应用名称
        <input name="name" required autocomplete="off" value="${escapeAttribute(application.name)}" placeholder="例如：Codex 工作区" />
      </label>
      <label>应用类型
        <select name="kind" required>
          <option value="codex" selected>Codex</option>
        </select>
      </label>
      <label>配置目录
        <input name="directory" required autocomplete="off" value="${escapeAttribute(application.directory)}" placeholder="C:\\Users\\Admin\\.codex" />
        <small>Codex 目录中必须包含 auth.json 和 config.toml</small>
      </label>
      <div class="form-actions">
        <button class="primary" type="submit" ${busy ? "disabled" : ""}>${busy ? "处理中..." : "保存应用"}</button>
        <button class="secondary" type="button" id="cancel-application">取消</button>
        ${application.id && state.applications.length > 1 ? '<button class="text danger push-right" type="button" id="delete-application">删除应用</button>' : ""}
      </div>
    </form>`;
}

function render(): void {
  const application = currentApplication();
  app.innerHTML = `
    <main class="shell">
      <header>
        <div><p class="eyebrow">KEY MANAGER</p><h1>配置中心</h1></div>
        <div class="header-actions">
          <button class="secondary" id="import-data" ${busy ? "disabled" : ""}>导入</button>
          <button class="secondary" id="export-data" ${busy || state.applications.length === 0 ? "disabled" : ""}>导出</button>
          <button class="text danger" id="exit-app" ${busy ? "disabled" : ""}>退出</button>
        </div>
      </header>
      <section class="workspace">
        <nav class="application-nav" aria-label="应用">
          <div class="column-heading"><h2>应用</h2><button class="icon-button" id="new-application" title="添加应用" ${busy ? "disabled" : ""}>+</button></div>
          <div class="application-list">
            ${state.applications.map((item) => `
              <button class="application-item ${item.id === application?.id ? "selected" : ""}" data-application-id="${escapeAttribute(item.id)}">
                <span class="application-icon">${escapeHtml(item.name.slice(0, 1).toUpperCase() || "C")}</span>
                <span><strong>${escapeHtml(item.name)}</strong><small>${item.profiles.length} 个账号</small></span>
              </button>`).join("")}
          </div>
        </nav>
        ${application ? `
          <aside class="account-column" aria-label="${escapeAttribute(application.name)} 账号">
            <div class="application-summary">
              <div><span class="kind-badge">${escapeHtml(application.kind)}</span><h2>${escapeHtml(application.name)}</h2></div>
              <button class="icon-button" id="edit-application" title="应用设置">⚙</button>
              <p title="${escapeAttribute(application.directory)}">${escapeHtml(application.directory)}</p>
            </div>
            <div class="column-heading"><h2>账号</h2><button class="icon-button" id="new-profile" title="新建账号" ${busy ? "disabled" : ""}>+</button></div>
            <div class="profile-list">
              ${application.profiles.length === 0 ? '<p class="empty">还没有账号</p>' : ""}
              ${application.profiles.map((profile) => `
                <article class="profile ${profile.id === application.activeId ? "active" : ""} ${profile.id === editingProfile.id && !editingApplication ? "selected" : ""}" data-profile-id="${escapeAttribute(profile.id)}" tabindex="0">
                  <div class="profile-title"><strong>${escapeHtml(profile.name)}</strong>${profile.id === application.activeId ? "<span>当前</span>" : ""}</div>
                  <p>${escapeHtml(profile.baseUrl)}</p>
                  <p class="key">${escapeHtml(maskedKey(profile.apiKey))}</p>
                  <div class="profile-actions">
                    <button class="primary apply" data-id="${escapeAttribute(profile.id)}" ${busy || profile.id === application.activeId ? "disabled" : ""}>切换</button>
                    <button class="text edit" data-id="${escapeAttribute(profile.id)}" ${busy ? "disabled" : ""}>编辑</button>
                    <button class="text danger delete" data-id="${escapeAttribute(profile.id)}" ${busy ? "disabled" : ""}>删除</button>
                  </div>
                </article>`).join("")}
            </div>
          </aside>
          <section class="editor-column">${editingApplication ? renderApplicationForm() : renderProfileForm(application)}</section>
        ` : '<section class="editor-column empty-workspace">请先添加应用</section>'}
      </section>
      <div id="toast" role="status" aria-live="polite"></div>
    </main>`;
  bindEvents();
}

function showToast(message: string, error = false): void {
  window.clearTimeout(toastTimer);
  const toast = document.querySelector<HTMLDivElement>("#toast")!;
  toast.textContent = message;
  toast.className = error ? "visible error" : "visible";
  toastTimer = window.setTimeout(() => toast.className = "", 3000);
}

async function runState(action: () => Promise<AppState>, success: string, after?: () => void): Promise<boolean> {
  busy = true;
  render();
  try {
    state = await action();
    after?.();
    syncSelection();
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

async function importData(): Promise<void> {
  try {
    const path = await open({ multiple: false, directory: false, filters: [{ name: "JSON 配置", extensions: ["json"] }] });
    if (!path) return;
    const preview = await invoke<ImportPreview>("preview_import", { path });
    if (!window.confirm(`导入 ${preview.applicationCount} 个应用、${preview.profileCount} 个账号，继续？`)) return;
    const importDirectories = preview.hasExistingDirectories
      && window.confirm("是否同时导入配置目录？\n换电脑时建议选择“取消”，保留本机目录。");
    await runState(
      () => invoke("import_profiles", { path, importDirectories }),
      "配置已导入",
      () => { selectedApplicationId = state.applications[0]?.id ?? ""; editingApplication = null; },
    );
  } catch (error) {
    showToast(String(error), true);
  }
}

async function exportData(): Promise<void> {
  if (!window.confirm("完整备份包含配置目录和明文 API Key，继续？")) return;
  busy = true;
  render();
  try {
    const exported = await invoke<boolean>("export_profiles");
    busy = false;
    render();
    if (exported) showToast("完整备份已导出");
  } catch (error) {
    busy = false;
    render();
    showToast(String(error), true);
  }
}

function bindEvents(): void {
  document.querySelectorAll<HTMLButtonElement>(".application-item").forEach((button) => {
    button.addEventListener("click", () => {
      selectedApplicationId = button.dataset.applicationId!;
      editingApplication = null;
      editingProfile = emptyProfile();
      syncSelection();
      render();
    });
  });
  document.querySelector("#new-application")?.addEventListener("click", () => {
    editingApplication = emptyApplication();
    render();
  });
  document.querySelector("#edit-application")?.addEventListener("click", () => {
    const application = currentApplication();
    if (application) editingApplication = { ...application, profiles: [...application.profiles] };
    render();
  });
  document.querySelector("#cancel-application")?.addEventListener("click", () => {
    editingApplication = null;
    render();
  });
  document.querySelector("#application-form")?.addEventListener("submit", (event) => {
    event.preventDefault();
    const form = new FormData(event.currentTarget as HTMLFormElement);
    const previousIds = new Set(state.applications.map((item) => item.id));
    const id = editingApplication?.id ?? "";
    void runState(
      () => invoke("save_application", {
        id,
        name: String(form.get("name") ?? ""),
        kind: String(form.get("kind") ?? "codex"),
        directory: String(form.get("directory") ?? ""),
      }),
      "应用已保存",
      () => {
        selectedApplicationId = id || state.applications.find((item) => !previousIds.has(item.id))?.id || selectedApplicationId;
        editingApplication = null;
        editingProfile = emptyProfile();
      },
    );
  });
  document.querySelector("#delete-application")?.addEventListener("click", () => {
    const application = currentApplication();
    if (application && window.confirm(`删除应用“${application.name}”及其全部账号？`)) {
      void runState(
        () => invoke("delete_application", { id: application.id }),
        "应用已删除",
        () => { selectedApplicationId = state.applications[0]?.id ?? ""; editingApplication = null; editingProfile = emptyProfile(); },
      );
    }
  });
  document.querySelector("#new-profile")?.addEventListener("click", () => {
    editingApplication = null;
    editingProfile = emptyProfile();
    render();
  });
  document.querySelector("#cancel-profile")?.addEventListener("click", () => {
    editingProfile = emptyProfile();
    render();
  });
  document.querySelector<HTMLInputElement>("#show-key")?.addEventListener("change", (event) => {
    document.querySelector<HTMLInputElement>('input[name="apiKey"]')!.type =
      (event.target as HTMLInputElement).checked ? "text" : "password";
  });
  document.querySelector("#profile-form")?.addEventListener("submit", (event) => {
    event.preventDefault();
    const application = currentApplication();
    if (!application) return;
    const form = new FormData(event.currentTarget as HTMLFormElement);
    const profile: Profile = {
      id: editingProfile.id,
      name: String(form.get("name") ?? ""),
      apiKey: String(form.get("apiKey") ?? ""),
      baseUrl: String(form.get("baseUrl") ?? ""),
    };
    const previousIds = new Set(application.profiles.map((item) => item.id));
    void runState(
      () => invoke("save_profile", { applicationId: application.id, profile }),
      "账号已保存",
      () => {
        editingApplication = null;
        const savedApplication = state.applications.find((item) => item.id === application.id);
        editingProfile = profile.id
          ? profile
          : savedApplication?.profiles.find((item) => !previousIds.has(item.id)) ?? emptyProfile();
      },
    );
  });
  document.querySelectorAll<HTMLButtonElement>(".apply").forEach((button) => {
    button.addEventListener("click", () => {
      const application = currentApplication();
      if (!application) return;
      const id = button.dataset.id!;
      void runState(
        () => invoke("apply_profile", { applicationId: application.id, id }),
        `已切换 ${application.name}，重启应用后生效`,
        () => { editingProfile = application.profiles.find((profile) => profile.id === id) ?? emptyProfile(); },
      );
    });
  });
  document.querySelectorAll<HTMLButtonElement>(".edit").forEach((button) => {
    button.addEventListener("click", () => {
      const profile = currentApplication()?.profiles.find((item) => item.id === button.dataset.id);
      if (profile) { editingApplication = null; editingProfile = { ...profile }; render(); }
    });
  });
  document.querySelectorAll<HTMLButtonElement>(".delete").forEach((button) => {
    button.addEventListener("click", () => {
      const application = currentApplication();
      const profile = application?.profiles.find((item) => item.id === button.dataset.id);
      if (application && profile && window.confirm(`删除账号“${profile.name}”？`)) {
        void runState(
          () => invoke("delete_profile", { applicationId: application.id, id: profile.id }),
          "账号已删除",
          () => { editingProfile = emptyProfile(); },
        );
      }
    });
  });
  document.querySelectorAll<HTMLElement>(".profile").forEach((card) => {
    const select = () => {
      const profile = currentApplication()?.profiles.find((item) => item.id === card.dataset.profileId);
      if (profile) { editingApplication = null; editingProfile = { ...profile }; render(); }
    };
    card.addEventListener("click", (event) => { if (!(event.target as HTMLElement).closest("button")) select(); });
    card.addEventListener("keydown", (event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); select(); } });
  });
  document.querySelector("#import-data")?.addEventListener("click", () => void importData());
  document.querySelector("#export-data")?.addEventListener("click", () => void exportData());
  document.querySelector("#exit-app")?.addEventListener("click", () => {
    if (window.confirm("退出 Codex Key Manager？")) void invoke("exit_app");
  });
}

async function start(): Promise<void> {
  render();
  try {
    state = await invoke("get_state");
    selectedApplicationId = state.applications[0]?.id ?? "";
    syncSelection();
    render();
  } catch (error) {
    showToast(String(error), true);
  }
}

void start();
