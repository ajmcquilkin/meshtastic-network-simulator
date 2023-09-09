import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";
import { resolve } from "path";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@app": resolve(__dirname, "./src"),
      "@bindings": resolve(__dirname, "./src/bindings"),
      "@components": resolve(__dirname, "./src/components"),
      "@features": resolve(__dirname, "./src/features"),
      "@store": resolve(__dirname, "./src/store"),
      "@utils": resolve(__dirname, "./src/utils"),
    },
  },
});
