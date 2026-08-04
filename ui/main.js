const invoke = window.__TAURI__.core.invoke;
const tauriEvent = window.__TAURI__.event;

let currentState = null;
let confirmResolver = null;
let isDirty = false;
let toastTimer = null;
const channelTimers = new Map();

const elements = {
  activeTitle: document.querySelector("#active-title"),
  confirmBody: document.querySelector("#confirm-body"),
  confirmCancel: document.querySelector("#confirm-cancel"),
  confirmDelete: document.querySelector("#confirm-delete"),
  confirmModal: document.querySelector("#confirm-modal"),
  configDir: document.querySelector("#config-dir"),
  deleteProfile: document.querySelector("#delete-profile"),
  dirtyIndicator: document.querySelector("#dirty-indicator"),
  duplicateName: document.querySelector("#duplicate-name"),
  duplicateProfile: document.querySelector("#duplicate-profile"),
  engineState: document.querySelector("#engine-state"),
  form: document.querySelector("#profile-form"),
  managedAudioDir: document.querySelector("#managed-audio-dir"),
  message: document.querySelector("#message"),
  messageIcon: document.querySelector("#message-icon"),
  messageText: document.querySelector("#message-text"),
  nextProfile: document.querySelector("#next-profile"),
  pressChannel: document.querySelector("#press-channel"),
  pressFileName: document.querySelector("#press-file-name"),
  pressFormat: document.querySelector("#press-format"),
  pressSound: document.querySelector("#press-sound"),
  profileCount: document.querySelector("#profile-count"),
  profileList: document.querySelector("#profile-list"),
  profileName: document.querySelector("#profile-name"),
  releaseChannel: document.querySelector("#release-channel"),
  releaseFileName: document.querySelector("#release-file-name"),
  releaseFormat: document.querySelector("#release-format"),
  releaseSound: document.querySelector("#release-sound"),
  saveLabel: document.querySelector("#save-label"),
  saveProfile: document.querySelector("#save-profile"),
  testPress: document.querySelector("#test-press"),
  testRelease: document.querySelector("#test-release"),
  toggleEnabled: document.querySelector("#toggle-enabled"),
  toggleLabel: document.querySelector("#toggle-label"),
};

async function loadState() {
  currentState = await invoke("get_state");
  render();
}

function render() {
  if (!currentState) {
    return;
  }

  const profile = currentState.active_profile;
  elements.activeTitle.textContent = profile.name || "未命名配置";
  elements.configDir.textContent = currentState.config_dir;
  elements.configDir.title = currentState.config_dir;
  elements.managedAudioDir.textContent = managedAudioPath(currentState.config_dir);
  elements.managedAudioDir.title = managedAudioPath(currentState.config_dir);
  elements.profileName.value = profile.name ?? "";
  elements.pressSound.value = profile.press_sound ?? "";
  elements.releaseSound.value = profile.release_sound ?? "";
  renderSoundFile("press", profile.press_sound);
  renderSoundFile("release", profile.release_sound);

  const enabled = Boolean(currentState.enabled);
  document.body.classList.toggle("engine-on", enabled);
  elements.engineState.textContent = enabled ? "运行中" : "已暂停";
  elements.toggleLabel.textContent = enabled ? "已启用" : "已暂停";
  elements.toggleEnabled.classList.toggle("active", enabled);
  elements.toggleEnabled.setAttribute("aria-checked", String(enabled));
  elements.profileCount.textContent = String(currentState.profiles.length).padStart(2, "0");
  elements.deleteProfile.disabled = currentState.profiles.length <= 1;
  setDirty(false);

  elements.profileList.replaceChildren(
    ...currentState.profiles.map((item, index) => {
      const button = document.createElement("button");
      const number = document.createElement("span");
      const name = document.createElement("span");
      const status = document.createElement("span");

      button.type = "button";
      button.className = `profile-item${item.active ? " active" : ""}`;
      button.title = item.path;
      button.setAttribute("aria-current", item.active ? "true" : "false");
      number.className = "profile-number";
      number.textContent = String(index + 1).padStart(2, "0");
      name.className = "profile-name";
      name.textContent = item.name;
      status.className = "profile-status";
      status.setAttribute("aria-hidden", "true");
      button.append(number, name, status);
      button.addEventListener("click", () => runAction(() => switchToProfile(item.name)));
      return button;
    }),
  );

  if (currentState.last_error) {
    showMessage(currentState.last_error, "error", 7000);
  }
}

function renderSoundFile(kind, path) {
  const value = path || "";
  const fileName = fileNameFromPath(value) || "等待选择";
  const format = audioFormat(value);
  const fileNameElement = kind === "press" ? elements.pressFileName : elements.releaseFileName;
  const formatElement = kind === "press" ? elements.pressFormat : elements.releaseFormat;
  fileNameElement.textContent = fileName;
  fileNameElement.title = value;
  formatElement.textContent = format;
}

function fileNameFromPath(path) {
  return String(path || "")
    .split(/[\\/]/)
    .filter(Boolean)
    .pop();
}

function audioFormat(path) {
  const fileName = fileNameFromPath(path) || "";
  const extension = fileName.includes(".") ? fileName.split(".").pop() : "";
  return extension ? extension.toUpperCase() : "AUDIO";
}

function managedAudioPath(configDir) {
  const value = String(configDir || "").replace(/[\\/]+$/, "");
  const separator = value.includes("\\") ? "\\" : "/";
  return `${value}${separator}sounds`;
}

function setDirty(dirty) {
  isDirty = dirty;
  elements.dirtyIndicator.hidden = !dirty;
  elements.saveLabel.textContent = dirty ? "保存更改" : "保存配置";
}

