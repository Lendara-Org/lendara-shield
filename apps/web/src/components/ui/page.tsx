/** A titled app page (the docs have their own wider layout). */
export function Page({ title, intro, children }: { title: string; intro: string; children: React.ReactNode }) {
  return (
    <main className="mx-auto max-w-[1280px] px-5 py-12 sm:px-8 lg:px-12 lg:py-16">
      <div className="space-y-6">
        <header className="space-y-2">
          <h1 className="font-display text-[30px] leading-[38px] font-semibold sm:text-4xl sm:leading-[44px]">{title}</h1>
          <p className="max-w-3xl text-lg leading-8 text-gray">{intro}</p>
        </header>
        {children}
      </div>
    </main>
  );
}
