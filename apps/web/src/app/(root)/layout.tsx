// Root layout of `/` only, which redirects to /en or /es before rendering.
// Every real page has its own root layout in app/[lang]/layout.tsx, which
// sets <html lang> to the page's language.
export default function RedirectLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
