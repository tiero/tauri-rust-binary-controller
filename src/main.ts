import { invoke } from "@tauri-apps/api/tauri";

let greetInputEl: HTMLInputElement | null;
let greetMsgEl: HTMLElement | null;

async function greet() {
  if (greetMsgEl && greetInputEl) {
    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
    greetMsgEl.textContent = await invoke("greet", {
      name: greetInputEl.value,
    });
  }
}

// HOOKS
async function onGreet(e: Event) {
  e.preventDefault();
  await greet();
}

async function onDownload(e: Event) {
  e.preventDefault();

  try {
    await invoke("download_services", {
      serviceIds: ["alice"],
    })
    setTimeout(async () => {
      const result = await invoke("get_download_progress", {serviceId: "alice"});
      if (!result) return;
      document.getElementById("download-status")!.textContent = `${result}%`;
    }, 500);
  } catch (e) {
    console.log(e);
  }

}

async function onRun(e: Event) {
  e.preventDefault();
  try {
    await invoke("run_service", {
      serviceId: "alice",
    })
  } catch (e) {
    console.log(e);
  }
}
async function onStop(e: Event) {
  e.preventDefault();
  try {
    await invoke("stop_service", {
      serviceId: "alice",
    })
  } catch (e) {
    console.log(e);
  }
}


window.addEventListener("DOMContentLoaded", () => {
  greetInputEl = document.querySelector("#greet-input");
  greetMsgEl = document.querySelector("#greet-msg");
  document.querySelector("#greet-form")?.addEventListener("submit", onGreet);
  document.querySelector("#download-form")?.addEventListener("submit", onDownload);
  document.querySelector("#run-form")?.addEventListener("submit", onRun);
  document.querySelector("#stop-form")?.addEventListener("submit", onStop);

});
