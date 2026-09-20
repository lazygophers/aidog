import type { Metadata, Viewport } from "next";
import "../../src/styles/globals.css";

export const metadata: Metadata = {
  title: "AiDog",
  description: "AiDog proxy dashboard",
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="zh-Hans">
      <body>
        <div id="root">{children}</div>
      </body>
    </html>
  );
}