function profileFromForm(nameOverride) {
  return {
    name: nameOverride || elements.profileName.value.trim(),
    press_sound: elements.pressSound.value.trim(),
    release_sound: elements.releaseSound.value.trim(),
  };
}

async function switchToProfile(name) {
  if (isDirty) {
    showMessage("当前配置有未保存的更改", "error");
    return;
  }
  currentState = await invoke("switch_profile", { name });
  render();
}

async function saveProfile() {
  currentState = await invoke("save_active_profile", { profile: profileFromForm() });
  render();
  if (!currentState.last_error) {
    showMessage("配置已保存，音频文件已归档");
  }
}

async function duplicateProfile() {
  const name = elements.duplicateName.value.trim();
  if (!name) {
    showMessage("请输入新配置名称", "error");
    return;
  }
  currentState = await invoke("save_profile", { profile: profileFromForm(name) });
  elements.duplicateName.value = "";
  render();
  if (!currentState.last_error) {
    showMessage("新配置已创建，音频文件已归档");
  }
}

async function deleteProfile() {
  if (isDirty) {
    showMessage("请先保存当前配置的更改", "error");
    return;
  }

  const name =
    currentState?.profiles?.find((profile) => profile.active)?.name ??
    currentState?.active_profile?.name;
  if (!name || currentState.profiles.length <= 1) {
    showMessage("至少需要保留一个配置", "error");
    return;
  }

  if (!(await confirmDeleteProfile(name))) {
    return;
  }

  currentState = await invoke("delete_profile", { name });
  render();
  showMessage(`配置“${name}”已删除`);
}

function confirmDeleteProfile(name) {
  elements.confirmBody.textContent = `确认删除“${name}”？此操作不会删除原始音频文件。`;
  elements.confirmModal.hidden = false;
  elements.confirmDelete.focus();
  return new Promise((resolve) => {
    confirmResolver = resolve;
  });
}

function closeConfirmModal(result) {
  elements.confirmModal.hidden = true;
  if (confirmResolver) {
    confirmResolver(result);
    confirmResolver = null;
  }
}

function showMessage(message, tone = "success", duration = 3600) {
  window.clearTimeout(toastTimer);
  elements.messageText.textContent = String(message);
  elements.messageIcon.setAttribute("href", tone === "error" ? "#icon-alert" : "#icon-check");
  elements.message.classList.toggle("error", tone === "error");
  elements.message.classList.add("visible");
  toastTimer = window.setTimeout(() => {
    elements.message.classList.remove("visible");
  }, duration);
}

function clearMessage() {
  window.clearTimeout(toastTimer);
  elements.message.classList.remove("visible");
}

function audition(channel, command) {
  window.clearTimeout(channelTimers.get(channel));
  channel.classList.remove("is-playing");
  void channel.offsetWidth;
  channel.classList.add("is-playing");
  channelTimers.set(
    channel,
    window.setTimeout(() => channel.classList.remove("is-playing"), 1100),
  );
  return invoke(command);
}

async function nextProfile() {
  if (isDirty) {
    showMessage("当前配置有未保存的更改", "error");
    return;
  }
  currentState = await invoke("next_profile");
  render();
}

async function runAction(action) {
  try {
    clearMessage();
    await action();
  } catch (error) {
    showMessage(String(error), "error", 7000);
  }
}

document.querySelectorAll("[data-pick]").forEach((button) => {
  button.addEventListener("click", () =>
    runAction(async () => {
      const path = await invoke("pick_sound_file");
      if (!path) {
        return;
      }
      const input = document.querySelector(`#${button.dataset.pick}`);
      input.value = path;
      renderSoundFile(button.dataset.pick === "press-sound" ? "press" : "release", path);
      setDirty(true);
      showMessage(`已选择 ${fileNameFromPath(path)}`);
    }),
  );
});

elements.form.addEventListener("submit", (event) => {
  event.preventDefault();
  runAction(saveProfile);
});
elements.profileName.addEventListener("input", () => setDirty(true));
elements.saveProfile.addEventListener("click", () => runAction(saveProfile));
elements.testPress.addEventListener("click", () =>
  runAction(() => audition(elements.pressChannel, "test_press")),
);
elements.testRelease.addEventListener("click", () =>
  runAction(() => audition(elements.releaseChannel, "test_release")),
);
elements.nextProfile.addEventListener("click", () => runAction(nextProfile));
elements.toggleEnabled.addEventListener("click", () =>
  runAction(async () => {
    if (isDirty) {
      showMessage("请先保存当前配置的更改", "error");
      return;
    }
    currentState = await invoke("set_enabled", { enabled: !currentState.enabled });
    render();
  }),
);
elements.duplicateProfile.addEventListener("click", () => runAction(duplicateProfile));
elements.duplicateName.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    runAction(duplicateProfile);
  }
});
elements.deleteProfile.addEventListener("click", () => runAction(deleteProfile));
elements.confirmCancel.addEventListener("click", () => closeConfirmModal(false));
elements.confirmDelete.addEventListener("click", () => closeConfirmModal(true));
elements.confirmModal.addEventListener("click", (event) => {
  if (event.target === elements.confirmModal) {
    closeConfirmModal(false);
  }
});
window.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !elements.confirmModal.hidden) {
    closeConfirmModal(false);
  }
});

tauriEvent.listen("runtime-state-changed", (event) => {
  currentState = event.payload;
  render();
});

loadState().catch((error) => {
  showMessage(String(error), "error", 7000);
});
