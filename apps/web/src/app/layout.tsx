import type { Metadata, Viewport } from "next";
import { Bebas_Neue, Inter, JetBrains_Mono } from "next/font/google";

import { SessionProvider } from "@/components/session";
import { SiteHeader } from "@/components/site-header";

import "./globals.css";

const inter = Inter({ subsets: ["latin"], variable: "--font-inter" });
const jetbrainsMono = JetBrains_Mono({ subsets: ["latin"], variable: "--font-jetbrains-mono" });
const bebasNeue = Bebas_Neue({ subsets: ["latin"], weight: "400", variable: "--font-bebas-neue" });

export const metadata: Metadata = {
  title: { default: "Lendara Shield", template: "%s · Lendara Shield" },
  description: "Compliant private payments on Stellar: KYC once, pay privately, disclose selectively.",
};

export const viewport: Viewport = {
  themeColor: "#f6f4f4",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" className={`${inter.variable} ${jetbrainsMono.variable} ${bebasNeue.variable}`}>
      <body>
        <SessionProvider>
          <SiteHeader />
          <main className="mx-auto max-w-4xl px-4 py-10">{children}</main>
          <footer className="mx-auto max-w-4xl px-4 pb-10 text-xs text-gray">
            Testnet only. Built on Stellar Private Payments (developer preview, unaudited contracts).
          </footer>
        </SessionProvider>
      </body>
    </html>
  );
}
