/** A titled app page (the docs have their own wider layout). */
export function Page({ title, intro, children }: { title: string; intro: string; children: React.ReactNode }) {
  return (
    <main className="mx-auto max-w-4xl px-4 py-10">
      <div className="space-y-6">
        <header className="space-y-2">
          <h1 className="text-2xl font-semibold">{title}</h1>
          <p className="text-gray">{intro}</p>
        </header>
        {children}
      </div>
    </main>
  );
}
