import { Geist_Mono, Inter, Sora } from "next/font/google";

const inter = Inter({ subsets: ["latin"], variable: "--font-inter" });
const geistMono = Geist_Mono({ subsets: ["latin"], variable: "--font-geist-mono" });
const sora = Sora({ subsets: ["latin"], weight: "600", variable: "--font-sora" });

/** Class names that expose the app's fonts as CSS variables on `<html>`. */
export const fontVariables = `${inter.variable} ${geistMono.variable} ${sora.variable}`;
