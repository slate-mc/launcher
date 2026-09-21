export function FatalError() {
  return (
    <main className="grid min-h-screen place-items-center bg-app-bg p-8 text-app-text">
      <section className="max-w-md rounded-control border border-app-danger/60 bg-app-surface p-6 text-center">
        <h1 className="m-0 text-xl font-bold">slate needs to restart this screen</h1>
        <p className="mt-3 mb-5 text-sm/6 text-app-secondary">
          Your instances are safe. Reload slate to continue.
        </p>
        <button
          type="button"
          className="rounded-control bg-app-accent px-5 py-2.5 text-sm font-bold text-app-on-accent"
          onClick={() => window.location.reload()}
        >
          Reload slate
        </button>
      </section>
    </main>
  );
}
