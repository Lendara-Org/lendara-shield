import type { Viewport } from "next";
import { Geist_Mono, Inter, Sora } from "next/font/google";

import "./globals.css";

const inter = Inter({ subsets: ["latin"], variable: "--font-inter" });
const geistMono = Geist_Mono({ subsets: ["latin"], variable: "--font-geist-mono" });
const sora = Sora({ subsets: ["latin"], weight: "600", variable: "--font-sora" });

export const viewport: Viewport = {
  themeColor: "#FFFFFF",
};

// The page language is set by app/[lang]/layout.tsx on its wrapper.
export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" className={`${inter.variable} ${geistMono.variable} ${sora.variable}`}>
      <body>{children}</body>
    </html>
  );
}
