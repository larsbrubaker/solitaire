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

  // Probe the GPU's max texture size up-front. wgpu's WebGL2 backend
  // rejects a Surface whose width or height exceeds MAX_TEXTURE_SIZE,
  // and on low-end Android phones that limit is just 2048 — well below
  // a 1080×2400-logical-px screen at DPR=3 (3240×7200 buffer).
  // Probe with a throwaway canvas so we don't disturb the real
  // canvas's wgpu surface.
  const probeMaxTextureDim = (): number => {
    try {
      const probe = document.createElement("canvas");
      const gl = probe.getContext("webgl2") as WebGL2RenderingContext | null;
      if (!gl) return 2048;
      const max = gl.getParameter(gl.MAX_TEXTURE_SIZE) as number;
      return Math.max(1024, max | 0);
    } catch {
      return 2048;
    }
  };
  const maxBufferDim = probeMaxTextureDim();
  console.log("solitaire: GPU max texture dim", maxBufferDim);

  const resizeCanvas = () => {
    // Canvas fills the entire viewport via explicit CSS pixel sizes
    // (no `width: 100%` cascade — that green-screened on Android
    // Chrome when the html→body→canvas chain resolved to 0 height).
    // The 4:3 playfield is letterboxed INSIDE the Rust app via
    // `playfield_transform`; the chrome (menu / HUD) re-positions
    // itself to a left sidebar on narrow / landscape-mobile screens.
    const cssWidth = window.innerWidth;
    const cssHeight = window.innerHeight;
    const requestedDpr = Math.max(0.5, window.devicePixelRatio || 1);
    // Cap DPR so the backing buffer never exceeds MAX_TEXTURE_SIZE on
    // either axis. Without this, tall Android viewports panic wgpu
    // during Surface::configure (`width and height must be within the
    // maximum supported texture size`). The cap is computed against
    // BOTH axes; we deliberately don't apply a 0.5 floor here because
    // a genuinely huge viewport on a 2048-max GPU may need sub-half
    // DPR to fit, and a slightly fuzzy game beats a crashed one.
    const capX = maxBufferDim / Math.max(1, cssWidth);
    const capY = maxBufferDim / Math.max(1, cssHeight);
    const dpr = Math.min(requestedDpr, capX, capY);
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
