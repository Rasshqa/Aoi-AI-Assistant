import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import process from "node:process";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";

const host = process.env.TAURI_DEV_HOST;

// Plugin sinkronisasi avatar antara Web Preview (localhost:1420) & Tauri Desktop App (~/.config/aoi/avatar)
function avatarSyncPlugin() {
  const avatarDir = path.join(os.homedir(), ".config", "aoi", "avatar");
  const manifestPath = path.join(avatarDir, "manifest.json");

  const saveFrameFile = (frame: string, dataUrl: string) => {
    if (!dataUrl || !dataUrl.startsWith("data:")) return;
    const match = dataUrl.match(/^data:image\/([a-zA-Z0-9+.-]+);base64,(.+)$/);
    if (!match) return;
    const mimeSub = match[1];
    const b64 = match[2];
    let ext = "png";
    if (mimeSub === "webp") ext = "webp";
    else if (mimeSub.includes("svg")) ext = "svg";
    else if (mimeSub.includes("jpeg") || mimeSub.includes("jpg")) ext = "jpg";
    const filePath = path.join(avatarDir, `${frame}.${ext}`);
    fs.writeFileSync(filePath, Buffer.from(b64, "base64"));
  };

  return {
    name: "avatar-sync-plugin",
    configureServer(server: any) {
      server.middlewares.use(async (req: any, res: any, next: any) => {
        if (req.url === "/api/avatar-pack" && req.method === "GET") {
          try {
            if (fs.existsSync(manifestPath)) {
              const data = fs.readFileSync(manifestPath, "utf-8");
              res.setHeader("Content-Type", "application/json");
              return res.end(data);
            }
          } catch (e) {
            console.error("Vite avatar sync GET error:", e);
          }
          res.setHeader("Content-Type", "application/json");
          return res.end(JSON.stringify({}));
        }

        if (req.url === "/api/avatar-pack" && req.method === "POST") {
          let body = "";
          req.on("data", (chunk: any) => {
            body += chunk;
          });
          req.on("end", () => {
            try {
              if (!fs.existsSync(avatarDir)) {
                fs.mkdirSync(avatarDir, { recursive: true });
              }
              const pack = JSON.parse(body);
              fs.writeFileSync(manifestPath, JSON.stringify(pack, null, 2), "utf-8");
              for (const [frame, dataUrl] of Object.entries(pack)) {
                if (typeof dataUrl === "string" && dataUrl.startsWith("data:")) {
                  saveFrameFile(frame, dataUrl);
                }
              }
              res.setHeader("Content-Type", "application/json");
              return res.end(JSON.stringify({ success: true }));
            } catch (e) {
              res.statusCode = 500;
              return res.end(JSON.stringify({ error: String(e) }));
            }
          });
          return;
        }

        if (req.url === "/api/avatar-sprite" && req.method === "POST") {
          let body = "";
          req.on("data", (chunk: any) => {
            body += chunk;
          });
          req.on("end", () => {
            try {
              if (!fs.existsSync(avatarDir)) {
                fs.mkdirSync(avatarDir, { recursive: true });
              }
              const { frameName, dataUrl } = JSON.parse(body);
              let pack: Record<string, string> = {};
              if (fs.existsSync(manifestPath)) {
                try {
                  pack = JSON.parse(fs.readFileSync(manifestPath, "utf-8"));
                } catch {}
              }
              pack[frameName] = dataUrl;
              fs.writeFileSync(manifestPath, JSON.stringify(pack, null, 2), "utf-8");
              if (typeof dataUrl === "string" && dataUrl.startsWith("data:")) {
                saveFrameFile(frameName, dataUrl);
              }
              res.setHeader("Content-Type", "application/json");
              return res.end(JSON.stringify({ success: true }));
            } catch (e) {
              res.statusCode = 500;
              return res.end(JSON.stringify({ error: String(e) }));
            }
          });
          return;
        }

        if (req.url === "/api/avatar-reset" && req.method === "POST") {
          try {
            if (fs.existsSync(avatarDir)) {
              const files = fs.readdirSync(avatarDir);
              for (const file of files) {
                fs.unlinkSync(path.join(avatarDir, file));
              }
            }
            res.setHeader("Content-Type", "application/json");
            return res.end(JSON.stringify({ success: true }));
          } catch (e) {
            res.statusCode = 500;
            return res.end(JSON.stringify({ error: String(e) }));
          }
        }

        next();
      });
    },
  };
}

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [vue(), tailwindcss(), avatarSyncPlugin()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
