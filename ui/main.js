const invoke = window.__TAURI__.core.invoke;
const tauriEvent = window.__TAURI__.event;

let currentState = null;

const elements = {
  activeTitle: document.querySelector("#active-title"),
  confirmBody: document.querySelector("#confirm-body"),
  confirmCancel: document.querySelector("#confirm-cancel"),
  confirmDelete: document.querySelector("#confirm-delete"),
  confirmModal: document.querySelector("#confirm-modal"),
  configDir: document.querySelector("#config-dir"),
  deleteProfile: document.querySelector("#delete-profile"),
  duplicateName: document.querySelector("#duplicate-name"),
  form: document.querySelector("#profile-form"),
  message: document.querySelector("#message"),
  nextProfile: document.querySelector("#next-profile"),
  profileList: document.querySelector("#profile-list"),
  profileName: document.querySelector("#profile-name"),
  pressSound: document.querySelector("#press-sound"),
  releaseSound: document.querySelector("#release-sound"),
  saveProfile: document.querySelector("#save-profile"),
  testPress: document.querySelector("#test-press"),
  testRelease: document.querySelector("#test-release"),
  toggleEnabled: document.querySelector("#toggle-enabled"),
};

let confirmResolver = null;

async function loadState() {
  currentState = await invoke("get_state");
  render();
}

function render() {
  if (!currentState) {
    return;
  }

  const profile = currentState.active_profile;
  elements.activeTitle.textContent = `${profile.name} Settings`;
  elements.configDir.textContent = currentState.config_dir;
  elements.profileName.value = profile.name ?? "";
  elements.pressSound.value = profile.press_sound ?? "";
  elements.releaseSound.value = profile.release_sound ?? "";
  elements.toggleEnabled.textContent = currentState.enabled ? "Enabled" : "Paused";
  elements.toggleEnabled.classList.toggle("active", currentState.enabled);
  elements.deleteProfile.disabled = currentState.profiles.length <= 1;
  elements.message.textContent = currentState.last_error ?? "";

  elements.profileList.replaceChildren(
    ...currentState.profiles.map((item) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = `profile-item${item.active ? " active" : ""}`;
      button.textContent = item.name;
      button.title = item.path;
      button.addEventListener("click", () => runAction(() => switchToProfile(item.name)));
      return button;
    }),
  );
}

function profileFromForm(nameOverride) {
  return {
    name: nameOverride || elements.profileName.value.trim(),
    press_sound: elements.pressSound.value.trim(),
    release_sound: elements.releaseSound.value.trim(),
  };
}

async function switchToProfile(name) {
  currentState = await invoke("switch_profile", { name });
  render();
}

async function saveProfile() {
  currentState = await invoke("save_active_profile", { profile: profileFromForm() });
  render();
}

async function duplicateProfile() {
  const name = elements.duplicateName.value.trim();
  if (!name) {
    elements.message.textContent = "Enter a profile name.";
    return;
  }
  await invoke("save_profile", { profile: profileFromForm(name) });
  elements.duplicateName.value = "";
  currentState = await invoke("switch_profile", { name });
  render();
}

async function deleteProfile() {
  const name =
    currentState?.profiles?.find((profile) => profile.active)?.name ??
    currentState?.active_profile?.name;
  if (!name || currentState.profiles.length <= 1) {
    elements.message.textContent = "Cannot delete the last profile.";
    return;
  }

  if (!(await confirmDeleteProfile(name))) {
    return;
  }

  currentState = await invoke("delete_profile", { name });
  render();
}

function confirmDeleteProfile(name) {
  elements.confirmBody.textContent = `Delete "${name}" from your sound profiles?`;
  elements.confirmModal.hidden = false;
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

async function runAction(action) {
  try {
    elements.message.textContent = "";
    await action();
  } catch (error) {
    elements.message.textContent = String(error);
  }
}

document.querySelectorAll("[data-pick]").forEach((button) => {
  button.addEventListener("click", () =>
    runAction(async () => {
      const path = await invoke("pick_sound_file");
      if (path) {
        document.querySelector(`#${button.dataset.pick}`).value = path;
      }
    }),
  );
});

elements.saveProfile.addEventListener("click", () => runAction(saveProfile));
elements.testPress.addEventListener("click", () => runAction(() => invoke("test_press")));
elements.testRelease.addEventListener("click", () => runAction(() => invoke("test_release")));
elements.nextProfile.addEventListener("click", () =>
  runAction(async () => {
    currentState = await invoke("next_profile");
    render();
  }),
);
elements.toggleEnabled.addEventListener("click", () =>
  runAction(async () => {
    currentState = await invoke("set_enabled", { enabled: !currentState.enabled });
    render();
  }),
);
document
  .querySelector("#duplicate-profile")
  .addEventListener("click", () => runAction(duplicateProfile));
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
  elements.message.textContent = String(error);
});
