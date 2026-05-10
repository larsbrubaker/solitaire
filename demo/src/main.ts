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

  const resizeCanvas = () => {
    const dpr = Math.max(0.5, window.devicePixelRatio || 1);
    const maxWidth = window.innerWidth;
    const maxHeight = window.innerHeight;
    const aspect = 1024 / 768;
    const cssWidth = Math.floor(
      maxWidth / aspect <= maxHeight ? maxWidth : maxHeight * aspect,
    );
    const cssHeight = Math.floor(cssWidth / aspect);

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

  // Mobile: on the first tap, ask the browser for fullscreen so the
  // URL/address bar disappears and the playfield gets the entire
  // viewport. Required to be called from a user gesture; we hook it
  // into the canvas pointerdown handler. No-ops if already fullscreen,
  // or if the device isn't touch-capable, or if requestFullscreen isn't
  // supported (iOS Safari has its own bag of quirks — there a separate
  // "Add to Home Screen" launch is the way to remove the URL bar).
  let fullscreenAttempted = false;
  const maybeRequestFullscreen = () => {
    if (fullscreenAttempted) return;
    if (document.fullscreenElement) {
      fullscreenAttempted = true;
      return;
    }
    const isTouch =
      (navigator.maxTouchPoints ?? 0) > 0 || "ontouchstart" in window;
    if (!isTouch) return;
    fullscreenAttempted = true;
    const el = document.documentElement as HTMLElement & {
      webkitRequestFullscreen?: () => Promise<void>;
    };
    const req = el.requestFullscreen ?? el.webkitRequestFullscreen;
    if (!req) return;
    Promise.resolve(req.call(el)).catch(() => {
      // User denied or browser doesn't allow it on this gesture — let
      // a future tap try again.
      fullscreenAttempted = false;
    });
  };

  canvas.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    maybeRequestFullscreen();
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

  window.addEventListener("resize", resizeCanvas);
  resizeCanvas();

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
