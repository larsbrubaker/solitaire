// Browser platform shell for Solitaire.
//
// This file owns only DOM/canvas concerns: load wasm-pack output, fetch runtime
// config, resize the canvas, forward browser input, and drive requestAnimationFrame.
// Game rules, widget trees, menus, and layout live in solitaire-core.

type RuntimeConfig = {
  SUPABASE_URL: string;
  SUPABASE_ANON_KEY: string;
};

async function loadConfig(): Promise<RuntimeConfig> {
  // Relative path so this works under any base URL (e.g. larsbrubaker.github.io/solitaire/).
  const resp = await fetch("runtime-config.json", { cache: "no-store" });
  if (!resp.ok) throw new Error(`runtime-config.json missing: ${resp.status}`);
  return await resp.json();
}

async function main() {
  const config = await loadConfig();
  console.log("solitaire: loaded runtime config for", config.SUPABASE_URL);

  const canvas = document.getElementById("solitaire-canvas") as HTMLCanvasElement | null;
  if (!canvas) throw new Error("missing #solitaire-canvas");

  // Vite copies `public/pkg` to the site root. Resolve through `import.meta.url`
  // so both dev (`/src/main.ts`) and Pages (`/assets/index-*.js`) find `/pkg`.
  const wasmJsUrl = new URL("../pkg/solitaire_wasm.js", import.meta.url).href;
  const wasmBgUrl = new URL("../pkg/solitaire_wasm_bg.wasm", import.meta.url).href;
  const wasm = await import(/* @vite-ignore */ wasmJsUrl);
  await wasm.default(wasmBgUrl);

  // Feed the client platform into the Rust/agg-gui input profile BEFORE
  // the first render so touch-device sizing (larger menu rows + HUD
  // buttons + a taller chrome strip) is active from the first frame on
  // phones and tablets. `(pointer: coarse)` catches iPad-mode Safari,
  // which hides "iPad" from the user-agent string.
  if (typeof wasm.set_client_platform === "function") {
    const pointerCoarse = window.matchMedia?.("(pointer: coarse)")?.matches ?? false;
    wasm.set_client_platform(navigator.userAgent, pointerCoarse);
  }

  const resizeCanvas = () => {
    const dpr = Math.max(0.5, window.devicePixelRatio || 1);
    // Canvas fills the entire viewport via explicit CSS pixel sizes
    // (no `width: 100%` cascade — that green-screened on Android
    // Chrome when the html→body→canvas chain resolved to 0 height).
    // The 4:3 playfield is letterboxed INSIDE the Rust app via
    // `playfield_transform`; the chrome (menu / HUD) re-positions
    // itself to a left sidebar on narrow / landscape-mobile screens.
    //
    // Read dims from `documentElement.clientWidth/Height` rather than
    // `window.innerWidth/innerHeight` — the former reflects the actual
    // laid-out viewport, while the latter races with the address bar
    // collapse on Android Chrome and reports 0 inside Vite's
    // preview iframe.
    const root = document.documentElement;
    const cssWidth = root.clientWidth;
    const cssHeight = root.clientHeight;
    if (cssWidth === 0 || cssHeight === 0) return;
    canvas.style.width = `${cssWidth}px`;
    canvas.style.height = `${cssHeight}px`;
    canvas.width = Math.max(1, Math.floor(cssWidth * dpr));
    canvas.height = Math.max(1, Math.floor(cssHeight * dpr));
    wasm.set_device_pixel_ratio(dpr);
  };

  const canvasPoint = (event: PointerEvent) => {
    const rect = canvas.getBoundingClientRect();
    return {
      x: ((event.clientX - rect.left) / rect.width) * canvas.width,
      y: ((event.clientY - rect.top) / rect.height) * canvas.height,
    };
  };

  // Toggle fullscreen on demand. Called from Rust via the
  // `solitaire_core::platform` fullscreen hook (registered below) so
  // the player explicitly opts in via the Options → Toggle Fullscreen
  // menu item rather than being kicked into fullscreen on the first
  // tap. iOS Safari doesn't expose `requestFullscreen` — there the
  // "Add to Home Screen" launch is the supported route — so this
  // silently no-ops if the API isn't available.
  const toggleFullscreen = () => {
    const el = document.documentElement as HTMLElement & {
      webkitRequestFullscreen?: () => Promise<void>;
    };
    const d = document as Document & {
      webkitExitFullscreen?: () => Promise<void>;
    };
    if (document.fullscreenElement) {
      const exit = document.exitFullscreen ?? d.webkitExitFullscreen;
      if (exit) Promise.resolve(exit.call(document)).catch(() => {});
      return;
    }
    const req = el.requestFullscreen ?? el.webkitRequestFullscreen;
    if (!req) return;
    Promise.resolve(req.call(el)).catch(() => {});
  };
  if (typeof wasm.register_fullscreen_toggle === "function") {
    wasm.register_fullscreen_toggle(toggleFullscreen);
  }

  canvas.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    canvas.setPointerCapture(event.pointerId);
    const point = canvasPoint(event);
    wasm.on_mouse_down(point.x, point.y, event.button);
  });
  canvas.addEventListener("pointermove", (event) => {
    event.preventDefault();
    const point = canvasPoint(event);
    wasm.on_mouse_move(point.x, point.y);
  });
  canvas.addEventListener("pointerup", (event) => {
    event.preventDefault();
    const point = canvasPoint(event);
    wasm.on_mouse_up(point.x, point.y, event.button);
  });
  canvas.addEventListener("pointercancel", () => {
    wasm.on_mouse_leave();
  });
  canvas.addEventListener("pointerleave", () => {
    wasm.on_mouse_leave();
  });

  // Drive layout from BOTH the `resize` event (for changes after first
  // paint) AND a requestAnimationFrame retry loop that runs until the
  // viewport reports a non-zero size. The retry guards against a race
  // where wasm finished loading before the host iframe got laid out —
  // happens reliably in Vite's preview iframe and intermittently on
  // Android Chrome when the URL bar is mid-collapse. ResizeObserver is
  // unreliable here (does not fire its initial observation in some
  // iframe contexts), so we don't depend on it.
  window.addEventListener("resize", resizeCanvas);
  const tryInitialSize = () => {
    const root = document.documentElement;
    if (root.clientWidth > 0 && root.clientHeight > 0) {
      resizeCanvas();
      return;
    }
    requestAnimationFrame(tryInitialSize);
  };
  tryInitialSize();

  let last = performance.now();
  const frame = (now: number) => {
    const frameMs = now - last;
    last = now;
    if (wasm.needs_draw()) {
      wasm.render(canvas.width, canvas.height, frameMs);
    }
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

main().catch((err) => {
  console.error(err);
});
