/** A titled app page (the docs have their own wider layout). */
export function Page({ title, intro, children }: { title: string; intro: string; children: React.ReactNode }) {
  return (
    <main className="mx-auto max-w-[1440px] px-5 py-12 sm:px-8 lg:px-12 lg:py-24">
      <div className="space-y-10 lg:space-y-12">
        <header className="space-y-4">
          <h1 className="font-display text-[40px] leading-[46px] font-semibold sm:text-[52px] sm:leading-[60px] lg:text-[64px] lg:leading-[72px]">{title}</h1>
          <p className="max-w-[1000px] text-lg leading-8 text-gray lg:text-[22px] lg:leading-[34px]">{intro}</p>
        </header>
        {children}
      </div>
    </main>
  );
}
