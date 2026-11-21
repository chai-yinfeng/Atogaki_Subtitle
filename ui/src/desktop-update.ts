import { Channel, invoke } from "@tauri-apps/api/core";

type UpdateInfo = { currentVersion: string; availableVersion: string | null; enabled: boolean };

export function mountDesktopUpdate(dialog: HTMLDialogElement, prepare: () => Promise<boolean>) {
  const check = dialog.querySelector<HTMLButtonElement>("#check-app-update")!;
  const install = dialog.querySelector<HTMLButtonElement>("#install-app-update")!;
  const status = dialog.querySelector<HTMLElement>("#app-update-status")!;
  const close = dialog.querySelector<HTMLButtonElement>("#close-settings")!;
  dialog.addEventListener("cancel", event => { if (dialog.dataset.installing) event.preventDefault(); });
  let version: string | null = null;
  const busy = (value: boolean) => { check.disabled = value; install.disabled = value; };
  check.onclick = async () => {
    busy(true); version = null; install.hidden = true;
    status.textContent = "正在检查 GitHub Release…";
    try {
      const result = await invoke<UpdateInfo>("check_desktop_update");
      version = result.availableVersion;
      status.textContent = !result.enabled ? `当前 v${result.currentVersion} 未启用应用内更新。此功能需要包含发行公钥的 Apple Silicon 版本。`
        : version ? `当前 v${result.currentVersion}，可更新到 v${version}。`
        : `当前 v${result.currentVersion} 已是最新正式版。`;
      install.hidden = !version;
    } catch (error) { status.textContent = `无法检查更新：${String(error)}`; }
    finally { busy(false); }
  };
  install.onclick = async () => {
    if (!version) return;
    busy(true);
    dialog.dataset.installing = "true";
    close.disabled = true;
    try {
      if (!await prepare()) { status.textContent = "请先完成或保存当前编辑，再安装更新。"; return; }
      status.textContent = "正在下载并验证更新，完成后将重启…";
      const onProgress = new Channel<number>();
      onProgress.onmessage = mib => { status.textContent = `已下载 ${mib} MiB，完成后将验签并重启…`; };
      await invoke("install_desktop_update", {version, onProgress});
      status.textContent = "已安装，正在重启…";
      install.hidden = true; version = null;
    } catch (error) { status.textContent = `安装未完成：${String(error)}`; }
    finally { busy(false); delete dialog.dataset.installing; close.disabled = false; }
  };
}
